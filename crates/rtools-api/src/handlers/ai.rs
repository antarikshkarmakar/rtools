use super::artifact::ArtifactResponse;
use super::artifact::PendingArtifact;
use super::{
    parse_f64, parse_multipart, parse_u32, require_multipart, ApiError, ApiResult, FieldKind,
    IncomingFile, MultipartInput, RequestFiles,
};
use axum::{extract::State, Json};
use rtools_core::{FileInput, ImageFormat, Processor};
use serde::Serialize;
use std::path::Path;
use std::sync::Arc;

use crate::AppState;

#[derive(Serialize)]
pub struct AiResponse {
    pub success: bool,
    pub message: String,
    pub results: serde_json::Value,
}

fn incoming_image(
    request: &RequestFiles,
    upload: IncomingFile,
    index: usize,
    limits: &rtools_core::ResourceLimits,
) -> ApiResult<(FileInput, String, ImageFormat)> {
    let extension = Path::new(&upload.client_name)
        .extension()
        .and_then(|value| value.to_str())
        .ok_or_else(|| {
            ApiError::invalid("Uploaded image filename requires a supported extension")
        })?;
    let format = ImageFormat::from_extension(extension)
        .filter(|format| !matches!(format, ImageFormat::Pdf))
        .ok_or_else(|| ApiError::invalid(format!("Unsupported image format '{extension}'")))?;
    let path = request.write(index, format.extensions()[0], &upload.bytes)?;
    if let Err(error) = rtools_image::format::decode_bounded(&path, limits) {
        return match error.code() {
            rtools_core::ErrorCode::ResourceLimitExceeded
            | rtools_core::ErrorCode::CapabilityUnavailable
            | rtools_core::ErrorCode::UnsupportedFormat => Err(error.into()),
            _ => Err(ApiError::invalid("Uploaded image data is malformed")),
        };
    }
    let actual = rtools_image::format::identify_bounded_format(&path, limits)
        .map_err(|_| ApiError::invalid("Uploaded image data is malformed"))?;
    if actual != format {
        return Err(ApiError::invalid(
            "Uploaded image bytes do not match the filename extension",
        ));
    }
    let mut input = FileInput::from_path(path);
    input.format = Some(format);
    input.name = Some(upload.client_name.clone());
    input.mime_type = Some(format.mime_type().to_string());
    Ok((input, upload.client_name, format))
}

fn artifact_path(output: &rtools_core::FileOutput) -> ApiResult<&std::path::PathBuf> {
    output
        .destination
        .as_path()
        .ok_or_else(|| ApiError::invalid("Processor did not return a path-based artifact"))
}

fn safe_output_stem(name: &str) -> String {
    Path::new(name)
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("upload")
        .chars()
        .map(|character| {
            if character.is_alphanumeric() || matches!(character, '-' | '_') {
                character
            } else {
                '_'
            }
        })
        .collect()
}

fn validate_rest_rename_pattern(pattern: &str) -> ApiResult<()> {
    if pattern == "."
        || pattern == ".."
        || pattern.ends_with(['.', ' '])
        || pattern
            .chars()
            .any(|character| character.is_control() || "<>:\"/\\|?*".contains(character))
    {
        return Err(ApiError::invalid(
            "Rename pattern must produce one portable filename, not a path",
        ));
    }
    let stem = pattern
        .split('.')
        .next()
        .unwrap_or_default()
        .to_ascii_uppercase();
    let reserved = matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || stem
            .strip_prefix("COM")
            .or_else(|| stem.strip_prefix("LPT"))
            .is_some_and(|suffix| {
                suffix.len() == 1
                    && suffix
                        .as_bytes()
                        .first()
                        .is_some_and(|digit| (b'1'..=b'9').contains(digit))
            });
    if reserved {
        return Err(ApiError::invalid(
            "Rename pattern resolves to a reserved portable filename",
        ));
    }
    Ok(())
}

pub async fn organize(
    State(state): State<Arc<AppState>>,
    multipart: MultipartInput,
) -> ApiResult<Json<AiResponse>> {
    let mut form = parse_multipart(
        require_multipart(multipart)?,
        &[("files", FieldKind::Files), ("strategy", FieldKind::Text)],
        state.config.api.max_upload_size,
    )
    .await?;
    let strategy = form
        .optional_text("strategy")
        .unwrap_or_else(|| "date".to_string());
    match strategy.as_str() {
        "date" => {}
        "subject" | "type" | "gps" => {
            return Err(ApiError::unavailable(
                &format!("ai.organize.{strategy}"),
                "Only date organization is implemented",
                "Use strategy=date",
            ));
        }
        _ => {
            return Err(ApiError::invalid(format!(
                "Unsupported organization strategy '{strategy}'"
            )));
        }
    }
    let uploads = form.files("files")?;
    state.config.limits.check_batch_items(
        u64::try_from(uploads.len()).map_err(|_| ApiError::invalid("Too many input files"))?,
    )?;
    let request = RequestFiles::new()?;
    let mut inputs = Vec::with_capacity(uploads.len());
    let mut metadata = Vec::with_capacity(uploads.len());
    for (index, upload) in uploads.into_iter().enumerate() {
        let (input, name, format) = incoming_image(&request, upload, index, &state.config.limits)?;
        inputs.push(input);
        metadata.push((name, format));
    }
    let output_dir = request.path().join("organized");
    let organize_config = rtools_ai::organize::OrganizeConfig {
        output_dir: output_dir.clone(),
        strategy: rtools_ai::organize::OrganizeStrategy::ByDate,
        by_date: true,
        by_subject: false,
        dry_run: true,
    };
    let planned = rtools_ai::OrganizeProcessor.process(inputs.clone(), organize_config)?;
    for output in &planned {
        let path = artifact_path(output)?;
        if !path.starts_with(&output_dir) {
            return Err(ApiError::invalid(
                "Organize processor planned an output outside the request directory",
            ));
        }
        let parent = path
            .parent()
            .ok_or_else(|| ApiError::invalid("Organize output has no parent directory"))?;
        std::fs::create_dir_all(parent).map_err(rtools_core::RToolsError::from)?;
    }
    let outputs = rtools_ai::OrganizeProcessor.process(
        inputs,
        rtools_ai::organize::OrganizeConfig {
            output_dir,
            strategy: rtools_ai::organize::OrganizeStrategy::ByDate,
            by_date: true,
            by_subject: false,
            dry_run: false,
        },
    )?;
    let pending = outputs
        .iter()
        .zip(metadata)
        .map(|(output, (name, format))| {
            Ok(PendingArtifact {
                source: artifact_path(output)?,
                name,
                media_type: format.mime_type().to_string(),
            })
        })
        .collect::<ApiResult<Vec<_>>>()?;
    let artifacts = state.artifacts.publish_batch(pending).await?;
    Ok(Json(AiResponse {
        success: true,
        message: format!("Organized {} files", artifacts.len()),
        results: serde_json::json!({ "artifacts": artifacts }),
    }))
}

pub async fn rename(
    State(state): State<Arc<AppState>>,
    multipart: MultipartInput,
) -> ApiResult<Json<AiResponse>> {
    let mut form = parse_multipart(
        require_multipart(multipart)?,
        &[
            ("files", FieldKind::Files),
            ("pattern", FieldKind::Text),
            ("start_number", FieldKind::Text),
        ],
        state.config.api.max_upload_size,
    )
    .await?;
    let pattern = form
        .optional_text("pattern")
        .unwrap_or_else(|| "{date}_{name}_{index}".to_string());
    rtools_ai::rename::validate_deterministic_pattern(&pattern)?;
    validate_rest_rename_pattern(&pattern)?;
    let start_number = parse_u32("start_number", form.optional_text("start_number"))?.unwrap_or(1);
    let uploads = form.files("files")?;
    state.config.limits.check_batch_items(
        u64::try_from(uploads.len()).map_err(|_| ApiError::invalid("Too many input files"))?,
    )?;
    let input_request = RequestFiles::new()?;
    let output_request = RequestFiles::new()?;
    let mut inputs = Vec::with_capacity(uploads.len());
    let mut metadata = Vec::with_capacity(uploads.len());
    for (index, upload) in uploads.into_iter().enumerate() {
        let (mut input, client_name, format) =
            incoming_image(&input_request, upload, index, &state.config.limits)?;
        input.name = Some(format!(
            "{}.{}",
            safe_output_stem(&client_name),
            format.extensions()[0]
        ));
        inputs.push(input);
        metadata.push((client_name, format));
    }

    let planned_names = inputs
        .iter()
        .enumerate()
        .map(|(offset, input)| {
            let offset = u32::try_from(offset)
                .map_err(|_| ApiError::invalid("Rename sequence exceeds the u32 range"))?;
            let index = start_number
                .checked_add(offset)
                .ok_or_else(|| ApiError::invalid("Rename sequence exceeds the u32 range"))?;
            let source = input
                .source
                .as_path()
                .ok_or_else(|| ApiError::invalid("Rename requires file path inputs"))?;
            let source_name = input
                .name
                .as_deref()
                .ok_or_else(|| ApiError::invalid("Rename requires a source filename"))?;
            rtools_ai::rename::render_filename_with_source_name(
                &pattern,
                source,
                source_name,
                index,
            )
            .map_err(ApiError::from)
        })
        .collect::<ApiResult<Vec<_>>>()?;
    rtools_ai::rename::validate_unique_portable_filenames(&planned_names)?;

    let outputs = rtools_ai::RenameProcessor.process(
        inputs,
        rtools_ai::rename::RenameConfig {
            pattern,
            output_dir: Some(output_request.path().to_path_buf()),
            start_number,
            use_ai_descriptions: false,
            dry_run: false,
        },
    )?;
    if outputs.len() != metadata.len() {
        return Err(ApiError::internal(
            "Rename processor returned an incomplete artifact batch",
        ));
    }
    let mut names = Vec::with_capacity(outputs.len());
    let mut pending = Vec::with_capacity(outputs.len());
    for (output, (_, format)) in outputs.iter().zip(&metadata) {
        let name = output
            .name
            .clone()
            .unwrap_or_else(|| "renamed-upload".to_string());
        names.push(name.clone());
        pending.push(PendingArtifact {
            source: artifact_path(output)?,
            name,
            media_type: format.mime_type().to_string(),
        });
    }
    let artifacts: Vec<ArtifactResponse> = state.artifacts.publish_batch(pending).await?;
    Ok(Json(AiResponse {
        success: true,
        message: format!("Renamed {} files", artifacts.len()),
        results: serde_json::json!({ "names": names, "artifacts": artifacts }),
    }))
}

pub async fn alt_text() -> ApiResult<Json<AiResponse>> {
    Err(ApiError::unavailable(
        "ai.alt_text",
        "No image captioning provider is configured",
        "Configure a supported image captioning provider",
    ))
}

pub async fn duplicates(
    State(state): State<Arc<AppState>>,
    multipart: MultipartInput,
) -> ApiResult<Json<AiResponse>> {
    let mut form = parse_multipart(
        require_multipart(multipart)?,
        &[
            ("files", FieldKind::Files),
            ("threshold", FieldKind::Text),
            ("algorithm", FieldKind::Text),
        ],
        state.config.api.max_upload_size,
    )
    .await?;
    let threshold = parse_f64("threshold", form.optional_text("threshold"), 0.9)?;
    let algorithm = match form
        .optional_text("algorithm")
        .as_deref()
        .unwrap_or("perceptual")
    {
        "average" => rtools_ai::duplicates::HashAlgorithm::Average,
        "perceptual" => rtools_ai::duplicates::HashAlgorithm::Perceptual,
        "difference" => rtools_ai::duplicates::HashAlgorithm::Difference,
        value => {
            return Err(ApiError::invalid(format!(
                "Unsupported duplicate algorithm '{value}'"
            )))
        }
    };
    let uploads = form.files("files")?;
    state.config.limits.check_batch_items(
        u64::try_from(uploads.len()).map_err(|_| ApiError::invalid("Too many input files"))?,
    )?;
    let request = RequestFiles::new()?;
    let mut inputs = Vec::with_capacity(uploads.len());
    for (index, upload) in uploads.into_iter().enumerate() {
        let (input, _, _) = incoming_image(&request, upload, index, &state.config.limits)?;
        inputs.push(input);
    }
    let result = rtools_ai::DuplicatesProcessor.process(
        inputs,
        rtools_ai::duplicates::DuplicatesConfig {
            threshold,
            algorithm,
            action: rtools_ai::duplicates::DuplicateAction::Report,
            dry_run: false,
            limits: state.config.limits.clone(),
        },
    )?;
    Ok(Json(AiResponse {
        success: true,
        message: format!("Found {} duplicate groups", result.groups.len()),
        results: serde_json::json!({
            "groups": result.groups.len(),
            "originals": result.total_originals,
            "duplicates": result.total_duplicates,
        }),
    }))
}
