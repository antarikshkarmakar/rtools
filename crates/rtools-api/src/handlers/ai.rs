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
pub struct AiResponse {
    pub success: bool,
    pub message: String,
    pub results: Option<serde_json::Value>,
}

pub async fn organize(
    State(_state): State<Arc<AppState>>,
    mut multipart: Multipart,
) -> Result<Json<AiResponse>, (StatusCode, String)> {
    let temp_dir =
        tempfile::tempdir().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let mut files = Vec::new();

    while let Some(field) = multipart
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
        files.push(rtools_core::FileInput::from_path(temp_path));
    }

    let config = rtools_ai::organize::OrganizeConfig {
        output_dir: std::env::temp_dir().join("organized"),
        strategy: rtools_ai::organize::OrganizeStrategy::ByDate,
        by_date: true,
        by_subject: false,
        dry_run: false,
    };

    let processor = rtools_ai::OrganizeProcessor;
    match processor.process(files, config) {
        Ok(outputs) => Ok(Json(AiResponse {
            success: true,
            message: format!("Organized {} files", outputs.len()),
            results: Some(serde_json::json!({
                "count": outputs.len(),
            })),
        })),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    }
}

pub async fn rename(
    State(_state): State<Arc<AppState>>,
    mut multipart: Multipart,
) -> Result<Json<AiResponse>, (StatusCode, String)> {
    let temp_dir =
        tempfile::tempdir().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let mut files = Vec::new();

    while let Some(field) = multipart
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
        files.push(rtools_core::FileInput::from_path(temp_path));
    }

    let config = rtools_ai::rename::RenameConfig {
        pattern: "{date}_{subject}_{index}".to_string(),
        output_dir: None,
        start_number: 1,
        use_ai_descriptions: true,
        dry_run: false,
    };

    let processor = rtools_ai::RenameProcessor;
    match processor.process(files, config) {
        Ok(outputs) => {
            let names: Vec<String> = outputs
                .iter()
                .filter_map(|o| o.name.clone())
                .collect();
            Ok(Json(AiResponse {
                success: true,
                message: format!("Renamed {} files", names.len()),
                results: Some(serde_json::json!({
                    "names": names,
                })),
            }))
        }
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    }
}

#[derive(Serialize)]
pub struct AltTextResult {
    pub path: String,
    pub alt_text: String,
    pub confidence: f64,
}

pub async fn alt_text(
    State(_state): State<Arc<AppState>>,
    mut multipart: Multipart,
) -> Result<Json<AiResponse>, (StatusCode, String)> {
    let temp_dir =
        tempfile::tempdir().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let mut results = Vec::new();

    while let Some(field) = multipart
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
        let config = rtools_ai::alt_text::AltTextConfig::default();

        let processor = rtools_ai::AltTextProcessor;
        match processor.process(input, config) {
            Ok(result) => {
                results.push(AltTextResult {
                    path: file_name,
                    alt_text: result.alt_text,
                    confidence: result.confidence,
                });
            }
            Err(e) => {
                return Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string()));
            }
        }
    }

    Ok(Json(AiResponse {
        success: true,
        message: format!("Generated alt text for {} images", results.len()),
        results: Some(serde_json::json!({
            "results": results,
        })),
    }))
}

pub async fn duplicates(
    State(_state): State<Arc<AppState>>,
    mut multipart: Multipart,
) -> Result<Json<AiResponse>, (StatusCode, String)> {
    let temp_dir =
        tempfile::tempdir().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let mut files = Vec::new();

    while let Some(field) = multipart
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
        files.push(rtools_core::FileInput::from_path(temp_path));
    }

    let config = rtools_ai::duplicates::DuplicatesConfig {
        threshold: 0.9,
        algorithm: rtools_ai::duplicates::HashAlgorithm::Perceptual,
        action: rtools_ai::duplicates::DuplicateAction::Report,
        dry_run: false,
    };

    let processor = rtools_ai::DuplicatesProcessor;
    match processor.process(files, config) {
        Ok(result) => Ok(Json(AiResponse {
            success: true,
            message: format!("Found {} duplicate groups", result.groups.len()),
            results: Some(serde_json::json!({
                "groups": result.groups.len(),
                "originals": result.total_originals,
                "duplicates": result.total_duplicates,
            })),
        })),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    }
}