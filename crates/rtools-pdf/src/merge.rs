use lopdf::{Document, Object, ObjectId};
use rtools_core::error::{RToolsError, RToolsResult};
use rtools_core::types::ProcessStats;
use rtools_core::{FileInput, FileOutput, Processor};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
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

/// Configuration for PDF merging
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PdfMergeConfig {
    /// List of PDF files to merge (in order)
    pub inputs: Vec<PathBuf>,
    /// Output path; its parent directory must already exist
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

    #[allow(clippy::too_many_lines)] // Task 7 will separate PDF document assembly.
    fn process_validated(
        &self,
        inputs: Vec<FileInput>,
        config: PdfMergeConfig,
    ) -> RToolsResult<FileOutput> {
        let start = Instant::now();

        let mut input_paths: Vec<PathBuf> = inputs
            .iter()
            .filter_map(|i| i.source.as_path().cloned())
            .collect();

        if input_paths.is_empty() {
            input_paths.clone_from(&config.inputs);
        }

        if input_paths.is_empty() {
            return Err(RToolsError::invalid_input("No PDF files to merge"));
        }

        let mut max_id = 1u32;
        let mut pagenum = 1u32;
        let mut documents_pages = BTreeMap::new();
        let mut documents_objects = BTreeMap::new();
        let mut document = Document::with_version("1.5");

        for input_path in &input_paths {
            let mut doc = Document::load(input_path).map_err(|e| {
                RToolsError::pdf(format!(
                    "Failed to load PDF {}: {}",
                    input_path.display(),
                    e
                ))
            })?;

            doc.renumber_objects_with(max_id);
            max_id = doc.max_id + 1;

            let pages = doc.get_pages();
            for (_p_num, object_id) in pages {
                documents_pages.insert(pagenum, object_id);
                pagenum += 1;
            }

            documents_objects.extend(doc.objects);
        }

        let mut catalog_id: Option<ObjectId> = None;
        let mut pages_id: Option<ObjectId> = None;

        for (object_id, object) in documents_objects {
            match object.type_name().unwrap_or(b"") {
                b"Catalog" => {
                    if catalog_id.is_none() {
                        catalog_id = Some(object_id);
                    }
                }
                b"Pages" => {
                    if pages_id.is_none() {
                        pages_id = Some(object_id);
                    }
                }
                _ => {
                    document.objects.insert(object_id, object);
                }
            }
        }

        let final_pages_id = pages_id.unwrap_or_else(|| {
            let id = (max_id, 0);
            max_id += 1;
            id
        });

        let mut kids = Vec::new();
        let mut count = 0;
        for (_, page_id) in documents_pages {
            kids.push(Object::Reference(page_id));
            count += 1;

            if let Some(dict) = document
                .objects
                .get_mut(&page_id)
                .and_then(|o| o.as_dict_mut().ok())
            {
                dict.set("Parent", Object::Reference(final_pages_id));
            }
        }

        let mut pages_dict = lopdf::Dictionary::new();
        pages_dict.set("Type", Object::Name(b"Pages".to_vec()));
        pages_dict.set("Count", Object::Integer(count));
        pages_dict.set("Kids", Object::Array(kids));
        document
            .objects
            .insert(final_pages_id, Object::Dictionary(pages_dict));

        let final_catalog_id = catalog_id.unwrap_or((max_id, 0));

        let mut catalog_dict = lopdf::Dictionary::new();
        catalog_dict.set("Type", Object::Name(b"Catalog".to_vec()));
        catalog_dict.set("Pages", Object::Reference(final_pages_id));
        document
            .objects
            .insert(final_catalog_id, Object::Dictionary(catalog_dict));
        document
            .trailer
            .set("Root", Object::Reference(final_catalog_id));

        let output = crate::output::save_pdf(&mut document, &config.output, "merged PDF")?;

        let elapsed = start.elapsed();
        let output_size = std::fs::metadata(&output)?.len();

        let input_size: u64 = input_paths
            .iter()
            .filter_map(|p| std::fs::metadata(p).ok())
            .map(|m| m.len())
            .sum();

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
            warnings: Vec::new(),
        })
    }

    fn validate_config(&self, config: &PdfMergeConfig) -> RToolsResult<()> {
        for path in &config.inputs {
            if !path.exists() {
                return Err(RToolsError::file_not_found(path.display().to_string()));
            }
        }
        Ok(())
    }

    fn name(&self) -> &'static str {
        "PdfMergeProcessor"
    }
}
