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

        let _source_format = input.format.ok_or_else(|| {
            RToolsError::invalid_input("Cannot determine input format")
        })?;

        let output_path = config.output.unwrap_or_else(|| {
            let mut out = path.clone();
            out.set_extension(config.target_format.extensions()[0]);
            out
        });

        let img = image::open(path)?;

        match config.target_format {
            ImageFormat::Jpeg => {
                img.save_with_format(&output_path, image::ImageFormat::Jpeg)?;
            }
            ImageFormat::Png => {
                img.save_with_format(&output_path, image::ImageFormat::Png)?;
            }
            ImageFormat::WebP => {
                let webp_data = webp::Encoder::new(&img.to_rgb8())
                    .encode(config.quality as f32 / 100.0);
                std::fs::write(&output_path, &*webp_data)?;
            }
            ImageFormat::Avif => {
                let rgb = img.to_rgb8();
                let (width, height) = rgb.dimensions();
                let decoder = ravif::Encoder::new()
                    .with_quality(config.quality as f32)
                    .encode(&rgb.into_raw(), width as usize, height as usize)?;
                std::fs::write(&output_path, decoder.data)?;
            }
            ImageFormat::Tiff => {
                img.save_with_format(&output_path, image::ImageFormat::Tiff)?;
            }
            ImageFormat::Bmp => {
                img.save_with_format(&output_path, image::ImageFormat::Bmp)?;
            }
            ImageFormat::Gif => {
                img.save_with_format(&output_path, image::ImageFormat::Gif)?;
            }
            ImageFormat::Hdr => {
                img.save_with_format(&output_path, image::ImageFormat::Hdr)?;
            }
            _ => {
                return Err(RToolsError::unsupported_format(
                    format!("Conversion to {:?} not supported", config.target_format),
                ));
            }
        }

        let elapsed = start.elapsed();
        let input_size = std::fs::metadata(path)?.len();
        let output_size = std::fs::metadata(&output_path)?.len();

        Ok(FileOutput {
            destination: rtools_core::output::OutputDestination::File(output_path),
            name: None,
            mime_type: Some(config.target_format.mime_type()),
            stats: Some(ProcessStats {
                input_size,
                output_size,
                compression_ratio: output_size as f64 / input_size as f64,
                processing_time_ms: elapsed.as_millis() as u64,
                memory_used_mb: 0.0,
            }),
        })
    }

    fn validate_config(&self, config: &ConvertConfig) -> RToolsResult<()> {
        if config.quality > 100 {
            return Err(RToolsError::invalid_input("Quality must be between 0 and 100"));
        }
        Ok(())
    }

    fn name(&self) -> &str {
        "ConvertProcessor"
    }
}