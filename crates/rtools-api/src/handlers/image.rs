use axum::{
    extract::{Multipart, State},
    http::StatusCode,
    Json,
};
use rtools_core::Processor;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::AppState;

#[derive(Deserialize)]
pub struct CompressRequest {
    pub quality: Option<u8>,
    pub format: Option<String>,
}

#[derive(Serialize)]
pub struct CompressResponse {
    pub success: bool,
    pub message: String,
    pub output_path: Option<String>,
    pub stats: Option<rtools_core::types::ProcessStats>,
}

pub async fn compress(
    State(state): State<Arc<AppState>>,
    mut multipart: Multipart,
) -> Result<Json<CompressResponse>, (StatusCode, String)> {
    // Create temp directory OUTSIDE the loop so it persists
    let temp_dir =
        tempfile::tempdir().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?
    {
        let file_name = field.file_name().unwrap_or("unknown").to_string();
        let data = field
            .bytes()
            .await
            .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

        let temp_path = temp_dir.path().join(&file_name);
        std::fs::write(&temp_path, &data)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        let quality = CompressRequest {
            quality: None,
            format: None,
        };

        let input = rtools_core::FileInput::from_path(temp_path);
        let config = rtools_image::CompressConfig {
            preset: rtools_image::compress::CompressionPreset::Custom(
                quality
                    .quality
                    .unwrap_or(state.config.image.default_quality),
            ),
            format: quality
                .format
                .and_then(|f| rtools_core::ImageFormat::from_extension(&f)),
            output: None,
            output_policy: rtools_core::OutputPolicy::default(),
            preserve_metadata: true,
            strip_gps: false,
            limits: state.config.limits.clone(),
        };

        let processor = rtools_image::CompressProcessor;
        match processor.process(input, config) {
            Ok(output) => {
                return Ok(Json(CompressResponse {
                    success: true,
                    message: format!("Compressed {file_name}"),
                    output_path: output
                        .destination
                        .as_path()
                        .map(|p| p.display().to_string()),
                    stats: output.stats,
                }));
            }
            Err(e) => {
                return Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string()));
            }
        }
    }

    Err((StatusCode::BAD_REQUEST, "No file uploaded".to_string()))
}

#[derive(Serialize)]
pub struct ConvertResponse {
    pub success: bool,
    pub message: String,
    pub output_path: Option<String>,
}

pub async fn convert(
    State(state): State<Arc<AppState>>,
    mut multipart: Multipart,
) -> Result<Json<ConvertResponse>, (StatusCode, String)> {
    let temp_dir =
        tempfile::tempdir().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?
    {
        let file_name = field.file_name().unwrap_or("unknown").to_string();
        let data = field
            .bytes()
            .await
            .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

        let temp_path = temp_dir.path().join(&file_name);
        std::fs::write(&temp_path, &data)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        let input = rtools_core::FileInput::from_path(temp_path);
        let config = rtools_image::ConvertConfig {
            target_format: rtools_core::ImageFormat::from_extension(&file_name)
                .unwrap_or(rtools_core::ImageFormat::Jpeg),
            output: None,
            output_policy: rtools_core::OutputPolicy::default(),
            output_dir: None,
            quality: state.config.image.default_quality,
            preserve_metadata: true,
            strip_gps: false,
            limits: state.config.limits.clone(),
        };

        let processor = rtools_image::ConvertProcessor;
        match processor.process(input, config) {
            Ok(output) => {
                return Ok(Json(ConvertResponse {
                    success: true,
                    message: format!("Converted {file_name}"),
                    output_path: output
                        .destination
                        .as_path()
                        .map(|p| p.display().to_string()),
                }));
            }
            Err(e) => {
                return Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string()));
            }
        }
    }

    Err((StatusCode::BAD_REQUEST, "No file uploaded".to_string()))
}

pub async fn resize(
    State(state): State<Arc<AppState>>,
    mut multipart: Multipart,
) -> Result<Json<ConvertResponse>, (StatusCode, String)> {
    let temp_dir =
        tempfile::tempdir().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?
    {
        let file_name = field.file_name().unwrap_or("unknown").to_string();
        let data = field
            .bytes()
            .await
            .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

        let temp_path = temp_dir.path().join(&file_name);
        std::fs::write(&temp_path, &data)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        let input = rtools_core::FileInput::from_path(temp_path);
        let config = rtools_image::ResizeConfig {
            width: Some(800),
            height: None,
            maintain_aspect: true,
            algorithm: rtools_image::resize::ResizeAlgorithm::default(),
            output: None,
            output_policy: rtools_core::OutputPolicy::default(),
            quality: state.config.image.default_quality,
            limits: state.config.limits.clone(),
        };

        let processor = rtools_image::ResizeProcessor;
        match processor.process(input, config) {
            Ok(output) => {
                return Ok(Json(ConvertResponse {
                    success: true,
                    message: format!("Resized {file_name}"),
                    output_path: output
                        .destination
                        .as_path()
                        .map(|p| p.display().to_string()),
                }));
            }
            Err(e) => {
                return Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string()));
            }
        }
    }

    Err((StatusCode::BAD_REQUEST, "No file uploaded".to_string()))
}

pub async fn crop(
    State(_state): State<Arc<AppState>>,
    mut _multipart: Multipart,
) -> Result<Json<ConvertResponse>, (StatusCode, String)> {
    Err((
        StatusCode::NOT_IMPLEMENTED,
        "Crop not yet implemented".to_string(),
    ))
}

pub async fn watermark(
    State(_state): State<Arc<AppState>>,
    mut _multipart: Multipart,
) -> Result<Json<ConvertResponse>, (StatusCode, String)> {
    Err((
        StatusCode::NOT_IMPLEMENTED,
        "Watermark not yet implemented".to_string(),
    ))
}

pub async fn filter(
    State(_state): State<Arc<AppState>>,
    mut _multipart: Multipart,
) -> Result<Json<ConvertResponse>, (StatusCode, String)> {
    Err((
        StatusCode::NOT_IMPLEMENTED,
        "Filter not yet implemented".to_string(),
    ))
}

#[derive(Serialize)]
pub struct MetadataResponse {
    pub success: bool,
    pub metadata: Option<rtools_core::types::ImageMetadata>,
}

pub async fn metadata(
    State(_state): State<Arc<AppState>>,
    mut multipart: Multipart,
) -> Result<Json<MetadataResponse>, (StatusCode, String)> {
    let temp_dir =
        tempfile::tempdir().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?
    {
        let file_name = field.file_name().unwrap_or("unknown").to_string();
        let data = field
            .bytes()
            .await
            .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

        let temp_path = temp_dir.path().join(&file_name);
        std::fs::write(&temp_path, &data)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        let input = rtools_core::FileInput::from_path(temp_path);
        let config = rtools_image::MetadataConfig::default();

        let processor = rtools_image::MetadataProcessor;
        match processor.process(input, config) {
            Ok(metadata) => {
                return Ok(Json(MetadataResponse {
                    success: true,
                    metadata: Some(metadata),
                }));
            }
            Err(e) => {
                return Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string()));
            }
        }
    }

    Err((StatusCode::BAD_REQUEST, "No file uploaded".to_string()))
}
