pub mod ai;
pub mod artifact;
pub mod image;
pub mod pdf;

use axum::{
    extract::{multipart::MultipartRejection, Multipart},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use rtools_core::{ErrorCode, RToolsError};
use serde::Serialize;
use std::collections::BTreeMap;
use std::fs::OpenOptions;
use std::io::Write as _;
use std::path::PathBuf;

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub success: bool,
    pub code: ErrorCode,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

#[derive(Debug)]
pub struct ApiError {
    status: StatusCode,
    error: RToolsError,
}

impl ApiError {
    pub fn invalid(message: impl Into<String>) -> Self {
        Self::from_error(RToolsError::invalid_input(message))
    }

    pub fn unavailable(operation_id: &str, reason: &str, remediation: &str) -> Self {
        Self::from_error(RToolsError::capability_unavailable(
            operation_id,
            reason,
            remediation,
        ))
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::from_error(RToolsError::Internal(message.into()))
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            error: RToolsError::invalid_input(message),
        }
    }

    pub const fn from_error(error: RToolsError) -> Self {
        let status = match error.code() {
            ErrorCode::InvalidInput | ErrorCode::UnsupportedFormat => StatusCode::BAD_REQUEST,
            ErrorCode::CapabilityUnavailable => StatusCode::NOT_IMPLEMENTED,
            ErrorCode::ResourceLimitExceeded => StatusCode::PAYLOAD_TOO_LARGE,
            ErrorCode::OutputExists => StatusCode::CONFLICT,
            ErrorCode::AuthenticationRequired => StatusCode::UNAUTHORIZED,
            ErrorCode::ConfigurationInvalid
            | ErrorCode::PathPolicyViolation
            | ErrorCode::ProcessingFailed
            | ErrorCode::PartialFailure
            | ErrorCode::Cancelled
            | ErrorCode::RollbackFailed => StatusCode::INTERNAL_SERVER_ERROR,
        };
        Self { status, error }
    }

    fn from_multipart(
        error: &axum::extract::multipart::MultipartError,
        max_upload_size: u64,
    ) -> Self {
        let status = error.status();
        if status == StatusCode::PAYLOAD_TOO_LARGE {
            return Self {
                status,
                error: RToolsError::ResourceLimitExceededUnknownActual {
                    resource: "request_body_bytes",
                    limit: max_upload_size,
                },
            };
        }
        Self {
            status: StatusCode::BAD_REQUEST,
            error: RToolsError::invalid_input(format!("Invalid multipart body: {error}")),
        }
    }
}

impl From<RToolsError> for ApiError {
    fn from(error: RToolsError) -> Self {
        Self::from_error(error)
    }
}

impl From<std::io::Error> for ApiError {
    fn from(error: std::io::Error) -> Self {
        Self::from_error(error.into())
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let code = self.error.code();
        let details = match &self.error {
            RToolsError::ResourceLimitExceeded {
                resource,
                actual,
                limit,
            } => Some(serde_json::json!({
                "resource": resource,
                "actual": actual,
                "limit": limit,
            })),
            RToolsError::ResourceLimitExceededUnknownActual { resource, limit } => {
                Some(serde_json::json!({
                    "resource": resource,
                    "limit": limit,
                }))
            }
            RToolsError::CapabilityUnavailable { operation_id, .. } => {
                Some(serde_json::json!({ "operation_id": operation_id }))
            }
            _ => None,
        };
        let message = match code {
            ErrorCode::InvalidInput => "The request is invalid.",
            ErrorCode::CapabilityUnavailable => "The requested capability is unavailable.",
            ErrorCode::UnsupportedFormat => "The requested format is not supported.",
            ErrorCode::ResourceLimitExceeded => "A configured resource limit was exceeded.",
            ErrorCode::OutputExists => "An output with that name already exists.",
            ErrorCode::AuthenticationRequired => "Authentication is required.",
            ErrorCode::ConfigurationInvalid
            | ErrorCode::PathPolicyViolation
            | ErrorCode::ProcessingFailed
            | ErrorCode::PartialFailure
            | ErrorCode::Cancelled
            | ErrorCode::RollbackFailed => "The request could not be completed.",
        };
        (
            self.status,
            Json(ErrorResponse {
                success: false,
                code,
                message: message.to_string(),
                details,
            }),
        )
            .into_response()
    }
}

pub type ApiResult<T> = Result<T, ApiError>;
pub type MultipartInput = Result<Multipart, MultipartRejection>;

pub fn require_multipart(input: MultipartInput) -> ApiResult<Multipart> {
    input.map_err(|_| ApiError::invalid("A valid multipart/form-data request is required"))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldKind {
    File,
    Files,
    Text,
}

#[derive(Debug)]
pub struct IncomingFile {
    pub client_name: String,
    pub bytes: axum::body::Bytes,
}

#[derive(Debug, Default)]
pub struct ParsedMultipart {
    files: BTreeMap<String, Vec<IncomingFile>>,
    text: BTreeMap<String, String>,
}

impl ParsedMultipart {
    pub fn one_file(&mut self, name: &str) -> ApiResult<IncomingFile> {
        let mut files = self.files.remove(name).unwrap_or_default();
        if files.len() != 1 {
            return Err(ApiError::invalid(format!(
                "Multipart field '{name}' requires exactly one file"
            )));
        }
        Ok(files.remove(0))
    }

    pub fn files(&mut self, name: &str) -> ApiResult<Vec<IncomingFile>> {
        let files = self.files.remove(name).unwrap_or_default();
        if files.is_empty() {
            return Err(ApiError::invalid(format!(
                "Multipart field '{name}' requires at least one file"
            )));
        }
        Ok(files)
    }

    pub fn optional_text(&mut self, name: &str) -> Option<String> {
        self.text.remove(name)
    }
}

pub async fn parse_multipart(
    mut multipart: Multipart,
    schema: &[(&str, FieldKind)],
    max_upload_size: u64,
) -> ApiResult<ParsedMultipart> {
    let mut parsed = ParsedMultipart::default();
    let mut total_bytes = 0_u64;
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|error| ApiError::from_multipart(&error, max_upload_size))?
    {
        let name = field
            .name()
            .ok_or_else(|| ApiError::invalid("Every multipart part must have a field name"))?
            .to_string();
        let kind = schema
            .iter()
            .find_map(|(candidate, kind)| (*candidate == name).then_some(*kind))
            .ok_or_else(|| ApiError::invalid(format!("Unknown multipart field '{name}'")))?;
        match kind {
            FieldKind::File | FieldKind::Files => {
                let client_name = normalize_client_name(field.file_name())?;
                let bytes = field
                    .bytes()
                    .await
                    .map_err(|error| ApiError::from_multipart(&error, max_upload_size))?;
                let length = u64::try_from(bytes.len()).map_err(|_| {
                    ApiError::from(RToolsError::resource_limit_exceeded(
                        "input_bytes",
                        u64::MAX,
                        max_upload_size,
                    ))
                })?;
                total_bytes = total_bytes.checked_add(length).ok_or_else(|| {
                    ApiError::from(RToolsError::resource_limit_exceeded(
                        "input_bytes",
                        u64::MAX,
                        max_upload_size,
                    ))
                })?;
                if total_bytes > max_upload_size {
                    return Err(ApiError::from(RToolsError::resource_limit_exceeded(
                        "input_bytes",
                        total_bytes,
                        max_upload_size,
                    )));
                }
                let entries = parsed.files.entry(name.clone()).or_default();
                if kind == FieldKind::File && !entries.is_empty() {
                    return Err(ApiError::invalid(format!(
                        "Multipart field '{name}' must not be repeated"
                    )));
                }
                entries.push(IncomingFile { client_name, bytes });
            }
            FieldKind::Text => {
                if field.file_name().is_some() {
                    return Err(ApiError::invalid(format!(
                        "Multipart field '{name}' must be text, not a file"
                    )));
                }
                if parsed.text.contains_key(&name) {
                    return Err(ApiError::invalid(format!(
                        "Multipart field '{name}' must not be repeated"
                    )));
                }
                let value = field.text().await.map_err(|error| {
                    if error.status() == StatusCode::PAYLOAD_TOO_LARGE {
                        ApiError::from_multipart(&error, max_upload_size)
                    } else {
                        ApiError::invalid(format!(
                            "Multipart field '{name}' is not valid UTF-8: {error}"
                        ))
                    }
                })?;
                parsed.text.insert(name, value);
            }
        }
    }
    Ok(parsed)
}

fn normalize_client_name(name: Option<&str>) -> ApiResult<String> {
    let name = name.ok_or_else(|| ApiError::invalid("Uploaded files require a filename"))?;
    let basename = name.rsplit(['/', '\\']).next().unwrap_or_default();
    if basename.is_empty() || matches!(basename, "." | "..") {
        return Err(ApiError::invalid(
            "Uploaded filename has no usable basename",
        ));
    }
    let normalized: String = basename
        .chars()
        .map(|character| {
            if character.is_control() {
                '_'
            } else {
                character
            }
        })
        .collect();
    Ok(normalized)
}

pub struct RequestFiles {
    directory: tempfile::TempDir,
}

impl RequestFiles {
    pub fn new() -> ApiResult<Self> {
        Ok(Self {
            directory: tempfile::Builder::new()
                .prefix("rtools-api-request-")
                .tempdir()?,
        })
    }

    pub fn path(&self) -> &std::path::Path {
        self.directory.path()
    }

    pub fn write(&self, index: usize, extension: &str, bytes: &[u8]) -> ApiResult<PathBuf> {
        let path = self
            .directory
            .path()
            .join(format!("upload-{index:08}.{extension}"));
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        Ok(path)
    }
}

pub fn parse_u8(name: &str, value: Option<String>, default: u8) -> ApiResult<u8> {
    value.map_or(Ok(default), |value| {
        value
            .parse::<u8>()
            .map_err(|_| ApiError::invalid(format!("Multipart field '{name}' must be an integer")))
    })
}

pub fn parse_u32(name: &str, value: Option<String>) -> ApiResult<Option<u32>> {
    value
        .map(|value| {
            value.parse::<u32>().map_err(|_| {
                ApiError::invalid(format!("Multipart field '{name}' must be an integer"))
            })
        })
        .transpose()
}

pub fn parse_f64(name: &str, value: Option<String>, default: f64) -> ApiResult<f64> {
    let parsed = value.map_or(Ok(default), |value| {
        value
            .parse::<f64>()
            .map_err(|_| ApiError::invalid(format!("Multipart field '{name}' must be a number")))
    })?;
    if !parsed.is_finite() {
        return Err(ApiError::invalid(format!(
            "Multipart field '{name}' must be finite"
        )));
    }
    Ok(parsed)
}

pub fn parse_bool(name: &str, value: Option<String>, default: bool) -> ApiResult<bool> {
    value.map_or(Ok(default), |value| match value.as_str() {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(ApiError::invalid(format!(
            "Multipart field '{name}' must be 'true' or 'false'"
        ))),
    })
}
