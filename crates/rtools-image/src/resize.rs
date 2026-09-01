use rtools_core::error::{RToolsError, RToolsResult};
use rtools_core::types::ProcessStats;
use rtools_core::{FileInput, FileOutput, Processor};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Instant;

/// Resize algorithm
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ResizeAlgorithm {
    /// Lanczos resampling (high quality)
    Lanczos,
    /// Triangle/Bilinear
    Triangle,
    /// Catmull-Rom
    CatmullRom,
    /// Nearest neighbor (fast, pixelated)
    NearestNeighbor,
    /// Mitchell-Netravali
    MitchellNetravali,
}

impl Default for ResizeAlgorithm {
    fn default() -> Self {
        ResizeAlgorithm::Lanczos
    }
}

/// Configuration for image resizing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResizeConfig {
    /// Target width (optional)
    pub width: Option<u32>,
    /// Target height (optional)
    pub height: Option<u32>,
    /// Maintain aspect ratio
    pub maintain_aspect: bool,
    /// Resize algorithm
    pub algorithm: ResizeAlgorithm,
    /// Output path (None = overwrite)
    pub output: Option<PathBuf>,
    /// Output quality for lossy formats (0-100)
    pub quality: u8,
}

impl Default for ResizeConfig {
    fn default() -> Self {
        Self {
            width: None,
            height: None,
            maintain_aspect: true,
            algorithm: ResizeAlgorithm::default(),
            output: None,
            quality: 85,
        }
    }
}

/// Image resize processor
pub struct ResizeProcessor;

impl Processor for ResizeProcessor {
    type Input = FileInput;
    type Output = FileOutput;
    type Config = ResizeConfig;
    type Error = RToolsError;

    fn process(&self, input: FileInput, config: ResizeConfig) -> RToolsResult<FileOutput> {
        let start = Instant::now();

        let path = input.source.as_path().ok_or_else(|| {
            RToolsError::invalid_input("Resize requires a file path input")
        })?;

        if config.width.is_none() && config.height.is_none() {
            return Err(RToolsError::invalid_input(
                "At least one of width or height must be specified",
            ));
        }

        let img = image::open(path)
            .map_err(|e| RToolsError::image(format!("Failed to open image {}: {}", path.display(), e)))?;
        let orig_width = img.width();
        let orig_height = img.height();

        if orig_width == 0 || orig_height == 0 {
            return Err(RToolsError::invalid_input("Image has 0 width or height"));
        }

        let (new_width, new_height) = calculate_dimensions(
            orig_width,
            orig_height,
            config.width,
            config.height,
            config.maintain_aspect,
        );

        let filter = match config.algorithm {
            ResizeAlgorithm::Lanczos => image::imageops::FilterType::Lanczos3,
            ResizeAlgorithm::Triangle => image::imageops::FilterType::Triangle,
            ResizeAlgorithm::CatmullRom => image::imageops::FilterType::CatmullRom,
            ResizeAlgorithm::NearestNeighbor => image::imageops::FilterType::Nearest,
            ResizeAlgorithm::MitchellNetravali => image::imageops::FilterType::CatmullRom,
        };

        let resized = if config.maintain_aspect {
            img.resize(new_width.max(1), new_height.max(1), filter)
        } else {
            img.resize_exact(new_width.max(1), new_height.max(1), filter)
        };

        let stem = path.file_stem().unwrap_or_default().to_string_lossy().to_string();
        let ext = path.extension().unwrap_or_default().to_string_lossy().to_string();
        let file_name = format!("{stem}_{new_width}x{new_height}.{ext}");
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

        resized.save(&output)
            .map_err(|e| RToolsError::image(format!("Failed to save resized image: {}", e)))?;

        let elapsed = start.elapsed();
        let input_size = std::fs::metadata(path)?.len();
        let output_size = std::fs::metadata(&output)?.len();

        // Derive MIME type from output path, not input
        let output_format = rtools_core::ImageFormat::from_path(&output)
            .or_else(|| input.format)
            .unwrap_or(rtools_core::types::ImageFormat::Jpeg);

        Ok(FileOutput {
            destination: rtools_core::output::OutputDestination::File(output),
            name: None,
            mime_type: Some(output_format.mime_type().to_string()),
            stats: Some(ProcessStats {
                input_size,
                output_size,
                compression_ratio: if input_size > 0 { output_size as f64 / input_size as f64 } else { 1.0 },
                processing_time_ms: elapsed.as_millis() as u64,
                memory_used_mb: 0.0,
            }),
        })
    }

    fn validate_config(&self, config: &ResizeConfig) -> RToolsResult<()> {
        if let Some(w) = config.width {
            if w == 0 || w > 32768 {
                return Err(RToolsError::invalid_input(format!("Invalid width: {}", w)));
            }
        }
        if let Some(h) = config.height {
            if h == 0 || h > 32768 {
                return Err(RToolsError::invalid_input(format!("Invalid height: {}", h)));
            }
        }
        Ok(())
    }

    fn name(&self) -> &str {
        "ResizeProcessor"
    }
}

/// Calculate output dimensions based on constraints
fn calculate_dimensions(
    orig_width: u32,
    orig_height: u32,
    target_width: Option<u32>,
    target_height: Option<u32>,
    maintain_aspect: bool,
) -> (u32, u32) {
    match (target_width, target_height) {
        (Some(w), Some(h)) => {
            if maintain_aspect {
                let ratio = (w as f64 / orig_width as f64).min(h as f64 / orig_height as f64);
                let nw = (orig_width as f64 * ratio).round() as u32;
                let nh = (orig_height as f64 * ratio).round() as u32;
                (nw.max(1), nh.max(1))
            } else {
                (w.max(1), h.max(1))
            }
        }
        (Some(w), None) => {
            if maintain_aspect {
                let ratio = w as f64 / orig_width as f64;
                let nh = (orig_height as f64 * ratio).round() as u32;
                (w.max(1), nh.max(1))
            } else {
                (w.max(1), orig_height.max(1))
            }
        }
        (None, Some(h)) => {
            if maintain_aspect {
                let ratio = h as f64 / orig_height as f64;
                let nw = (orig_width as f64 * ratio).round() as u32;
                (nw.max(1), h.max(1))
            } else {
                (orig_width.max(1), h.max(1))
            }
        }
        (None, None) => (orig_width, orig_height),
    }
}