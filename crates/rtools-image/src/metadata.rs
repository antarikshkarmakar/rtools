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
        Ok(exif) => {
            let retained = exif
                .fields()
                .filter(|field| {
                    format != image::ImageFormat::Tiff || !is_structural_tiff_field(field)
                })
                .map(|field| field.tag.to_string())
                .collect::<Vec<_>>();
            if retained.is_empty() {
                Ok(())
            } else {
                Err(RToolsError::image(format!(
                    "encoded artifact retained EXIF metadata under the drop-all policy: {}",
                    retained.join(", ")
                )))
            }
        }
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

fn is_structural_tiff_field(field: &exif::Field) -> bool {
    field.ifd_num == exif::In::PRIMARY
        && field.tag.context() == exif::Context::Tiff
        && matches!(
            field.tag.number(),
            0x0100 // ImageWidth
                | 0x0101 // ImageLength
                | 0x0102 // BitsPerSample
                | 0x0103 // Compression
                | 0x0106 // PhotometricInterpretation
                | 0x0111 // StripOffsets
                | 0x0115 // SamplesPerPixel
                | 0x0116 // RowsPerStrip
                | 0x0117 // StripByteCounts
                | 0x011a // XResolution
                | 0x011b // YResolution
                | 0x011c // PlanarConfiguration
                | 0x0128 // ResolutionUnit
                | 0x013d // Predictor
                | 0x0152 // ExtraSamples
                | 0x0153 // SampleFormat
        )
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

#[cfg(test)]
mod tests {
    use super::is_structural_tiff_field;

    #[test]
    fn tiff_allowlist_is_primary_ifd_only_and_rejects_non_structural_tags() {
        let mut field = exif::Field {
            tag: exif::Tag::ImageWidth,
            ifd_num: exif::In::PRIMARY,
            value: exif::Value::Long(vec![2]),
        };
        assert!(is_structural_tiff_field(&field));

        field.ifd_num = exif::In::THUMBNAIL;
        assert!(!is_structural_tiff_field(&field));

        field.ifd_num = exif::In::PRIMARY;
        field.tag = exif::Tag::Software;
        assert!(!is_structural_tiff_field(&field));
    }
}
