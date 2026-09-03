use rtools_core::error::{RToolsError, RToolsResult};
use rtools_core::types::ImageMetadata;
use rtools_core::{FileInput, Processor, ResourceLimits};
use serde::{Deserialize, Serialize};
use std::io::Cursor;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MetadataPolicy {
    DropAll,
    Preserve,
    StripGps,
}

impl MetadataPolicy {
    pub(crate) fn from_flags(preserve_metadata: bool, strip_gps: bool) -> RToolsResult<Self> {
        let policy = match (preserve_metadata, strip_gps) {
            (false, false) => Self::DropAll,
            (true, false) => Self::Preserve,
            (false, true) => Self::StripGps,
            (true, true) => {
                return Err(RToolsError::invalid_input(
                    "Metadata cannot be preserved while GPS metadata is stripped",
                ));
            }
        };

        match policy {
            Self::DropAll => Ok(policy),
            Self::Preserve => Err(RToolsError::capability_unavailable(
                "image.metadata.preserve",
                "Image metadata preservation is not implemented",
                "Disable metadata preservation until verified metadata export is available",
            )),
            Self::StripGps => Err(RToolsError::capability_unavailable(
                "image.metadata.strip_gps",
                "Selective GPS metadata removal is not implemented",
                "Use the default drop-all metadata policy until selective removal is available",
            )),
        }
    }
}

/// Verify that an encoded artifact contains no EXIF fields before publication.
///
/// # Errors
///
/// Returns a processing error when EXIF exists or cannot be inspected, and a
/// structured resource/I/O error when the artifact cannot be read safely.
pub fn verify_drop_all_artifact(path: &Path, limits: &ResourceLimits) -> RToolsResult<()> {
    let encoded = crate::format::read_bounded_snapshot(path, limits)?;
    let format = image::guess_format(&encoded).map_err(|error| {
        RToolsError::image(format!(
            "encoded artifact format validation failed: {error}"
        ))
    })?;
    let mut cursor = Cursor::new(encoded);
    match exif::Reader::new().read_from_container(&mut cursor) {
        Ok(exif) if exif.fields().next().is_none() => Ok(()),
        Ok(_) => Err(RToolsError::image(
            "encoded artifact retained EXIF metadata under the drop-all policy",
        )),
        Err(exif::Error::NotFound(_)) => Ok(()),
        Err(exif::Error::InvalidFormat(_))
            if !matches!(
                format,
                image::ImageFormat::Jpeg
                    | image::ImageFormat::Png
                    | image::ImageFormat::WebP
                    | image::ImageFormat::Avif
                    | image::ImageFormat::Tiff
            ) =>
        {
            Ok(())
        }
        Err(error) => Err(RToolsError::image(format!(
            "encoded artifact metadata validation failed: {error}"
        ))),
    }
}

/// Metadata processor configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetadataConfig {
    /// Include EXIF data
    pub include_exif: bool,
    /// Include dimensions
    pub include_dimensions: bool,
    /// Include file info
    pub include_file_info: bool,
    /// Resource limits enforced before image decoding.
    #[serde(default)]
    pub limits: ResourceLimits,
}

impl Default for MetadataConfig {
    fn default() -> Self {
        Self {
            include_exif: true,
            include_dimensions: true,
            include_file_info: true,
            limits: ResourceLimits::default(),
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

    fn process_validated(
        &self,
        input: FileInput,
        config: MetadataConfig,
    ) -> RToolsResult<ImageMetadata> {
        let path = input
            .source
            .as_path()
            .ok_or_else(|| RToolsError::invalid_input("Metadata requires a file path input"))?;

        let img = crate::format::decode_bounded(path, &config.limits)?.image;
        let width = img.width();
        let height = img.height();
        let metadata = std::fs::metadata(path)?;

        let format = input
            .format
            .or_else(|| rtools_core::ImageFormat::from_path(path))
            .unwrap_or(rtools_core::types::ImageFormat::Jpeg);

        let exif_data = if config.include_exif {
            let exif_proc = crate::exif::ExifProcessor;
            exif_proc
                .process(input, crate::exif::ExifConfig::default())
                .ok()
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

    fn name(&self) -> &'static str {
        "MetadataProcessor"
    }
}
