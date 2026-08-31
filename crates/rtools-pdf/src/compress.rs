use rtools_core::error::{RToolsError, RToolsResult};
use rtools_core::types::{PdfMetadata, ProcessStats};
use rtools_core::{FileInput, FileOutput, Processor};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Instant;

/// PDF compression level
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PdfCompressionLevel {
    Light,
    Medium,
    Heavy,
}

/// Configuration for PDF compression
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PdfCompressConfig {
    /// Compression level
    pub level: PdfCompressionLevel,
    /// Output path (None = overwrite)
    pub output: Option<PathBuf>,
    /// Remove metadata
    pub remove_metadata: bool,
    /// Remove images
    pub remove_images: bool,
}

impl Default for PdfCompressConfig {
    fn default() -> Self {
        Self {
            level: PdfCompressionLevel::Medium,
            output: None,
            remove_metadata: false,
            remove_images: false,
        }
    }
}

/// PDF compression processor
pub struct PdfCompressProcessor;

impl Processor for PdfCompressProcessor {
    type Input = FileInput;
    type Output = FileOutput;
    type Config = PdfCompressConfig;
    type Error = RToolsError;

    fn process(&self, input: FileInput, config: PdfCompressConfig) -> RToolsResult<FileOutput> {
        let start = Instant::now();

        let path = input.source.as_path().ok_or_else(|| {
            RToolsError::invalid_input("PDF compress requires a file path input")
        })?;

        let output = config.output.unwrap_or_else(|| {
            let mut out = path.clone();
            let stem = out.file_stem().unwrap_or_default();
            out.set_file_name(format!("{}_compressed", stem.to_string_lossy()));
            out.set_extension("pdf");
            out
        });

        // Load PDF
        let mut doc = lopdf::Document::load(path)
            .map_err(|e| RToolsError::pdf(format!("Failed to load PDF: {}", e)))?;

        // Apply compression based on level
        match config.level {
            PdfCompressionLevel::Light => {
                // Basic compression - remove duplicate objects
                doc.compress();
            }
            PdfCompressionLevel::Medium => {
                doc.compress();
                // Remove metadata if requested
                if config.remove_metadata {
                    let _ = doc.truncate();
                }
            }
            PdfCompressionLevel::Heavy => {
                doc.compress();
                if config.remove_metadata {
                    let _ = doc.truncate();
                }
                // Additional optimizations could be done here
            }
        }

        // Save compressed PDF
        doc.save(&output)
            .map_err(|e| RToolsError::pdf(format!("Failed to save PDF: {}", e)))?;

        let elapsed = start.elapsed();
        let input_size = std::fs::metadata(path)?.len();
        let output_size = std::fs::metadata(&output)?.len();

        Ok(FileOutput {
            destination: rtools_core::output::OutputDestination::File(output),
            name: None,
            mime_type: Some("application/pdf".to_string()),
            stats: Some(ProcessStats {
                input_size,
                output_size,
                compression_ratio: output_size as f64 / input_size as f64,
                processing_time_ms: elapsed.as_millis() as u64,
                memory_used_mb: 0.0,
            }),
        })
    }

    fn validate_config(&self, _config: &PdfCompressConfig) -> RToolsResult<()> {
        Ok(())
    }

    fn name(&self) -> &str {
        "PdfCompressProcessor"
    }
}