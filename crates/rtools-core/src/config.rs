use crate::{error::RToolsResult, ResourceLimits};
use figment::{
    providers::{Env, Format, Serialized, Toml},
    Provider,
};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Discovered configuration files, ordered from system to project scope.
///
/// This is public so integrations can provide sandboxed locations without
/// changing the process home or working directory.
#[doc(hidden)]
#[derive(Debug, Clone, Default)]
pub struct ConfigLocations {
    /// Machine-wide configuration file.
    pub system: Option<PathBuf>,
    /// Per-user configuration file.
    pub user: Option<PathBuf>,
    /// Project configuration file.
    pub project: Option<PathBuf>,
}

impl ConfigLocations {
    fn discover() -> Self {
        #[cfg(windows)]
        let system = std::env::var_os("PROGRAMDATA")
            .map(PathBuf::from)
            .map(|path| path.join("rtools").join("config.toml"));
        #[cfg(not(windows))]
        let system = Some(PathBuf::from("/etc/rtools/config.toml"));

        let user = dirs::config_dir().map(|path| path.join("rtools").join("config.toml"));
        let project = std::env::current_dir().ok().map(|directory| {
            let primary = directory.join("rtools.toml");
            let legacy = directory.join(".rtools.toml");
            if matches!(primary.try_exists(), Ok(false)) {
                legacy
            } else {
                primary
            }
        });

        Self {
            system,
            user,
            project,
        }
    }
}

/// Main application configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppConfig {
    /// General settings
    pub general: GeneralConfig,
    /// Central resource limits for input processing.
    #[serde(default)]
    pub limits: ResourceLimits,
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
    /// Returns an error when configuration cannot be loaded or is invalid.
    pub fn load(path: Option<&PathBuf>) -> RToolsResult<Self> {
        Self::load_with_provider(
            path,
            &ConfigLocations::discover(),
            Env::prefixed("RTOOLS_").split("__"),
        )
    }

    /// Load from explicit sandboxed discovery locations.
    ///
    /// # Errors
    ///
    /// Returns an error when any present configuration source is unreadable,
    /// malformed, or semantically invalid.
    #[doc(hidden)]
    pub fn load_from_locations(
        path: Option<&PathBuf>,
        locations: &ConfigLocations,
    ) -> RToolsResult<Self> {
        Self::load_with_provider(path, locations, Env::prefixed("RTOOLS_").split("__"))
    }

    fn load_with_provider<P>(
        path: Option<&PathBuf>,
        locations: &ConfigLocations,
        environment: P,
    ) -> RToolsResult<Self>
    where
        P: Provider,
    {
        let mut figment = figment::Figment::from(Serialized::defaults(Self::default()));

        for discovered in [
            locations.system.as_deref(),
            locations.user.as_deref(),
            locations.project.as_deref(),
        ]
        .into_iter()
        .flatten()
        {
            figment = merge_config_file(figment, discovered, false)?;
        }

        if let Some(explicit) = path {
            figment = merge_config_file(figment, explicit, true)?;
        }

        let config: Self = figment
            .merge(environment)
            .extract()
            .map_err(|error| crate::error::RToolsError::configuration_invalid(error.to_string()))?;
        config.validate()?;
        Ok(config)
    }

    /// Validate configuration relationships and documented value ranges.
    ///
    /// # Errors
    ///
    /// Returns `ConfigurationInvalid` when a setting cannot be used safely.
    pub fn validate(&self) -> RToolsResult<()> {
        if self.general.parallel_jobs == 0 {
            return Err(invalid_setting(
                "general.parallel_jobs",
                "must be at least 1",
            ));
        }
        if self.general.temp_dir.as_os_str().is_empty() {
            return Err(invalid_setting("general.temp_dir", "must not be empty"));
        }
        if !matches!(
            self.general.log_level.to_ascii_lowercase().as_str(),
            "trace" | "debug" | "info" | "warn" | "error"
        ) {
            return Err(invalid_setting(
                "general.log_level",
                "must be trace, debug, info, warn, or error",
            ));
        }
        if self.general.max_file_size == 0 {
            return Err(invalid_setting("general.max_file_size", "must be positive"));
        }

        validate_nonzero_limits(&self.limits)?;
        validate_quality("image.default_quality", self.image.default_quality)?;
        validate_quality("image.jpeg_quality", self.image.jpeg_quality)?;
        if self.image.max_dimension == 0 {
            return Err(invalid_setting("image.max_dimension", "must be positive"));
        }
        if self.image.png_compression > 9 {
            return Err(invalid_setting(
                "image.png_compression",
                "must be between 0 and 9",
            ));
        }

        if self.pdf.ocr_language.trim().is_empty() {
            return Err(invalid_setting("pdf.ocr_language", "must not be empty"));
        }
        if self.pdf.ocr_dpi == 0 {
            return Err(invalid_setting("pdf.ocr_dpi", "must be positive"));
        }
        validate_quality("pdf.image_quality", self.pdf.image_quality)?;

        if self.ai.batch_size == 0 {
            return Err(invalid_setting("ai.batch_size", "must be at least 1"));
        }
        if self.api.host.trim().is_empty() {
            return Err(invalid_setting("api.host", "must not be empty"));
        }
        if self.api.port == 0 {
            return Err(invalid_setting("api.port", "must be positive"));
        }
        if self.api.max_upload_size == 0 {
            return Err(invalid_setting("api.max_upload_size", "must be positive"));
        }
        if self.api.auth_enabled
            && self
                .api
                .api_key
                .as_deref()
                .is_none_or(|key| key.trim().is_empty())
        {
            return Err(invalid_setting(
                "api.api_key",
                "must be set when authentication is enabled",
            ));
        }
        if self.api.tls_enabled
            && (self.api.tls_cert_path.is_none() || self.api.tls_key_path.is_none())
        {
            return Err(invalid_setting(
                "api.tls_cert_path/api.tls_key_path",
                "must both be set when TLS is enabled",
            ));
        }
        if self.mcp.server_name.trim().is_empty() {
            return Err(invalid_setting("mcp.server_name", "must not be empty"));
        }

        Ok(())
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

fn merge_config_file(
    figment: figment::Figment,
    path: &Path,
    explicit: bool,
) -> RToolsResult<figment::Figment> {
    match path.try_exists() {
        Ok(false) if !explicit => Ok(figment),
        Ok(false) => Err(crate::error::RToolsError::configuration_invalid(format!(
            "explicit configuration file does not exist: {}",
            path.display()
        ))),
        Err(error) => Err(crate::error::RToolsError::configuration_invalid(format!(
            "cannot access configuration file {}: {error}",
            path.display()
        ))),
        Ok(true) => Ok(figment.merge(Toml::file_exact(path))),
    }
}

fn invalid_setting(name: &str, reason: &str) -> crate::error::RToolsError {
    crate::error::RToolsError::configuration_invalid(format!("{name} {reason}"))
}

fn validate_quality(name: &str, value: u8) -> RToolsResult<()> {
    if !(1..=100).contains(&value) {
        return Err(invalid_setting(name, "must be between 1 and 100"));
    }
    Ok(())
}

fn validate_nonzero_limits(limits: &ResourceLimits) -> RToolsResult<()> {
    for (name, value) in [
        ("limits.max_input_bytes", limits.max_input_bytes),
        ("limits.max_decoded_pixels", limits.max_decoded_pixels),
        ("limits.max_pdf_pages", limits.max_pdf_pages),
        ("limits.max_batch_items", limits.max_batch_items),
        ("limits.max_duration_ms", limits.max_duration_ms),
    ] {
        if value == 0 {
            return Err(invalid_setting(name, "must be positive"));
        }
    }
    Ok(())
}

/// Get number of CPUs
fn num_cpus() -> Option<usize> {
    std::thread::available_parallelism()
        .ok()
        .map(std::num::NonZeroUsize::get)
}
