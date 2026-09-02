use image::ImageEncoder;
use rtools_core::error::{RToolsError, RToolsResult};
use rtools_core::types::{ImageFormat, ProcessStats};
use rtools_core::{FileInput, FileOutput, Processor};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::Instant;

/// Compression quality preset
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CompressionPreset {
    /// Web optimized (smaller size)
    Web,
    /// Balanced quality/size
    Balanced,
    /// High quality
    High,
    /// Maximum quality
    Maximum,
    /// Lossless
    Lossless,
    /// Custom quality value
    Custom(u8),
}

impl CompressionPreset {
    /// Get quality value (0-100)
    pub const fn quality(&self) -> u8 {
        match self {
            CompressionPreset::Web => 60,
            CompressionPreset::Balanced => 75,
            CompressionPreset::High => 85,
            CompressionPreset::Maximum => 95,
            CompressionPreset::Lossless => 100,
            CompressionPreset::Custom(q) => *q,
        }
    }
}

/// Configuration for image compression
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressConfig {
    /// Output quality preset or custom value
    pub preset: CompressionPreset,
    /// Target output format (None = same as input)
    pub format: Option<ImageFormat>,
    /// Output path (None = {stem}_compressed.{ext})
    pub output: Option<PathBuf>,
    /// Preserve metadata
    pub preserve_metadata: bool,
    /// Strip GPS data
    pub strip_gps: bool,
}

impl Default for CompressConfig {
    fn default() -> Self {
        Self {
            preset: CompressionPreset::Balanced,
            format: None,
            output: None,
            preserve_metadata: true,
            strip_gps: false,
        }
    }
}

/// Get a unique output path by appending a numeric suffix if file exists
fn unique_output_path(path: &Path) -> PathBuf {
    if !path.exists() {
        return path.to_path_buf();
    }
    let stem = path.file_stem().unwrap_or_default().to_string_lossy();
    let ext = path.extension().unwrap_or_default().to_string_lossy();
    let parent = path.parent().unwrap_or_else(|| std::path::Path::new("."));
    for i in 1..1000 {
        let new_name = format!("{stem}_{i}.{ext}");
        let new_path = parent.join(new_name);
        if !new_path.exists() {
            return new_path;
        }
    }
    path.to_path_buf()
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

/// Image compression processor
pub struct CompressProcessor;

impl Processor for CompressProcessor {
    type Input = FileInput;
    type Output = FileOutput;
    type Config = CompressConfig;
    type Error = RToolsError;

    #[allow(clippy::too_many_lines)] // Task 4 will split encoding by output format.
    fn process_validated(
        &self,
        input: FileInput,
        config: CompressConfig,
    ) -> RToolsResult<FileOutput> {
        let start = Instant::now();

        let path = input
            .source
            .as_path()
            .ok_or_else(|| RToolsError::invalid_input("Compress requires a file path input"))?;

        let format = input
            .format
            .or_else(|| ImageFormat::from_path(path))
            .ok_or_else(|| RToolsError::invalid_input("Cannot determine input format"))?;

        let quality = config.preset.quality();
        let target_format = config.format.unwrap_or(format);

        let stem = path.file_stem().unwrap_or_default().to_string_lossy();
        let ext = target_format.extensions()[0];
        let file_name = format!("{stem}_compressed.{ext}");
        let output_path = match config.output {
            Some(out) => rtools_core::resolve_output_path(&out, &file_name),
            None => path
                .parent()
                .unwrap_or_else(|| std::path::Path::new("."))
                .join(file_name),
        };

        let output_path = unique_output_path(&output_path);

        if let Some(parent) = output_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let img = image::open(path).map_err(|e| {
            RToolsError::image(format!("Failed to open image {}: {}", path.display(), e))
        })?;

        // Log alpha channel warning for JPEG targets
        if target_format == ImageFormat::Jpeg && img.color().has_alpha() {
            tracing::warn!(
                "Image has alpha channel but JPEG target selected — transparency will be lost"
            );
        }

        match target_format {
            ImageFormat::Jpeg => {
                let rgb = img.to_rgb8();
                let file = std::fs::File::create(&output_path)?;
                let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(file, quality);
                encoder
                    .encode_image(&rgb)
                    .map_err(|e| RToolsError::image(format!("JPEG compression failed: {e}")))?;
            }
            ImageFormat::Png => {
                let file = std::fs::File::create(&output_path)?;
                let encoder = image::codecs::png::PngEncoder::new_with_quality(
                    file,
                    image::codecs::png::CompressionType::Best,
                    image::codecs::png::FilterType::Adaptive,
                );
                if img.color().has_alpha() {
                    let rgba = img.to_rgba8();
                    encoder
                        .write_image(
                            &rgba,
                            rgba.width(),
                            rgba.height(),
                            image::ExtendedColorType::Rgba8,
                        )
                        .map_err(|e| RToolsError::image(format!("PNG compression failed: {e}")))?;
                } else {
                    let rgb = img.to_rgb8();
                    encoder
                        .write_image(
                            &rgb,
                            rgb.width(),
                            rgb.height(),
                            image::ExtendedColorType::Rgb8,
                        )
                        .map_err(|e| RToolsError::image(format!("PNG compression failed: {e}")))?;
                }
            }
            ImageFormat::Webp => {
                // image 0.25 only exposes lossless WebP encoding (VP8L)
                let file = std::fs::File::create(&output_path)?;
                let encoder = image::codecs::webp::WebPEncoder::new_lossless(file);
                if img.color().has_alpha() {
                    let rgba = img.to_rgba8();
                    encoder
                        .write_image(
                            &rgba,
                            rgba.width(),
                            rgba.height(),
                            image::ExtendedColorType::Rgba8,
                        )
                        .map_err(|e| RToolsError::image(format!("WebP compression failed: {e}")))?;
                } else {
                    let rgb = img.to_rgb8();
                    encoder
                        .write_image(
                            &rgb,
                            rgb.width(),
                            rgb.height(),
                            image::ExtendedColorType::Rgb8,
                        )
                        .map_err(|e| RToolsError::image(format!("WebP compression failed: {e}")))?;
                }
            }
            ImageFormat::Avif => {
                img.save_with_format(&output_path, image::ImageFormat::Avif)
                    .map_err(|e| RToolsError::image(format!("AVIF compression failed: {e}")))?;
            }
            _ => {
                img.save(&output_path).map_err(|e| {
                    RToolsError::image(format!("Compression failed for {target_format:?}: {e}"))
                })?;
            }
        }

        let elapsed = start.elapsed();
        let input_size = std::fs::metadata(path)?.len();
        let output_size = std::fs::metadata(&output_path)?.len();

        Ok(FileOutput {
            destination: rtools_core::output::OutputDestination::File(output_path),
            name: None,
            mime_type: Some(target_format.mime_type().to_string()),
            stats: Some(ProcessStats {
                input_size,
                output_size,
                compression_ratio: compression_ratio(output_size, input_size),
                processing_time_ms: u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX),
                memory_used_mb: 0.0,
            }),
        })
    }

    fn validate_config(&self, config: &CompressConfig) -> RToolsResult<()> {
        if let CompressionPreset::Custom(q) = config.preset {
            if q > 100 {
                return Err(RToolsError::invalid_input(
                    "Quality must be between 0 and 100",
                ));
            }
        }
        Ok(())
    }

    fn name(&self) -> &'static str {
        "CompressProcessor"
    }
}
