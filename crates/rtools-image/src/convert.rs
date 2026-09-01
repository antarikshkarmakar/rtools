use image::ImageEncoder;
use rtools_core::error::{RToolsError, RToolsResult};
use rtools_core::types::{ImageFormat, ProcessStats};
use rtools_core::{FileInput, FileOutput, Processor};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Instant;

/// Configuration for format conversion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConvertConfig {
    /// Target output format
    pub target_format: ImageFormat,
    /// Output path (None = auto-generate)
    pub output: Option<PathBuf>,
    /// Output directory (for batch operations)
    pub output_dir: Option<PathBuf>,
    /// Quality for lossy formats (0-100)
    pub quality: u8,
    /// Preserve EXIF metadata
    pub preserve_metadata: bool,
    /// Strip GPS data
    pub strip_gps: bool,
}

impl Default for ConvertConfig {
    fn default() -> Self {
        Self {
            target_format: ImageFormat::Webp,
            output: None,
            output_dir: None,
            quality: 85,
            preserve_metadata: true,
            strip_gps: false,
        }
    }
}

/// Get a unique output path by appending a numeric suffix if file exists
fn unique_output_path(path: &PathBuf) -> PathBuf {
    if !path.exists() {
        return path.clone();
    }
    let stem = path.file_stem().unwrap_or_default().to_string_lossy();
    let ext = path.extension().unwrap_or_default().to_string_lossy();
    let parent = path.parent().unwrap_or_else(|| std::path::Path::new("."));
    for i in 1..1000 {
        let new_name = format!("{}_{}.{}", stem, i, ext);
        let new_path = parent.join(new_name);
        if !new_path.exists() {
            return new_path;
        }
    }
    path.clone()
}

/// Format conversion processor
pub struct ConvertProcessor;

impl Processor for ConvertProcessor {
    type Input = FileInput;
    type Output = FileOutput;
    type Config = ConvertConfig;
    type Error = RToolsError;

    fn process(&self, input: FileInput, config: ConvertConfig) -> RToolsResult<FileOutput> {
        let start = Instant::now();

        let path = input.source.as_path().ok_or_else(|| {
            RToolsError::invalid_input("Convert requires a file path input")
        })?;

        // Generate output path using target format extension
        let output_path = config.output.unwrap_or_else(|| {
            let stem = path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy();
            let ext = config.target_format.extensions()[0];
            path.parent()
                .unwrap_or_else(|| std::path::Path::new("."))
                .join(format!("{}.{}", stem, ext))
        });

        let output_path = unique_output_path(&output_path);

        if let Some(parent) = output_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let img = image::open(path).map_err(|e| {
            RToolsError::image(format!("Failed to open image {}: {}", path.display(), e))
        })?;

        // Log alpha channel warning for JPEG targets
        if config.target_format == ImageFormat::Jpeg && img.color().has_alpha() {
            tracing::warn!(
                "Image has alpha channel but JPEG target selected — transparency will be lost"
            );
        }

        match config.target_format {
            ImageFormat::Jpeg => {
                let rgb = img.to_rgb8();
                let file = std::fs::File::create(&output_path)?;
                let mut encoder =
                    image::codecs::jpeg::JpegEncoder::new_with_quality(file, config.quality);
                encoder
                    .encode_image(&rgb)
                    .map_err(|e| RToolsError::image(format!("JPEG conversion failed: {}", e)))?;
            }
            ImageFormat::Png => {
                let file = std::fs::File::create(&output_path)?;
                let encoder = image::codecs::png::PngEncoder::new(file);
                if img.color().has_alpha() {
                    let rgba = img.to_rgba8();
                    encoder
                        .write_image(
                            &rgba,
                            rgba.width(),
                            rgba.height(),
                            image::ExtendedColorType::Rgba8,
                        )
                        .map_err(|e| {
                            RToolsError::image(format!("PNG conversion failed: {}", e))
                        })?;
                } else {
                    let rgb = img.to_rgb8();
                    encoder
                        .write_image(
                            &rgb,
                            rgb.width(),
                            rgb.height(),
                            image::ExtendedColorType::Rgb8,
                        )
                        .map_err(|e| {
                            RToolsError::image(format!("PNG conversion failed: {}", e))
                        })?;
                }
            }
            ImageFormat::Webp => {
                // image 0.25 only exposes lossless WebP encoding (VP8L);
                // the requested quality is logged but cannot be applied.
                if config.quality < 100 {
                    tracing::debug!(
                        "WebP target requested at quality {} — image 0.25 only supports lossless WebP",
                        config.quality
                    );
                }
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
                        .map_err(|e| RToolsError::image(format!("WebP conversion failed: {}", e)))?;
                } else {
                    let rgb = img.to_rgb8();
                    encoder
                        .write_image(
                            &rgb,
                            rgb.width(),
                            rgb.height(),
                            image::ExtendedColorType::Rgb8,
                        )
                        .map_err(|e| RToolsError::image(format!("WebP conversion failed: {}", e)))?;
                }
            }
            ImageFormat::Avif => {
                img.save_with_format(&output_path, image::ImageFormat::Avif)
                    .map_err(|e| {
                        RToolsError::image(format!("AVIF conversion failed: {}", e))
                    })?;
            }
            ImageFormat::Tiff => {
                img.save_with_format(&output_path, image::ImageFormat::Tiff)
                    .map_err(|e| {
                        RToolsError::image(format!("TIFF conversion failed: {}", e))
                    })?;
            }
            ImageFormat::Bmp => {
                img.save_with_format(&output_path, image::ImageFormat::Bmp)
                    .map_err(|e| {
                        RToolsError::image(format!("BMP conversion failed: {}", e))
                    })?;
            }
            ImageFormat::Gif => {
                img.save_with_format(&output_path, image::ImageFormat::Gif)
                    .map_err(|e| {
                        RToolsError::image(format!("GIF conversion failed: {}", e))
                    })?;
            }
            ImageFormat::Hdr => {
                img.save_with_format(&output_path, image::ImageFormat::Hdr)
                    .map_err(|e| {
                        RToolsError::image(format!("HDR conversion failed: {}", e))
                    })?;
            }
            _ => {
                return Err(RToolsError::unsupported_format(format!(
                    "Conversion to {:?} not supported",
                    config.target_format
                )));
            }
        }

        let elapsed = start.elapsed();
        let input_size = std::fs::metadata(path)?.len();
        let output_size = std::fs::metadata(&output_path)?.len();

        Ok(FileOutput {
            destination: rtools_core::output::OutputDestination::File(output_path),
            name: None,
            mime_type: Some(config.target_format.mime_type().to_string()),
            stats: Some(ProcessStats {
                input_size,
                output_size,
                compression_ratio: if input_size > 0 {
                    output_size as f64 / input_size as f64
                } else {
                    1.0
                },
                processing_time_ms: elapsed.as_millis() as u64,
                memory_used_mb: 0.0,
            }),
        })
    }

    fn validate_config(&self, config: &ConvertConfig) -> RToolsResult<()> {
        if config.quality > 100 {
            return Err(RToolsError::invalid_input(
                "Quality must be between 0 and 100",
            ));
        }
        Ok(())
    }

    fn name(&self) -> &str {
        "ConvertProcessor"
    }
}