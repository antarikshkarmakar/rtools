use crate::{error::RToolsResult, OutputPolicy, PendingOutput, ResourceLimits};
use figment::{
    providers::{Env, Format, Serialized, Toml},
    value::{Dict, Map, Value},
    Metadata, Profile, Provider,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    fmt,
    path::{Path, PathBuf},
};

/// Internal configuration file locations, ordered from system to project scope.
#[derive(Debug, Clone, Default)]
struct ConfigLocations {
    /// Machine-wide configuration file.
    system: Option<PathBuf>,
    /// Per-user configuration file.
    user: Option<PathBuf>,
    /// Project configuration file.
    project: Option<PathBuf>,
}

#[derive(Clone)]
struct EnvironmentSource {
    values: Vec<(String, String)>,
}

impl EnvironmentSource {
    fn from_process() -> RToolsResult<Self> {
        Self::from_pairs(
            Env::prefixed("RTOOLS_")
                .split("__")
                .iter()
                .map(|(key, value)| (key.to_string(), value)),
        )
    }

    fn from_pairs<I, K, V>(pairs: I) -> RToolsResult<Self>
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<String>,
        V: Into<String>,
    {
        let mut values: Vec<_> = pairs
            .into_iter()
            .map(|(key, value)| (key.into().to_ascii_lowercase(), value.into()))
            .collect();
        let mut paths = BTreeSet::new();
        for (path, _) in &values {
            let components: Vec<_> = path.split('.').collect();
            if components.iter().any(|component| component.is_empty()) {
                return Err(crate::error::RToolsError::configuration_invalid(
                    "invalid RTOOLS_ environment variable path",
                ));
            }
            if paths.iter().any(|existing: &String| {
                let existing: Vec<_> = existing.split('.').collect();
                existing == components
                    || existing.as_slice().starts_with(&components)
                    || components.as_slice().starts_with(&existing)
            }) {
                return Err(crate::error::RToolsError::configuration_invalid(
                    "conflicting RTOOLS_ environment variable paths",
                ));
            }
            paths.insert(path.clone());
        }
        values.sort_by(|left, right| left.0.cmp(&right.0));
        Ok(Self { values })
    }

    #[cfg(test)]
    const fn empty() -> Self {
        Self { values: Vec::new() }
    }
}

impl Provider for EnvironmentSource {
    fn metadata(&self) -> Metadata {
        Metadata::named("RTOOLS_ environment variables")
    }

    fn data(&self) -> Result<Map<Profile, Dict>, figment::Error> {
        let mut dictionary = Dict::new();
        for (path, raw_value) in &self.values {
            let value = raw_value
                .parse::<Value>()
                .expect("value parsing is infallible");
            insert_environment_value(&mut dictionary, path, value);
        }
        let mut profiles = Map::new();
        profiles.insert(Profile::Default, dictionary);
        Ok(profiles)
    }
}

fn insert_environment_value(dictionary: &mut Dict, path: &str, value: Value) {
    fn insert(dictionary: &mut Dict, components: &[&str], value: Value) {
        if let [leaf] = components {
            dictionary.insert((*leaf).to_string(), value);
            return;
        }

        let child = dictionary
            .entry(components[0].to_string())
            .or_insert_with(|| Value::from(Dict::new()));
        if let Value::Dict(_, child) = child {
            insert(child, &components[1..], value);
        }
    }

    let components: Vec<_> = path.split('.').collect();
    insert(dictionary, &components, value);
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
            match std::fs::symlink_metadata(&primary) {
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => legacy,
                _ => primary,
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

#[derive(Clone, Serialize, Deserialize)]
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

impl fmt::Debug for ApiConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ApiConfig")
            .field("host", &self.host)
            .field("port", &self.port)
            .field("max_upload_size", &self.max_upload_size)
            .field("cors_origins", &self.cors_origins)
            .field("auth_enabled", &self.auth_enabled)
            .field("api_key", &self.api_key.as_ref().map(|_| "<redacted>"))
            .field("tls_enabled", &self.tls_enabled)
            .field("tls_cert_path", &self.tls_cert_path)
            .field("tls_key_path", &self.tls_key_path)
            .finish()
    }
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
        let environment = EnvironmentSource::from_process()?;
        Self::load_from_sources(path, &ConfigLocations::discover(), environment)
    }

    /// Load from explicit sandboxed discovery locations.
    ///
    /// # Errors
    ///
    /// Returns an error when any present configuration source is unreadable,
    /// malformed, or semantically invalid.
    #[cfg(test)]
    fn load_from_locations(
        path: Option<&PathBuf>,
        locations: &ConfigLocations,
    ) -> RToolsResult<Self> {
        Self::load_from_sources(path, locations, EnvironmentSource::from_process()?)
    }

    fn load_from_sources(
        path: Option<&PathBuf>,
        locations: &ConfigLocations,
        environment: EnvironmentSource,
    ) -> RToolsResult<Self> {
        let mut figment = figment::Figment::from(Serialized::defaults(Self::default()));

        for (source, discovered) in [
            ("system", locations.system.as_deref()),
            ("user", locations.user.as_deref()),
            ("project", locations.project.as_deref()),
        ] {
            if let Some(discovered) = discovered {
                figment = merge_config_file(figment, discovered, false, source)?;
            }
        }

        if let Some(explicit) = path {
            figment = merge_config_file(figment, explicit, true, "explicit")?;
        }

        let config: Self = figment.merge(environment).extract().map_err(|_| {
            crate::error::RToolsError::configuration_invalid(
                "invalid value or type in RTOOLS_ environment configuration",
            )
        })?;
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
        if self.ai.model_dir.as_os_str().is_empty() {
            return Err(invalid_setting("ai.model_dir", "must not be empty"));
        }
        if self
            .ai
            .ort_path
            .as_ref()
            .is_some_and(|path| path.as_os_str().is_empty())
        {
            return Err(invalid_setting("ai.ort_path", "must not be empty"));
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
        validate_api_config(&self.api)?;
        if self.mcp.server_name.trim().is_empty() {
            return Err(invalid_setting("mcp.server_name", "must not be empty"));
        }

        Ok(())
    }

    /// Save configuration to file.
    ///
    /// # Errors
    ///
    /// Returns an error when the parent directory does not already exist and
    /// validate, the destination exists, or the configuration cannot be
    /// serialized or atomically written.
    pub fn save(&self, path: &PathBuf) -> RToolsResult<()> {
        let toml = toml::to_string_pretty(self)?;
        let pending = PendingOutput::new(path, OutputPolicy::FailIfExists)?;
        std::fs::write(pending.temporary_path(), toml)?;
        pending.commit(|_| Ok(()))?;
        Ok(())
    }
}

fn merge_config_file(
    figment: figment::Figment,
    path: &Path,
    explicit: bool,
    source: &str,
) -> RToolsResult<figment::Figment> {
    match std::fs::symlink_metadata(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound && !explicit => Ok(figment),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Err(crate::error::RToolsError::configuration_invalid(format!(
                "explicit configuration file does not exist: {}",
                path.display()
            )))
        }
        Err(_) => Err(invalid_config_source(source, path)),
        Ok(_) => {
            let merged = figment.merge(Toml::file_exact(path));
            merged
                .extract::<AppConfig>()
                .map_err(|_| invalid_config_source(source, path))?;
            Ok(merged)
        }
    }
}

fn invalid_config_source(source: &str, path: &Path) -> crate::error::RToolsError {
    crate::error::RToolsError::configuration_invalid(format!(
        "invalid or unreadable {source} configuration file: {}",
        path.display()
    ))
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

fn validate_api_config(api: &ApiConfig) -> RToolsResult<()> {
    for (enabled, name, reason) in [
        (
            api.auth_enabled,
            "api.auth_enabled",
            "is unavailable because the REST server does not enforce authentication",
        ),
        (
            api.tls_enabled,
            "api.tls_enabled",
            "is unavailable because the REST server only supports plaintext HTTP",
        ),
    ] {
        if enabled {
            return Err(invalid_setting(name, reason));
        }
    }
    for (configured, name, reason) in [
        (
            api.api_key.is_some(),
            "api.api_key",
            "must not be set because REST authentication is unavailable",
        ),
        (
            api.tls_cert_path.is_some(),
            "api.tls_cert_path",
            "must not be set because REST TLS is unavailable",
        ),
        (
            api.tls_key_path.is_some(),
            "api.tls_key_path",
            "must not be set because REST TLS is unavailable",
        ),
    ] {
        if configured {
            return Err(invalid_setting(name, reason));
        }
    }
    if api.cors_origins.as_slice() != ["*"] {
        return Err(invalid_setting(
            "api.cors_origins",
            "currently supports only the default wildcard origin",
        ));
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

#[cfg(test)]
mod tests {
    use super::{AppConfig, ConfigLocations, EnvironmentSource};
    use crate::ErrorCode;
    use std::{
        ffi::OsStr,
        fs,
        path::{Path, PathBuf},
        process::Command,
    };
    use tempfile::{tempdir, TempDir};

    fn load_isolated(
        explicit: Option<&PathBuf>,
        locations: &ConfigLocations,
    ) -> crate::RToolsResult<AppConfig> {
        AppConfig::load_from_sources(explicit, locations, EnvironmentSource::empty())
    }

    fn load_with_environment(
        explicit: Option<&PathBuf>,
        locations: &ConfigLocations,
        pairs: &[(&str, &str)],
    ) -> crate::RToolsResult<AppConfig> {
        AppConfig::load_from_sources(
            explicit,
            locations,
            EnvironmentSource::from_pairs(pairs.iter().copied())?,
        )
    }

    #[test]
    fn missing_explicit_config_is_an_error() {
        let sandbox = tempdir().unwrap();
        let missing = sandbox.path().join("missing.toml");

        let error = load_isolated(Some(&missing), &ConfigLocations::default()).unwrap_err();

        assert_eq!(error.code(), ErrorCode::ConfigurationInvalid);
    }

    #[test]
    fn loading_config_has_no_directory_side_effect() {
        let sandbox = tempdir().unwrap();
        let explicit = sandbox.path().join("explicit.toml");
        let temporary = sandbox.path().join("not-created");
        fs::write(
            &explicit,
            format!("[general]\ntemp_dir = {:?}\n", temporary.to_string_lossy()),
        )
        .unwrap();

        load_isolated(Some(&explicit), &ConfigLocations::default()).unwrap();

        assert!(!temporary.exists());
    }

    #[cfg(unix)]
    fn create_directory_symlink(target: &Path, link: &Path) {
        std::os::unix::fs::symlink(target, link).unwrap();
    }

    #[cfg(windows)]
    fn create_directory_symlink(target: &Path, link: &Path) {
        std::os::windows::fs::symlink_dir(target, link).unwrap_or_else(|error| {
            panic!(
                "failed to create directory symlink {} -> {}: {error}. Enable Windows Developer Mode or grant SeCreateSymbolicLinkPrivilege so this safety regression can run",
                link.display(),
                target.display()
            )
        });
    }

    #[cfg(any(unix, windows))]
    #[test]
    fn save_rejects_linked_ancestor_before_creating_a_missing_parent() {
        let sandbox = tempdir().unwrap();
        let selected = sandbox.path().join("selected");
        let outside = sandbox.path().join("outside");
        fs::create_dir(&selected).unwrap();
        fs::create_dir(&outside).unwrap();
        create_directory_symlink(&outside, &selected.join("link"));
        let outside_child = outside.join("new-child");
        let output = selected.join("link/new-child/config.toml");

        let error = AppConfig::default().save(&output).unwrap_err();

        assert_eq!(error.code(), ErrorCode::PathPolicyViolation);
        assert!(!outside_child.exists());
        assert!(!output.exists());
        assert!(fs::read_dir(&outside).unwrap().next().is_none());
    }

    #[test]
    fn precedence_is_default_system_user_project_explicit_environment() {
        let sandbox = tempdir().unwrap();
        let system = write(&sandbox, "system.toml", "[general]\nlog_level = \"error\"\nmax_file_size = 100\n\n[image]\ndefault_quality = 10\n");
        let user = write(&sandbox, "user.toml", "[general]\nlog_level = \"warn\"\nmax_file_size = 200\n\n[image]\ndefault_quality = 20\n");
        let project = write(
            &sandbox,
            "project.toml",
            "[general]\nlog_level = \"debug\"\nmax_file_size = 300\n",
        );
        let explicit = write(
            &sandbox,
            "explicit.toml",
            "[general]\nlog_level = \"trace\"\n",
        );
        let locations = ConfigLocations {
            system: Some(system),
            user: Some(user),
            project: Some(project),
        };

        let config = load_with_environment(
            Some(&explicit),
            &locations,
            &[("general.log_level", "info"), ("api.port", "9091")],
        )
        .unwrap();

        assert_eq!(config.general.log_level, "info");
        assert_eq!(config.general.max_file_size, 300);
        assert_eq!(config.image.default_quality, 20);
        assert_eq!(config.api.port, 9091);
        assert_eq!(config.image.jpeg_quality, 85);
    }

    #[test]
    fn missing_discovered_files_are_skipped() {
        let sandbox = tempdir().unwrap();
        let locations = ConfigLocations {
            system: Some(sandbox.path().join("missing-system.toml")),
            user: Some(sandbox.path().join("missing-user.toml")),
            project: Some(sandbox.path().join("missing-project.toml")),
        };

        let config = load_isolated(None, &locations).unwrap();

        assert_eq!(config.general.log_level, "info");
    }

    #[test]
    fn invalid_and_unreadable_discovered_sources_are_value_free_errors() {
        const CANARY: &str = "discovered-file-secret-canary";
        let sandbox = tempdir().unwrap();
        let invalid = write(
            &sandbox,
            "invalid.toml",
            &format!("[api]\napi_key = \"{CANARY}\"\ninvalid = [\n"),
        );
        let invalid_locations = ConfigLocations {
            project: Some(invalid),
            ..ConfigLocations::default()
        };

        let error = load_isolated(None, &invalid_locations).unwrap_err();
        assert_eq!(error.code(), ErrorCode::ConfigurationInvalid);
        assert!(!error.to_string().contains(CANARY));
        assert!(error.to_string().contains("project configuration file"));
        assert!(error.to_string().contains("invalid.toml"));

        let unreadable_locations = ConfigLocations {
            project: Some(sandbox.path().to_path_buf()),
            ..ConfigLocations::default()
        };
        assert!(load_isolated(None, &unreadable_locations).is_err());
    }

    #[test]
    fn explicit_parse_and_type_errors_do_not_leak_values() {
        const PARSE_CANARY: &str = "file-parse-secret-canary";
        const TYPE_CANARY: &str = "file-type-secret-canary";
        let sandbox = tempdir().unwrap();
        let malformed = write(
            &sandbox,
            "malformed.toml",
            &format!("[api]\napi_key = \"{PARSE_CANARY}\"\ninvalid = [\n"),
        );
        let wrong_type = write(
            &sandbox,
            "wrong-type.toml",
            &format!("[api]\nport = \"{TYPE_CANARY}\"\n"),
        );

        for (path, canary) in [(&malformed, PARSE_CANARY), (&wrong_type, TYPE_CANARY)] {
            let error = load_isolated(Some(path), &ConfigLocations::default()).unwrap_err();
            assert_eq!(error.code(), ErrorCode::ConfigurationInvalid);
            assert!(!error.to_string().contains(canary));
            assert!(error.to_string().contains("explicit configuration file"));
        }
    }

    #[test]
    fn environment_type_errors_do_not_leak_values() {
        const CANARY: &str = "environment-secret-canary";

        for pairs in [
            vec![("api.port", CANARY)],
            vec![("api", "{api_key=environment-secret-canary")],
        ] {
            let error =
                load_with_environment(None, &ConfigLocations::default(), &pairs).unwrap_err();
            assert_eq!(error.code(), ErrorCode::ConfigurationInvalid);
            assert!(!error.to_string().contains(CANARY));
            assert!(error.to_string().contains("RTOOLS_ environment"));
        }
    }

    #[test]
    fn app_config_debug_redacts_api_key() {
        const CANARY: &str = "debug-secret-canary";
        let mut config = AppConfig::default();
        config.api.api_key = Some(CANARY.to_string());

        let debug = format!("{config:?}");

        assert!(!debug.contains(CANARY));
        assert!(debug.contains("<redacted>"));
    }

    #[test]
    fn canonical_environment_conflicts_are_order_independent() {
        let forward = EnvironmentSource::from_pairs([("api", "{port=7001}"), ("api.port", "9091")])
            .err()
            .expect("forward conflict must fail");
        let reverse = EnvironmentSource::from_pairs([("api.port", "9091"), ("api", "{port=7001}")])
            .err()
            .expect("reverse conflict must fail");
        let duplicate = EnvironmentSource::from_pairs([("API.PORT", "7001"), ("api.port", "9091")])
            .err()
            .expect("duplicate conflict must fail");

        assert_eq!(forward.code(), ErrorCode::ConfigurationInvalid);
        assert_eq!(forward.to_string(), reverse.to_string());
        assert_eq!(duplicate.code(), ErrorCode::ConfigurationInvalid);
    }

    #[test]
    fn semantic_validation_rejects_invalid_values() {
        let sandbox = tempdir().unwrap();
        let explicit = write(&sandbox, "explicit.toml", "[general]\nparallel_jobs = 0\n");

        let error = load_isolated(Some(&explicit), &ConfigLocations::default()).unwrap_err();

        assert_eq!(error.code(), ErrorCode::ConfigurationInvalid);
    }

    #[test]
    fn api_security_settings_fail_closed_until_the_server_enforces_them() {
        let mut auth = AppConfig::default();
        auth.api.auth_enabled = true;
        auth.api.api_key = Some("configured-secret".to_string());
        let auth_error = auth.validate().unwrap_err();
        assert_eq!(auth_error.code(), ErrorCode::ConfigurationInvalid);
        assert!(auth_error.to_string().contains("api.auth_enabled"));
        assert!(!auth_error.to_string().contains("configured-secret"));

        let mut unused_key = AppConfig::default();
        unused_key.api.api_key = Some("configured-secret".to_string());
        let key_error = unused_key.validate().unwrap_err();
        assert_eq!(key_error.code(), ErrorCode::ConfigurationInvalid);
        assert!(key_error.to_string().contains("api.api_key"));
        assert!(!key_error.to_string().contains("configured-secret"));

        let mut tls = AppConfig::default();
        tls.api.tls_enabled = true;
        let tls_error = tls.validate().unwrap_err();
        assert_eq!(tls_error.code(), ErrorCode::ConfigurationInvalid);
        assert!(tls_error.to_string().contains("api.tls_enabled"));
    }

    #[test]
    fn unused_api_transport_material_and_custom_cors_fail_closed() {
        let mut certificate = AppConfig::default();
        certificate.api.tls_cert_path = Some(PathBuf::from("unused-cert.pem"));
        let cert_error = certificate.validate().unwrap_err();
        assert_eq!(cert_error.code(), ErrorCode::ConfigurationInvalid);
        assert!(cert_error.to_string().contains("api.tls_cert_path"));

        let mut key = AppConfig::default();
        key.api.tls_key_path = Some(PathBuf::from("unused-key.pem"));
        let key_error = key.validate().unwrap_err();
        assert_eq!(key_error.code(), ErrorCode::ConfigurationInvalid);
        assert!(key_error.to_string().contains("api.tls_key_path"));

        let mut cors = AppConfig::default();
        cors.api.cors_origins = vec!["https://example.test".to_string()];
        let cors_error = cors.validate().unwrap_err();
        assert_eq!(cors_error.code(), ErrorCode::ConfigurationInvalid);
        assert!(cors_error.to_string().contains("api.cors_origins"));
    }

    #[cfg(unix)]
    #[test]
    fn dangling_discovered_symlink_is_an_error() {
        use std::os::unix::fs::symlink;

        let sandbox = tempdir().unwrap();
        let dangling = sandbox.path().join("dangling.toml");
        symlink(sandbox.path().join("missing-target.toml"), &dangling).unwrap();
        let locations = ConfigLocations {
            project: Some(dangling),
            ..ConfigLocations::default()
        };

        assert!(load_isolated(None, &locations).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn default_project_discovery_preserves_dangling_primary_symlink() {
        use std::os::unix::fs::symlink;

        const MARKER: &str = "CONFIG_TEST_DANGLING_DISCOVERY";
        if std::env::var_os(MARKER).is_some() {
            let expected_project = std::env::current_dir().unwrap().join("rtools.toml");
            let mut locations = ConfigLocations::discover();
            assert_eq!(
                locations.project.as_deref(),
                Some(expected_project.as_path())
            );
            locations.system = None;
            locations.user = None;

            let error = AppConfig::load_from_sources(None, &locations, EnvironmentSource::empty())
                .unwrap_err();
            let message = error.to_string();
            assert_eq!(error.code(), ErrorCode::ConfigurationInvalid);
            assert!(message.contains("project configuration file"));
            assert!(message.contains("rtools.toml"));
            assert!(!message.contains("ambient-user-secret-canary"));
            assert!(!message.contains("missing-target-secret-canary"));
            return;
        }

        let sandbox = tempdir().unwrap();
        let ambient_user = sandbox.path().join("ambient-user");
        fs::create_dir_all(ambient_user.join("rtools")).unwrap();
        fs::write(
            ambient_user.join("rtools").join("config.toml"),
            "[api]\napi_key = \"ambient-user-secret-canary\"\ninvalid = [\n",
        )
        .unwrap();
        symlink(
            sandbox.path().join("missing-target-secret-canary.toml"),
            sandbox.path().join("rtools.toml"),
        )
        .unwrap();
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .arg("--exact")
            .arg("config::tests::default_project_discovery_preserves_dangling_primary_symlink")
            .arg("--nocapture")
            .current_dir(sandbox.path())
            .env(MARKER, "1")
            .env("XDG_CONFIG_HOME", &ambient_user)
            .env("APPDATA", &ambient_user);
        remove_rtools_environment(&mut command);

        assert_child_passed(command);
    }

    #[test]
    fn tls_paths_must_be_readable_regular_files() {
        let sandbox = tempdir().unwrap();
        let absent = write(&sandbox, "absent.toml", "[api]\ntls_enabled = true\n");
        assert!(load_isolated(Some(&absent), &ConfigLocations::default()).is_err());
        let empty = write(
            &sandbox,
            "empty.toml",
            "[api]\ntls_enabled = true\ntls_cert_path = \"\"\ntls_key_path = \"\"\n",
        );
        assert!(load_isolated(Some(&empty), &ConfigLocations::default()).is_err());

        let missing = write(
            &sandbox,
            "missing.toml",
            &format!(
                "[api]\ntls_enabled = true\ntls_cert_path = {:?}\ntls_key_path = {:?}\n",
                sandbox.path().join("missing.crt").to_string_lossy(),
                sandbox.path().join("missing.key").to_string_lossy()
            ),
        );
        assert!(load_isolated(Some(&missing), &ConfigLocations::default()).is_err());

        let directory = write(
            &sandbox,
            "directory.toml",
            &format!(
                "[api]\ntls_enabled = true\ntls_cert_path = {:?}\ntls_key_path = {:?}\n",
                sandbox.path().to_string_lossy(),
                sandbox.path().to_string_lossy()
            ),
        );
        assert!(load_isolated(Some(&directory), &ConfigLocations::default()).is_err());
    }

    #[test]
    fn readable_tls_files_still_fail_closed_until_tls_is_implemented() {
        let sandbox = tempdir().unwrap();
        let certificate = write(&sandbox, "server.crt", "certificate fixture");
        let key = write(&sandbox, "server.key", "key fixture");
        let explicit = write(
            &sandbox,
            "explicit.toml",
            &format!(
                "[api]\ntls_enabled = true\ntls_cert_path = {:?}\ntls_key_path = {:?}\n",
                certificate.to_string_lossy(),
                key.to_string_lossy()
            ),
        );

        let error = load_isolated(Some(&explicit), &ConfigLocations::default()).unwrap_err();
        assert_eq!(error.code(), ErrorCode::ConfigurationInvalid);
        assert!(error.to_string().contains("api.tls_enabled"));
    }

    #[test]
    fn ai_paths_are_nonempty_but_need_not_exist() {
        let sandbox = tempdir().unwrap();
        let empty_model = write(&sandbox, "empty-model.toml", "[ai]\nmodel_dir = \"\"\n");
        assert!(load_isolated(Some(&empty_model), &ConfigLocations::default()).is_err());
        let empty_ort = write(&sandbox, "empty-ort.toml", "[ai]\nort_path = \"\"\n");
        assert!(load_isolated(Some(&empty_ort), &ConfigLocations::default()).is_err());

        let missing_paths = write(
            &sandbox,
            "missing-ai-paths.toml",
            "[ai]\nmodel_dir = \"not-provisioned/models\"\nort_path = \"not-provisioned/ort\"\n",
        );
        load_isolated(Some(&missing_paths), &ConfigLocations::default()).unwrap();
    }

    #[test]
    fn process_environment_conflict_is_rejected_forward_order() {
        run_conflict_child(
            "config::tests::process_environment_conflict_is_rejected_forward_order",
            "CONFIG_TEST_CONFLICT_FORWARD",
            &[("RTOOLS_API", "{port=7001}"), ("RTOOLS_API__PORT", "9091")],
        );
    }

    #[test]
    fn process_environment_conflict_is_rejected_reverse_order() {
        run_conflict_child(
            "config::tests::process_environment_conflict_is_rejected_reverse_order",
            "CONFIG_TEST_CONFLICT_REVERSE",
            &[("RTOOLS_API__PORT", "9091"), ("RTOOLS_API", "{port=7001}")],
        );
    }

    #[test]
    fn process_environment_scalar_and_nested_values_are_loaded() {
        const MARKER: &str = "CONFIG_TEST_ENV_VALUES";
        if std::env::var_os(MARKER).is_some() {
            let config = AppConfig::load_from_locations(None, &ConfigLocations::default()).unwrap();
            assert_eq!(config.general.log_level, "error");
            assert_eq!(config.api.port, 9091);
            assert!(config.image.webp_lossless);
            return;
        }

        run_child(
            "config::tests::process_environment_scalar_and_nested_values_are_loaded",
            MARKER,
            &[
                ("RTOOLS_GENERAL__LOG_LEVEL", "error"),
                ("RTOOLS_API__PORT", "9091"),
                ("RTOOLS_IMAGE__WEBP_LOSSLESS", "true"),
            ],
        );
    }

    #[test]
    fn process_malformed_environment_error_is_value_free() {
        process_environment_error_is_value_free(
            "config::tests::process_malformed_environment_error_is_value_free",
            "CONFIG_TEST_MALFORMED_ENV",
            "RTOOLS_API",
            "{api_key=malformed-environment-secret-canary",
            "malformed-environment-secret-canary",
        );
    }

    #[test]
    fn process_wrong_type_environment_error_is_value_free() {
        process_environment_error_is_value_free(
            "config::tests::process_wrong_type_environment_error_is_value_free",
            "CONFIG_TEST_WRONG_TYPE_ENV",
            "RTOOLS_API__PORT",
            "wrong-type-environment-secret-canary",
            "wrong-type-environment-secret-canary",
        );
    }

    #[test]
    fn injected_sources_ignore_hostile_ambient_environment_and_discovery() {
        const MARKER: &str = "CONFIG_TEST_HOSTILE_AMBIENT";
        if std::env::var_os(MARKER).is_some() {
            let config = load_isolated(None, &ConfigLocations::default()).unwrap();
            assert_eq!(config.general.log_level, "info");
            return;
        }

        run_child(
            "config::tests::injected_sources_ignore_hostile_ambient_environment_and_discovery",
            MARKER,
            &[("RTOOLS_GENERAL", "{log_level=error}")],
        );
    }

    fn run_conflict_child(test_name: &str, marker: &str, environment: &[(&str, &str)]) {
        if std::env::var_os(marker).is_some() {
            let error =
                AppConfig::load_from_locations(None, &ConfigLocations::default()).unwrap_err();
            assert_eq!(error.code(), ErrorCode::ConfigurationInvalid);
            return;
        }
        run_child(test_name, marker, environment);
    }

    fn process_environment_error_is_value_free(
        test_name: &str,
        marker: &str,
        key: &str,
        value: &str,
        canary: &str,
    ) {
        if std::env::var_os(marker).is_some() {
            let error =
                AppConfig::load_from_locations(None, &ConfigLocations::default()).unwrap_err();
            assert_eq!(error.code(), ErrorCode::ConfigurationInvalid);
            assert!(!error.to_string().contains(canary));
            assert!(!format!("{error:?}").contains(canary));
            return;
        }
        run_child(test_name, marker, &[(key, value)]);
    }

    fn run_child(test_name: &str, marker: &str, environment: &[(&str, &str)]) {
        let sandbox = tempdir().unwrap();
        let discovery_root = sandbox.path().join("hostile-discovery");
        fs::create_dir_all(discovery_root.join("rtools")).unwrap();
        fs::write(
            discovery_root.join("rtools").join("config.toml"),
            "[general]\nlog_level = \"error\"\n",
        )
        .unwrap();

        let mut command = Command::new(std::env::current_exe().unwrap());
        command.arg("--exact").arg(test_name).arg("--nocapture");
        remove_rtools_environment(&mut command);
        command
            .env(marker, "1")
            .env("XDG_CONFIG_HOME", &discovery_root)
            .env("APPDATA", &discovery_root);
        for (key, value) in environment {
            command.env(key, value);
        }

        assert_child_passed(command);
    }

    fn assert_child_passed(mut command: Command) {
        let output = command.output().unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            output.status.success(),
            "isolated test failed\nstdout:\n{stdout}\nstderr:\n{stderr}"
        );
        assert!(
            stdout.contains("running 1 test") && stdout.contains("1 passed"),
            "isolated test did not execute the requested test\nstdout:\n{stdout}\nstderr:\n{stderr}"
        );
    }

    fn remove_rtools_environment(command: &mut Command) {
        for (key, _) in std::env::vars_os() {
            if starts_with_rtools(&key) {
                command.env_remove(key);
            }
        }
    }

    fn starts_with_rtools(key: &OsStr) -> bool {
        key.to_string_lossy()
            .get(..7)
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case("RTOOLS_"))
    }

    fn write(sandbox: &TempDir, name: &str, contents: &str) -> PathBuf {
        let path = sandbox.path().join(name);
        fs::write(&path, contents).unwrap();
        path
    }
}
