use super::{ApiError, ApiResult};
use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, HeaderValue, Response},
};
use serde::{Deserialize, Serialize};
use std::io::{Read as _, Write as _};
use std::path::{Component, Path as FsPath};
use std::sync::Arc;

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

pub fn media_extension(media_type: &str) -> &'static str {
    match media_type {
        "image/jpeg" => "jpg",
        "image/png" => "png",
        "image/webp" => "webp",
        "image/avif" => "avif",
        "image/tiff" => "tiff",
        "image/bmp" => "bmp",
        "image/gif" => "gif",
        "image/x-icon" | "image/ico" => "ico",
        "application/pdf" => "pdf",
        _ => "bin",
    }
}

pub fn valid_artifact_id(id: &str) -> bool {
    let mut components = FsPath::new(id).components();
    matches!(components.next(), Some(Component::Normal(_)))
        && components.next().is_none()
        && id.starts_with("artifact-")
        && !id.contains(['/', '\\'])
}

impl ArtifactStore {
    pub(crate) fn publish(
        &self,
        source: &FsPath,
        name: String,
        media_type: String,
    ) -> ApiResult<ArtifactResponse> {
        let suffix = format!(".{}", media_extension(&media_type));
        let mut destination = tempfile::Builder::new()
            .prefix("artifact-")
            .suffix(&suffix)
            .tempfile_in(self.root.path())?;
        let mut input = std::fs::File::open(source)?;
        std::io::copy(&mut input, destination.as_file_mut())?;
        destination.as_file_mut().flush()?;
        destination.as_file().sync_all()?;
        let (_file, path) = destination
            .keep()
            .map_err(|error| ApiError::from(error.error))?;
        let id = path
            .file_name()
            .and_then(|value| value.to_str())
            .ok_or_else(|| ApiError::invalid("Generated artifact identifier is not Unicode"))?
            .to_string();
        Ok(ArtifactResponse::new(id, name, media_type))
    }
}

pub async fn download(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> ApiResult<Response<Body>> {
    if !valid_artifact_id(&id) {
        return Err(ApiError::invalid("Invalid artifact identifier"));
    }
    let path = state.artifacts.root.path().join(&id);
    let mut file = std::fs::File::open(path)
        .map_err(|_| ApiError::invalid("Artifact does not exist or has expired"))?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    let media_type = match FsPath::new(&id)
        .extension()
        .and_then(|value| value.to_str())
    {
        Some("jpg") => "image/jpeg",
        Some("png") => "image/png",
        Some("webp") => "image/webp",
        Some("avif") => "image/avif",
        Some("tiff") => "image/tiff",
        Some("bmp") => "image/bmp",
        Some("gif") => "image/gif",
        Some("ico") => "image/x-icon",
        Some("pdf") => "application/pdf",
        _ => "application/octet-stream",
    };
    let mut response = Response::new(Body::from(bytes));
    response
        .headers_mut()
        .insert(header::CONTENT_TYPE, HeaderValue::from_static(media_type));
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, no-store"),
    );
    Ok(response)
}
