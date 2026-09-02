pub mod config;
pub mod error;
pub mod input;
pub mod output;
pub mod traits;
pub mod types;

pub use config::AppConfig;
pub use error::{ErrorCode, RToolsError, RToolsResult};
pub use input::{FileInput, InputSource};
pub use output::{resolve_output_path, FileOutput, OutputDestination};
pub use traits::{AIProcessor, BatchProcessor, MetadataExtractor, Processor};
pub use types::{
    ContentType, ExifData, ImageFormat, ImageMetadata, PageSize, PageSizeUnit, PdfMetadata,
    PdfOutputFormat, ProcessStats,
};
