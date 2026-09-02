use rtools_core::error::{RToolsError, RToolsResult};
use rtools_core::{FileInput, FileOutput, Processor};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PdfEncryptConfig {
    pub password: String,
    pub permissions: Vec<String>,
    pub output: Option<PathBuf>,
}

pub struct PdfEncryptProcessor;

impl Processor for PdfEncryptProcessor {
    type Input = FileInput;
    type Output = FileOutput;
    type Config = PdfEncryptConfig;
    type Error = RToolsError;

    fn process(&self, _input: FileInput, _config: PdfEncryptConfig) -> RToolsResult<FileOutput> {
        Err(RToolsError::not_implemented(
            "PDF encryption not yet implemented",
        ))
    }

    fn validate_config(&self, _config: &PdfEncryptConfig) -> RToolsResult<()> {
        Ok(())
    }

    fn name(&self) -> &'static str {
        "PdfEncryptProcessor"
    }
}
