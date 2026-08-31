use rtools_core::error::{RToolsError, RToolsResult};
use rtools_core::{FileInput, FileOutput, Processor};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PdfExtractConfig {
    pub output_dir: PathBuf,
    pub image_format: String,
    pub dpi: u32,
    pub pages: Option<Vec<u32>>,
}

impl Default for PdfExtractConfig {
    fn default() -> Self {
        Self {
            output_dir: PathBuf::from("extracted"),
            image_format: "png".to_string(),
            dpi: 300,
            pages: None,
        }
    }
}

pub struct PdfExtractProcessor;

impl Processor for PdfExtractProcessor {
    type Input = FileInput;
    type Output = Vec<FileOutput>;
    type Config = PdfExtractConfig;
    type Error = RToolsError;

    fn process(&self, _input: FileInput, _config: PdfExtractConfig) -> RToolsResult<Vec<FileOutput>> {
        Err(RToolsError::not_implemented("PDF image extraction not yet implemented"))
    }

    fn validate_config(&self, _config: &PdfExtractConfig) -> RToolsResult<()> {
        Ok(())
    }

    fn name(&self) -> &str {
        "PdfExtractProcessor"
    }
}
