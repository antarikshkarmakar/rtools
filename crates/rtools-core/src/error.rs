use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Stable machine-readable error code for rtools operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCode {
    /// The supplied input is invalid.
    InvalidInput,
    /// A required capability is unavailable.
    CapabilityUnavailable,
    /// The requested format is unsupported.
    UnsupportedFormat,
    /// A resource limit was exceeded.
    ResourceLimitExceeded,
    /// An output path already exists.
    OutputExists,
    /// A path violates an output policy.
    PathPolicyViolation,
    /// Processing failed.
    ProcessingFailed,
    /// Processing partially failed.
    PartialFailure,
    /// Authentication is required.
    AuthenticationRequired,
    /// Configuration is invalid.
    ConfigurationInvalid,
    /// Processing was cancelled.
    Cancelled,
    /// Rollback failed.
    RollbackFailed,
}

impl ErrorCode {
    /// Return the stable string representation of this code.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InvalidInput => "INVALID_INPUT",
            Self::CapabilityUnavailable => "CAPABILITY_UNAVAILABLE",
            Self::UnsupportedFormat => "UNSUPPORTED_FORMAT",
            Self::ResourceLimitExceeded => "RESOURCE_LIMIT_EXCEEDED",
            Self::OutputExists => "OUTPUT_EXISTS",
            Self::PathPolicyViolation => "PATH_POLICY_VIOLATION",
            Self::ProcessingFailed => "PROCESSING_FAILED",
            Self::PartialFailure => "PARTIAL_FAILURE",
            Self::AuthenticationRequired => "AUTHENTICATION_REQUIRED",
            Self::ConfigurationInvalid => "CONFIGURATION_INVALID",
            Self::Cancelled => "CANCELLED",
            Self::RollbackFailed => "ROLLBACK_FAILED",
        }
    }
}

/// Main error type for rtools operations
#[derive(Error, Debug)]
pub enum RToolsError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Image processing error: {0}")]
    Image(String),

    #[error("PDF processing error: {0}")]
    Pdf(String),

    #[error("AI processing error: {0}")]
    Ai(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("Unsupported format: {0}")]
    UnsupportedFormat(String),

    #[error("File not found: {0}")]
    FileNotFound(String),

    #[error("File too large: {size} bytes (max: {max} bytes)")]
    FileTooLarge { size: u64, max: u64 },

    #[error("Processing timeout after {0}ms")]
    Timeout(u64),

    #[error("Model not loaded: {0}")]
    ModelNotLoaded(String),

    #[error("Output directory does not exist: {0}")]
    OutputDirectoryNotFound(String),

    #[error("Batch processing error: {0}")]
    BatchError(String),

    #[error("MCP error: {0}")]
    Mcp(String),

    #[error("API error: {0}")]
    Api(String),

    #[error("Feature not implemented: {0}")]
    NotImplemented(String),

    #[error("Internal error: {0}")]
    Internal(String),

    #[error("Capability unavailable: {0}")]
    CapabilityUnavailable(String),

    #[error("Output already exists: {0}")]
    OutputExists(String),

    #[error("Path policy violation: {0}")]
    PathPolicyViolation(String),

    #[error("Resource limit exceeded for {resource}: {actual} (limit: {limit})")]
    ResourceLimitExceeded {
        resource: &'static str,
        actual: u64,
        limit: u64,
    },

    #[error("Resource limit exceeded for {resource}: actual usage unavailable (limit: {limit})")]
    ResourceLimitExceededUnknownActual { resource: &'static str, limit: u64 },

    #[error("Configuration invalid: {0}")]
    ConfigurationInvalid(String),
}

/// Result type alias for rtools operations
pub type RToolsResult<T> = Result<T, RToolsError>;

impl RToolsError {
    /// Create a new image processing error
    pub fn image<S: Into<String>>(msg: S) -> Self {
        RToolsError::Image(msg.into())
    }

    /// Create a new PDF processing error
    pub fn pdf<S: Into<String>>(msg: S) -> Self {
        RToolsError::Pdf(msg.into())
    }

    /// Create a new AI processing error
    pub fn ai<S: Into<String>>(msg: S) -> Self {
        RToolsError::Ai(msg.into())
    }

    /// Create a new configuration error
    pub fn config<S: Into<String>>(msg: S) -> Self {
        RToolsError::Config(msg.into())
    }

    /// Create a new invalid input error
    pub fn invalid_input<S: Into<String>>(msg: S) -> Self {
        RToolsError::InvalidInput(msg.into())
    }

    /// Create a new unsupported format error
    pub fn unsupported_format<S: Into<String>>(msg: S) -> Self {
        RToolsError::UnsupportedFormat(msg.into())
    }

    /// Create a new file not found error
    pub fn file_not_found<S: Into<String>>(path: S) -> Self {
        RToolsError::FileNotFound(path.into())
    }

    /// Create a new output directory not found error
    pub fn output_directory_not_found<S: Into<String>>(path: S) -> Self {
        RToolsError::OutputDirectoryNotFound(path.into())
    }

    /// Create a new not implemented error
    pub fn not_implemented<S: Into<String>>(msg: S) -> Self {
        RToolsError::NotImplemented(msg.into())
    }

    /// Create a new batch processing error
    pub fn batch_error<S: Into<String>>(msg: S) -> Self {
        RToolsError::BatchError(msg.into())
    }

    /// Create a new capability unavailable error.
    pub fn capability_unavailable<S: Into<String>>(msg: S) -> Self {
        RToolsError::CapabilityUnavailable(msg.into())
    }

    /// Create a new output exists error.
    pub fn output_exists<S: Into<String>>(path: S) -> Self {
        RToolsError::OutputExists(path.into())
    }

    /// Create a new path policy violation error.
    pub fn path_policy_violation<S: Into<String>>(msg: S) -> Self {
        RToolsError::PathPolicyViolation(msg.into())
    }

    /// Create a new resource limit exceeded error.
    pub const fn resource_limit_exceeded(resource: &'static str, actual: u64, limit: u64) -> Self {
        RToolsError::ResourceLimitExceeded {
            resource,
            actual,
            limit,
        }
    }

    /// Create a new configuration invalid error.
    pub fn configuration_invalid<S: Into<String>>(msg: S) -> Self {
        RToolsError::ConfigurationInvalid(msg.into())
    }

    /// Return the stable machine-readable code for this error.
    pub const fn code(&self) -> ErrorCode {
        match self {
            Self::Io(_)
            | Self::Image(_)
            | Self::Pdf(_)
            | Self::Ai(_)
            | Self::Mcp(_)
            | Self::Api(_)
            | Self::NotImplemented(_)
            | Self::Internal(_) => ErrorCode::ProcessingFailed,
            Self::Config(_) | Self::ConfigurationInvalid(_) => ErrorCode::ConfigurationInvalid,
            Self::InvalidInput(_) | Self::FileNotFound(_) => ErrorCode::InvalidInput,
            Self::UnsupportedFormat(_) => ErrorCode::UnsupportedFormat,
            Self::FileTooLarge { .. }
            | Self::Timeout(_)
            | Self::ResourceLimitExceeded { .. }
            | Self::ResourceLimitExceededUnknownActual { .. } => ErrorCode::ResourceLimitExceeded,
            Self::ModelNotLoaded(_) | Self::CapabilityUnavailable(_) => {
                ErrorCode::CapabilityUnavailable
            }
            Self::OutputDirectoryNotFound(_) | Self::PathPolicyViolation(_) => {
                ErrorCode::PathPolicyViolation
            }
            Self::BatchError(_) => ErrorCode::PartialFailure,
            Self::OutputExists(_) => ErrorCode::OutputExists,
        }
    }
}

impl From<toml::de::Error> for RToolsError {
    fn from(e: toml::de::Error) -> Self {
        RToolsError::Config(e.to_string())
    }
}

impl From<toml::ser::Error> for RToolsError {
    fn from(e: toml::ser::Error) -> Self {
        RToolsError::Config(e.to_string())
    }
}

impl From<serde_json::Error> for RToolsError {
    fn from(e: serde_json::Error) -> Self {
        RToolsError::Config(e.to_string())
    }
}

impl From<figment::Error> for RToolsError {
    fn from(e: figment::Error) -> Self {
        RToolsError::Config(e.to_string())
    }
}
