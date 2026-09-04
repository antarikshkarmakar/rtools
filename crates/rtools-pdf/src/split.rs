use rtools_core::error::{RToolsError, RToolsResult};
use rtools_core::{FileInput, FileOutput, OutputPolicy, PendingOutput, Processor};
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
    /// Output directory, which must already exist
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

    fn process_validated(
        &self,
        input: FileInput,
        config: PdfSplitConfig,
    ) -> RToolsResult<Vec<FileOutput>> {
        let _start = Instant::now();

        let path = input
            .source
            .as_path()
            .ok_or_else(|| RToolsError::invalid_input("PDF split requires a file path input"))?;

        let doc = lopdf::Document::load(path)
            .map_err(|e| RToolsError::pdf(format!("Failed to load PDF: {e}")))?;

        let pages = doc.get_pages();
        let page_count = u32::try_from(pages.len()).unwrap_or(u32::MAX);

        let pages_to_extract = resolve_page_range(&config.range, page_count);
        if pages_to_extract.is_empty() {
            return Err(RToolsError::invalid_input(
                "The requested page selection contains no pages in this PDF",
            ));
        }
        let planned_outputs: Vec<_> = pages_to_extract
            .iter()
            .map(|&page_num| {
                let filename = config
                    .filename_pattern
                    .replace("{n}", &page_num.to_string())
                    .replace("{total}", &page_count.to_string());
                let output_path = config.output_dir.join(&filename);

                (page_num, filename, output_path)
            })
            .collect();
        let mut pending_outputs = Vec::with_capacity(planned_outputs.len());
        for (page_num, filename, output_path) in planned_outputs {
            let pending = PendingOutput::new(&output_path, OutputPolicy::FailIfExists)?;
            pending_outputs.push((page_num, filename, pending));
        }

        for (page_num, _, pending) in &pending_outputs {
            // Extract single page by cloning document and pruning pages
            let mut page_doc = doc.clone();
            let pages_to_delete: Vec<u32> = page_doc
                .get_pages()
                .keys()
                .copied()
                .filter(|p| p != page_num)
                .collect();
            page_doc.delete_pages(&pages_to_delete);

            crate::output::encode_pdf(
                &mut page_doc,
                pending.temporary_path(),
                &format!("page {page_num}"),
            )?;
        }

        for (_, _, pending) in &pending_outputs {
            crate::output::validate_pdf_artifact(pending.temporary_path())?;
        }

        let mut outputs = Vec::with_capacity(pending_outputs.len());
        for (_, filename, pending) in pending_outputs {
            let output_path = crate::output::commit_pdf(pending)?;
            outputs.push(FileOutput {
                destination: rtools_core::output::OutputDestination::File(output_path),
                name: Some(filename),
                mime_type: Some("application/pdf".to_string()),
                stats: None,
                warnings: Vec::new(),
            });
        }

        Ok(outputs)
    }

    fn validate_config(&self, config: &PdfSplitConfig) -> RToolsResult<()> {
        if config.filename_pattern.is_empty() {
            return Err(RToolsError::invalid_input(
                "Filename pattern cannot be empty",
            ));
        }
        Ok(())
    }

    fn name(&self) -> &'static str {
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
            let s = (*start).max(1);
            let e = (*end).min(total_pages);
            if s <= e {
                (s..=e).collect()
            } else {
                vec![]
            }
        }
        PageRange::Multiple(ranges) => ranges
            .iter()
            .flat_map(|r| resolve_page_range(r, total_pages))
            .collect(),
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
