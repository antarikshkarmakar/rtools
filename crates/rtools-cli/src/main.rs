use clap::{CommandFactory, Parser, Subcommand, ValueHint};
use std::path::PathBuf;

mod capabilities;
mod commands;

#[derive(Parser)]
#[command(
    name = "rtools",
    about = "A high-performance image and PDF processing toolkit",
    version,
    long_about = "rtools is a privacy-first, local processing toolkit for images and PDFs.\n\nIt provides CLI, API, and MCP interfaces for powerful file processing operations.",
    next_line_help = true
)]
#[command(
    propagate_version = true,
    after_long_help = "Examples:\n  Compress an image:          rtools image compress -i photo.jpg -q 80\n  Convert to WebP:            rtools image convert -i photo.png -f webp\n  Resize to 1920px wide:      rtools image resize -i photo.jpg -w 1920\n  Crop to 16:9:               rtools image crop -i photo.jpg -a 16:9\n  Add an image watermark:     rtools image watermark -i photo.jpg --image logo.png\n  Apply a film filter:        rtools image filter -i photo.jpg -p portra\n  Read EXIF metadata:         rtools image exif -i photo.jpg\n  Merge PDFs:                 rtools pdf merge -i a.pdf b.pdf -o merged.pdf\n  Split a PDF:                rtools pdf split -i doc.pdf -o out/ -p 1-5,10\n  Find duplicate photos:      rtools ai duplicates -i photos/ -a report\n\nTip: run `rtools <command> --help` for full details on any command."
)]
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

    /// Batch processing (unavailable until typed recipe execution is implemented)
    #[command(
        after_long_help = "Unavailable: run operations individually until typed batch execution is available."
    )]
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
    #[command(
        after_long_help = "Examples:\n  rtools image compress -i photo.jpg -q 70\n  rtools image compress -i a.jpg b.jpg -f webp -o out/"
    )]
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

        /// Output format (jpg, jpeg, png, webp, avif, tiff, bmp, gif)
        #[arg(short, long)]
        format: Option<String>,

        /// Preserve metadata (unavailable; use drop-all until verified export exists)
        #[arg(long)]
        preserve_metadata: bool,

        /// Strip only GPS metadata (unavailable; use drop-all until selective removal exists)
        #[arg(long)]
        strip_gps: bool,
    },

    /// Convert image format
    #[command(after_long_help = "Example:\n  rtools image convert -i photo.png -f webp -q 80")]
    Convert {
        /// Input file(s)
        #[arg(short, long, num_args = 1..)]
        input: Vec<PathBuf>,

        /// Target format (jpg, jpeg, png, webp, avif, tiff, bmp, gif)
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
    #[command(
        after_long_help = "Examples:\n  rtools image resize -i photo.jpg -w 1920\n  rtools image resize -i photo.jpg -w 800 -h 600 --no-maintain-aspect"
    )]
    Resize {
        /// Input file(s)
        #[arg(short, long, num_args = 1..)]
        input: Vec<PathBuf>,

        /// Target width
        #[arg(short, long)]
        width: Option<u32>,

        /// Target height
        #[arg(long)]
        height: Option<u32>,

        /// Maintain aspect ratio
        #[arg(long, default_value = "true")]
        maintain_aspect: bool,

        /// Output directory
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Crop images
    #[command(
        after_long_help = "Examples:\n  rtools image crop -i photo.jpg -a 16:9\n  rtools image crop -i photo.jpg -r 100,100,800,600"
    )]
    Crop {
        /// Input file(s)
        #[arg(short, long, num_args = 1..)]
        input: Vec<PathBuf>,

        /// Crop region (x,y,width,height)
        #[arg(short, long)]
        region: Option<String>,

        /// Aspect ratio (16:9, 4:3, 1:1, etc.)
        #[arg(short = 'a', long)]
        ratio: Option<String>,

        /// Gravity point
        #[arg(short, long, default_value = "center")]
        gravity: String,

        /// Output directory
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Add a watermark (image available; text unavailable)
    #[command(
        after_long_help = "Example:\n  rtools image watermark -i photo.jpg --image logo.png -p top-left\n\nText watermarks are unavailable; use --image until text rendering is configured."
    )]
    Watermark {
        /// Input file(s)
        #[arg(short, long, num_args = 1..)]
        input: Vec<PathBuf>,

        /// Watermark text (unavailable; use --image until text rendering is configured)
        #[arg(short, long)]
        text: Option<String>,

        /// Watermark image
        #[arg(long)]
        image: Option<PathBuf>,

        /// Position (top-left, top-right, bottom-left, bottom-right, center)
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
    #[command(
        after_long_help = "Presets: portra, gold, fuji, velvia, polaroid, trix, cinestill\n\nExample:\n  rtools image filter -i photo.jpg -p portra --strength 0.8"
    )]
    Filter {
        /// Input file(s)
        #[arg(short, long, num_args = 1..)]
        input: Vec<PathBuf>,

        /// Filter preset (portra, gold, fuji, velvia, polaroid, trix, cinestill)
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
    #[command(
        after_long_help = "Examples:\n  rtools image exif -i photo.jpg\n  rtools image exif -i photo.jpg -f json"
    )]
    Exif {
        /// Input file(s)
        #[arg(short, long, num_args = 1..)]
        input: Vec<PathBuf>,

        /// Output format (json, text)
        #[arg(short, long, default_value = "text")]
        format: String,
    },

    /// Extract text from images (unavailable until an OCR provider is configured)
    #[command(
        after_long_help = "Unavailable: configure an OCR provider; run rtools doctor once available in this release."
    )]
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
    #[command(after_long_help = "Example:\n  rtools pdf merge -i a.pdf b.pdf c.pdf -o merged.pdf")]
    Merge {
        /// Input PDF files
        #[arg(short, long, num_args = 2..)]
        input: Vec<PathBuf>,

        /// Output file
        #[arg(short, long)]
        output: PathBuf,
    },

    /// Compress PDF
    #[command(
        after_long_help = "Levels: light, medium (default), heavy\n\nExample:\n  rtools pdf compress -i doc.pdf -l heavy -o small.pdf"
    )]
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
    #[command(
        after_long_help = "Page range syntax: comma-separated pages and ranges, e.g. \"1-5,10,15-20\"\n\nExamples:\n  rtools pdf split -i doc.pdf -o pages/\n  rtools pdf split -i doc.pdf -o pages/ --pages 1-5,10"
    )]
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

    /// Extract text from PDF (unavailable in this release)
    #[command(
        after_long_help = "Unavailable: use a verified PDF text provider once one is registered."
    )]
    Text {
        /// Input PDF file
        #[arg(short, long)]
        input: PathBuf,

        /// Output file
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Convert PDF to images (unavailable until a rendering provider is configured)
    #[command(
        after_long_help = "Unavailable: configure a PDF rendering provider; run rtools doctor once available in this release."
    )]
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
    /// Organize photos using AI (unavailable until a classifier is configured)
    #[command(
        after_long_help = "Unavailable: use explicit filesystem organization until a supported classifier is configured."
    )]
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

    /// Rename photos using AI (unavailable until a description provider is configured)
    #[command(
        after_long_help = "Unavailable: use a deterministic rename tool until a supported description provider is configured."
    )]
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

    /// Generate alt text (unavailable until a captioning provider is configured)
    #[command(
        after_long_help = "Unavailable: configure a captioning provider; run rtools doctor once available in this release."
    )]
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
    #[command(
        after_long_help = "Action: report (default), move, delete\n\nExample:\n  rtools ai duplicates -i photos/ -t 0.95 -a report"
    )]
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

    let capability_registry = capabilities::cli_capability_registry()?;
    for operation_id in capabilities::required_operation_ids(&cli.command)? {
        capability_registry.require_available(operation_id)?;
    }

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
        Commands::Image { command } => {
            commands::image::handle_image_command(command, &config).await
        }
        Commands::Pdf { command } => commands::pdf::handle_pdf_command(command, &config).await,
        Commands::Ai { command } => commands::ai::handle_ai_command(command, &config).await,
        Commands::Batch {
            config: batch_config,
            jobs,
        } => commands::batch::handle_batch_command(batch_config, jobs, &config).await,
        Commands::Completions { shell } => {
            let mut cmd = Cli::command();
            clap_complete::generate(shell, &mut cmd, "rtools", &mut std::io::stdout());
            Ok(())
        }
        Commands::Config { command } => commands::config::handle_config_command(command, &config),
    }
}
