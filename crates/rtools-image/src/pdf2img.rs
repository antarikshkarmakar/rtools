use rtools_core::error::{RToolsError, RToolsResult};
use rtools_core::{FileInput, FileOutput, Processor};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// PDF to image configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pdf2ImgConfig {
    pub output_dir: PathBuf,
    pub format: rtools_core::ImageFormat,
    pub dpi: u32,
    pub page: Option<u32>,
}

impl Default for Pdf2ImgConfig {
    fn default() -> Self {
        Self {
            output_dir: PathBuf::from("pdf_images"),
            format: rtools_core::ImageFormat::Png,
            dpi: 300,
            page: None,
        }
    }
}

/// PDF to image processor
pub struct Pdf2ImgProcessor;

impl Processor for Pdf2ImgProcessor {
    type Input = FileInput;
    type Output = Vec<FileOutput>;
    type Config = Pdf2ImgConfig;
    type Error = RToolsError;

    fn process(&self, input: FileInput, _config: Pdf2ImgConfig) -> RToolsResult<Vec<FileOutput>> {
        let _path = input
            .source
            .as_path()
            .ok_or_else(|| RToolsError::invalid_input("Pdf2Img requires a file path input"))?;

        // PDFium or external renderer binding
        Ok(Vec::new())
    }

    fn validate_config(&self, _config: &Pdf2ImgConfig) -> RToolsResult<()> {
        Ok(())
    }

    fn name(&self) -> &'static str {
        "Pdf2ImgProcessor"
    }
}
