use crate::error::RToolsResult;
use figment::{
    providers::{Format, Serialized, Toml},
    Figment,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Main application configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppConfig {
    /// General settings
    pub general: GeneralConfig,
    /// Image processing settings
    pub image: ImageConfig,
    /// PDF processing settings
    pub pdf: PdfConfig,
    /// AI/ML settings
    pub ai: AiConfig,
    /// API server settings
    pub api: ApiConfig,
    /// MCP server settings
    #[serde(alias = "mc")]
    pub mcp: McpConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralConfig {
    /// Number of parallel processing jobs
    pub parallel_jobs: usize,
    /// Temporary directory for processing
    pub temp_dir: PathBuf,
    /// Log level
    pub log_level: String,
    /// Enable verbose output
    pub verbose: bool,
    /// Maximum file size in bytes (default 100MB)
    pub max_file_size: u64,
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            parallel_jobs: num_cpus().unwrap_or(4),
            temp_dir: std::env::temp_dir().join("rtools"),
            log_level: "info".to_string(),
            verbose: false,
            max_file_size: 100 * 1024 * 1024, // 100MB
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageConfig {
    /// Default compression quality (1-100)
    pub default_quality: u8,
    /// Enable WebP lossless mode
    pub webp_lossless: bool,
    /// Enable AVIF support
    pub avif_enabled: bool,
    /// Maximum image dimension
    pub max_dimension: u32,
    /// Default JPEG quality
    pub jpeg_quality: u8,
    /// Default PNG compression level
    pub png_compression: u8,
    /// Enable dithering for palette images
    pub dither: bool,
}

impl Default for ImageConfig {
    fn default() -> Self {
        Self {
            default_quality: 85,
            webp_lossless: false,
            avif_enabled: true,
            max_dimension: 8192,
            jpeg_quality: 85,
            png_compression: 6,
            dither: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PdfConfig {
    /// `PDFium` library path
    pub pdfium_path: Option<PathBuf>,
    /// OCR language
    pub ocr_language: String,
    /// OCR DPI
    pub ocr_dpi: u32,
    /// Default compression level
    pub compression_level: PdfCompressionLevel,
    /// Default image quality for PDF extraction
    pub image_quality: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PdfCompressionLevel {
    Light,
    Medium,
    Heavy,
}

impl Default for PdfConfig {
    fn default() -> Self {
        Self {
            pdfium_path: None,
            ocr_language: "eng".to_string(),
            ocr_dpi: 300,
            compression_level: PdfCompressionLevel::Medium,
            image_quality: 85,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiConfig {
    /// Model directory
    pub model_dir: PathBuf,
    /// Compute device
    pub device: AiDevice,
    /// Batch size for AI operations
    pub batch_size: usize,
    /// Enable GPU acceleration
    pub gpu_enabled: bool,
    /// ONNX runtime path
    pub ort_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AiDevice {
    Cpu,
    Cuda,
    Metal,
    OpenCl,
}

impl Default for AiConfig {
    fn default() -> Self {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        Self {
            model_dir: home.join(".rtools").join("models"),
            device: AiDevice::Cpu,
            batch_size: 8,
            gpu_enabled: false,
            ort_path: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiConfig {
    /// Server host
    pub host: String,
    /// Server port
    pub port: u16,
    /// Maximum upload size in bytes
    pub max_upload_size: u64,
    /// CORS allowed origins
    pub cors_origins: Vec<String>,
    /// Enable authentication
    pub auth_enabled: bool,
    /// API key (for auth)
    pub api_key: Option<String>,
    /// Enable TLS
    pub tls_enabled: bool,
    /// TLS certificate path
    pub tls_cert_path: Option<PathBuf>,
    /// TLS key path
    pub tls_key_path: Option<PathBuf>,
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 8080,
            max_upload_size: 100 * 1024 * 1024, // 100MB
            cors_origins: vec!["*".to_string()],
            auth_enabled: false,
            api_key: None,
            tls_enabled: false,
            tls_cert_path: None,
            tls_key_path: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpConfig {
    /// MCP server name
    pub server_name: String,
    /// Server version
    pub server_version: String,
    /// Use stdio transport
    pub stdio_transport: bool,
    /// HTTP transport URL (if using HTTP)
    pub http_url: Option<String>,
}

impl Default for McpConfig {
    fn default() -> Self {
        Self {
            server_name: "rtools".to_string(),
            server_version: env!("CARGO_PKG_VERSION").to_string(),
            stdio_transport: true,
            http_url: None,
        }
    }
}

impl AppConfig {
    /// Load configuration from file.
    ///
    /// # Errors
    ///
    /// Returns an error when configuration cannot be loaded or the temporary
    /// directory cannot be created.
    pub fn load(path: Option<&PathBuf>) -> RToolsResult<Self> {
        let mut figment = Figment::from(Serialized::defaults(Self::default()));

        if let Some(path) = path {
            if path.exists() {
                figment = figment.merge(Toml::file(path));
            }
        }

        // Also try default locations
        let default_locations = [
            PathBuf::from("rtools.toml"),
            PathBuf::from(".rtools.toml"),
            dirs::config_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("rtools")
                .join("config.toml"),
        ];

        for loc in &default_locations {
            if loc.exists() {
                figment = figment.merge(Toml::file(loc));
                break;
            }
        }

        let config: Self = figment
            .extract()
            .map_err(|e| crate::error::RToolsError::config(e.to_string()))?;

        // Ensure temp directory exists
        std::fs::create_dir_all(&config.general.temp_dir)
            .map_err(|e| crate::error::RToolsError::config(e.to_string()))?;

        Ok(config)
    }

    /// Save configuration to file.
    ///
    /// # Errors
    ///
    /// Returns an error when the parent directory cannot be created or the
    /// configuration cannot be serialized or written.
    pub fn save(&self, path: &PathBuf) -> RToolsResult<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let toml = toml::to_string_pretty(self)?;
        std::fs::write(path, toml)?;
        Ok(())
    }
}

/// Get number of CPUs
fn num_cpus() -> Option<usize> {
    std::thread::available_parallelism()
        .ok()
        .map(std::num::NonZeroUsize::get)
}
