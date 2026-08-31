use thiserror::Error;

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

    /// Create a new not implemented error
    pub fn not_implemented<S: Into<String>>(msg: S) -> Self {
        RToolsError::NotImplemented(msg.into())
    }
}