use super::artifact::ArtifactResponse;
use super::{parse_bool, parse_multipart, ApiError, ApiResult, FieldKind, RequestFiles};
use axum::{extract::State, Json};
use rtools_core::{FileInput, Processor};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::AppState;

#[derive(Serialize, Deserialize)]
pub struct PdfResponse {
    pub success: bool,
    pub message: String,
    pub artifact: ArtifactResponse,
}

pub async fn merge(
    State(state): State<Arc<AppState>>,
    multipart: axum::extract::Multipart,
) -> ApiResult<Json<PdfResponse>> {
    let mut form = parse_multipart(
        multipart,
        &[("files", FieldKind::Files)],
        state.config.api.max_upload_size,
    )
    .await?;
    let uploads = form.files("files")?;
    if uploads.len() < 2 {
        return Err(ApiError::invalid("At least 2 PDF files are required"));
    }
    state.config.limits.check_batch_items(
        u64::try_from(uploads.len()).map_err(|_| ApiError::invalid("Too many input files"))?,
    )?;
    let request = RequestFiles::new()?;
    let mut paths = Vec::with_capacity(uploads.len());
    for (index, upload) in uploads.into_iter().enumerate() {
        paths.push(request.write(index, "pdf", &upload.bytes)?);
    }
    let output_path = request.path().join("merged.pdf");
    let inputs = paths.iter().cloned().map(FileInput::from_path).collect();
    let output = rtools_pdf::PdfMergeProcessor.process(
        inputs,
        rtools_pdf::PdfMergeConfig {
            inputs: paths,
            output: output_path,
            add_page_numbers: false,
        },
    )?;
    let path = output
        .destination
        .as_path()
        .ok_or_else(|| ApiError::invalid("PDF merge returned no path-based artifact"))?;
    let artifact = state.artifacts.publish(
        path,
        "merged.pdf".to_string(),
        "application/pdf".to_string(),
    )?;
    Ok(Json(PdfResponse {
        success: true,
        message: "PDFs merged successfully".to_string(),
        artifact,
    }))
}

pub async fn compress(
    State(state): State<Arc<AppState>>,
    multipart: axum::extract::Multipart,
) -> ApiResult<Json<PdfResponse>> {
    let mut form = parse_multipart(
        multipart,
        &[
            ("file", FieldKind::File),
            ("level", FieldKind::Text),
            ("remove_metadata", FieldKind::Text),
        ],
        state.config.api.max_upload_size,
    )
    .await?;
    let configured_level = match &state.config.pdf.compression_level {
        rtools_core::config::PdfCompressionLevel::Light => "light",
        rtools_core::config::PdfCompressionLevel::Medium => "medium",
        rtools_core::config::PdfCompressionLevel::Heavy => "heavy",
    };
    let level = form
        .optional_text("level")
        .unwrap_or_else(|| configured_level.to_string());
    match level.as_str() {
        "medium" => {}
        "light" | "heavy" => {
            return Err(ApiError::unavailable(
                "api.pdf.compress.level",
                "The REST adapter currently supports only the effective medium level",
                "Use level=medium or omit the field",
            ));
        }
        _ => {
            return Err(ApiError::invalid(format!(
                "Unsupported PDF compression level '{level}'"
            )));
        }
    }
    let remove_metadata = parse_bool(
        "remove_metadata",
        form.optional_text("remove_metadata"),
        false,
    )?;
    let upload = form.one_file("file")?;
    let display_name = std::path::Path::new(&upload.client_name)
        .file_stem()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .unwrap_or("document")
        .to_string();
    let request = RequestFiles::new()?;
    let path = request.write(0, "pdf", &upload.bytes)?;
    let output = rtools_pdf::PdfCompressProcessor.process(
        FileInput::from_path(path),
        rtools_pdf::PdfCompressConfig {
            level: rtools_pdf::compress::PdfCompressionLevel::Medium,
            output: None,
            remove_metadata,
        },
    )?;
    let path = output
        .destination
        .as_path()
        .ok_or_else(|| ApiError::invalid("PDF compression returned no path-based artifact"))?;
    let artifact = state.artifacts.publish(
        path,
        format!("{display_name}_compressed.pdf"),
        "application/pdf".to_string(),
    )?;
    Ok(Json(PdfResponse {
        success: true,
        message: "PDF compressed successfully".to_string(),
        artifact,
    }))
}

pub async fn split() -> ApiResult<Json<PdfResponse>> {
    Err(ApiError::unavailable(
        "api.pdf.split",
        "The REST split adapter is not implemented",
        "Use the CLI PDF split command",
    ))
}

pub async fn ocr() -> ApiResult<Json<PdfResponse>> {
    Err(ApiError::unavailable(
        "pdf.ocr",
        "PDF OCR is not implemented",
        "Use an external OCR tool",
    ))
}
