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

/// Converts an already-bounded pixel value to `u32`.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn bounded_pixels(value: f64) -> u32 {
    // Crop coordinates are clamped to a non-negative image dimension before
    // conversion, so the narrowing conversion is intentional and bounded.
    value.clamp(0.0, f64::from(u32::MAX)) as u32
}

/// Calculates a finite compression ratio for two filesystem byte counts.
#[allow(clippy::cast_precision_loss)]
fn compression_ratio(output_size: u64, input_size: u64) -> f64 {
    if input_size == 0 {
        1.0
    } else {
        // File sizes beyond f64's exact integer range still have a stable,
        // useful ratio; the conversion is intentionally localized here.
        output_size as f64 / input_size as f64
    }
}

impl Processor for CropProcessor {
    type Input = FileInput;
    type Output = FileOutput;
    type Config = CropConfig;
    type Error = RToolsError;

    #[allow(clippy::too_many_lines)] // Task 4 will separate crop-region resolution.
    fn process_validated(&self, input: FileInput, config: CropConfig) -> RToolsResult<FileOutput> {
        let start = Instant::now();

        let path = input
            .source
            .as_path()
            .ok_or_else(|| RToolsError::invalid_input("Crop requires a file path input"))?;

        let img = image::open(path).map_err(|e| {
            RToolsError::image(format!("Failed to open image {}: {}", path.display(), e))
        })?;
        let orig_width = img.width();
        let orig_height = img.height();

        if orig_width == 0 || orig_height == 0 {
            return Err(RToolsError::invalid_input("Image has 0 width or height"));
        }

        let (x, y, crop_width, crop_height) = match &config.region {
            CropRegion::Pixels {
                x,
                y,
                width,
                height,
            } => {
                if *x >= orig_width || *y >= orig_height || *width == 0 || *height == 0 {
                    return Err(RToolsError::invalid_input(
                        "Crop region outside image bounds",
                    ));
                }
                let cw = (*width).min(orig_width.saturating_sub(*x));
                let ch = (*height).min(orig_height.saturating_sub(*y));
                (*x, *y, cw, ch)
            }
            CropRegion::AspectRatio { ratio, gravity } => {
                calculate_aspect_crop(orig_width, orig_height, ratio, gravity)
            }
            CropRegion::Percentage {
                x,
                y,
                width,
                height,
            } => {
                let clamped_x = x.clamp(0.0, 100.0);
                let clamped_y = y.clamp(0.0, 100.0);
                let clamped_w = width.clamp(0.0, 100.0 - clamped_x);
                let clamped_h = height.clamp(0.0, 100.0 - clamped_y);

                let px = bounded_pixels((clamped_x * f64::from(orig_width) / 100.0).floor());
                let py = bounded_pixels((clamped_y * f64::from(orig_height) / 100.0).floor());
                let pw = bounded_pixels((clamped_w * f64::from(orig_width) / 100.0).ceil())
                    .max(1)
                    .min(orig_width.saturating_sub(px));
                let ph = bounded_pixels((clamped_h * f64::from(orig_height) / 100.0).ceil())
                    .max(1)
                    .min(orig_height.saturating_sub(py));
                (px, py, pw, ph)
            }
        };

        let cropped = img.crop_imm(x, y, crop_width.max(1), crop_height.max(1));

        let stem = path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let ext = path
            .extension()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let file_name = format!("{stem}_cropped.{ext}");
        let output = match config.output {
            Some(out) => rtools_core::resolve_output_path(&out, &file_name),
            None => path
                .parent()
                .unwrap_or_else(|| std::path::Path::new("."))
                .join(file_name),
        };

        if let Some(parent) = output.parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        cropped
            .save(&output)
            .map_err(|e| RToolsError::image(format!("Failed to save cropped image: {e}")))?;

        let elapsed = start.elapsed();
        let input_size = std::fs::metadata(path)?.len();
        let output_size = std::fs::metadata(&output)?.len();

        let format = input
            .format
            .or_else(|| rtools_core::ImageFormat::from_path(path))
            .unwrap_or(rtools_core::types::ImageFormat::Jpeg);

        Ok(FileOutput {
            destination: rtools_core::output::OutputDestination::File(output),
            name: None,
            mime_type: Some(format.mime_type().to_string()),
            stats: Some(ProcessStats {
                input_size,
                output_size,
                compression_ratio: compression_ratio(output_size, input_size),
                processing_time_ms: u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX),
                memory_used_mb: 0.0,
            }),
        })
    }

    fn validate_config(&self, config: &CropConfig) -> RToolsResult<()> {
        match &config.region {
            CropRegion::Percentage {
                x,
                y,
                width,
                height,
            } => {
                if *x < 0.0 || *x > 100.0 {
                    return Err(RToolsError::invalid_input("Percentage x must be 0.0-100.0"));
                }
                if *y < 0.0 || *y > 100.0 {
                    return Err(RToolsError::invalid_input("Percentage y must be 0.0-100.0"));
                }
                if *width <= 0.0 || *width > 100.0 {
                    return Err(RToolsError::invalid_input(
                        "Percentage width must be 0.0-100.0",
                    ));
                }
                if *height <= 0.0 || *height > 100.0 {
                    return Err(RToolsError::invalid_input(
                        "Percentage height must be 0.0-100.0",
                    ));
                }
                if x + width > 100.0 || y + height > 100.0 {
                    return Err(RToolsError::invalid_input(
                        "Percentage crop region exceeds image bounds",
                    ));
                }
            }
            CropRegion::AspectRatio {
                ratio: AspectRatio::Custom(w, h),
                ..
            } if (*w <= 0.0 || *h <= 0.0) => {
                return Err(RToolsError::invalid_input(
                    "Custom aspect ratio dimensions must be positive",
                ));
            }
            _ => {}
        }
        Ok(())
    }

    fn name(&self) -> &'static str {
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
        AspectRatio::Custom(w, h) => {
            if *w <= 0.0 || *h <= 0.0 {
                (1.0, 1.0)
            } else {
                (*w, *h)
            }
        }
    };

    let target_ratio = target_ratio_w / target_ratio_h;
    let current_ratio = f64::from(width) / f64::from(height);

    let (crop_width, crop_height) = if current_ratio > target_ratio {
        let w = bounded_pixels((f64::from(height) * target_ratio).round()).clamp(1, width);
        (w, height)
    } else {
        let h = bounded_pixels((f64::from(width) / target_ratio).round()).clamp(1, height);
        (width, h)
    };

    let x = match gravity {
        Gravity::East | Gravity::NorthEast | Gravity::SouthEast => width.saturating_sub(crop_width),
        Gravity::West | Gravity::NorthWest | Gravity::SouthWest => 0,
        _ => (width.saturating_sub(crop_width)) / 2,
    };

    let y = match gravity {
        Gravity::South | Gravity::SouthEast | Gravity::SouthWest => {
            height.saturating_sub(crop_height)
        }
        Gravity::North | Gravity::NorthEast | Gravity::NorthWest => 0,
        _ => (height.saturating_sub(crop_height)) / 2,
    };

    (x, y, crop_width, crop_height)
}
