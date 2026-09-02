use rtools_core::error::{RToolsError, RToolsResult};
use rtools_core::types::ProcessStats;
use rtools_core::{FileInput, FileOutput, Processor};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Instant;

#[allow(clippy::cast_precision_loss)]
fn compression_ratio(output_size: u64, input_size: u64) -> f64 {
    if input_size == 0 {
        1.0
    } else {
        // A ratio remains stable even when very large byte counts are rounded.
        output_size as f64 / input_size as f64
    }
}

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
        input: FileInput,
        config: PdfOcrConfig,
    ) -> RToolsResult<FileOutput> {
        let start = Instant::now();

        let path = input
            .source
            .as_path()
            .ok_or_else(|| RToolsError::invalid_input("PDF OCR requires a file path input"))?;

        let ext = match config.output_format {
            OcrOutputFormat::Text => "txt",
            OcrOutputFormat::SearchablePdf => "pdf",
        };

        let output = config.output.unwrap_or_else(|| {
            let mut out = path.clone();
            let stem = out
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            out.set_file_name(format!("{stem}_ocr.{ext}"));
            out
        });

        if let Some(parent) = output.parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        match config.output_format {
            OcrOutputFormat::Text => {
                let text_content = pdf_extract::extract_text(path).map_err(|e| {
                    RToolsError::pdf(format!("Failed to extract text from PDF: {e}"))
                })?;
                std::fs::write(&output, text_content)?;
            }
            OcrOutputFormat::SearchablePdf => {
                std::fs::copy(path, &output)?;
            }
        }

        let elapsed = start.elapsed();
        let input_size = std::fs::metadata(path)?.len();
        let output_size = std::fs::metadata(&output)?.len();

        let mime = match config.output_format {
            OcrOutputFormat::Text => "text/plain",
            OcrOutputFormat::SearchablePdf => "application/pdf",
        };

        Ok(FileOutput {
            destination: rtools_core::output::OutputDestination::File(output),
            name: None,
            mime_type: Some(mime.to_string()),
            stats: Some(ProcessStats {
                input_size,
                output_size,
                compression_ratio: compression_ratio(output_size, input_size),
                processing_time_ms: u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX),
                memory_used_mb: 0.0,
            }),
        })
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
