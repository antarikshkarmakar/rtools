use rtools_core::error::{RToolsError, RToolsResult};
use rtools_core::types::ExifData;
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

        let file = match std::fs::File::open(path) {
            Ok(f) => f,
            Err(e) => return Err(RToolsError::Io(e)),
        };

        let mut bufreader = std::io::BufReader::new(file);
        let exif = match exif::Reader::new().read_from_container(&mut bufreader) {
            Ok(e) => e,
            Err(_) => {
                // Return empty ExifData when no EXIF metadata is found or unsupported
                return Ok(ExifData {
                    camera_make: None,
                    camera_model: None,
                    lens_model: None,
                    datetime_original: None,
                    datetime_digitized: None,
                    gps_latitude: None,
                    gps_longitude: None,
                    gps_altitude: None,
                    exposure_time: None,
                    f_number: None,
                    iso: None,
                    focal_length: None,
                    flash: None,
                    orientation: None,
                });
            }
        };

        let camera_make = exif.get_field(exif::Tag::Make, exif::In::PRIMARY)
            .map(|v| v.display_value().to_string().trim_matches('"').to_string());
        let camera_model = exif.get_field(exif::Tag::Model, exif::In::PRIMARY)
            .map(|v| v.display_value().to_string().trim_matches('"').to_string());
        let lens_model = exif.get_field(exif::Tag::LensModel, exif::In::PRIMARY)
            .map(|v| v.display_value().to_string().trim_matches('"').to_string());
        let datetime_original = exif.get_field(exif::Tag::DateTimeOriginal, exif::In::PRIMARY)
            .map(|v| v.display_value().to_string().trim_matches('"').to_string());
        let datetime_digitized = exif.get_field(exif::Tag::DateTimeDigitized, exif::In::PRIMARY)
            .map(|v| v.display_value().to_string().trim_matches('"').to_string());

        // Parse GPS DMS coordinates
        let mut gps_latitude = exif.get_field(exif::Tag::GPSLatitude, exif::In::PRIMARY)
            .and_then(|v| match &v.value {
                exif::Value::Rational(coords) => decode_dms(coords),
                _ => None,
            });

        if let (Some(lat), Some(lat_ref)) = (gps_latitude, exif.get_field(exif::Tag::GPSLatitudeRef, exif::In::PRIMARY)) {
            let ref_str = lat_ref.display_value().to_string();
            if ref_str.contains('S') || ref_str.contains('s') {
                gps_latitude = Some(-lat.abs());
            }
        }

        let mut gps_longitude = exif.get_field(exif::Tag::GPSLongitude, exif::In::PRIMARY)
            .and_then(|v| match &v.value {
                exif::Value::Rational(coords) => decode_dms(coords),
                _ => None,
            });

        if let (Some(lon), Some(lon_ref)) = (gps_longitude, exif.get_field(exif::Tag::GPSLongitudeRef, exif::In::PRIMARY)) {
            let ref_str = lon_ref.display_value().to_string();
            if ref_str.contains('W') || ref_str.contains('w') {
                gps_longitude = Some(-lon.abs());
            }
        }

        let mut gps_altitude = exif.get_field(exif::Tag::GPSAltitude, exif::In::PRIMARY)
            .and_then(|v| match &v.value {
                exif::Value::Rational(alt) => alt.first().map(|a| a.to_f64()),
                _ => None,
            });

        // Apply altitude sign from GPSAltitudeRef (0 = above sea level, 1 = below)
        if let Some(alt) = gps_altitude {
            if let Some(alt_ref) = exif.get_field(exif::Tag::GPSAltitudeRef, exif::In::PRIMARY) {
                if let Ok(val) = alt_ref.try_into::<u8>() {
                    if val == 1 {
                        gps_altitude = Some(-alt);
                    }
                }
            }
        }

        let exposure_time = exif.get_field(exif::Tag::ExposureTime, exif::In::PRIMARY)
            .map(|v| v.display_value().to_string());

        let f_number = exif.get_field(exif::Tag::FNumber, exif::In::PRIMARY)
            .and_then(|v| match &v.value {
                exif::Value::Rational(f) => f.first().map(|f| f.to_f64()),
                _ => None,
            });

        let iso = exif.get_field(exif::Tag::PhotographicSensitivity, exif::In::PRIMARY)
            .or_else(|| exif.get_field(exif::Tag::ISOSpeed, exif::In::PRIMARY))
            .and_then(|v| match &v.value {
                exif::Value::Short(s) => s.first().map(|&i| i as u32),
                exif::Value::Long(l) => l.first().copied(),
                _ => None,
            });

        let focal_length = exif.get_field(exif::Tag::FocalLength, exif::In::PRIMARY)
            .and_then(|v| match &v.value {
                exif::Value::Rational(f) => f.first().map(|f| f.to_f64()),
                _ => None,
            });

        let flash = exif.get_field(exif::Tag::Flash, exif::In::PRIMARY)
            .and_then(|v| match &v.value {
                exif::Value::Short(s) => s.first().map(|&v| v as u16),
                _ => None,
            });

        let orientation = exif.get_field(exif::Tag::Orientation, exif::In::PRIMARY)
            .and_then(|v| match &v.value {
                exif::Value::Short(s) => s.first().map(|&o| o as u32),
                _ => None,
            });

        Ok(ExifData {
            camera_make,
            camera_model,
            lens_model,
            datetime_original,
            datetime_digitized,
            gps_latitude,
            gps_longitude,
            gps_altitude,
            exposure_time,
            f_number,
            iso,
            focal_length,
            flash,
            orientation,
        })
    }

    fn validate_config(&self, _config: &ExifConfig) -> RToolsResult<()> {
        Ok(())
    }

    fn name(&self) -> &str {
        "ExifProcessor"
    }
}

fn decode_dms(values: &[exif::Rational]) -> Option<f64> {
    if values.len() >= 3 {
        let deg = values[0].to_f64();
        let min = values[1].to_f64();
        let sec = values[2].to_f64();
        Some(deg + (min / 60.0) + (sec / 3600.0))
    } else if !values.is_empty() {
        Some(values[0].to_f64())
    } else {
        None
    }
}