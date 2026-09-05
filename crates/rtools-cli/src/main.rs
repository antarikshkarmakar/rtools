use anyhow as _;
use clap::{CommandFactory, Parser, Subcommand, ValueEnum, ValueHint};
use report::{CliReport, OutputFormat, ReportStatus};
use rtools_core::{
    Capability, CapabilityRegistry, CapabilityState, ProviderDiagnostic, RToolsError, RToolsResult,
};
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use tracing as _;

#[cfg(test)]
use image as _;
#[cfg(test)]
use lopdf as _;

mod capabilities;
mod commands;
mod exit;
mod report;

#[derive(Parser)]
#[command(
    name = "rtools",
    about = "A high-performance image and PDF processing toolkit",
    version,
    long_about = "rtools is a privacy-first, local processing toolkit for images and PDFs.\n\nIt provides CLI, API, and MCP interfaces for powerful file processing operations.",
    next_line_help = true,
    propagate_version = true
)]
struct Cli {
    #[arg(short, long, value_hint = ValueHint::FilePath)]
    config: Option<PathBuf>,
    #[arg(short, long)]
    verbose: bool,
    #[arg(short, long)]
    dry_run: bool,
    #[arg(long, value_enum, default_value_t = OutputFormat::Human)]
    output_format: OutputFormat,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    #[command(alias("img"))]
    Image {
        #[command(subcommand)]
        command: ImageCommands,
    },
    Pdf {
        #[command(subcommand)]
        command: PdfCommands,
    },
    Ai {
        #[command(subcommand)]
        command: AiCommands,
    },
    #[command(
        after_long_help = "Unavailable: run operations individually until typed batch execution is available."
    )]
    Batch {
        #[arg(short, long, value_hint = ValueHint::FilePath)]
        config: PathBuf,
        #[arg(short, long)]
        jobs: Option<usize>,
    },
    Completions {
        #[arg(value_enum)]
        shell: clap_complete::Shell,
    },
    Config {
        #[command(subcommand)]
        command: ConfigCommands,
    },
    /// Diagnose capabilities, configured limits, and writable directories
    Doctor,
}

#[derive(Subcommand)]
enum ImageCommands {
    Compress {
        #[arg(short, long, num_args = 1..)]
        input: Vec<PathBuf>,
        #[arg(short, long)]
        output: Option<PathBuf>,
        #[arg(short, long)]
        quality: Option<u8>,
        #[arg(short, long, value_enum)]
        format: Option<ImageFormatArg>,
        #[arg(long)]
        preserve_metadata: bool,
        #[arg(long)]
        strip_gps: bool,
    },
    Convert {
        #[arg(short, long, num_args = 1..)]
        input: Vec<PathBuf>,
        #[arg(short, long, value_enum)]
        format: ImageFormatArg,
        #[arg(short, long)]
        output: Option<PathBuf>,
        #[arg(short, long)]
        quality: Option<u8>,
    },
    Resize {
        #[arg(short, long, num_args = 1..)]
        input: Vec<PathBuf>,
        #[arg(short, long)]
        width: Option<u32>,
        #[arg(long)]
        height: Option<u32>,
        #[arg(long, default_value = "true")]
        maintain_aspect: bool,
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    Crop {
        #[arg(short, long, num_args = 1..)]
        input: Vec<PathBuf>,
        #[arg(short, long, conflicts_with = "ratio")]
        region: Option<CropRegionArg>,
        #[arg(short = 'a', long, conflicts_with = "region")]
        ratio: Option<AspectRatioArg>,
        #[arg(short, long, value_enum, default_value_t = GravityArg::Center)]
        gravity: GravityArg,
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    Watermark {
        #[arg(short, long, num_args = 1..)]
        input: Vec<PathBuf>,
        #[arg(short, long, conflicts_with = "image")]
        text: Option<String>,
        #[arg(long, conflicts_with = "text")]
        image: Option<PathBuf>,
        #[arg(short, long, value_enum, default_value_t = WatermarkPositionArg::BottomRight)]
        position: WatermarkPositionArg,
        #[arg(long, default_value = "0.5")]
        opacity: f64,
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    Filter {
        #[arg(short, long, num_args = 1..)]
        input: Vec<PathBuf>,
        #[arg(short, long, value_enum)]
        preset: FilmFilterArg,
        #[arg(long, default_value = "1.0")]
        strength: f64,
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    Exif {
        #[arg(short, long, num_args = 1..)]
        input: Vec<PathBuf>,
        #[arg(short, long, value_enum, default_value_t = ExifOutputFormat::Human)]
        format: ExifOutputFormat,
    },
    Ocr {
        #[arg(short, long, num_args = 1..)]
        input: Vec<PathBuf>,
        #[arg(short, long, default_value = "eng")]
        language: String,
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
enum PdfCommands {
    Merge {
        #[arg(short, long, num_args = 2..)]
        input: Vec<PathBuf>,
        #[arg(short, long)]
        output: PathBuf,
    },
    Compress {
        #[arg(short, long)]
        input: PathBuf,
        #[arg(short, long)]
        output: Option<PathBuf>,
        #[arg(short, long, value_enum)]
        level: Option<PdfCompressionArg>,
        #[arg(long)]
        remove_metadata: bool,
    },
    Split {
        #[arg(short, long)]
        input: PathBuf,
        #[arg(short, long)]
        pages: Option<PageSelection>,
        #[arg(short, long)]
        output: PathBuf,
        #[arg(long, default_value = "page_{n}.pdf")]
        filename_pattern: String,
    },
    Text {
        #[arg(short, long)]
        input: PathBuf,
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    ToImage {
        #[arg(short, long)]
        input: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
        #[arg(short, long, value_enum, default_value_t = PdfImageFormatArg::Png)]
        format: PdfImageFormatArg,
        #[arg(long, default_value = "300")]
        dpi: u32,
    },
}

#[derive(Subcommand)]
enum AiCommands {
    Organize {
        #[arg(short, long)]
        input: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
        #[arg(short, long, value_enum, default_value_t = OrganizeMode::Date)]
        strategy: OrganizeMode,
    },
    Rename {
        #[arg(short, long)]
        input: PathBuf,
        #[arg(short, long, default_value = "{date}_{name}_{index}")]
        pattern: String,
        #[arg(long)]
        dry_run: bool,
    },
    AltText {
        #[arg(short, long, num_args = 1..)]
        input: Vec<PathBuf>,
        #[arg(short, long, default_value = "en")]
        language: String,
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    Duplicates {
        #[arg(short, long)]
        input: PathBuf,
        #[arg(short, long, default_value = "0.9")]
        threshold: f64,
        #[arg(short, long, value_enum, default_value_t = DuplicateMode::Report)]
        action: DuplicateMode,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum ImageFormatArg {
    #[value(name = "jpg", alias = "jpeg")]
    Jpeg,
    Png,
    Webp,
    Avif,
    #[value(alias = "tif")]
    Tiff,
    Bmp,
    Gif,
}

impl ImageFormatArg {
    const fn into_core(self) -> rtools_core::ImageFormat {
        match self {
            Self::Jpeg => rtools_core::ImageFormat::Jpeg,
            Self::Png => rtools_core::ImageFormat::Png,
            Self::Webp => rtools_core::ImageFormat::Webp,
            Self::Avif => rtools_core::ImageFormat::Avif,
            Self::Tiff => rtools_core::ImageFormat::Tiff,
            Self::Bmp => rtools_core::ImageFormat::Bmp,
            Self::Gif => rtools_core::ImageFormat::Gif,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct CropRegionArg {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

impl std::str::FromStr for CropRegionArg {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let values = value
            .split(',')
            .map(str::trim)
            .map(|part| {
                part.parse::<u32>()
                    .map_err(|_| "crop region must contain four unsigned integers".to_string())
            })
            .collect::<Result<Vec<_>, _>>()?;
        let [x, y, width, height] = values.as_slice() else {
            return Err("crop region must be x,y,width,height".to_string());
        };
        if *width == 0 || *height == 0 {
            return Err("crop width and height must be positive".to_string());
        }
        Ok(Self {
            x: *x,
            y: *y,
            width: *width,
            height: *height,
        })
    }
}

#[derive(Debug, Clone, Copy)]
struct AspectRatioArg(f64, f64);

impl std::str::FromStr for AspectRatioArg {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let Some((width, height)) = value.split_once(':') else {
            return Err("aspect ratio must be width:height".to_string());
        };
        if height.contains(':') {
            return Err("aspect ratio must contain exactly one colon".to_string());
        }
        let width = width
            .parse::<f64>()
            .map_err(|_| "aspect ratio width must be numeric".to_string())?;
        let height = height
            .parse::<f64>()
            .map_err(|_| "aspect ratio height must be numeric".to_string())?;
        if !width.is_finite() || !height.is_finite() || width <= 0.0 || height <= 0.0 {
            return Err("aspect ratio values must be positive and finite".to_string());
        }
        Ok(Self(width, height))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum GravityArg {
    #[value(alias = "n")]
    North,
    #[value(alias = "s")]
    South,
    #[value(alias = "e")]
    East,
    #[value(alias = "w")]
    West,
    #[value(alias = "ne", alias = "northeast")]
    NorthEast,
    #[value(alias = "nw", alias = "northwest")]
    NorthWest,
    #[value(alias = "se", alias = "southeast")]
    SouthEast,
    #[value(alias = "sw", alias = "southwest")]
    SouthWest,
    Center,
}

impl GravityArg {
    const fn into_image(self) -> rtools_image::crop::Gravity {
        match self {
            Self::North => rtools_image::crop::Gravity::North,
            Self::South => rtools_image::crop::Gravity::South,
            Self::East => rtools_image::crop::Gravity::East,
            Self::West => rtools_image::crop::Gravity::West,
            Self::NorthEast => rtools_image::crop::Gravity::NorthEast,
            Self::NorthWest => rtools_image::crop::Gravity::NorthWest,
            Self::SouthEast => rtools_image::crop::Gravity::SouthEast,
            Self::SouthWest => rtools_image::crop::Gravity::SouthWest,
            Self::Center => rtools_image::crop::Gravity::Center,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum WatermarkPositionArg {
    #[value(alias = "topleft")]
    TopLeft,
    #[value(alias = "topright")]
    TopRight,
    #[value(alias = "bottomleft")]
    BottomLeft,
    #[value(alias = "bottomright")]
    BottomRight,
    Center,
}

impl WatermarkPositionArg {
    const fn into_image(self) -> rtools_image::watermark::WatermarkPosition {
        match self {
            Self::TopLeft => rtools_image::watermark::WatermarkPosition::TopLeft,
            Self::TopRight => rtools_image::watermark::WatermarkPosition::TopRight,
            Self::BottomLeft => rtools_image::watermark::WatermarkPosition::BottomLeft,
            Self::BottomRight => rtools_image::watermark::WatermarkPosition::BottomRight,
            Self::Center => rtools_image::watermark::WatermarkPosition::Center,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum FilmFilterArg {
    #[value(name = "portra", alias = "kodak-portra-400")]
    Portra,
    #[value(name = "gold", alias = "kodak-gold-200")]
    Gold,
    #[value(name = "fuji", alias = "fuji-pro-400h")]
    Fuji,
    #[value(name = "velvia", alias = "fuji-velvia-50")]
    Velvia,
    #[value(name = "polaroid", alias = "polaroid-sx70")]
    Polaroid,
    #[value(name = "trix", alias = "trix-400")]
    Trix,
    #[value(name = "cinestill", alias = "cinestill-800t")]
    Cinestill,
}

impl FilmFilterArg {
    const fn into_image(self) -> rtools_image::filter::FilmFilter {
        match self {
            Self::Portra => rtools_image::filter::FilmFilter::KodakPortra400,
            Self::Gold => rtools_image::filter::FilmFilter::KodakGold200,
            Self::Fuji => rtools_image::filter::FilmFilter::FujiPro400H,
            Self::Velvia => rtools_image::filter::FilmFilter::FujiVelvia50,
            Self::Polaroid => rtools_image::filter::FilmFilter::PolaroidSX70,
            Self::Trix => rtools_image::filter::FilmFilter::TriX400,
            Self::Cinestill => rtools_image::filter::FilmFilter::Cinestill800T,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum PdfCompressionArg {
    Light,
    Medium,
    Heavy,
}

impl PdfCompressionArg {
    const fn into_pdf(self) -> rtools_pdf::compress::PdfCompressionLevel {
        match self {
            Self::Light => rtools_pdf::compress::PdfCompressionLevel::Light,
            Self::Medium => rtools_pdf::compress::PdfCompressionLevel::Medium,
            Self::Heavy => rtools_pdf::compress::PdfCompressionLevel::Heavy,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum PdfImageFormatArg {
    Png,
    #[value(name = "jpg", alias = "jpeg")]
    Jpeg,
    Webp,
}

#[derive(Debug, Clone)]
struct PageSelection(rtools_pdf::split::PageRange);

impl std::str::FromStr for PageSelection {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let mut ranges = Vec::new();
        for part in value.split(',') {
            let part = part.trim();
            if part.is_empty() {
                return Err("page selection contains an empty item".to_string());
            }
            if let Some((start, end)) = part.split_once('-') {
                if end.contains('-') {
                    return Err("page range must contain exactly one hyphen".to_string());
                }
                let start = parse_positive_page(start)?;
                let end = parse_positive_page(end)?;
                if start > end {
                    return Err("page range start must not exceed its end".to_string());
                }
                ranges.push(rtools_pdf::split::PageRange::Range { start, end });
            } else {
                ranges.push(rtools_pdf::split::PageRange::Single(parse_positive_page(
                    part,
                )?));
            }
        }
        let range = if ranges.len() == 1 {
            ranges.remove(0)
        } else {
            rtools_pdf::split::PageRange::Multiple(ranges)
        };
        Ok(Self(range))
    }
}

fn parse_positive_page(value: &str) -> Result<u32, String> {
    let page = value
        .trim()
        .parse::<u32>()
        .map_err(|_| "page numbers must be unsigned integers".to_string())?;
    if page == 0 {
        Err("page numbers are one-indexed and must be positive".to_string())
    } else {
        Ok(page)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum OrganizeMode {
    Date,
    Subject,
    Location,
    Camera,
    Custom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum DuplicateMode {
    Report,
    Move,
    Delete,
    Symlink,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum ExifOutputFormat {
    Human,
    Json,
}

#[derive(Subcommand)]
enum ConfigCommands {
    Show,
    Init {
        #[arg(short, long, default_value = "rtools.toml")]
        output: PathBuf,
    },
    Validate {
        #[arg(short, long)]
        config: PathBuf,
    },
}

#[derive(Serialize)]
struct DoctorResult {
    capabilities: Vec<Capability>,
    provider_diagnostics: Vec<DoctorProviderDiagnostic>,
    configured_limits: rtools_core::ResourceLimits,
    writable_directories: Vec<WritableDirectoryCheck>,
}

#[derive(Serialize)]
struct DoctorProviderDiagnostic {
    provider_id: String,
    state: CapabilityState,
    reason: Option<String>,
    remediation: Option<String>,
    adapter_registered: bool,
    configuration: ProviderConfiguration,
    operations: Vec<ProviderOperation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    executable: Option<ExecutableDiagnostic>,
}

#[derive(Serialize)]
struct ProviderOperation {
    operation_id: String,
    capability_state: CapabilityState,
}

#[derive(Serialize)]
struct ProviderConfiguration {
    state: ProviderConfigurationState,
    reason: String,
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
enum ProviderConfigurationState {
    Configured,
    NotConfigured,
    NotApplicable,
}

#[derive(Serialize)]
struct ExecutableDiagnostic {
    executable: String,
    status: ExecutableProbeStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
enum ExecutableProbeStatus {
    Available,
    Missing,
    Failed,
}

#[derive(Debug, Clone)]
struct ExecutableProbe {
    status: ExecutableProbeStatus,
    version: Option<String>,
    reason: Option<String>,
}

impl ExecutableProbe {
    fn available(version: impl Into<String>) -> Self {
        Self {
            status: ExecutableProbeStatus::Available,
            version: Some(version.into()),
            reason: None,
        }
    }

    fn missing(reason: impl Into<String>) -> Self {
        Self {
            status: ExecutableProbeStatus::Missing,
            version: None,
            reason: Some(reason.into()),
        }
    }

    fn failed(reason: impl Into<String>) -> Self {
        Self {
            status: ExecutableProbeStatus::Failed,
            version: None,
            reason: Some(reason.into()),
        }
    }
}

#[derive(Serialize)]
struct WritableDirectoryCheck {
    label: &'static str,
    path: PathBuf,
    writable: bool,
    reason: String,
    remediation: Option<String>,
}

#[tokio::main]
async fn main() -> ExitCode {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(error) => return render_parse_error(&error),
    };
    let output_format = cli.output_format;
    let operation_id = operation_id_hint(&cli.command);
    if let Commands::Completions { shell } = &cli.command {
        if !cli.dry_run && cli.output_format == OutputFormat::Human {
            if let Err(error) = rtools_core::AppConfig::load(cli.config.as_ref()) {
                let exit_code = exit::for_error(&error);
                let report = CliReport::failure(operation_id, &error);
                return render_and_exit(&report, output_format, exit_code);
            }
            report::render_completions(*shell, &mut Cli::command());
            return ExitCode::SUCCESS;
        }
    }

    match run(cli).await {
        Ok(report) => render_and_exit(&report, output_format, report_exit_code(&report)),
        Err(error) => {
            let exit_code = exit::for_error(&error);
            let report = CliReport::failure(operation_id, &error);
            render_and_exit(&report, output_format, exit_code)
        }
    }
}

fn report_exit_code(report: &CliReport<Value>) -> ExitCode {
    match report.status {
        ReportStatus::Success => ExitCode::SUCCESS,
        ReportStatus::PartialFailure => ExitCode::from(7),
        ReportStatus::Failure => report.failures.first().map_or_else(
            || ExitCode::from(6),
            |failure| exit::for_error_code(failure.code),
        ),
    }
}

fn render_parse_error(error: &clap::Error) -> ExitCode {
    if requested_json_output()
        && !matches!(
            error.kind(),
            clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion
        )
    {
        let parse_error = RToolsError::invalid_input(error.to_string());
        let report = CliReport::failure("cli.parse", &parse_error);
        return render_and_exit(&report, OutputFormat::Json, ExitCode::from(2));
    }

    let exit_code = if error.exit_code() == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(2)
    };
    let _ = error.print();
    exit_code
}

fn requested_json_output() -> bool {
    let mut arguments = std::env::args_os().skip(1);
    while let Some(argument) = arguments.next() {
        if argument == "--output-format" {
            return arguments.next().is_some_and(|format| format == "json");
        }
        if argument == "--output-format=json" {
            return true;
        }
    }
    false
}

fn render_and_exit(
    report: &CliReport<Value>,
    output_format: OutputFormat,
    exit_code: ExitCode,
) -> ExitCode {
    if let Err(error) = report::render(report, output_format) {
        report::render_write_error(&error);
        ExitCode::from(6)
    } else {
        exit_code
    }
}

async fn run(cli: Cli) -> RToolsResult<CliReport<Value>> {
    std::future::ready(()).await;
    if cli.output_format == OutputFormat::Human {
        initialize_human_diagnostics(cli.verbose)?;
    }
    let registry = capabilities::cli_capability_registry()?;
    validate_global_dry_run(&cli)?;
    for operation_id in capabilities::required_operation_ids(&cli.command)? {
        registry.require_available(operation_id)?;
    }
    let mut config = rtools_core::AppConfig::load(cli.config.as_ref())?;
    apply_cli_behavioral_config(&cli.command, &mut config)?;

    let result = match cli.command {
        Commands::Image { command } => commands::image::handle_image_command(command, &config)?,
        Commands::Pdf { command } => commands::pdf::handle_pdf_command(command, &config)?,
        Commands::Ai { command } => commands::ai::handle_ai_command(command, &config, cli.dry_run)?,
        Commands::Batch {
            config: batch_config,
            jobs,
        } => commands::batch::handle_batch_command(batch_config, jobs, &config)?,
        Commands::Config { command } => commands::config::handle_config_command(command, &config)?,
        Commands::Doctor => doctor_result(&registry, &config)?,
        Commands::Completions { .. } => {
            return Err(RToolsError::invalid_input(
                "Completions support only human output without dry-run",
            ));
        }
    };
    Ok(CliReport::from_command_result(result))
}

fn apply_cli_behavioral_config(
    command: &Commands,
    config: &mut rtools_core::AppConfig,
) -> RToolsResult<()> {
    if !matches!(
        command,
        Commands::Image { .. }
            | Commands::Pdf { .. }
            | Commands::Ai { .. }
            | Commands::Batch { .. }
    ) {
        return Ok(());
    }
    let defaults = rtools_core::AppConfig::default();
    let unsupported = if config.general.parallel_jobs != defaults.general.parallel_jobs {
        Some("general.parallel_jobs")
    } else if config.general.temp_dir != defaults.general.temp_dir {
        Some("general.temp_dir")
    } else if config.general.log_level != defaults.general.log_level {
        Some("general.log_level")
    } else if config.general.verbose != defaults.general.verbose {
        Some("general.verbose")
    } else if config.image.webp_lossless != defaults.image.webp_lossless {
        Some("image.webp_lossless")
    } else if config.image.avif_enabled != defaults.image.avif_enabled {
        Some("image.avif_enabled")
    } else if config.image.jpeg_quality != defaults.image.jpeg_quality {
        Some("image.jpeg_quality")
    } else if config.image.png_compression != defaults.image.png_compression {
        Some("image.png_compression")
    } else if config.image.dither != defaults.image.dither {
        Some("image.dither")
    } else if !matches!(
        config.pdf.compression_level,
        rtools_core::config::PdfCompressionLevel::Medium
    ) {
        Some("pdf.compression_level")
    } else if config.pdf.image_quality != defaults.pdf.image_quality {
        Some("pdf.image_quality")
    } else if config.pdf.ocr_language != defaults.pdf.ocr_language {
        Some("pdf.ocr_language")
    } else if config.pdf.ocr_dpi != defaults.pdf.ocr_dpi {
        Some("pdf.ocr_dpi")
    } else if config.limits.max_pdf_pages != defaults.limits.max_pdf_pages {
        Some("limits.max_pdf_pages")
    } else if config.limits.max_duration_ms != defaults.limits.max_duration_ms {
        Some("limits.max_duration_ms")
    } else {
        None
    };
    if let Some(setting) = unsupported {
        return Err(RToolsError::configuration_invalid(format!(
            "{setting} is not honored by executable CLI operations; restore its default value"
        )));
    }

    config.limits.max_input_bytes = config
        .limits
        .max_input_bytes
        .min(config.general.max_file_size);
    let configured_pixels = u64::from(config.image.max_dimension)
        .checked_mul(u64::from(config.image.max_dimension))
        .ok_or_else(|| RToolsError::configuration_invalid("image.max_dimension overflows"))?;
    config.limits.max_decoded_pixels = config.limits.max_decoded_pixels.min(configured_pixels);
    Ok(())
}

fn initialize_human_diagnostics(verbose: bool) -> RToolsResult<()> {
    let default_level = if verbose { "debug" } else { "info" };
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(default_level));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .try_init()
        .map_err(|error| {
            RToolsError::configuration_invalid(format!("failed to initialize diagnostics: {error}"))
        })
}

fn validate_global_dry_run(cli: &Cli) -> RToolsResult<()> {
    if !cli.dry_run {
        return Ok(());
    }
    if matches!(
        &cli.command,
        Commands::Ai {
            command: AiCommands::Organize {
                strategy: OrganizeMode::Date,
                ..
            } | AiCommands::Rename { .. }
        }
    ) {
        return Ok(());
    }
    Err(RToolsError::capability_unavailable(
        "cli.dry_run",
        "This operation does not produce a complete dry-run manifest",
        "Run without --dry-run or use deterministic AI rename/date organization",
    ))
}

fn doctor_result(
    registry: &CapabilityRegistry,
    config: &rtools_core::AppConfig,
) -> RToolsResult<commands::CommandResult> {
    doctor_result_with_probe(registry, config, probe_executable)
}

fn doctor_result_with_probe<F>(
    registry: &CapabilityRegistry,
    config: &rtools_core::AppConfig,
    probe: F,
) -> RToolsResult<commands::CommandResult>
where
    F: Fn(&str) -> ExecutableProbe,
{
    let current_dir = std::env::current_dir()?;
    let result = DoctorResult {
        capabilities: registry.list().into_iter().cloned().collect(),
        provider_diagnostics: provider_diagnostics(registry, config, probe),
        configured_limits: config.limits.clone(),
        writable_directories: vec![
            check_writable_directory("working_directory", &current_dir),
            check_writable_directory("temporary_directory", &config.general.temp_dir),
            check_writable_directory("model_directory", &config.ai.model_dir),
        ],
    };
    Ok(commands::CommandResult::new(
        "doctor.report",
        serde_json::to_value(result)?,
        Vec::new(),
    ))
}

fn provider_diagnostics<F>(
    registry: &CapabilityRegistry,
    config: &rtools_core::AppConfig,
    probe: F,
) -> Vec<DoctorProviderDiagnostic>
where
    F: Fn(&str) -> ExecutableProbe,
{
    let mut diagnostics: BTreeMap<String, (ProviderDiagnostic, Vec<ProviderOperation>)> =
        BTreeMap::new();
    for capability in registry.list() {
        for provider in &capability.provider_diagnostics {
            let entry = diagnostics
                .entry(provider.provider_id.clone())
                .or_insert_with(|| (provider.clone(), Vec::new()));
            entry.1.push(ProviderOperation {
                operation_id: capability.operation_id.clone(),
                capability_state: capability.state,
            });
        }
    }

    diagnostics
        .into_iter()
        .map(
            |(provider_id, (provider, operations))| DoctorProviderDiagnostic {
                configuration: provider_configuration(&provider_id, config),
                executable: (provider_id == "tesseract")
                    .then(|| executable_diagnostic("tesseract", probe("tesseract"))),
                provider_id,
                state: provider.state,
                reason: provider.reason,
                remediation: provider.remediation,
                adapter_registered: false,
                operations,
            },
        )
        .collect()
}

fn provider_configuration(
    provider_id: &str,
    config: &rtools_core::AppConfig,
) -> ProviderConfiguration {
    match provider_id {
        "onnx-runtime" => configured_provider(
            config.ai.ort_path.is_some(),
            "An ONNX Runtime path is configured",
            "No ONNX Runtime path is configured",
        ),
        "pdfium" => configured_provider(
            config.pdf.pdfium_path.is_some(),
            "A PDFium path is configured",
            "No PDFium path is configured",
        ),
        "tesseract" => ProviderConfiguration {
            state: ProviderConfigurationState::NotApplicable,
            reason: "Tesseract is discovered by an executable probe, not configuration".to_string(),
        },
        _ => ProviderConfiguration {
            state: ProviderConfigurationState::NotApplicable,
            reason: "This provider has no configuration field".to_string(),
        },
    }
}

fn configured_provider(
    configured: bool,
    configured_reason: &str,
    missing_reason: &str,
) -> ProviderConfiguration {
    ProviderConfiguration {
        state: if configured {
            ProviderConfigurationState::Configured
        } else {
            ProviderConfigurationState::NotConfigured
        },
        reason: if configured {
            configured_reason.to_string()
        } else {
            missing_reason.to_string()
        },
    }
}

fn executable_diagnostic(executable: &str, probe: ExecutableProbe) -> ExecutableDiagnostic {
    ExecutableDiagnostic {
        executable: executable.to_string(),
        status: probe.status,
        version: probe.version,
        reason: probe.reason,
    }
}

fn probe_executable(executable: &str) -> ExecutableProbe {
    let output = std::process::Command::new(executable)
        .arg("--version")
        .output();
    match output {
        Ok(output) if output.status.success() => {
            let version = [output.stdout, output.stderr]
                .into_iter()
                .filter_map(|stream| String::from_utf8(stream).ok())
                .flat_map(|stream| {
                    stream
                        .lines()
                        .map(str::trim)
                        .filter(|line| !line.is_empty())
                        .map(str::to_string)
                        .collect::<Vec<_>>()
                })
                .next();
            version.map_or_else(
                || ExecutableProbe::failed("Version probe returned no version text"),
                |version| ExecutableProbe::available(version.chars().take(200).collect::<String>()),
            )
        }
        Ok(output) => ExecutableProbe::failed(format!(
            "Version probe exited with status {}",
            output.status.code().unwrap_or(-1)
        )),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            ExecutableProbe::missing("Executable was not found on PATH")
        }
        Err(error) => {
            ExecutableProbe::failed(format!("Version probe could not start: {:?}", error.kind()))
        }
    }
}

fn check_writable_directory(label: &'static str, path: &Path) -> WritableDirectoryCheck {
    if path.exists() && !path.is_dir() {
        return WritableDirectoryCheck {
            label,
            path: path.to_path_buf(),
            writable: false,
            reason: "Configured path exists but is not a directory".to_string(),
            remediation: Some("Configure a directory path".to_string()),
        };
    }
    let mut existing = path;
    while !existing.exists() {
        let Some(parent) = existing.parent() else {
            return WritableDirectoryCheck {
                label,
                path: path.to_path_buf(),
                writable: false,
                reason: "No existing ancestor can be checked".to_string(),
                remediation: Some("Create a writable parent directory".to_string()),
            };
        };
        existing = parent;
    }
    let writable = probe_writable(existing);
    WritableDirectoryCheck {
        label,
        path: path.to_path_buf(),
        writable,
        reason: if path.exists() {
            if writable {
                "Directory accepted a create-and-remove probe".to_string()
            } else {
                "Directory rejected a create-and-remove probe".to_string()
            }
        } else if writable {
            format!(
                "Directory does not exist; writable ancestor {} can create it",
                existing.display()
            )
        } else {
            format!(
                "Directory does not exist and ancestor {} is not writable",
                existing.display()
            )
        },
        remediation: (!writable)
            .then(|| "Grant write access or configure another directory".to_string()),
    }
}

fn probe_writable(directory: &Path) -> bool {
    use std::fs::OpenOptions;
    use std::time::{SystemTime, UNIX_EPOCH};

    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    for attempt in 0..4_u8 {
        let probe = directory.join(format!(
            ".rtools-doctor-{}-{nonce}-{attempt}",
            std::process::id()
        ));
        match OpenOptions::new().write(true).create_new(true).open(&probe) {
            Ok(file) => {
                drop(file);
                return std::fs::remove_file(probe).is_ok();
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(_) => return false,
        }
    }
    false
}

fn operation_id_hint(command: &Commands) -> &'static str {
    match command {
        Commands::Image { command } => match command {
            ImageCommands::Compress { .. } => "image.compress",
            ImageCommands::Convert { .. } => "image.convert",
            ImageCommands::Resize { .. } => "image.resize",
            ImageCommands::Crop { .. } => "image.crop",
            ImageCommands::Watermark { text: Some(_), .. } => "image.watermark.text",
            ImageCommands::Watermark { .. } => "image.watermark.image",
            ImageCommands::Filter { .. } => "image.filter",
            ImageCommands::Exif {
                format: ExifOutputFormat::Human,
                ..
            } => "image.exif.human",
            ImageCommands::Exif {
                format: ExifOutputFormat::Json,
                ..
            } => "image.exif.json",
            ImageCommands::Ocr { .. } => "image.ocr",
        },
        Commands::Pdf { command } => match command {
            PdfCommands::Merge { .. } => "pdf.merge",
            PdfCommands::Compress {
                remove_metadata: true,
                ..
            } => "pdf.compress.metadata",
            PdfCommands::Compress { .. } => "pdf.compress",
            PdfCommands::Split { .. } => "pdf.split",
            PdfCommands::Text { .. } => "pdf.text",
            PdfCommands::ToImage { .. } => "pdf.to_image",
        },
        Commands::Ai { command } => match command {
            AiCommands::Organize { strategy, .. } => match strategy {
                OrganizeMode::Date => "ai.organize.date",
                OrganizeMode::Subject => "ai.organize.subject",
                OrganizeMode::Location => "ai.organize.location",
                OrganizeMode::Camera => "ai.organize.camera",
                OrganizeMode::Custom => "ai.organize.custom",
            },
            AiCommands::Rename { pattern, .. } if pattern.contains("{subject}") => "ai.rename.ai",
            AiCommands::Rename { .. } => "ai.rename.deterministic",
            AiCommands::AltText { .. } => "ai.alt_text",
            AiCommands::Duplicates { action, .. } => match action {
                DuplicateMode::Report => "ai.duplicates.report",
                DuplicateMode::Move => "ai.duplicates.move",
                DuplicateMode::Delete => "ai.duplicates.delete",
                DuplicateMode::Symlink => "ai.duplicates.symlink",
            },
        },
        Commands::Batch { .. } => "batch.run",
        Commands::Completions { .. } => "completions.generate",
        Commands::Config { command } => match command {
            ConfigCommands::Show => "config.show",
            ConfigCommands::Init { .. } => "config.init",
            ConfigCommands::Validate { .. } => "config.validate",
        },
        Commands::Doctor => "doctor.report",
    }
}

#[cfg(test)]
mod tests {
    use super::{capabilities, doctor_result_with_probe, ExecutableProbe};
    use rtools_core::AppConfig;
    use std::path::PathBuf;

    #[test]
    fn doctor_reports_a_found_executable_without_enabling_its_operations() {
        let registry = capabilities::cli_capability_registry().unwrap();
        let mut config = AppConfig::default();
        config.ai.ort_path = Some(PathBuf::from("/configured/onnx-runtime"));
        config.pdf.pdfium_path = Some(PathBuf::from("/configured/pdfium"));

        let report = doctor_result_with_probe(&registry, &config, |executable| {
            if executable == "tesseract" {
                ExecutableProbe::available("tesseract 5.4.0")
            } else {
                ExecutableProbe::missing("not installed")
            }
        })
        .unwrap();

        let providers = report.result["provider_diagnostics"].as_array().unwrap();
        let tesseract = providers
            .iter()
            .find(|provider| provider["provider_id"] == "tesseract")
            .unwrap();
        assert_eq!(tesseract["state"], "unavailable");
        assert_eq!(tesseract["adapter_registered"], false);
        assert_eq!(tesseract["executable"]["status"], "available");
        assert_eq!(tesseract["executable"]["version"], "tesseract 5.4.0");
        assert_eq!(
            tesseract["operations"],
            serde_json::json!([
                { "operation_id": "ai.ocr", "capability_state": "unavailable" },
                { "operation_id": "image.ocr", "capability_state": "unavailable" },
                { "operation_id": "pdf.ocr", "capability_state": "unavailable" },
            ])
        );

        let onnx = providers
            .iter()
            .find(|provider| provider["provider_id"] == "onnx-runtime")
            .unwrap();
        assert_eq!(onnx["configuration"]["state"], "configured");
        assert_eq!(onnx["adapter_registered"], false);
        assert!(
            !report
                .result
                .to_string()
                .contains("/configured/onnx-runtime"),
            "provider diagnostics must not expose configured paths or secrets"
        );
    }
}
