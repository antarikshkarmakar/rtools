use rtools_core::error::{RToolsError, RToolsResult};
use rtools_core::types::ProcessStats;
use rtools_core::{FileInput, Processor};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// AI alt text configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AltTextConfig {
    /// Language for alt text
    pub language: String,
    /// Maximum length of alt text
    pub max_length: usize,
    /// Output format
    pub output_format: AltTextOutputFormat,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AltTextOutputFormat {
    /// Plain text
    Text,
    /// JSON with metadata
    Json,
    /// CSV
    Csv,
}

impl Default for AltTextConfig {
    fn default() -> Self {
        Self {
            language: "en".to_string(),
            max_length: 125,
            output_format: AltTextOutputFormat::Text,
        }
    }
}

/// AI alt text processor
pub struct AltTextProcessor;

impl Processor for AltTextProcessor {
    type Input = FileInput;
    type Output = AltTextResult;
    type Config = AltTextConfig;
    type Error = RToolsError;

    fn process_validated(
        &self,
        _input: FileInput,
        _config: AltTextConfig,
    ) -> RToolsResult<AltTextResult> {
        Err(RToolsError::capability_unavailable(
            "ai.alt_text",
            "No image captioning provider is configured",
            "Configure a supported image captioning provider",
        ))
    }

    fn validate_config(&self, config: &AltTextConfig) -> RToolsResult<()> {
        if config.max_length == 0 {
            return Err(RToolsError::invalid_input(
                "Max length must be greater than 0",
            ));
        }
        Ok(())
    }

    fn name(&self) -> &'static str {
        "AltTextProcessor"
    }
}

/// Alt text result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AltTextResult {
    pub path: PathBuf,
    pub alt_text: String,
    pub confidence: f64,
    pub stats: ProcessStats,
}
