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
    /// Output path (None = auto-generate)
    pub output: Option<PathBuf>,
    /// Remove metadata
    pub remove_metadata: bool,
}

impl Default for PdfCompressConfig {
    fn default() -> Self {
        Self {
            level: PdfCompressionLevel::Medium,
            output: None,
            remove_metadata: false,
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

        let path = input
            .source
            .as_path()
            .ok_or_else(|| RToolsError::invalid_input("PDF compress requires a file path input"))?;

        let output = config.output.unwrap_or_else(|| {
            let mut out = path.clone();
            let stem = out
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            out.set_file_name(format!("{stem}_compressed.pdf"));
            out
        });

        if let Some(parent) = output.parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        // Load PDF
        let mut doc = lopdf::Document::load(path).map_err(|e| {
            RToolsError::pdf(format!("Failed to load PDF {}: {}", path.display(), e))
        })?;

        // Apply stream compression
        doc.compress();

        // Safe metadata removal
        if config.remove_metadata {
            let info_opt = doc
                .trailer
                .get(b"Info")
                .ok()
                .and_then(|v| v.as_reference().ok());
            if let Some(info_id) = info_opt {
                doc.objects.remove(&info_id);
            }
            doc.trailer.remove(b"Info");
        }

        // Save compressed PDF
        doc.save(&output)
            .map_err(|e| RToolsError::pdf(format!("Failed to save compressed PDF: {e}")))?;

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
                compression_ratio: compression_ratio(output_size, input_size),
                processing_time_ms: u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX),
                memory_used_mb: 0.0,
            }),
        })
    }

    fn validate_config(&self, _config: &PdfCompressConfig) -> RToolsResult<()> {
        Ok(())
    }

    fn name(&self) -> &'static str {
        "PdfCompressProcessor"
    }
}
