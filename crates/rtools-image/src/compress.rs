use rtools_core::error::{RToolsError, RToolsResult};
use rtools_core::types::{ImageFormat, ProcessStats};
use rtools_core::{FileInput, FileOutput, Processor};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
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
    pub fn quality(&self) -> u8 {
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
    /// Output path (None = overwrite input)
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

/// Image compression processor
pub struct CompressProcessor;

impl Processor for CompressProcessor {
    type Input = FileInput;
    type Output = FileOutput;
    type Config = CompressConfig;
    type Error = RToolsError;

    fn process(&self, input: FileInput, config: CompressConfig) -> RToolsResult<FileOutput> {
        let start = Instant::now();

        let path = input.source.as_path().ok_or_else(|| {
            RToolsError::invalid_input("Compress requires a file path input")
        })?;

        let format = input.format.ok_or_else(|| {
            RToolsError::invalid_input("Cannot determine input format")
        })?;

        let quality = config.preset.quality();
        let output_format = config.format.unwrap_or(format);

        let result = match format {
            ImageFormat::Jpeg => self.compress_jpeg(path, quality, output_format)?,
            ImageFormat::Png => self.compress_png(path, quality, output_format)?,
            ImageFormat::WebP => self.compress_webp(path, quality, output_format)?,
            ImageFormat::Avif => self.compress_avif(path, quality, output_format)?,
            _ => return Err(RToolsError::unsupported_format(format!("Compression not supported for {:?}", format))),
        };

        let elapsed = start.elapsed();

        let input_size = std::fs::metadata(path)?.len();
        let output_size = std::fs::metadata(&result)?.len();

        Ok(FileOutput {
            destination: rtools_core::output::OutputDestination::File(result),
            name: None,
            mime_type: Some(format.mime_type()),
            stats: Some(ProcessStats {
                input_size,
                output_size,
                compression_ratio: output_size as f64 / input_size as f64,
                processing_time_ms: elapsed.as_millis() as u64,
                memory_used_mb: 0.0, // TODO: track memory
            }),
        })
    }

    fn validate_config(&self, config: &CompressConfig) -> RToolsResult<()> {
        if let CompressionPreset::Custom(q) = config.preset {
            if q > 100 {
                return Err(RToolsError::invalid_input("Quality must be between 0 and 100"));
            }
        }
        Ok(())
    }

    fn name(&self) -> &str {
        "CompressProcessor"
    }
}

impl CompressProcessor {
    fn compress_jpeg(&self, input: &PathBuf, quality: u8, format: ImageFormat) -> RToolsResult<PathBuf> {
        let output = self.get_output_path(input, format);
        let img = image::open(input)?;

        match format {
            ImageFormat::Jpeg => {
                let mut encoder = mozjpeg::Encoder::new_file(&output, quality)?;
                let rgb = img.to_rgb8();
                let (width, height) = rgb.dimensions();
                encoder.write_header()?;
                encoder.write_scanlines(&rgb.as_raw())?;
                encoder.finish()?;
            }
            ImageFormat::WebP => {
                let webp_data = webp::Encoder::new(&img.to_rgb8()).encode(quality as f32 / 100.0);
                std::fs::write(&output, &*webp_data)?;
            }
            _ => {
                let mut output_img = img;
                output_img.save(&output)?;
            }
        }

        Ok(output)
    }

    fn compress_png(&self, input: &PathBuf, quality: u8, format: ImageFormat) -> RToolsResult<PathBuf> {
        let output = self.get_output_path(input, format);
        let img = image::open(input)?;

        match format {
            ImageFormat::Png => {
                let mut encoder = image::codecs::png::PngEncoder::new(std::fs::File::create(&output)?);
                let rgb = img.to_rgb8();
                encoder.write_image(
                    &rgb,
                    rgb.width(),
                    rgb.height(),
                    image::ColorType::Rgb8.into(),
                )?;
            }
            ImageFormat::WebP => {
                let webp_data = webp::Encoder::new(&img.to_rgb8()).encode(quality as f32 / 100.0);
                std::fs::write(&output, &*webp_data)?;
            }
            ImageFormat::Jpeg => {
                img.save_with_format(&output, image::ImageFormat::Jpeg)?;
            }
            _ => {
                img.save(&output)?;
            }
        }

        Ok(output)
    }

    fn compress_webp(&self, input: &PathBuf, quality: u8, format: ImageFormat) -> RToolsResult<PathBuf> {
        let output = self.get_output_path(input, format);
        let img = image::open(input)?;

        match format {
            ImageFormat::WebP => {
                let webp_data = webp::Encoder::new(&img.to_rgb8()).encode(quality as f32 / 100.0);
                std::fs::write(&output, &*webp_data)?;
            }
            ImageFormat::Jpeg => {
                let mut encoder = mozjpeg::Encoder::new_file(&output, quality)?;
                let rgb = img.to_rgb8();
                encoder.write_header()?;
                encoder.write_scanlines(&rgb.as_raw())?;
                encoder.finish()?;
            }
            ImageFormat::Png => {
                img.save(&output)?;
            }
            _ => {
                img.save(&output)?;
            }
        }

        Ok(output)
    }

    fn compress_avif(&self, input: &PathBuf, quality: u8, format: ImageFormat) -> RToolsResult<PathBuf> {
        let output = self.get_output_path(input, format);
        let img = image::open(input)?;

        match format {
            ImageFormat::Avif => {
                let rgb = img.to_rgb8();
                let (width, height) = rgb.dimensions();
                let decoder = ravif::Encoder::new()
                    .with_quality(quality as f32)
                    .encode(&rgb.into_raw(), width as usize, height as usize)?;
                std::fs::write(&output, decoder.data)?;
            }
            ImageFormat::Jpeg => {
                let mut encoder = mozjpeg::Encoder::new_file(&output, quality)?;
                let rgb = img.to_rgb8();
                encoder.write_header()?;
                encoder.write_scanlines(&rgb.as_raw())?;
                encoder.finish()?;
            }
            ImageFormat::WebP => {
                let webp_data = webp::Encoder::new(&img.to_rgb8()).encode(quality as f32 / 100.0);
                std::fs::write(&output, &*webp_data)?;
            }
            _ => {
                img.save(&output)?;
            }
        }

        Ok(output)
    }

    fn get_output_path(&self, input: &PathBuf, format: ImageFormat) -> PathBuf {
        let mut output = input.clone();
        output.set_extension(format.extensions()[0]);
        output
    }
}

impl ImageFormat {
    fn mime_type(&self) -> String {
        match self {
            ImageFormat::Jpeg => "image/jpeg",
            ImageFormat::Png => "image/png",
            ImageFormat::WebP => "image/webp",
            ImageFormat::Avif => "image/avif",
            ImageFormat::Heic => "image/heic",
            ImageFormat::Heif => "image/heif",
            ImageFormat::Tiff => "image/tiff",
            ImageFormat::Bmp => "image/bmp",
            ImageFormat::Gif => "image/gif",
            ImageFormat::Ico => "image/ico",
            ImageFormat::Jxl => "image/jxl",
            ImageFormat::Hdr => "image/hdr",
            ImageFormat::Exr => "image/exr",
            ImageFormat::Pdf => "application/pdf",
        }
        .to_string()
    }
}