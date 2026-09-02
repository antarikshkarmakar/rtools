use axum::{
    extract::{Multipart, State},
    http::StatusCode,
    Json,
};
use rtools_core::Processor;
use serde::Serialize;
use std::sync::Arc;

use crate::AppState;

#[derive(Serialize)]
pub struct PdfResponse {
    pub success: bool,
    pub message: String,
    pub output_path: Option<String>,
}

pub async fn merge(
    State(_state): State<Arc<AppState>>,
    mut multipart: Multipart,
) -> Result<Json<PdfResponse>, (StatusCode, String)> {
    let temp_dir =
        tempfile::tempdir().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let mut files = Vec::new();

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
        files.push(temp_path);
    }

    if files.len() < 2 {
        return Err((
            StatusCode::BAD_REQUEST,
            "At least 2 PDF files required".to_string(),
        ));
    }

    let output_path = std::env::temp_dir().join("merged.pdf");
    let config = rtools_pdf::PdfMergeConfig {
        inputs: files,
        output: output_path.clone(),
        add_page_numbers: false,
    };

    let processor = rtools_pdf::PdfMergeProcessor;
    let inputs: Vec<rtools_core::FileInput> = config
        .inputs
        .iter()
        .map(|p| rtools_core::FileInput::from_path(p.clone()))
        .collect();

    match processor.process(inputs, config) {
        Ok(output) => Ok(Json(PdfResponse {
            success: true,
            message: "PDFs merged successfully".to_string(),
            output_path: output
                .destination
                .as_path()
                .map(|p| p.display().to_string()),
        })),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    }
}

pub async fn compress(
    State(_state): State<Arc<AppState>>,
    mut multipart: Multipart,
) -> Result<Json<PdfResponse>, (StatusCode, String)> {
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
        let config = rtools_pdf::PdfCompressConfig {
            level: rtools_pdf::compress::PdfCompressionLevel::Medium,
            output: None,
            remove_metadata: false,
        };

        let processor = rtools_pdf::PdfCompressProcessor;
        match processor.process(input, config) {
            Ok(output) => {
                return Ok(Json(PdfResponse {
                    success: true,
                    message: format!("Compressed {file_name}"),
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

pub async fn split(
    State(_state): State<Arc<AppState>>,
    mut _multipart: Multipart,
) -> Result<Json<PdfResponse>, (StatusCode, String)> {
    Err((
        StatusCode::NOT_IMPLEMENTED,
        "PDF split not yet implemented".to_string(),
    ))
}

pub async fn ocr(
    State(_state): State<Arc<AppState>>,
    mut _multipart: Multipart,
) -> Result<Json<PdfResponse>, (StatusCode, String)> {
    Err((
        StatusCode::NOT_IMPLEMENTED,
        "PDF OCR not yet implemented".to_string(),
    ))
}
