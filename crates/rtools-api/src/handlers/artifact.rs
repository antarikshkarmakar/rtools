use super::{ApiError, ApiResult};
use axum::{
    body::{Body, Bytes},
    extract::{rejection::PathRejection, Path, State},
    http::{header, HeaderValue, Response},
};
use futures_util::stream;
use serde::{Deserialize, Serialize};
use std::path::{Path as FsPath, PathBuf};
use std::sync::Arc;
use tokio::io::AsyncReadExt as _;

use crate::{AppState, ArtifactStore};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactResponse {
    pub id: String,
    pub download_url: String,
    pub name: String,
    pub media_type: String,
}

impl ArtifactResponse {
    pub(crate) fn new(id: String, name: String, media_type: String) -> Self {
        Self {
            download_url: format!("/api/v1/artifacts/{id}"),
            id,
            name,
            media_type,
        }
    }
}

#[derive(Debug, Clone)]
#[allow(clippy::redundant_pub_crate)]
pub(crate) struct ArtifactRecord {
    path: std::path::PathBuf,
    name: String,
    media_type: String,
}

#[allow(clippy::redundant_pub_crate)]
pub(crate) struct PendingArtifact<'a> {
    pub(crate) source: &'a FsPath,
    pub(crate) name: String,
    pub(crate) media_type: String,
}

#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(clippy::redundant_pub_crate)]
pub(crate) enum ArtifactPublishStage {
    Copy,
    Sync,
    BetweenCopies,
    RecordLock,
}

#[cfg(test)]
impl ArtifactPublishStage {
    const fn index(self) -> usize {
        match self {
            Self::Copy => 0,
            Self::Sync => 1,
            Self::BetweenCopies => 2,
            Self::RecordLock => 3,
        }
    }
}

#[cfg(test)]
#[derive(Default)]
struct ArtifactTestState {
    stage_counts: [usize; 4],
    pause: Option<ArtifactPauseConfig>,
    copy_count: usize,
    fail_copy_on: Option<usize>,
    delete_count: usize,
    fail_delete_on: Option<usize>,
    fail_delete_attempts: usize,
}

#[cfg(test)]
struct ArtifactPauseConfig {
    stage: ArtifactPublishStage,
    occurrence: usize,
    entered: Arc<tokio::sync::Semaphore>,
    release: Arc<tokio::sync::Semaphore>,
}

#[cfg(test)]
#[derive(Default)]
#[allow(clippy::redundant_pub_crate)]
pub(crate) struct ArtifactTestControl {
    state: std::sync::Mutex<ArtifactTestState>,
}

#[cfg(test)]
#[allow(clippy::redundant_pub_crate)]
pub(crate) struct ArtifactPause {
    entered: Arc<tokio::sync::Semaphore>,
    release: Arc<tokio::sync::Semaphore>,
}

#[cfg(test)]
impl ArtifactPause {
    pub(crate) async fn wait_until_entered(&self) {
        self.entered
            .acquire()
            .await
            .expect("artifact pause semaphore remains open")
            .forget();
    }
}

#[cfg(test)]
impl Drop for ArtifactPause {
    fn drop(&mut self) {
        self.release.add_permits(1);
    }
}

#[cfg(test)]
impl ArtifactTestControl {
    fn pause_at(&self, stage: ArtifactPublishStage, occurrence: usize) -> ArtifactPause {
        let entered = Arc::new(tokio::sync::Semaphore::new(0));
        let release = Arc::new(tokio::sync::Semaphore::new(0));
        let mut control = self.state.lock().expect("artifact test control lock");
        control.stage_counts = [0; 4];
        control.pause = Some(ArtifactPauseConfig {
            stage,
            occurrence,
            entered: Arc::clone(&entered),
            release: Arc::clone(&release),
        });
        drop(control);
        ArtifactPause { entered, release }
    }

    async fn checkpoint(&self, stage: ArtifactPublishStage) {
        let pause = {
            let mut control = self.state.lock().expect("artifact test control lock");
            control.stage_counts[stage.index()] += 1;
            let occurrence = control.stage_counts[stage.index()];
            control.pause.as_ref().and_then(|pause| {
                (pause.stage == stage && pause.occurrence == occurrence)
                    .then(|| (Arc::clone(&pause.entered), Arc::clone(&pause.release)))
            })
        };
        if let Some((entered, release)) = pause {
            entered.add_permits(1);
            release
                .acquire()
                .await
                .expect("artifact pause semaphore remains open")
                .forget();
        }
    }

    fn fail_copy_on(&self, occurrence: usize) {
        let mut state = self.state.lock().expect("artifact test control lock");
        state.copy_count = 0;
        state.fail_copy_on = Some(occurrence);
    }

    fn should_fail_copy(&self) -> bool {
        let mut state = self.state.lock().expect("artifact test control lock");
        state.copy_count += 1;
        state.fail_copy_on == Some(state.copy_count)
    }

    fn fail_delete_on(&self, occurrence: usize) {
        let mut state = self.state.lock().expect("artifact test control lock");
        state.delete_count = 0;
        state.fail_delete_on = Some(occurrence);
        state.fail_delete_attempts = 0;
    }

    fn fail_delete_attempts(&self, attempts: usize) {
        let mut state = self.state.lock().expect("artifact test control lock");
        state.delete_count = 0;
        state.fail_delete_on = None;
        state.fail_delete_attempts = attempts;
    }

    fn should_fail_delete(&self) -> bool {
        let mut state = self.state.lock().expect("artifact test control lock");
        state.delete_count += 1;
        state.fail_delete_on == Some(state.delete_count)
            || state.delete_count <= state.fail_delete_attempts
    }
}

struct ArtifactBatchCleanup {
    paths: Vec<PathBuf>,
    armed: bool,
    #[cfg(test)]
    test_control: Arc<ArtifactTestControl>,
}

const DROP_CLEANUP_ATTEMPTS: usize = 3;

impl ArtifactBatchCleanup {
    #[cfg(not(test))]
    const fn new() -> Self {
        Self {
            paths: Vec::new(),
            armed: true,
        }
    }

    #[cfg(test)]
    const fn new(test_control: Arc<ArtifactTestControl>) -> Self {
        Self {
            paths: Vec::new(),
            armed: true,
            test_control,
        }
    }

    fn track(&mut self, path: PathBuf) {
        self.paths.push(path);
    }

    fn cleanup_once(&mut self) -> usize {
        let mut failed = Vec::new();
        for path in std::mem::take(&mut self.paths) {
            #[cfg(test)]
            let result = if self.test_control.should_fail_delete() {
                Err(std::io::Error::other("injected artifact cleanup failure"))
            } else {
                std::fs::remove_file(&path)
            };
            #[cfg(not(test))]
            let result = std::fs::remove_file(&path);
            match result {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(_) => failed.push(path),
            }
        }
        let failure_count = failed.len();
        self.paths = failed;
        failure_count
    }

    fn rollback(&mut self, original: ApiError) -> ApiError {
        let failures = self.cleanup_once();
        if failures == 0 {
            self.armed = false;
            original
        } else {
            ApiError::from_error(rtools_core::RToolsError::rollback_failed(format!(
                "artifact publication could not remove {failures} owned path(s)"
            )))
        }
    }

    fn disarm(&mut self) {
        self.paths.clear();
        self.armed = false;
    }
}

impl Drop for ArtifactBatchCleanup {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        for _ in 0..DROP_CLEANUP_ATTEMPTS {
            if self.paths.is_empty() {
                return;
            }
            let _ = self.cleanup_once();
        }
        if !self.paths.is_empty() {
            tracing::warn!(
                remaining_paths = self.paths.len(),
                "artifact cancellation cleanup incomplete after bounded retries"
            );
        }
    }
}

pub fn valid_artifact_id(id: &str) -> bool {
    id.len() == "artifact-".len() + 32
        && id.starts_with("artifact-")
        && id["artifact-".len()..]
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

impl ArtifactStore {
    pub(crate) fn new() -> std::io::Result<Self> {
        Ok(Self {
            root: tempfile::Builder::new()
                .prefix("rtools-api-artifacts-")
                .tempdir()?,
            records: tokio::sync::RwLock::new(std::collections::HashMap::new()),
            downloads: Arc::new(tokio::sync::Semaphore::new(16)),
            #[cfg(test)]
            test_control: Arc::new(ArtifactTestControl::default()),
        })
    }

    #[cfg(test)]
    pub(crate) fn pause_publication_at(
        &self,
        stage: ArtifactPublishStage,
        occurrence: usize,
    ) -> ArtifactPause {
        self.test_control.pause_at(stage, occurrence)
    }

    #[cfg(test)]
    pub(crate) fn fail_publication_copy_on(&self, occurrence: usize) {
        self.test_control.fail_copy_on(occurrence);
    }

    #[cfg(test)]
    pub(crate) fn fail_publication_delete_on(&self, occurrence: usize) {
        self.test_control.fail_delete_on(occurrence);
    }

    #[cfg(test)]
    pub(crate) fn fail_publication_delete_attempts(&self, attempts: usize) {
        self.test_control.fail_delete_attempts(attempts);
    }

    fn random_id() -> ApiResult<String> {
        let mut entropy = [0_u8; 16];
        getrandom::fill(&mut entropy)
            .map_err(|_| ApiError::internal("Could not generate an artifact identifier"))?;
        let mut id = String::with_capacity("artifact-".len() + entropy.len() * 2);
        id.push_str("artifact-");
        for byte in entropy {
            use std::fmt::Write as _;
            write!(&mut id, "{byte:02x}")
                .map_err(|_| ApiError::internal("Could not format an artifact identifier"))?;
        }
        Ok(id)
    }

    pub(crate) async fn publish(
        &self,
        source: &FsPath,
        name: String,
        media_type: String,
    ) -> ApiResult<ArtifactResponse> {
        let mut published = self
            .publish_batch(vec![PendingArtifact {
                source,
                name,
                media_type,
            }])
            .await?;
        published
            .pop()
            .ok_or_else(|| ApiError::internal("Artifact publication returned no result"))
    }

    pub(crate) async fn publish_batch(
        &self,
        pending: Vec<PendingArtifact<'_>>,
    ) -> ApiResult<Vec<ArtifactResponse>> {
        for artifact in &pending {
            HeaderValue::from_str(&artifact.media_type)
                .map_err(|_| ApiError::invalid("Artifact media type is invalid"))?;
        }
        #[cfg(test)]
        let mut cleanup = ArtifactBatchCleanup::new(Arc::clone(&self.test_control));
        #[cfg(not(test))]
        let mut cleanup = ArtifactBatchCleanup::new();
        let pending_count = pending.len();
        let mut created = Vec::<(String, ArtifactRecord)>::with_capacity(pending_count);
        for (index, artifact) in pending.into_iter().enumerate() {
            match self.copy_one(&artifact, &mut cleanup).await {
                Ok(record) => created.push(record),
                Err(error) => return Err(cleanup.rollback(error)),
            }
            if index + 1 < pending_count {
                #[cfg(test)]
                self.test_control
                    .checkpoint(ArtifactPublishStage::BetweenCopies)
                    .await;
            }
        }

        let responses = created
            .iter()
            .map(|(id, record)| {
                ArtifactResponse::new(id.clone(), record.name.clone(), record.media_type.clone())
            })
            .collect();
        #[cfg(test)]
        self.test_control
            .checkpoint(ArtifactPublishStage::RecordLock)
            .await;
        let mut records = self.records.write().await;
        records.extend(created);
        cleanup.disarm();
        drop(records);
        Ok(responses)
    }

    async fn copy_one(
        &self,
        artifact: &PendingArtifact<'_>,
        cleanup: &mut ArtifactBatchCleanup,
    ) -> ApiResult<(String, ArtifactRecord)> {
        let source_metadata = tokio::fs::symlink_metadata(artifact.source).await?;
        if source_metadata.file_type().is_symlink() || !source_metadata.is_file() {
            return Err(ApiError::invalid("Artifact source must be a regular file"));
        }
        let mut source = tokio::fs::File::open(artifact.source).await?;
        for _ in 0..16 {
            let id = Self::random_id()?;
            let path = self.root.path().join(&id);
            let open = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path);
            let destination = match open {
                Ok(file) => file,
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error.into()),
            };
            cleanup.track(path.clone());
            let mut destination = tokio::fs::File::from_std(destination);
            #[cfg(test)]
            self.test_control
                .checkpoint(ArtifactPublishStage::Copy)
                .await;
            #[cfg(test)]
            if self.test_control.should_fail_copy() {
                return Err(ApiError::internal("Injected artifact copy failure"));
            }
            tokio::io::copy(&mut source, &mut destination).await?;
            #[cfg(test)]
            self.test_control
                .checkpoint(ArtifactPublishStage::Sync)
                .await;
            destination.sync_all().await?;
            return Ok((
                id,
                ArtifactRecord {
                    path,
                    name: artifact.name.clone(),
                    media_type: artifact.media_type.clone(),
                },
            ));
        }
        Err(ApiError::internal(
            "Could not allocate a unique artifact identifier",
        ))
    }
}

fn content_disposition(name: &str) -> ApiResult<HeaderValue> {
    let mut fallback = String::with_capacity(name.len());
    for character in name.chars() {
        if character.is_ascii_alphanumeric() || matches!(character, ' ' | '.' | '-' | '_') {
            fallback.push(character);
        } else {
            fallback.push('_');
        }
    }
    let fallback = fallback.trim_matches([' ', '.']);
    let fallback = if fallback.is_empty() {
        "artifact"
    } else {
        fallback
    };
    let mut encoded = String::with_capacity(name.len());
    for byte in name.as_bytes() {
        if byte.is_ascii_alphanumeric() || b"!#$&+-.^_`|~".contains(byte) {
            encoded.push(char::from(*byte));
        } else {
            use std::fmt::Write as _;
            write!(&mut encoded, "%{byte:02X}")
                .map_err(|_| ApiError::internal("Could not encode artifact filename"))?;
        }
    }
    HeaderValue::from_str(&format!(
        "attachment; filename=\"{fallback}\"; filename*=UTF-8''{encoded}"
    ))
    .map_err(|_| ApiError::internal("Could not create artifact response headers"))
}

#[allow(clippy::significant_drop_tightening)] // The permit intentionally lives in the body stream.
pub async fn download(
    State(state): State<Arc<AppState>>,
    path: Result<Path<String>, PathRejection>,
) -> ApiResult<Response<Body>> {
    let Path(id) = path.map_err(|_| ApiError::invalid("Invalid artifact identifier"))?;
    if !valid_artifact_id(&id) {
        return Err(ApiError::invalid("Invalid artifact identifier"));
    }
    let record = state
        .artifacts
        .records
        .read()
        .await
        .get(&id)
        .cloned()
        .ok_or_else(|| ApiError::not_found("Artifact does not exist or has expired"))?;
    let permit = Arc::clone(&state.artifacts.downloads)
        .acquire_owned()
        .await
        .map_err(|_| ApiError::internal("Artifact downloads are shutting down"))?;
    let file = tokio::fs::File::open(&record.path)
        .await
        .map_err(|_| ApiError::not_found("Artifact does not exist or has expired"))?;
    let length = file
        .metadata()
        .await
        .map_err(|_| ApiError::not_found("Artifact does not exist or has expired"))?
        .len();
    let body = Body::from_stream(stream::try_unfold(
        (file, permit),
        |(mut file, permit)| async move {
            let mut buffer = vec![0_u8; 64 * 1024];
            let count = file.read(&mut buffer).await?;
            if count == 0 {
                return Ok::<_, std::io::Error>(None);
            }
            buffer.truncate(count);
            Ok(Some((Bytes::from(buffer), (file, permit))))
        },
    ));
    let mut response = Response::new(body);
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(&record.media_type)
            .map_err(|_| ApiError::internal("Invalid stored artifact media type"))?,
    );
    response.headers_mut().insert(
        header::CONTENT_LENGTH,
        HeaderValue::from_str(&length.to_string())
            .map_err(|_| ApiError::internal("Invalid artifact length"))?,
    );
    response.headers_mut().insert(
        header::CONTENT_DISPOSITION,
        content_disposition(&record.name)?,
    );
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, no-store"),
    );
    Ok(response)
}
