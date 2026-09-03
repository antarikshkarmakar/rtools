use rtools_core::error::{RToolsError, RToolsResult};
use rtools_core::{FileInput, FileOutput, Processor};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// OCR configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PdfOcrConfig {
    /// Tesseract language
    pub language: String,
    /// DPI for OCR
    pub dpi: u32,
    /// Output path (None = auto-generate)
    pub output: Option<PathBuf>,
    /// Output format
    pub output_format: OcrOutputFormat,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OcrOutputFormat {
    Text,
    SearchablePdf,
}

impl Default for PdfOcrConfig {
    fn default() -> Self {
        Self {
            language: "eng".to_string(),
            dpi: 300,
            output: None,
            output_format: OcrOutputFormat::Text,
        }
    }
}

/// PDF OCR processor
pub struct PdfOcrProcessor;

impl Processor for PdfOcrProcessor {
    type Input = FileInput;
    type Output = FileOutput;
    type Config = PdfOcrConfig;
    type Error = RToolsError;

    fn process_validated(
        &self,
        _input: FileInput,
        _config: PdfOcrConfig,
    ) -> RToolsResult<FileOutput> {
        Err(RToolsError::capability_unavailable(
            "pdf.ocr",
            "No searchable PDF OCR provider is configured",
            "Configure a supported searchable PDF OCR provider",
        ))
    }

    fn validate_config(&self, config: &PdfOcrConfig) -> RToolsResult<()> {
        if config.dpi < 72 || config.dpi > 1200 {
            return Err(RToolsError::invalid_input(
                "DPI must be between 72 and 1200",
            ));
        }
        Ok(())
    }

    fn name(&self) -> &'static str {
        "PdfOcrProcessor"
    }
}
