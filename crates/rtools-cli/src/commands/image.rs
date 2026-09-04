use crate::commands::{CommandResult, ItemFailure};
use crate::{AspectRatioArg, CropRegionArg, ExifOutputFormat, ImageCommands, ImageFormatArg};
use rtools_core::{AppConfig, FileInput, Processor, RToolsError, RToolsResult};
use rtools_image::{
    CompressConfig, CompressProcessor, ConvertConfig, ConvertProcessor, CropConfig, CropProcessor,
    FilterConfig, FilterProcessor, ResizeConfig, ResizeProcessor, WatermarkConfig,
    WatermarkProcessor,
};
use serde::Serialize;

#[derive(Serialize)]
struct ExifJsonDocument {
    results: Vec<ExifJsonResult>,
}

#[derive(Serialize)]
struct ExifJsonResult {
    path: String,
    exif: rtools_core::types::ExifData,
}

#[allow(clippy::too_many_lines)]
pub fn handle_image_command(
    command: ImageCommands,
    config: &AppConfig,
) -> RToolsResult<CommandResult> {
    match command {
        ImageCommands::Compress {
            input,
            output,
            quality,
            format,
            preserve_metadata,
            strip_gps,
        } => {
            let processor = CompressProcessor;
            let processor_config = CompressConfig {
                preset: rtools_image::compress::CompressionPreset::Custom(quality),
                format: format.map(ImageFormatArg::into_core),
                output,
                output_policy: rtools_core::OutputPolicy::default(),
                preserve_metadata,
                strip_gps,
                limits: config.limits.clone(),
            };
            let outcomes = process_each(&processor, input, processor_config);
            CommandResult::from_file_output_outcomes(
                "image.compress",
                format!("Compressed {} image(s)", outcomes.outputs.len()),
                outcomes.outputs,
                outcomes.failures,
            )
        }
        ImageCommands::Convert {
            input,
            format,
            output,
            quality,
        } => {
            let processor = ConvertProcessor;
            let processor_config = ConvertConfig {
                target_format: format.into_core(),
                output,
                output_policy: rtools_core::OutputPolicy::default(),
                output_dir: None,
                quality,
                preserve_metadata: false,
                strip_gps: false,
                limits: config.limits.clone(),
            };
            let outcomes = process_each(&processor, input, processor_config);
            CommandResult::from_file_output_outcomes(
                "image.convert",
                format!("Converted {} image(s)", outcomes.outputs.len()),
                outcomes.outputs,
                outcomes.failures,
            )
        }
        ImageCommands::Resize {
            input,
            width,
            height,
            maintain_aspect,
            output,
        } => {
            let processor = ResizeProcessor;
            let processor_config = ResizeConfig {
                width,
                height,
                maintain_aspect,
                algorithm: rtools_image::resize::ResizeAlgorithm::default(),
                output,
                output_policy: rtools_core::OutputPolicy::default(),
                quality: 85,
                limits: config.limits.clone(),
            };
            let outcomes = process_each(&processor, input, processor_config);
            CommandResult::from_file_output_outcomes(
                "image.resize",
                format!("Resized {} image(s)", outcomes.outputs.len()),
                outcomes.outputs,
                outcomes.failures,
            )
        }
        ImageCommands::Crop {
            input,
            region,
            ratio,
            gravity,
            output,
        } => {
            let processor = CropProcessor;
            let gravity = gravity.into_image();
            let region = crop_region(region, ratio, gravity);
            let processor_config = CropConfig {
                region,
                output,
                output_policy: rtools_core::OutputPolicy::default(),
                quality: 85,
                limits: config.limits.clone(),
            };
            let outcomes = process_each(&processor, input, processor_config);
            CommandResult::from_file_output_outcomes(
                "image.crop",
                format!("Cropped {} image(s)", outcomes.outputs.len()),
                outcomes.outputs,
                outcomes.failures,
            )
        }
        ImageCommands::Watermark {
            input,
            text,
            image,
            position,
            opacity,
            output,
        } => {
            let processor = WatermarkProcessor;
            let watermark = match (text, image) {
                (Some(text), None) => rtools_image::watermark::WatermarkType::Text {
                    text,
                    font_size: 24,
                    font_color: "#ffffff".to_string(),
                },
                (None, Some(image_path)) => rtools_image::watermark::WatermarkType::Image {
                    image_path,
                    scale: 0.2,
                },
                (None, None) => {
                    return Err(RToolsError::invalid_input(
                        "Either --text or --image watermark must be specified",
                    ));
                }
                (Some(_), Some(_)) => {
                    return Err(RToolsError::invalid_input(
                        "--text and --image watermark cannot be combined",
                    ));
                }
            };
            let processor_config = WatermarkConfig {
                watermark,
                position: position.into_image(),
                opacity,
                output,
                output_policy: rtools_core::OutputPolicy::default(),
                quality: 85,
                limits: config.limits.clone(),
            };
            let outcomes = process_each(&processor, input, processor_config);
            CommandResult::from_file_output_outcomes(
                "image.watermark.image",
                format!("Watermarked {} image(s)", outcomes.outputs.len()),
                outcomes.outputs,
                outcomes.failures,
            )
        }
        ImageCommands::Filter {
            input,
            preset,
            strength,
            output,
        } => {
            let processor = FilterProcessor;
            let processor_config = FilterConfig {
                filter: preset.into_image(),
                strength,
                output,
                output_policy: rtools_core::OutputPolicy::default(),
                quality: 85,
                limits: config.limits.clone(),
            };
            let outcomes = process_each(&processor, input, processor_config);
            CommandResult::from_file_output_outcomes(
                "image.filter",
                format!("Filtered {} image(s)", outcomes.outputs.len()),
                outcomes.outputs,
                outcomes.failures,
            )
        }
        ImageCommands::Exif { input, format } => {
            let processor = rtools_image::ExifProcessor;
            let processor_config = rtools_image::exif::ExifConfig::default();
            let mut results = Vec::new();
            let mut failures = Vec::new();
            for path in input {
                match processor
                    .process(FileInput::from_path(path.clone()), processor_config.clone())
                {
                    Ok(exif) => results.push(ExifJsonResult {
                        path: path.display().to_string(),
                        exif,
                    }),
                    Err(error) => {
                        failures.push(ItemFailure::from_error(&error, path.display().to_string()));
                    }
                }
            }
            let operation_id = match format {
                ExifOutputFormat::Human => "image.exif.human",
                ExifOutputFormat::Json => "image.exif.json",
            };
            let has_successes = !results.is_empty();
            CommandResult::from_serializable_with_outcomes(
                operation_id,
                ExifJsonDocument { results },
                Vec::new(),
                failures,
                has_successes,
            )
        }
        ImageCommands::Ocr { .. } => Err(RToolsError::capability_unavailable(
            "image.ocr",
            "No image OCR provider is configured",
            "Configure a supported image OCR provider",
        )),
    }
}

fn crop_region(
    region: Option<CropRegionArg>,
    ratio: Option<AspectRatioArg>,
    gravity: rtools_image::crop::Gravity,
) -> rtools_image::crop::CropRegion {
    match (region, ratio) {
        (
            Some(CropRegionArg {
                x,
                y,
                width,
                height,
            }),
            None,
        ) => rtools_image::crop::CropRegion::Pixels {
            x,
            y,
            width,
            height,
        },
        (None, Some(AspectRatioArg(width, height))) => {
            rtools_image::crop::CropRegion::AspectRatio {
                ratio: rtools_image::crop::AspectRatio::Custom(width, height),
                gravity,
            }
        }
        (None, None) => rtools_image::crop::CropRegion::AspectRatio {
            ratio: rtools_image::crop::AspectRatio::Square,
            gravity,
        },
        (Some(_), Some(_)) => unreachable!("clap rejects simultaneous crop region and ratio"),
    }
}

struct FileOutputOutcomes {
    outputs: Vec<rtools_core::FileOutput>,
    failures: Vec<ItemFailure>,
}

fn process_each<P>(
    processor: &P,
    inputs: Vec<std::path::PathBuf>,
    config: P::Config,
) -> FileOutputOutcomes
where
    P: Processor,
    P::Input: From<FileInput>,
    P::Output: Into<rtools_core::FileOutput>,
    P::Config: Clone,
{
    let mut outcomes = FileOutputOutcomes {
        outputs: Vec::new(),
        failures: Vec::new(),
    };
    for path in inputs {
        match processor.process(FileInput::from_path(path.clone()).into(), config.clone()) {
            Ok(output) => outcomes.outputs.push(output.into()),
            Err(error) => outcomes
                .failures
                .push(ItemFailure::from_error(&error, path.display().to_string())),
        }
    }
    outcomes
}

#[cfg(test)]
mod tests {
    use super::handle_image_command;
    use crate::ImageCommands;
    use rtools_core::{AppConfig, ErrorCode, RToolsError};

    #[tokio::test]
    async fn image_ocr_processor_error_is_propagated() {
        let error = handle_image_command(
            ImageCommands::Ocr {
                input: vec!["private.png".into()],
                language: "eng".to_string(),
                output: None,
            },
            &AppConfig::default(),
        )
        .unwrap_err();

        assert_eq!(error.code(), ErrorCode::CapabilityUnavailable);
        assert!(matches!(
            error,
            RToolsError::CapabilityUnavailable { operation_id, .. }
                if operation_id == "image.ocr"
        ));
    }

    #[test]
    fn available_image_processor_error_is_retained_with_its_item_path() {
        let result = handle_image_command(
            ImageCommands::Resize {
                input: vec!["missing.png".into()],
                width: Some(10),
                height: None,
                maintain_aspect: true,
                output: None,
            },
            &AppConfig::default(),
        )
        .expect("item failures must be returned as a command result");

        assert_eq!(result.failures.len(), 1);
        assert_eq!(result.failures[0].item.as_deref(), Some("missing.png"));
        assert!(matches!(
            result.failures[0].code,
            ErrorCode::ProcessingFailed | ErrorCode::InvalidInput
        ));
    }
}
