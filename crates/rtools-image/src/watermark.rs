use rtools_core::error::{RToolsError, RToolsResult};
use rtools_core::types::ProcessStats;
use rtools_core::{FileInput, FileOutput, Processor};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Instant;

/// Watermark position
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WatermarkPosition {
    /// Fixed pixel coordinates
    Pixels {
        x: u32,
        y: u32,
    },
    /// Preset positions
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
    Center,
    /// Custom percentage (x%, y%)
    Percentage {
        x: f64,
        y: f64,
    },
}

/// Watermark type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WatermarkType {
    /// Text watermark
    Text {
        text: String,
        font_size: u32,
        font_color: String,
    },
    /// Image watermark
    Image { image_path: PathBuf, scale: f64 },
}

/// Configuration for watermarking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatermarkConfig {
    /// Watermark type
    pub watermark: WatermarkType,
    /// Position
    pub position: WatermarkPosition,
    /// Opacity (0.0-1.0)
    pub opacity: f64,
    /// Output path (None = overwrite)
    pub output: Option<PathBuf>,
    /// Output quality for lossy formats (0-100)
    pub quality: u8,
}

impl Default for WatermarkConfig {
    fn default() -> Self {
        Self {
            watermark: WatermarkType::Text {
                text: "© rTools".to_string(),
                font_size: 24,
                font_color: "#ffffff".to_string(),
            },
            position: WatermarkPosition::BottomRight,
            opacity: 0.5,
            output: None,
            quality: 85,
        }
    }
}

/// Image watermark processor
pub struct WatermarkProcessor;

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn bounded_dimension(value: f64) -> u32 {
    // Scale and percentage inputs are clamped before conversion to pixels.
    value.clamp(0.0, f64::from(u32::MAX)).round() as u32
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn channel(value: f64) -> u8 {
    // Blend inputs are clamped to the inclusive 8-bit color-channel range.
    value.clamp(0.0, f64::from(u8::MAX)).round() as u8
}

#[allow(clippy::cast_precision_loss)]
fn compression_ratio(output_size: u64, input_size: u64) -> f64 {
    if input_size == 0 {
        1.0
    } else {
        // A ratio remains stable even when very large byte counts are rounded.
        output_size as f64 / input_size as f64
    }
}

impl Processor for WatermarkProcessor {
    type Input = FileInput;
    type Output = FileOutput;
    type Config = WatermarkConfig;
    type Error = RToolsError;

    #[allow(clippy::too_many_lines)] // Task 7 will separate text and image watermark paths.
    fn process(&self, input: FileInput, config: WatermarkConfig) -> RToolsResult<FileOutput> {
        let start = Instant::now();

        let path = input
            .source
            .as_path()
            .ok_or_else(|| RToolsError::invalid_input("Watermark requires a file path input"))?;

        let img = image::open(path).map_err(|e| {
            RToolsError::image(format!("Failed to open image {}: {}", path.display(), e))
        })?;
        let mut base_img = img.to_rgba8();
        let (base_width, base_height) = (base_img.width(), base_img.height());

        let opacity = config.opacity.clamp(0.0, 1.0);

        match &config.watermark {
            WatermarkType::Image { image_path, scale } => {
                if let Ok(wm_dyn) = image::open(image_path) {
                    let target_scale = scale.clamp(0.01, 2.0);
                    let wm_w = bounded_dimension(f64::from(wm_dyn.width()) * target_scale).max(1);
                    let wm_h = bounded_dimension(f64::from(wm_dyn.height()) * target_scale).max(1);
                    let wm_resized = wm_dyn
                        .resize(wm_w, wm_h, image::imageops::FilterType::Lanczos3)
                        .to_rgba8();

                    let (start_x, start_y) = calculate_watermark_pos(
                        base_width,
                        base_height,
                        wm_resized.width(),
                        wm_resized.height(),
                        &config.position,
                    );

                    // Blend watermark onto base image
                    for y in 0..wm_resized.height() {
                        for x in 0..wm_resized.width() {
                            let target_x = start_x + x;
                            let target_y = start_y + y;
                            if target_x < base_width && target_y < base_height {
                                let wm_pixel = wm_resized.get_pixel(x, y);
                                let base_pixel = base_img.get_pixel_mut(target_x, target_y);

                                let wm_alpha = (f64::from(wm_pixel[3]) / 255.0) * opacity;
                                let inv_alpha = 1.0 - wm_alpha;

                                base_pixel[0] = channel(
                                    f64::from(base_pixel[0])
                                        .mul_add(inv_alpha, f64::from(wm_pixel[0]) * wm_alpha),
                                );
                                base_pixel[1] = channel(
                                    f64::from(base_pixel[1])
                                        .mul_add(inv_alpha, f64::from(wm_pixel[1]) * wm_alpha),
                                );
                                base_pixel[2] = channel(
                                    f64::from(base_pixel[2])
                                        .mul_add(inv_alpha, f64::from(wm_pixel[2]) * wm_alpha),
                                );
                            }
                        }
                    }
                }
            }
            WatermarkType::Text { text, .. } => {
                // Approximate subtle corner watermark stamp
                if !text.is_empty() {
                    let stamp_w = u32::try_from(text.len().saturating_mul(12))
                        .unwrap_or(u32::MAX)
                        .min(base_width / 2)
                        .max(20);
                    let stamp_h = 24u32.min(base_height / 4).max(10);
                    let (start_x, start_y) = calculate_watermark_pos(
                        base_width,
                        base_height,
                        stamp_w,
                        stamp_h,
                        &config.position,
                    );

                    for y in 0..stamp_h {
                        for x in 0..stamp_w {
                            let target_x = start_x + x;
                            let target_y = start_y + y;
                            if target_x < base_width && target_y < base_height {
                                let base_pixel = base_img.get_pixel_mut(target_x, target_y);
                                let alpha = 0.3 * opacity;
                                base_pixel[0] = channel(
                                    f64::from(base_pixel[0]).mul_add(1.0 - alpha, 255.0 * alpha),
                                );
                                base_pixel[1] = channel(
                                    f64::from(base_pixel[1]).mul_add(1.0 - alpha, 255.0 * alpha),
                                );
                                base_pixel[2] = channel(
                                    f64::from(base_pixel[2]).mul_add(1.0 - alpha, 255.0 * alpha),
                                );
                            }
                        }
                    }
                }
            }
        }

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
        let file_name = format!("{stem}_watermarked.{ext}");
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

        base_img
            .save(&output)
            .map_err(|e| RToolsError::image(format!("Failed to save watermarked image: {e}")))?;

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

    fn validate_config(&self, config: &WatermarkConfig) -> RToolsResult<()> {
        if config.opacity < 0.0 || config.opacity > 1.0 {
            return Err(RToolsError::invalid_input(
                "Opacity must be between 0.0 and 1.0",
            ));
        }
        Ok(())
    }

    fn name(&self) -> &'static str {
        "WatermarkProcessor"
    }
}

fn calculate_watermark_pos(
    base_w: u32,
    base_h: u32,
    wm_w: u32,
    wm_h: u32,
    pos: &WatermarkPosition,
) -> (u32, u32) {
    let padding = 16u32;
    match pos {
        WatermarkPosition::TopLeft => (padding, padding),
        WatermarkPosition::TopRight => (base_w.saturating_sub(wm_w + padding), padding),
        WatermarkPosition::BottomLeft => (padding, base_h.saturating_sub(wm_h + padding)),
        WatermarkPosition::BottomRight => (
            base_w.saturating_sub(wm_w + padding),
            base_h.saturating_sub(wm_h + padding),
        ),
        WatermarkPosition::Center => (
            (base_w.saturating_sub(wm_w)) / 2,
            (base_h.saturating_sub(wm_h)) / 2,
        ),
        WatermarkPosition::Pixels { x, y } => (*x, *y),
        WatermarkPosition::Percentage { x, y } => (
            bounded_dimension(x * f64::from(base_w) / 100.0),
            bounded_dimension(y * f64::from(base_h) / 100.0),
        ),
    }
}
