use rtools_core::error::{RToolsError, RToolsResult};
use rtools_core::types::ProcessStats;
use rtools_core::{FileInput, FileOutput, Processor};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Instant;

/// Crop region specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CropRegion {
    /// Fixed pixel coordinates: x, y, width, height
    Pixels {
        x: u32,
        y: u32,
        width: u32,
        height: u32,
    },
    /// Standard aspect ratio with gravity
    AspectRatio {
        ratio: AspectRatio,
        gravity: Gravity,
    },
    /// Percentage-based crop
    Percentage {
        x: f64,
        y: f64,
        width: f64,
        height: f64,
    },
}

/// Standard aspect ratios
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AspectRatio {
    Original,
    Square,
    Portrait,
    Landscape,
    Wide,
    Ultrawide,
    Cinema,
    Custom(f64, f64),
}

/// Gravity point for aspect ratio cropping
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Gravity {
    Center,
    North,
    South,
    East,
    West,
    NorthEast,
    NorthWest,
    SouthEast,
    SouthWest,
}

/// Configuration for image cropping
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CropConfig {
    /// Crop region
    pub region: CropRegion,
    /// Output path (None = overwrite)
    pub output: Option<PathBuf>,
    /// Output quality for lossy formats (0-100)
    pub quality: u8,
}

impl Default for CropConfig {
    fn default() -> Self {
        Self {
            region: CropRegion::AspectRatio {
                ratio: AspectRatio::Square,
                gravity: Gravity::Center,
            },
            output: None,
            quality: 85,
        }
    }
}

/// Image crop processor
pub struct CropProcessor;

impl Processor for CropProcessor {
    type Input = FileInput;
    type Output = FileOutput;
    type Config = CropConfig;
    type Error = RToolsError;

    fn process(&self, input: FileInput, config: CropConfig) -> RToolsResult<FileOutput> {
        let start = Instant::now();

        let path = input.source.as_path().ok_or_else(|| {
            RToolsError::invalid_input("Crop requires a file path input")
        })?;

        let img = image::open(path)?;
        let (orig_width, orig_height) = img.dimensions();

        let (x, y, crop_width, crop_height) = match &config.region {
            CropRegion::Pixels { x, y, width, height } => {
                if x + width > orig_width || y + height > orig_height {
                    return Err(RToolsError::invalid_input(
                        "Crop region exceeds image dimensions",
                    ));
                }
                (*x, *y, *width, *height)
            }
            CropRegion::AspectRatio { ratio, gravity } => {
                calculate_aspect_crop(orig_width, orig_height, ratio, gravity)
            }
            CropRegion::Percentage { x, y, width, height } => {
                let px = (x * orig_width as f64 / 100.0) as u32;
                let py = (y * orig_height as f64 / 100.0) as u32;
                let pw = (width * orig_width as f64 / 100.0) as u32;
                let ph = (height * orig_height as f64 / 100.0) as u32;
                (px, py, pw, ph)
            }
        };

        let cropped = img.crop_imm(x, y, crop_width, crop_height);

        let output = config.output.unwrap_or_else(|| {
            let mut out = path.clone();
            let stem = out.file_stem().unwrap_or_default();
            let ext = out.extension().unwrap_or_default();
            out.set_file_name(format!("{}_cropped", stem.to_string_lossy()));
            out.set_extension(ext);
            out
        });

        cropped.save(&output)?;

        let elapsed = start.elapsed();
        let input_size = std::fs::metadata(path)?.len();
        let output_size = std::fs::metadata(&output)?.len();

        Ok(FileOutput {
            destination: rtools_core::output::OutputDestination::File(output),
            name: None,
            mime_type: Some(input.format.unwrap_or(rtools_core::types::ImageFormat::Jpeg).mime_type()),
            stats: Some(ProcessStats {
                input_size,
                output_size,
                compression_ratio: output_size as f64 / input_size as f64,
                processing_time_ms: elapsed.as_millis() as u64,
                memory_used_mb: 0.0,
            }),
        })
    }

    fn validate_config(&self, _config: &CropConfig) -> RToolsResult<()> {
        Ok(())
    }

    fn name(&self) -> &str {
        "CropProcessor"
    }
}

/// Calculate crop dimensions based on aspect ratio and gravity
fn calculate_aspect_crop(
    width: u32,
    height: u32,
    ratio: &AspectRatio,
    gravity: &Gravity,
) -> (u32, u32, u32, u32) {
    let (target_ratio_w, target_ratio_h) = match ratio {
        AspectRatio::Original => return (0, 0, width, height),
        AspectRatio::Square => (1.0, 1.0),
        AspectRatio::Portrait => (3.0, 4.0),
        AspectRatio::Landscape => (4.0, 3.0),
        AspectRatio::Wide => (16.0, 9.0),
        AspectRatio::Ultrawide => (21.0, 9.0),
        AspectRatio::Cinema => (2.39, 1.0),
        AspectRatio::Custom(w, h) => (*w, *h),
    };

    let target_ratio = target_ratio_w / target_ratio_h;
    let current_ratio = width as f64 / height as f64;

    let (crop_width, crop_height) = if current_ratio > target_ratio {
        // Image is wider than target - crop width
        let w = (height as f64 * target_ratio) as u32;
        (w, height)
    } else {
        // Image is taller than target - crop height
        let h = (width as f64 / target_ratio) as u32;
        (width, h)
    };

    // Calculate position based on gravity
    let x = match gravity {
        Gravity::East | Gravity::NorthEast | Gravity::SouthEast => width - crop_width,
        Gravity::West | Gravity::NorthWest | Gravity::SouthWest => 0,
        _ => (width - crop_width) / 2,
    };

    let y = match gravity {
        Gravity::South | Gravity::SouthEast | Gravity::SouthWest => height - crop_height,
        Gravity::North | Gravity::NorthEast | Gravity::NorthWest => 0,
        _ => (height - crop_height) / 2,
    };

    (x, y, crop_width, crop_height)
}