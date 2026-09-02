use rtools_core::error::{RToolsError, RToolsResult};
use rtools_core::{FileInput, FileOutput, Processor};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PdfRedactConfig {
    pub patterns: Vec<String>,
    pub output: Option<PathBuf>,
    pub flatten: bool,
}

pub struct PdfRedactProcessor;

impl Processor for PdfRedactProcessor {
    type Input = FileInput;
    type Output = FileOutput;
    type Config = PdfRedactConfig;
    type Error = RToolsError;

    fn process(&self, _input: FileInput, _config: PdfRedactConfig) -> RToolsResult<FileOutput> {
        Err(RToolsError::not_implemented(
            "PDF redaction not yet implemented",
        ))
    }

    fn validate_config(&self, _config: &PdfRedactConfig) -> RToolsResult<()> {
        Ok(())
    }

    fn name(&self) -> &'static str {
        "PdfRedactProcessor"
    }
}
