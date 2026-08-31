use rtools_core::error::{RToolsError, RToolsResult};
use rtools_core::types::PdfMetadata;
use rtools_core::{FileInput, Processor};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

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
            .map_err(|e| RToolsError::pdf(format!("Failed to load PDF: {}", e)))?;

        let pages = doc.get_pages();
        let page_count = pages.len();
        let file_size = std::fs::metadata(path)?.len();

        // Get metadata from document info dictionary
        let info = doc.get_info();
        let (title, author, subject, creator, producer, creation_date, modification_date) = match info {
            Ok(info_dict) => {
                let title = info_dict.get(b"Title")
                    .ok()
                    .and_then(|v| v.as_string().ok())
                    .map(|s| String::from_utf8_lossy(s).to_string());
                let author = info_dict.get(b"Author")
                    .ok()
                    .and_then(|v| v.as_string().ok())
                    .map(|s| String::from_utf8_lossy(s).to_string());
                let subject = info_dict.get(b"Subject")
                    .ok()
                    .and_then(|v| v.as_string().ok())
                    .map(|s| String::from_utf8_lossy(s).to_string());
                let creator = info_dict.get(b"Creator")
                    .ok()
                    .and_then(|v| v.as_string().ok())
                    .map(|s| String::from_utf8_lossy(s).to_string());
                let producer = info_dict.get(b"Producer")
                    .ok()
                    .and_then(|v| v.as_string().ok())
                    .map(|s| String::from_utf8_lossy(s).to_string());
                let creation_date = info_dict.get(b"CreationDate")
                    .ok()
                    .and_then(|v| v.as_string().ok())
                    .map(|s| String::from_utf8_lossy(s).to_string());
                let modification_date = info_dict.get(b"ModDate")
                    .ok()
                    .and_then(|v| v.as_string().ok())
                    .map(|s| String::from_utf8_lossy(s).to_string());
                (title, author, subject, creator, producer, creation_date, modification_date)
            }
            Err(_) => (None, None, None, None, None, None, None),
        };

        // Check for encryption
        let is_encrypted = doc.is_encrypted();

        Ok(PdfMetadata {
            page_count,
            page_sizes: Vec::new(), // TODO: extract page sizes
            title,
            author,
            subject,
            creator,
            producer,
            creation_date,
            modification_date,
            file_size,
            is_encrypted,
            has_images: false, // TODO: detect images
            has_text_layer: false, // TODO: detect text layer
        })
    }

    fn validate_config(&self, _config: &PdfMetadataConfig) -> RToolsResult<()> {
        Ok(())
    }

    fn name(&self) -> &str {
        "PdfMetadataProcessor"
    }
}