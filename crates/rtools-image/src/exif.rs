use rtools_core::error::{RToolsError, RToolsResult};
use rtools_core::types::{ExifData, ImageMetadata};
use rtools_core::{FileInput, Processor};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// EXIF viewer configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExifConfig {
    /// Remove GPS data
    pub remove_gps: bool,
    /// Remove all EXIF data
    pub remove_all: bool,
    /// Output path (None = overwrite)
    pub output: Option<PathBuf>,
}

impl Default for ExifConfig {
    fn default() -> Self {
        Self {
            remove_gps: false,
            remove_all: false,
            output: None,
        }
    }
}

/// EXIF processor
pub struct ExifProcessor;

impl Processor for ExifProcessor {
    type Input = FileInput;
    type Output = ExifData;
    type Config = ExifConfig;
    type Error = RToolsError;

    fn process(&self, input: FileInput, _config: ExifConfig) -> RToolsResult<ExifData> {
        let path = input.source.as_path().ok_or_else(|| {
            RToolsError::invalid_input("EXIF requires a file path input")
        })?;

        let file = std::fs::File::open(path)?;
        let mut bufreader = std::io::BufReader::new(file);
        let exif = exif::Reader::new().read_from_container(&mut bufreader)?;

        Ok(ExifData {
            camera_make: exif.get_field(exif::Tag::Make, exif::In::PRIMARY)
                .and_then(|v| v.display_string().into()),
            camera_model: exif.get_field(exif::Tag::Model, exif::In::PRIMARY)
                .and_then(|v| v.display_string().into()),
            lens_model: exif.get_field(exif::Tag::LensModel, exif::In::PRIMARY)
                .and_then(|v| v.display_string().into()),
            datetime_original: exif.get_field(exif::Tag::DateTimeOriginal, exif::In::PRIMARY)
                .and_then(|v| v.display_string().into()),
            datetime_digitized: exif.get_field(exif::Tag::DateTimeDigitized, exif::In::PRIMARY)
                .and_then(|v| v.display_string().into()),
            gps_latitude: exif.get_field(exif::Tag::GPSLatitude, exif::In::PRIMARY)
                .and_then(|v| {
                    if let exif::Value::Rational(ref coords) = v {
                        coords.first().map(|c| c.to_f64())
                    } else {
                        None
                    }
                }),
            gps_longitude: exif.get_field(exif::Tag::GPSLongitude, exif::In::PRIMARY)
                .and_then(|v| {
                    if let exif::Value::Rational(ref coords) = v {
                        coords.first().map(|c| c.to_f64())
                    } else {
                        None
                    }
                }),
            gps_altitude: exif.get_field(exif::Tag::GPSAltitude, exif::In::PRIMARY)
                .and_then(|v| {
                    if let exif::Value::Rational(ref alt) = v {
                        alt.first().map(|a| a.to_f64())
                    } else {
                        None
                    }
                }),
            exposure_time: exif.get_field(exif::Tag::ExposureTime, exif::In::PRIMARY)
                .and_then(|v| v.display_string().into()),
            f_number: exif.get_field(exif::Tag::FNumber, exif::In::PRIMARY)
                .and_then(|v| {
                    if let exif::Value::Rational(ref f) = v {
                        f.first().map(|f| f.to_f64())
                    } else {
                        None
                    }
                }),
            iso: exif.get_field(exif::Tag::ISOSpeedRatings, exif::In::PRIMARY)
                .and_then(|v| v.to_u32()),
            focal_length: exif.get_field(exif::Tag::FocalLength, exif::In::PRIMARY)
                .and_then(|v| {
                    if let exif::Value::Rational(ref f) = v {
                        f.first().map(|f| f.to_f64())
                    } else {
                        None
                    }
                }),
            flash: exif.get_field(exif::Tag::Flash, exif::In::PRIMARY)
                .and_then(|v| v.to_u32())
                .map(|v| v & 1 == 1),
            orientation: exif.get_field(exif::Tag::Orientation, exif::In::PRIMARY)
                .and_then(|v| v.to_u32()),
        })
    }

    fn validate_config(&self, _config: &ExifConfig) -> RToolsResult<()> {
        Ok(())
    }

    fn name(&self) -> &str {
        "ExifProcessor"
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

    fn process(&self, input: FileInput, _config: MetadataConfig) -> RToolsResult<ImageMetadata> {
        let path = input.source.as_path().ok_or_else(|| {
            RToolsError::invalid_input("Metadata requires a file path input")
        })?;

        let img = image::open(path)?;
        let (width, height) = img.dimensions();
        let metadata = std::fs::metadata(path)?;

        let format = input.format.unwrap_or(rtools_core::types::ImageFormat::Jpeg);

        Ok(ImageMetadata {
            width,
            height,
            format,
            file_size: metadata.len(),
            color_space: Some(format!("{:?}", img.color())),
            bit_depth: None,
            exif: None, // Would need to read EXIF separately
        })
    }

    fn validate_config(&self, _config: &MetadataConfig) -> RToolsResult<()> {
        Ok(())
    }

    fn name(&self) -> &str {
        "MetadataProcessor"
    }
}