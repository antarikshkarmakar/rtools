use rtools_core::error::{RToolsError, RToolsResult};
use rtools_core::types::ProcessStats;
use rtools_core::{FileInput, FileOutput, Processor};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Instant;

/// Configuration for PDF merging
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PdfMergeConfig {
    /// List of PDF files to merge (in order)
    pub inputs: Vec<PathBuf>,
    /// Output path
    pub output: PathBuf,
    /// Add page numbers
    pub add_page_numbers: bool,
}

impl Default for PdfMergeConfig {
    fn default() -> Self {
        Self {
            inputs: Vec::new(),
            output: PathBuf::from("merged.pdf"),
            add_page_numbers: false,
        }
    }
}

/// PDF merge processor
pub struct PdfMergeProcessor;

impl Processor for PdfMergeProcessor {
    type Input = Vec<FileInput>;
    type Output = FileOutput;
    type Config = PdfMergeConfig;
    type Error = RToolsError;

    fn process(&self, inputs: Vec<FileInput>, config: PdfMergeConfig) -> RToolsResult<FileOutput> {
        let start = Instant::now();

        if config.inputs.is_empty() && inputs.is_empty() {
            return Err(RToolsError::invalid_input("No PDF files to merge"));
        }

        let mut output_doc = lopdf::Document::new();
        let mut current_page_number = 1u32;

        // Merge all PDFs
        for (idx, input_path) in config.inputs.iter().enumerate() {
            let mut doc = lopdf::Document::load(input_path)
                .map_err(|e| RToolsError::pdf(format!("Failed to load PDF {}: {}", idx + 1, e)))?;

            let page_count = doc.get_pages().len() as u32;

            // Copy pages from source to output
            for page_num in 1..=page_count {
                // Copy page object
                if let Ok(page_obj) = doc.get_page_contents(page_num) {
                    let _ = output_doc.import_page(&mut doc, page_num);
                }
                current_page_number += 1;
            }
        }

        // Save merged PDF
        output_doc.save(&config.output)
            .map_err(|e| RToolsError::pdf(format!("Failed to save merged PDF: {}", e)))?;

        let elapsed = start.elapsed();
        let output_size = std::fs::metadata(&config.output)?.len();

        // Calculate total input size
        let input_size: u64 = config.inputs.iter()
            .filter_map(|p| std::fs::metadata(p).ok())
            .map(|m| m.len())
            .sum();

        Ok(FileOutput {
            destination: rtools_core::output::OutputDestination::File(config.output),
            name: None,
            mime_type: Some("application/pdf".to_string()),
            stats: Some(ProcessStats {
                input_size,
                output_size,
                compression_ratio: 1.0,
                processing_time_ms: elapsed.as_millis() as u64,
                memory_used_mb: 0.0,
            }),
        })
    }

    fn validate_config(&self, config: &PdfMergeConfig) -> RToolsResult<()> {
        if config.inputs.is_empty() {
            return Err(RToolsError::invalid_input("No input files specified"));
        }
        for path in &config.inputs {
            if !path.exists() {
                return Err(RToolsError::file_not_found(path.display().to_string()));
            }
        }
        Ok(())
    }

    fn name(&self) -> &str {
        "PdfMergeProcessor"
    }
}