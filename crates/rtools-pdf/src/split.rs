use rtools_core::error::{RToolsError, RToolsResult};
use rtools_core::types::ProcessStats;
use rtools_core::{FileInput, FileOutput, Processor};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Instant;

/// Page range specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PageRange {
    /// Single page (1-indexed)
    Single(u32),
    /// Range of pages (inclusive)
    Range { start: u32, end: u32 },
    /// Multiple ranges
    Multiple(Vec<PageRange>),
    /// All pages
    All,
    /// Every Nth page
    EveryN(u32),
}

/// Configuration for PDF splitting
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PdfSplitConfig {
    /// Page range to extract
    pub range: PageRange,
    /// Output directory
    pub output_dir: PathBuf,
    /// Output filename pattern (e.g., "page_{n}.pdf")
    pub filename_pattern: String,
    /// Output as images instead of PDF
    pub as_images: bool,
    /// Image format for image output
    pub image_format: Option<String>,
    /// Image DPI for image output
    pub image_dpi: u32,
}

impl Default for PdfSplitConfig {
    fn default() -> Self {
        Self {
            range: PageRange::All,
            output_dir: PathBuf::from("output"),
            filename_pattern: "page_{n}.pdf".to_string(),
            as_images: false,
            image_format: Some("png".to_string()),
            image_dpi: 300,
        }
    }
}

/// PDF split processor
pub struct PdfSplitProcessor;

impl Processor for PdfSplitProcessor {
    type Input = FileInput;
    type Output = Vec<FileOutput>;
    type Config = PdfSplitConfig;
    type Error = RToolsError;

    fn process(&self, input: FileInput, config: PdfSplitConfig) -> RToolsResult<Vec<FileOutput>> {
        let start = Instant::now();

        let path = input.source.as_path().ok_or_else(|| {
            RToolsError::invalid_input("PDF split requires a file path input")
        })?;

        // Create output directory
        std::fs::create_dir_all(&config.output_dir)?;

        let doc = lopdf::Document::load(path)
            .map_err(|e| RToolsError::pdf(format!("Failed to load PDF: {}", e)))?;

        let pages = doc.get_pages();
        let page_count = pages.len() as u32;

        let pages_to_extract = resolve_page_range(&config.range, page_count);

        let mut outputs = Vec::new();

        for &page_num in &pages_to_extract {
            let mut output_doc = lopdf::Document::new();
            let _ = output_doc.import_page(&mut doc, page_num);

            let filename = config.filename_pattern
                .replace("{n}", &page_num.to_string())
                .replace("{total}", &page_count.to_string());

            let output_path = config.output_dir.join(&filename);

            output_doc.save(&output_path)
                .map_err(|e| RToolsError::pdf(format!("Failed to save page {}: {}", page_num, e)))?;

            let output_size = std::fs::metadata(&output_path)?.len();

            outputs.push(FileOutput {
                destination: rtools_core::output::OutputDestination::File(output_path),
                name: Some(filename),
                mime_type: Some("application/pdf".to_string()),
                stats: None,
            });
        }

        let elapsed = start.elapsed();
        let input_size = std::fs::metadata(path)?.len();

        Ok(outputs)
    }

    fn validate_config(&self, config: &PdfSplitConfig) -> RToolsResult<()> {
        if config.filename_pattern.is_empty() {
            return Err(RToolsError::invalid_input("Filename pattern cannot be empty"));
        }
        Ok(())
    }

    fn name(&self) -> &str {
        "PdfSplitProcessor"
    }
}

/// Resolve page range to actual page numbers
fn resolve_page_range(range: &PageRange, total_pages: u32) -> Vec<u32> {
    match range {
        PageRange::Single(n) => {
            if *n > 0 && *n <= total_pages {
                vec![*n]
            } else {
                vec![]
            }
        }
        PageRange::Range { start, end } => {
            let s = start.max(&1);
            let e = end.min(&total_pages);
            if s <= e {
                (*s..=*e).collect()
            } else {
                vec![]
            }
        }
        PageRange::Multiple(ranges) => {
            ranges.iter()
                .flat_map(|r| resolve_page_range(r, total_pages))
                .collect()
        }
        PageRange::All => (1..=total_pages).collect(),
        PageRange::EveryN(n) => {
            if *n > 0 {
                (1..=total_pages).step_by(*n as usize).collect()
            } else {
                vec![]
            }
        }
    }
}