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
    Pixels { x: u32, y: u32 },
    /// Preset positions
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
    Center,
    /// Custom percentage (x%, y%)
    Percentage { x: f64, y: f64 },
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
    Image {
        image_path: PathBuf,
        scale: f64,
    },
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
                text: "© 2024".to_string(),
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

impl Processor for WatermarkProcessor {
    type Input = FileInput;
    type Output = FileOutput;
    type Config = WatermarkConfig;
    type Error = RToolsError;

    fn process(&self, input: FileInput, config: WatermarkConfig) -> RToolsResult<FileOutput> {
        let start = Instant::now();

        let path = input.source.as_path().ok_or_else(|| {
            RToolsError::invalid_input("Watermark requires a file path input")
        })?;

        let img = image::open(path)?;

        let output = config.output.unwrap_or_else(|| {
            let mut out = path.clone();
            let stem = out.file_stem().unwrap_or_default();
            let ext = out.extension().unwrap_or_default();
            out.set_file_name(format!("{}_watermarked", stem.to_string_lossy()));
            out.set_extension(ext);
            out
        });

        // For now, just save the image - actual watermark implementation will use imageproc
        // TODO: Implement proper watermarking with imageproc crate
        img.save(&output)?;

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

    fn validate_config(&self, config: &WatermarkConfig) -> RToolsResult<()> {
        if config.opacity < 0.0 || config.opacity > 1.0 {
            return Err(RToolsError::invalid_input("Opacity must be between 0.0 and 1.0"));
        }
        Ok(())
    }

    fn name(&self) -> &str {
        "WatermarkProcessor"
    }
}