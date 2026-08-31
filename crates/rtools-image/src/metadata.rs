use rtools_core::error::{RToolsError, RToolsResult};
use rtools_core::types::ImageMetadata;
use rtools_core::{FileInput, Processor};
use serde::{Deserialize, Serialize};

/// Metadata processor configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetadataConfig {
    /// Include EXIF data
    pub include_exif: bool,
    /// Include dimensions
    pub include_dimensions: bool,
    /// Include file info
    pub include_file_info: bool,
}

impl Default for MetadataConfig {
    fn default() -> Self {
        Self {
            include_exif: true,
            include_dimensions: true,
            include_file_info: true,
        }
    }
}

/// Metadata processor
pub struct MetadataProcessor;

impl Processor for MetadataProcessor {
    type Input = FileInput;
    type Output = ImageMetadata;
    type Config = MetadataConfig;
    type Error = RToolsError;

    fn process(&self, input: FileInput, config: MetadataConfig) -> RToolsResult<ImageMetadata> {
        let path = input.source.as_path().ok_or_else(|| {
            RToolsError::invalid_input("Metadata requires a file path input")
        })?;

        let img = image::open(path)
            .map_err(|e| RToolsError::image(format!("Failed to open image for metadata: {}", e)))?;
        let width = img.width();
        let height = img.height();
        let metadata = std::fs::metadata(path)?;

        let format = input.format.or_else(|| rtools_core::ImageFormat::from_path(path)).unwrap_or(rtools_core::types::ImageFormat::Jpeg);

        let exif_data = if config.include_exif {
            let exif_proc = crate::exif::ExifProcessor;
            exif_proc.process(input, crate::exif::ExifConfig::default()).ok()
        } else {
            None
        };

        Ok(ImageMetadata {
            width,
            height,
            format,
            file_size: metadata.len(),
            color_space: Some(format!("{:?}", img.color())),
            bit_depth: None,
            exif: exif_data,
        })
    }

    fn validate_config(&self, _config: &MetadataConfig) -> RToolsResult<()> {
        Ok(())
    }

    fn name(&self) -> &str {
        "MetadataProcessor"
    }
}
