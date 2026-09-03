use rtools_core::error::{RToolsError, RToolsResult};
use rtools_core::types::ProcessStats;
use rtools_core::{FileInput, FileOutput, OutputPolicy, PendingOutput, Processor, ResourceLimits};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Instant;

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn bounded_dimension(value: f64) -> u32 {
    // Dimension calculations are non-negative and bounded by image dimensions.
    value.clamp(0.0, f64::from(u32::MAX)).round() as u32
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

/// Resize algorithm
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub enum ResizeAlgorithm {
    /// Lanczos resampling (high quality)
    #[default]
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
    /// Collision behavior for the final output path.
    #[serde(default)]
    pub output_policy: OutputPolicy,
    /// Output quality for lossy formats (0-100)
    pub quality: u8,
    /// Resource limits enforced before image decoding.
    #[serde(default)]
    pub limits: ResourceLimits,
}

impl Default for ResizeConfig {
    fn default() -> Self {
        Self {
            width: None,
            height: None,
            maintain_aspect: true,
            algorithm: ResizeAlgorithm::default(),
            output: None,
            output_policy: OutputPolicy::default(),
            quality: 85,
            limits: ResourceLimits::default(),
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

    fn process_validated(
        &self,
        input: FileInput,
        config: ResizeConfig,
    ) -> RToolsResult<FileOutput> {
        let start = Instant::now();

        let path = input
            .source
            .as_path()
            .ok_or_else(|| RToolsError::invalid_input("Resize requires a file path input"))?;

        if config.width.is_none() && config.height.is_none() {
            return Err(RToolsError::invalid_input(
                "At least one of width or height must be specified",
            ));
        }

        let decoded = crate::format::decode_bounded(path, &config.limits)?;
        let warnings = decoded.warnings();
        let img = decoded.image;
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
            ResizeAlgorithm::CatmullRom | ResizeAlgorithm::MitchellNetravali => {
                image::imageops::FilterType::CatmullRom
            }
            ResizeAlgorithm::NearestNeighbor => image::imageops::FilterType::Nearest,
        };

        let resized = if config.maintain_aspect {
            img.resize(new_width.max(1), new_height.max(1), filter)
        } else {
            img.resize_exact(new_width.max(1), new_height.max(1), filter)
        };

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
        let file_name = format!("{stem}_{new_width}x{new_height}.{ext}");
        let output = match config.output {
            Some(out) => rtools_core::resolve_output_path(&out, &file_name),
            None => path
                .parent()
                .unwrap_or_else(|| std::path::Path::new("."))
                .join(file_name),
        };

        if let Some(parent) = output.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let image_format = image::ImageFormat::from_path(&output).map_err(|error| {
            RToolsError::image(format!("Invalid resize output format: {error}"))
        })?;
        let pending = PendingOutput::new(&output, config.output_policy)?;

        resized
            .save_with_format(pending.temporary_path(), image_format)
            .map_err(|e| RToolsError::image(format!("Failed to save resized image: {e}")))?;
        let output = pending.commit(crate::format::validate_image_artifact)?;

        let elapsed = start.elapsed();
        let input_size = std::fs::metadata(path)?.len();
        let output_size = std::fs::metadata(&output)?.len();

        // Derive MIME type from output path, not input
        let output_format = rtools_core::ImageFormat::from_path(&output)
            .or(input.format)
            .unwrap_or(rtools_core::types::ImageFormat::Jpeg);

        Ok(FileOutput {
            destination: rtools_core::output::OutputDestination::File(output),
            name: None,
            mime_type: Some(output_format.mime_type().to_string()),
            stats: Some(ProcessStats {
                input_size,
                output_size,
                compression_ratio: compression_ratio(output_size, input_size),
                processing_time_ms: u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX),
                memory_used_mb: 0.0,
            }),
            warnings,
        })
    }

    fn validate_config(&self, config: &ResizeConfig) -> RToolsResult<()> {
        if let Some(w) = config.width {
            if w == 0 || w > 32768 {
                return Err(RToolsError::invalid_input(format!("Invalid width: {w}")));
            }
        }
        if let Some(h) = config.height {
            if h == 0 || h > 32768 {
                return Err(RToolsError::invalid_input(format!("Invalid height: {h}")));
            }
        }
        Ok(())
    }

    fn name(&self) -> &'static str {
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
                let ratio = (f64::from(w) / f64::from(orig_width))
                    .min(f64::from(h) / f64::from(orig_height));
                let nw = bounded_dimension(f64::from(orig_width) * ratio);
                let nh = bounded_dimension(f64::from(orig_height) * ratio);
                (nw.max(1), nh.max(1))
            } else {
                (w.max(1), h.max(1))
            }
        }
        (Some(w), None) => {
            if maintain_aspect {
                let ratio = f64::from(w) / f64::from(orig_width);
                let nh = bounded_dimension(f64::from(orig_height) * ratio);
                (w.max(1), nh.max(1))
            } else {
                (w.max(1), orig_height.max(1))
            }
        }
        (None, Some(h)) => {
            if maintain_aspect {
                let ratio = f64::from(h) / f64::from(orig_height);
                let nw = bounded_dimension(f64::from(orig_width) * ratio);
                (nw.max(1), h.max(1))
            } else {
                (orig_width.max(1), h.max(1))
            }
        }
        (None, None) => (orig_width, orig_height),
    }
}
