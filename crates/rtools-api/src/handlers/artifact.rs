use super::{ApiError, ApiResult};
use axum::{
    body::{Body, Bytes},
    extract::{rejection::PathRejection, Path, State},
    http::{header, HeaderValue, Response},
};
use futures_util::stream;
use serde::{Deserialize, Serialize};
use std::path::Path as FsPath;
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
        })
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
        let mut created = Vec::<(String, ArtifactRecord)>::with_capacity(pending.len());
        for artifact in pending {
            match self.copy_one(&artifact).await {
                Ok(record) => created.push(record),
                Err(error) => {
                    for (_, record) in &created {
                        let _ = tokio::fs::remove_file(&record.path).await;
                    }
                    return Err(error);
                }
            }
        }

        let responses = created
            .iter()
            .map(|(id, record)| {
                ArtifactResponse::new(id.clone(), record.name.clone(), record.media_type.clone())
            })
            .collect();
        self.records.write().await.extend(created);
        Ok(responses)
    }

    async fn copy_one(
        &self,
        artifact: &PendingArtifact<'_>,
    ) -> ApiResult<(String, ArtifactRecord)> {
        let source_metadata = tokio::fs::symlink_metadata(artifact.source).await?;
        if source_metadata.file_type().is_symlink() || !source_metadata.is_file() {
            return Err(ApiError::invalid("Artifact source must be a regular file"));
        }
        let mut source = tokio::fs::File::open(artifact.source).await?;
        for _ in 0..16 {
            let id = Self::random_id()?;
            let path = self.root.path().join(&id);
            let open = tokio::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
                .await;
            let mut destination = match open {
                Ok(file) => file,
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error.into()),
            };
            if let Err(error) = tokio::io::copy(&mut source, &mut destination).await {
                let _ = tokio::fs::remove_file(&path).await;
                return Err(error.into());
            }
            if let Err(error) = destination.sync_all().await {
                let _ = tokio::fs::remove_file(&path).await;
                return Err(error.into());
            }
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
