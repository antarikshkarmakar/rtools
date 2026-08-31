use rtools_core::error::{RToolsError, RToolsResult};
use rtools_core::types::PdfMetadata;
use rtools_core::{FileInput, Processor};
use serde::{Deserialize, Serialize};

/// PDF metadata configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PdfMetadataConfig {
    /// Update metadata
    pub title: Option<String>,
    pub author: Option<String>,
    pub subject: Option<String>,
    pub creator: Option<String>,
    /// Remove all metadata
    pub strip_all: bool,
}

impl Default for PdfMetadataConfig {
    fn default() -> Self {
        Self {
            title: None,
            author: None,
            subject: None,
            creator: None,
            strip_all: false,
        }
    }
}

/// PDF metadata processor
pub struct PdfMetadataProcessor;

impl Processor for PdfMetadataProcessor {
    type Input = FileInput;
    type Output = PdfMetadata;
    type Config = PdfMetadataConfig;
    type Error = RToolsError;

    fn process(&self, input: FileInput, _config: PdfMetadataConfig) -> RToolsResult<PdfMetadata> {
        let path = input.source.as_path().ok_or_else(|| {
            RToolsError::invalid_input("PDF metadata requires a file path input")
        })?;

        let doc = lopdf::Document::load(path)
            .map_err(|e| RToolsError::pdf(format!("Failed to load PDF {}: {}", path.display(), e)))?;

        let pages = doc.get_pages();
        let page_count = pages.len();
        let file_size = std::fs::metadata(path)?.len();

        let info_dict = doc.trailer.get(b"Info")
            .ok()
            .and_then(|info_obj| {
                match info_obj {
                    lopdf::Object::Reference(id) => doc.get_object(*id).ok().and_then(|o| o.as_dict().ok()),
                    lopdf::Object::Dictionary(ref dict) => Some(dict),
                    _ => None,
                }
            });

        let (title, author, subject, creator, producer, creation_date, modification_date) = match info_dict {
            Some(dict) => {
                let get_str = |key: &[u8]| -> Option<String> {
                    dict.get(key)
                        .ok()
                        .and_then(|v| match v {
                            lopdf::Object::String(ref bytes, _) => Some(String::from_utf8_lossy(bytes).to_string()),
                            lopdf::Object::Name(ref bytes) => Some(String::from_utf8_lossy(bytes).to_string()),
                            _ => None,
                        })
                };

                (
                    get_str(b"Title"),
                    get_str(b"Author"),
                    get_str(b"Subject"),
                    get_str(b"Creator"),
                    get_str(b"Producer"),
                    get_str(b"CreationDate"),
                    get_str(b"ModDate"),
                )
            }
            None => (None, None, None, None, None, None, None),
        };

        let is_encrypted = doc.is_encrypted();

        Ok(PdfMetadata {
            page_count,
            page_sizes: Vec::new(),
            title,
            author,
            subject,
            creator,
            producer,
            creation_date,
            modification_date,
            file_size,
            is_encrypted,
            has_images: false,
            has_text_layer: false,
        })
    }

    fn validate_config(&self, _config: &PdfMetadataConfig) -> RToolsResult<()> {
        Ok(())
    }

    fn name(&self) -> &str {
        "PdfMetadataProcessor"
    }
}