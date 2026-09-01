use clap::{CommandFactory, Parser, Subcommand, ValueHint};
use std::path::PathBuf;

mod commands;

#[derive(Parser)]
#[command(
    name = "rtools",
    about = "A high-performance image and PDF processing toolkit",
    version,
    long_about = "rtools is a privacy-first, local processing toolkit for images and PDFs.\n\nIt provides CLI, API, and MCP interfaces for powerful file processing operations."
)]
#[command(propagate_version = true)]
struct Cli {
    /// Configuration file path
    #[arg(short, long, value_hint = ValueHint::FilePath)]
    config: Option<PathBuf>,

    /// Verbose output
    #[arg(short, long)]
    verbose: bool,

    /// Dry run mode (preview changes without applying)
    #[arg(short, long)]
    dry_run: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Image processing operations
    #[command(alias("img"))]
    Image {
        #[command(subcommand)]
        command: ImageCommands,
    },

    /// PDF processing operations
    Pdf {
        #[command(subcommand)]
        command: PdfCommands,
    },

    /// AI-powered operations
    Ai {
        #[command(subcommand)]
        command: AiCommands,
    },

    /// Batch processing with config file
    Batch {
        /// Batch configuration file
        #[arg(short, long, value_hint = ValueHint::FilePath)]
        config: PathBuf,

        /// Number of parallel jobs
        #[arg(short, long)]
        jobs: Option<usize>,
    },

    /// Generate shell completions
    Completions {
        /// Shell to generate for
        #[arg(value_enum)]
        shell: clap_complete::Shell,
    },

    /// Show configuration information
    Config {
        #[command(subcommand)]
        command: ConfigCommands,
    },
}

#[derive(Subcommand)]
enum ImageCommands {
    /// Compress images
    Compress {
        /// Input file(s)
        #[arg(short, long, num_args = 1..)]
        input: Vec<PathBuf>,

        /// Output path (file or directory)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Quality (1-100)
        #[arg(short, long, default_value = "85")]
        quality: u8,

        /// Output format
        #[arg(short, long)]
        format: Option<String>,

        /// Preserve metadata
        #[arg(long)]
        preserve_metadata: bool,

        /// Strip GPS data
        #[arg(long)]
        strip_gps: bool,
    },

    /// Convert image format
    Convert {
        /// Input file(s)
        #[arg(short, long, num_args = 1..)]
        input: Vec<PathBuf>,

        /// Target format
        #[arg(short, long)]
        format: String,

        /// Output directory
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Quality for lossy formats
        #[arg(short, long, default_value = "85")]
        quality: u8,
    },

    /// Resize images
    Resize {
        /// Input file(s)
        #[arg(short, long, num_args = 1..)]
        input: Vec<PathBuf>,

        /// Target width
        #[arg(short, long)]
        width: Option<u32>,

        /// Target height
        #[arg(short, long)]
        height: Option<u32>,

        /// Maintain aspect ratio
        #[arg(long, default_value = "true")]
        maintain_aspect: bool,

        /// Output directory
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Crop images
    Crop {
        /// Input file(s)
        #[arg(short, long, num_args = 1..)]
        input: Vec<PathBuf>,

        /// Crop region (x,y,width,height)
        #[arg(short, long)]
        region: Option<String>,

        /// Aspect ratio (16:9, 4:3, 1:1, etc.)
        #[arg(short, long)]
        ratio: Option<String>,

        /// Gravity point
        #[arg(short, long, default_value = "center")]
        gravity: String,

        /// Output directory
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Add watermark
    Watermark {
        /// Input file(s)
        #[arg(short, long, num_args = 1..)]
        input: Vec<PathBuf>,

        /// Watermark text
        #[arg(short, long)]
        text: Option<String>,

        /// Watermark image
        #[arg(short, long)]
        image: Option<PathBuf>,

        /// Position
        #[arg(short, long, default_value = "bottom-right")]
        position: String,

        /// Opacity (0.0-1.0)
        #[arg(long, default_value = "0.5")]
        opacity: f64,

        /// Output directory
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Apply film filter
    Filter {
        /// Input file(s)
        #[arg(short, long, num_args = 1..)]
        input: Vec<PathBuf>,

        /// Filter preset
        #[arg(short, long)]
        preset: String,

        /// Filter strength (0.0-1.0)
        #[arg(long, default_value = "1.0")]
        strength: f64,

        /// Output directory
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// View EXIF metadata
    Exif {
        /// Input file(s)
        #[arg(short, long, num_args = 1..)]
        input: Vec<PathBuf>,

        /// Output format (json, text)
        #[arg(short, long, default_value = "text")]
        format: String,
    },

    /// Extract text from images (OCR)
    Ocr {
        /// Input file(s)
        #[arg(short, long, num_args = 1..)]
        input: Vec<PathBuf>,

        /// Language
        #[arg(short, long, default_value = "eng")]
        language: String,

        /// Output file
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
enum PdfCommands {
    /// Merge PDF files
    Merge {
        /// Input PDF files
        #[arg(short, long, num_args = 2..)]
        input: Vec<PathBuf>,

        /// Output file
        #[arg(short, long)]
        output: PathBuf,
    },

    /// Compress PDF
    Compress {
        /// Input PDF file
        #[arg(short, long)]
        input: PathBuf,

        /// Output file
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Compression level (light, medium, heavy)
        #[arg(short, long, default_value = "medium")]
        level: String,
    },

    /// Split PDF into pages
    Split {
        /// Input PDF file
        #[arg(short, long)]
        input: PathBuf,

        /// Page range (e.g., "1-5,10,15-20")
        #[arg(short, long)]
        pages: Option<String>,

        /// Output directory
        #[arg(short, long)]
        output: PathBuf,
    },

    /// Extract text from PDF
    Text {
        /// Input PDF file
        #[arg(short, long)]
        input: PathBuf,

        /// Output file
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Convert PDF to images
    ToImage {
        /// Input PDF file
        #[arg(short, long)]
        input: PathBuf,

        /// Output directory
        #[arg(short, long)]
        output: PathBuf,

        /// Image format (png, jpg, webp)
        #[arg(short, long, default_value = "png")]
        format: String,

        /// DPI
        #[arg(long, default_value = "300")]
        dpi: u32,
    },
}

#[derive(Subcommand)]
enum AiCommands {
    /// Organize photos using AI
    Organize {
        /// Input directory
        #[arg(short, long)]
        input: PathBuf,

        /// Output directory
        #[arg(short, long)]
        output: PathBuf,

        /// Organization strategy (date, subject, location)
        #[arg(short, long, default_value = "date")]
        strategy: String,
    },

    /// Rename photos using AI
    Rename {
        /// Input directory
        #[arg(short, long)]
        input: PathBuf,

        /// Filename pattern
        #[arg(short, long, default_value = "{date}_{subject}_{index}")]
        pattern: String,

        /// Dry run mode
        #[arg(long)]
        dry_run: bool,
    },

    /// Generate alt text for images
    AltText {
        /// Input file(s)
        #[arg(short, long, num_args = 1..)]
        input: Vec<PathBuf>,

        /// Language
        #[arg(short, long, default_value = "en")]
        language: String,

        /// Output file
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Find duplicate images
    Duplicates {
        /// Input directory
        #[arg(short, long)]
        input: PathBuf,

        /// Similarity threshold (0.0-1.0)
        #[arg(short, long, default_value = "0.9")]
        threshold: f64,

        /// Action (report, move, delete)
        #[arg(short, long, default_value = "report")]
        action: String,
    },
}

#[derive(Subcommand)]
enum ConfigCommands {
    /// Show current configuration
    Show,

    /// Generate default configuration file
    Init {
        /// Output path
        #[arg(short, long, default_value = "rtools.toml")]
        output: PathBuf,
    },

    /// Validate configuration file
    Validate {
        /// Configuration file to validate
        #[arg(short, long)]
        config: PathBuf,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Initialize tracing
    let subscriber = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    // Load configuration
    let config = rtools_core::AppConfig::load(cli.config.as_ref())?;

    match cli.command {
        Commands::Image { command } => commands::image::handle_image_command(command, &config).await,
        Commands::Pdf { command } => commands::pdf::handle_pdf_command(command, &config).await,
        Commands::Ai { command } => commands::ai::handle_ai_command(command, &config).await,
        Commands::Batch { config: batch_config, jobs } => {
            commands::batch::handle_batch_command(batch_config, jobs, &config).await
        }
        Commands::Completions { shell } => {
            let mut cmd = Cli::command();
            clap_complete::generate(shell, &mut cmd, "rtools", &mut std::io::stdout());
            Ok(())
        }
        Commands::Config { command } => commands::config::handle_config_command(command),
    }
}