use rtools_core::error::{RToolsError, RToolsResult};
use rtools_core::types::ProcessStats;
use rtools_core::{FileInput, FileOutput, OutputPolicy, PendingOutput, Processor, ResourceLimits};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Instant;

/// Film filter presets
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FilmFilter {
    /// Kodak Portra 400
    KodakPortra400,
    /// Kodak Gold 200
    KodakGold200,
    /// Kodak Ektar 100
    KodakEktar100,
    /// Fujifilm Pro 400H
    FujiPro400H,
    /// Fujifilm Velvia 50
    FujiVelvia50,
    /// Fujifilm Superia 400
    FujiSuperia400,
    /// Polaroid SX-70
    PolaroidSX70,
    /// Polaroid 600
    Polaroid600,
    /// Ilford HP5 Plus (B&W)
    IlfordHP5,
    /// Ilford FP4 Plus (B&W)
    IlfordFP4,
    /// Tri-X 400 (B&W)
    TriX400,
    /// Cinestill 800T
    Cinestill800T,
    /// Lomography 400
    Lomography400,
    /// Agfa Vista 200
    AgfaVista200,
}

/// Filter configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilterConfig {
    /// Filter to apply
    pub filter: FilmFilter,
    /// Strength (0.0-1.0)
    pub strength: f64,
    /// Output path (None = generated alongside input); its parent must exist.
    pub output: Option<PathBuf>,
    /// Collision behavior for the final output path.
    #[serde(default)]
    pub output_policy: OutputPolicy,
    /// Legacy output quality; currently only the default value 85 is supported.
    pub quality: u8,
    /// Resource limits enforced before image decoding.
    #[serde(default)]
    pub limits: ResourceLimits,
}

impl Default for FilterConfig {
    fn default() -> Self {
        Self {
            filter: FilmFilter::KodakPortra400,
            strength: 1.0,
            output: None,
            output_policy: OutputPolicy::default(),
            quality: 85,
            limits: ResourceLimits::default(),
        }
    }
}

/// Film filter processor
pub struct FilterProcessor;

impl Processor for FilterProcessor {
    type Input = FileInput;
    type Output = FileOutput;
    type Config = FilterConfig;
    type Error = RToolsError;

    fn process_validated(
        &self,
        input: FileInput,
        config: FilterConfig,
    ) -> RToolsResult<FileOutput> {
        let start = Instant::now();

        let path = input
            .source
            .as_path()
            .ok_or_else(|| RToolsError::invalid_input("Filter requires a file path input"))?;

        let decoded = crate::format::decode_bounded(path, &config.limits)?;
        let warnings = decoded.warnings();
        let img = decoded.image;

        let output = config.output.unwrap_or_else(|| {
            let mut out = path.clone();
            let stem = out
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            let ext = out
                .extension()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            let filter_name = format!("{:?}", config.filter).to_lowercase();
            out.set_file_name(format!("{stem}_{filter_name}.{ext}"));
            out
        });

        let filtered = apply_film_filter(&img, &config.filter, config.strength.clamp(0.0, 1.0));
        let (format, image_format) = crate::format::resolve_output_format(&output, "Filter")?;
        let pending = PendingOutput::new(&output, config.output_policy)?;

        filtered
            .save_with_format(pending.temporary_path(), image_format)
            .map_err(|e| RToolsError::image(format!("Failed to save filtered image: {e}")))?;
        let output = pending.commit(|artifact| {
            crate::metadata::validate_drop_all_artifact(artifact, &config.limits)
        })?;

        let elapsed = start.elapsed();
        let input_size = std::fs::metadata(path)?.len();
        let output_size = std::fs::metadata(&output)?.len();

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
            warnings,
        })
    }

    fn validate_config(&self, config: &FilterConfig) -> RToolsResult<()> {
        if config.quality != 85 {
            return Err(RToolsError::invalid_input(
                "Filter quality is unsupported; use the legacy value 85",
            ));
        }
        if !config.strength.is_finite() || config.strength < 0.0 || config.strength > 1.0 {
            return Err(RToolsError::invalid_input(
                "Strength must be between 0.0 and 1.0",
            ));
        }
        Ok(())
    }

    fn name(&self) -> &'static str {
        "FilterProcessor"
    }
}

/// Apply film filter to image with realistic channel color grading
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn channel(value: f64) -> u8 {
    // Every caller clamps color-channel values to the inclusive u8 range.
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

fn apply_film_filter(
    img: &image::DynamicImage,
    filter: &FilmFilter,
    strength: f64,
) -> image::DynamicImage {
    let mut rgba = img.to_rgba8();

    // Color balance multipliers (R, G, B, Saturation multiplier, Is B&W)
    let (r_scale, g_scale, b_scale, sat_scale, is_bw) = match filter {
        FilmFilter::KodakPortra400 => (1.08, 1.02, 0.95, 1.10, false),
        FilmFilter::KodakGold200 => (1.15, 1.05, 0.90, 1.15, false),
        FilmFilter::KodakEktar100 => (1.05, 1.00, 1.05, 1.25, false),
        FilmFilter::FujiPro400H => (0.95, 1.05, 1.08, 0.95, false),
        FilmFilter::FujiVelvia50 => (1.05, 1.02, 1.10, 1.35, false),
        FilmFilter::FujiSuperia400 => (1.00, 1.04, 1.06, 1.10, false),
        FilmFilter::PolaroidSX70 => (1.15, 1.08, 0.85, 0.85, false),
        FilmFilter::Polaroid600 => (1.10, 1.05, 0.90, 0.90, false),
        FilmFilter::IlfordHP5 | FilmFilter::IlfordFP4 | FilmFilter::TriX400 => {
            (1.00, 1.00, 1.00, 0.0, true)
        }
        FilmFilter::Cinestill800T => (0.90, 1.05, 1.18, 1.05, false),
        FilmFilter::Lomography400 => (1.15, 1.00, 1.12, 1.20, false),
        FilmFilter::AgfaVista200 => (1.10, 0.98, 0.95, 1.05, false),
    };

    for pixel in rgba.pixels_mut() {
        let orig_r = f64::from(pixel[0]);
        let orig_g = f64::from(pixel[1]);
        let orig_b = f64::from(pixel[2]);
        let a = pixel[3];

        if is_bw {
            // Luminance grayscale conversion
            let lum = 0.114f64
                .mul_add(orig_b, 0.299f64.mul_add(orig_r, 0.587 * orig_g))
                .clamp(0.0, 255.0);
            let final_r = (lum - orig_r).mul_add(strength, orig_r);
            let final_g = (lum - orig_g).mul_add(strength, orig_g);
            let final_b = (lum - orig_b).mul_add(strength, orig_b);

            pixel[0] = channel(final_r);
            pixel[1] = channel(final_g);
            pixel[2] = channel(final_b);
        } else {
            // Channel color shift
            let shifted_r = (orig_r * r_scale).clamp(0.0, 255.0);
            let shifted_g = (orig_g * g_scale).clamp(0.0, 255.0);
            let shifted_b = (orig_b * b_scale).clamp(0.0, 255.0);

            // Saturation adjustment
            let lum = 0.114f64.mul_add(shifted_b, 0.299f64.mul_add(shifted_r, 0.587 * shifted_g));
            let sat_r = (shifted_r - lum).mul_add(sat_scale, lum);
            let sat_g = (shifted_g - lum).mul_add(sat_scale, lum);
            let sat_b = (shifted_b - lum).mul_add(sat_scale, lum);

            // Blend with original according to strength
            let target_r = (sat_r - orig_r).mul_add(strength, orig_r);
            let target_g = (sat_g - orig_g).mul_add(strength, orig_g);
            let target_b = (sat_b - orig_b).mul_add(strength, orig_b);

            pixel[0] = channel(target_r);
            pixel[1] = channel(target_g);
            pixel[2] = channel(target_b);
        }
        pixel[3] = a;
    }

    image::DynamicImage::ImageRgba8(rgba)
}
