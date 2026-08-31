pub mod config;
pub mod error;
pub mod input;
pub mod output;
pub mod processor;
pub mod traits;
pub mod types;

pub use config::AppConfig;
pub use error::{RToolsError, RToolsResult};
pub use input::{FileInput, InputSource, ProcessInput};
pub use output::{FileOutput, OutputDestination, ProcessOutput};
pub use traits::{BatchProcessor, Processor};
pub use types::{ContentType, ImageFormat, ImageMetadata, PdfMetadata, ProcessStats};