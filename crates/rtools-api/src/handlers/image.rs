use super::artifact::ArtifactResponse;
use super::{
    parse_bool, parse_multipart, parse_u32, parse_u8, require_multipart, ApiError, ApiResult,
    FieldKind, IncomingFile, MultipartInput, ParsedMultipart, RequestFiles,
};
use axum::{extract::State, Json};
use rtools_core::{FileInput, ImageFormat, Processor, RToolsError};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::AppState;

#[derive(Serialize, Deserialize)]
pub struct CompressResponse {
    pub success: bool,
    pub message: String,
    pub artifact: ArtifactResponse,
    pub stats: Option<rtools_core::types::ProcessStats>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
}

#[derive(Serialize, Deserialize)]
pub struct ConvertResponse {
    pub success: bool,
    pub message: String,
    pub artifact: ArtifactResponse,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
}

#[derive(Serialize)]
pub struct MetadataResponse {
    pub success: bool,
    pub metadata: rtools_core::types::ImageMetadata,
}

fn parse_image_format(value: &str) -> ApiResult<ImageFormat> {
    ImageFormat::from_extension(value)
        .ok_or_else(|| ApiError::invalid(format!("Unsupported image format '{value}'")))
}

fn require_rest_image_format(format: ImageFormat) -> ApiResult<ImageFormat> {
    if matches!(format, ImageFormat::Avif | ImageFormat::Ico) {
        return Err(ApiError::unavailable(
            "api.image.format",
            "This image format is not available through the REST encoder",
            "Use jpeg, png, webp, tiff, bmp, or gif",
        ));
    }
    if matches!(
        format,
        ImageFormat::Heic
            | ImageFormat::Heif
            | ImageFormat::Jxl
            | ImageFormat::Hdr
            | ImageFormat::Exr
            | ImageFormat::Pdf
    ) {
        return Err(ApiError::from(RToolsError::unsupported_format(
            format.mime_type(),
        )));
    }
    Ok(format)
}

fn parse_metadata_flags(form: &mut ParsedMultipart) -> ApiResult<(bool, bool)> {
    let preserve_metadata = parse_bool(
        "preserve_metadata",
        form.optional_text("preserve_metadata"),
        false,
    )?;
    let strip_gps = parse_bool("strip_gps", form.optional_text("strip_gps"), false)?;
    if preserve_metadata && strip_gps {
        return Err(ApiError::invalid(
            "preserve_metadata=true cannot be combined with strip_gps=true",
        ));
    }
    Ok((preserve_metadata, strip_gps))
}

fn incoming_image(
    request: &RequestFiles,
    upload: IncomingFile,
    index: usize,
    limits: &rtools_core::ResourceLimits,
) -> ApiResult<(FileInput, String)> {
    let extension = Path::new(&upload.client_name)
        .extension()
        .and_then(|value| value.to_str())
        .ok_or_else(|| {
            ApiError::invalid("Uploaded image filename requires a supported extension")
        })?;
    let format = require_rest_image_format(parse_image_format(extension)?)?;
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
    Ok((input, upload.client_name))
}

fn display_stem(name: &str) -> String {
    Path::new(name)
        .file_stem()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .unwrap_or("upload")
        .to_string()
}

fn output_path(output: &rtools_core::FileOutput) -> ApiResult<&PathBuf> {
    output.destination.as_path().ok_or_else(|| {
        ApiError::from(RToolsError::Internal(
            "Processor did not return a path-based artifact".to_string(),
        ))
    })
}

pub async fn compress(
    State(state): State<Arc<AppState>>,
    multipart: MultipartInput,
) -> ApiResult<Json<CompressResponse>> {
    let mut form = parse_multipart(
        require_multipart(multipart)?,
        &[
            ("file", FieldKind::File),
            ("quality", FieldKind::Text),
            ("format", FieldKind::Text),
            ("preserve_metadata", FieldKind::Text),
            ("strip_gps", FieldKind::Text),
        ],
        state.config.api.max_upload_size,
    )
    .await?;
    let requested_format = form
        .optional_text("format")
        .map(|value| parse_image_format(&value))
        .transpose()?;
    let (preserve_metadata, strip_gps) = parse_metadata_flags(&mut form)?;
    let requested_format = requested_format
        .map(require_rest_image_format)
        .transpose()?;
    let requested_quality = form.optional_text("quality");
    let has_requested_quality = requested_quality.is_some();
    let quality = parse_u8(
        "quality",
        requested_quality,
        state.config.image.default_quality,
    )?;
    if !(1..=100).contains(&quality) {
        return Err(ApiError::invalid(
            "Compression quality must be between 1 and 100",
        ));
    }
    if preserve_metadata {
        return Err(ApiError::unavailable(
            "image.metadata.preserve",
            "Metadata preservation is not implemented",
            "Omit preserve_metadata or set it to false",
        ));
    }
    if strip_gps {
        return Err(ApiError::unavailable(
            "image.metadata.strip_gps",
            "Selective GPS removal is not implemented",
            "Omit strip_gps or set it to false",
        ));
    }

    let upload = form.one_file("file")?;
    let request = RequestFiles::new()?;
    let (input, client_name) = incoming_image(&request, upload, 0, &state.config.limits)?;
    let target_format = requested_format
        .or(input.format)
        .ok_or_else(|| ApiError::invalid("Could not determine the uploaded image format"))?;
    if has_requested_quality && target_format != ImageFormat::Jpeg {
        return Err(ApiError::invalid(
            "Quality is effective only for JPEG output",
        ));
    }
    let output = rtools_image::CompressProcessor.process(
        input,
        rtools_image::CompressConfig {
            preset: rtools_image::compress::CompressionPreset::Custom(quality),
            format: Some(target_format),
            output: None,
            output_policy: rtools_core::OutputPolicy::default(),
            preserve_metadata: false,
            strip_gps: false,
            limits: state.config.limits.clone(),
        },
    )?;
    let name = format!(
        "{}_compressed.{}",
        display_stem(&client_name),
        target_format.extensions()[0]
    );
    let artifact = state
        .artifacts
        .publish(
            output_path(&output)?,
            name,
            target_format.mime_type().to_string(),
        )
        .await?;
    Ok(Json(CompressResponse {
        success: true,
        message: format!("Compressed {client_name}"),
        artifact,
        stats: output.stats,
        warnings: output.warnings,
    }))
}

pub async fn convert(
    State(state): State<Arc<AppState>>,
    multipart: MultipartInput,
) -> ApiResult<Json<ConvertResponse>> {
    let mut form = parse_multipart(
        require_multipart(multipart)?,
        &[
            ("file", FieldKind::File),
            ("format", FieldKind::Text),
            ("quality", FieldKind::Text),
            ("preserve_metadata", FieldKind::Text),
            ("strip_gps", FieldKind::Text),
        ],
        state.config.api.max_upload_size,
    )
    .await?;
    let target_format = parse_image_format(
        form.optional_text("format")
            .as_deref()
            .ok_or_else(|| ApiError::invalid("Multipart field 'format' is required"))?,
    )?;
    let (preserve_metadata, strip_gps) = parse_metadata_flags(&mut form)?;
    let target_format = require_rest_image_format(target_format)?;
    let requested_quality = form.optional_text("quality");
    let has_requested_quality = requested_quality.is_some();
    let quality = parse_u8(
        "quality",
        requested_quality,
        state.config.image.default_quality,
    )?;
    if !(1..=100).contains(&quality) {
        return Err(ApiError::invalid(
            "Conversion quality must be between 1 and 100",
        ));
    }
    if preserve_metadata || strip_gps {
        return Err(ApiError::unavailable(
            if preserve_metadata {
                "image.metadata.preserve"
            } else {
                "image.metadata.strip_gps"
            },
            "Requested metadata handling is not implemented",
            "Omit metadata flags or set them to false",
        ));
    }
    if has_requested_quality && target_format != ImageFormat::Jpeg {
        return Err(ApiError::invalid(
            "Quality is effective only for JPEG output",
        ));
    }
    let upload = form.one_file("file")?;
    let request = RequestFiles::new()?;
    let (input, client_name) = incoming_image(&request, upload, 0, &state.config.limits)?;
    if input.format == Some(target_format) {
        return Err(ApiError::invalid(
            "Conversion target must differ from the uploaded image format",
        ));
    }
    let output = rtools_image::ConvertProcessor.process(
        input,
        rtools_image::ConvertConfig {
            target_format,
            output: None,
            output_policy: rtools_core::OutputPolicy::default(),
            output_dir: None,
            quality,
            preserve_metadata: false,
            strip_gps: false,
            limits: state.config.limits.clone(),
        },
    )?;
    let name = format!(
        "{}.{}",
        display_stem(&client_name),
        target_format.extensions()[0]
    );
    let artifact = state
        .artifacts
        .publish(
            output_path(&output)?,
            name,
            target_format.mime_type().to_string(),
        )
        .await?;
    Ok(Json(ConvertResponse {
        success: true,
        message: format!("Converted {client_name}"),
        artifact,
        warnings: output.warnings,
    }))
}

pub async fn resize(
    State(state): State<Arc<AppState>>,
    multipart: MultipartInput,
) -> ApiResult<Json<ConvertResponse>> {
    let mut form = parse_multipart(
        require_multipart(multipart)?,
        &[
            ("file", FieldKind::File),
            ("width", FieldKind::Text),
            ("height", FieldKind::Text),
            ("maintain_aspect", FieldKind::Text),
        ],
        state.config.api.max_upload_size,
    )
    .await?;
    let width = parse_u32("width", form.optional_text("width"))?;
    let height = parse_u32("height", form.optional_text("height"))?;
    if width.is_none() && height.is_none() {
        return Err(ApiError::invalid(
            "At least one of width or height is required",
        ));
    }
    let maintain_aspect = parse_bool(
        "maintain_aspect",
        form.optional_text("maintain_aspect"),
        true,
    )?;
    let upload = form.one_file("file")?;
    let request = RequestFiles::new()?;
    let (input, client_name) = incoming_image(&request, upload, 0, &state.config.limits)?;
    let input_format = input
        .format
        .ok_or_else(|| ApiError::invalid("Unknown image format"))?;
    let output = rtools_image::ResizeProcessor.process(
        input,
        rtools_image::ResizeConfig {
            width,
            height,
            maintain_aspect,
            algorithm: rtools_image::resize::ResizeAlgorithm::default(),
            output: None,
            output_policy: rtools_core::OutputPolicy::default(),
            quality: 85,
            limits: state.config.limits.clone(),
        },
    )?;
    let name = format!(
        "{}_resized.{}",
        display_stem(&client_name),
        input_format.extensions()[0]
    );
    let artifact = state
        .artifacts
        .publish(
            output_path(&output)?,
            name,
            input_format.mime_type().to_string(),
        )
        .await?;
    Ok(Json(ConvertResponse {
        success: true,
        message: format!("Resized {client_name}"),
        artifact,
        warnings: output.warnings,
    }))
}

pub async fn crop() -> ApiResult<Json<ConvertResponse>> {
    Err(ApiError::unavailable(
        "api.image.crop",
        "The REST crop adapter is not implemented",
        "Use the CLI image crop command",
    ))
}

pub async fn watermark() -> ApiResult<Json<ConvertResponse>> {
    Err(ApiError::unavailable(
        "api.image.watermark",
        "The REST watermark adapter is not implemented",
        "Use the CLI image watermark command",
    ))
}

pub async fn filter() -> ApiResult<Json<ConvertResponse>> {
    Err(ApiError::unavailable(
        "api.image.filter",
        "The REST filter adapter is not implemented",
        "Use the CLI image filter command",
    ))
}

pub async fn metadata(
    State(state): State<Arc<AppState>>,
    multipart: MultipartInput,
) -> ApiResult<Json<MetadataResponse>> {
    let mut form = parse_multipart(
        require_multipart(multipart)?,
        &[
            ("file", FieldKind::File),
            ("include_exif", FieldKind::Text),
            ("include_dimensions", FieldKind::Text),
            ("include_file_info", FieldKind::Text),
        ],
        state.config.api.max_upload_size,
    )
    .await?;
    let include_exif = parse_bool("include_exif", form.optional_text("include_exif"), true)?;
    let include_dimensions = parse_bool(
        "include_dimensions",
        form.optional_text("include_dimensions"),
        true,
    )?;
    let include_file_info = parse_bool(
        "include_file_info",
        form.optional_text("include_file_info"),
        true,
    )?;
    let upload = form.one_file("file")?;
    let request = RequestFiles::new()?;
    let (input, _) = incoming_image(&request, upload, 0, &state.config.limits)?;
    let metadata = rtools_image::MetadataProcessor.process(
        input,
        rtools_image::MetadataConfig {
            include_exif,
            include_dimensions,
            include_file_info,
            limits: state.config.limits.clone(),
        },
    )?;
    Ok(Json(MetadataResponse {
        success: true,
        metadata,
    }))
}
