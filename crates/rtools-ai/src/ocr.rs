use rtools_core::error::{RToolsError, RToolsResult};
use rtools_core::types::ProcessStats;
use rtools_core::{FileInput, Processor};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// OCR configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrConfig {
    /// Language for OCR
    pub language: String,
    /// DPI for OCR
    pub dpi: u32,
    /// Output format
    pub output_format: OcrOutputFormat,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OcrOutputFormat {
    Text,
    Json,
}

impl Default for OcrConfig {
    fn default() -> Self {
        Self {
            language: "eng".to_string(),
            dpi: 300,
            output_format: OcrOutputFormat::Text,
        }
    }
}

/// OCR processor
pub struct OcrProcessor;

impl Processor for OcrProcessor {
    type Input = FileInput;
    type Output = OcrResult;
    type Config = OcrConfig;
    type Error = RToolsError;

    fn process_validated(&self, _input: FileInput, _config: OcrConfig) -> RToolsResult<OcrResult> {
        Err(RToolsError::capability_unavailable(
            "ai.ocr",
            "No image OCR provider is configured",
            "Configure a supported image OCR provider",
        ))
    }

    fn validate_config(&self, config: &OcrConfig) -> RToolsResult<()> {
        if config.dpi < 72 || config.dpi > 600 {
            return Err(RToolsError::invalid_input("DPI must be between 72 and 600"));
        }
        Ok(())
    }

    fn name(&self) -> &'static str {
        "OcrProcessor"
    }
}

/// OCR result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrResult {
    pub path: PathBuf,
    pub text: String,
    pub confidence: f64,
    pub stats: ProcessStats,
}
