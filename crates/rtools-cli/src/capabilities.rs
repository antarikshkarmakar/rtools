use crate::{AiCommands, Commands, ConfigCommands, ImageCommands, PdfCommands};
use rtools_core::{Capability, CapabilityRegistry, RToolsError, RToolsResult};

pub fn cli_capability_registry() -> RToolsResult<CapabilityRegistry> {
    let mut registry = CapabilityRegistry::default();
    register_available(&mut registry)?;
    register_experimental(&mut registry)?;
    register_unavailable(&mut registry)?;
    Ok(registry)
}

fn register_available(registry: &mut CapabilityRegistry) -> RToolsResult<()> {
    for operation_id in [
        "completions.generate",
        "config.init",
        "config.show",
        "config.validate",
        "image.compress",
        "image.convert",
        "image.crop",
        "image.exif",
        "image.filter",
        "image.resize",
        "image.watermark.image",
    ] {
        registry.register(Capability::available(operation_id))?;
    }
    Ok(())
}

fn register_experimental(registry: &mut CapabilityRegistry) -> RToolsResult<()> {
    for (operation_id, reason) in [
        (
            "ai.duplicates",
            "Duplicate ranking and destructive actions have limited release-safety coverage",
        ),
        (
            "pdf.compress",
            "PDF structure preservation is only partially verified",
        ),
        (
            "pdf.merge",
            "PDF structure preservation is only partially verified",
        ),
        (
            "pdf.split",
            "PDF structure preservation is only partially verified",
        ),
    ] {
        registry.register(Capability::experimental(operation_id, reason))?;
    }
    Ok(())
}

fn register_unavailable(registry: &mut CapabilityRegistry) -> RToolsResult<()> {
    for (operation_id, reason, remediation) in [
        (
            "ai.alt_text",
            "No image captioning provider is configured",
            "Configure a supported image captioning provider; run rtools doctor once available in this release",
        ),
        (
            "ai.ocr",
            "No image OCR provider is configured",
            "Configure a supported image OCR provider; run rtools doctor once available in this release",
        ),
        (
            "ai.organize",
            "AI subject and location classification are not implemented",
            "Use explicit filesystem organization until a supported classification provider is configured",
        ),
        (
            "ai.rename",
            "AI-generated filename descriptions are not implemented",
            "Use a deterministic rename tool until a supported description provider is configured",
        ),
        (
            "batch.run",
            "Batch recipe execution is not implemented",
            "Run operations individually until typed batch execution is available",
        ),
        (
            "image.metadata.preserve",
            "Image metadata preservation is not implemented",
            "Disable metadata preservation until verified metadata export is available",
        ),
        (
            "image.metadata.strip_gps",
            "Selective GPS metadata removal is not implemented",
            "Use the default drop-all metadata policy until selective removal is available",
        ),
        (
            "image.ocr",
            "No image OCR provider is configured",
            "Configure a supported image OCR provider; run rtools doctor once available in this release",
        ),
        (
            "image.watermark.text",
            "Text rendering is not implemented for image watermarks",
            "Use an image watermark or configure a supported text rendering provider",
        ),
        (
            "pdf.ocr",
            "No searchable PDF OCR provider is configured",
            "Configure a supported searchable PDF OCR provider; run rtools doctor once available in this release",
        ),
        (
            "pdf.text",
            "PDF text extraction is not implemented in the CLI",
            "Use a verified PDF text extraction provider once one is registered",
        ),
        (
            "pdf.to_image",
            "No PDF rendering provider is configured",
            "Configure a supported PDF rendering provider; run rtools doctor once available in this release",
        ),
    ] {
        registry.register(Capability::unavailable(operation_id, reason, remediation))?;
    }
    Ok(())
}

pub fn required_operation_ids(command: &Commands) -> RToolsResult<Vec<&'static str>> {
    let operation_ids = match command {
        Commands::Image { command } => match command {
            ImageCommands::Compress {
                preserve_metadata,
                strip_gps,
                ..
            } => {
                if *preserve_metadata && *strip_gps {
                    return Err(RToolsError::invalid_input(
                        "Metadata cannot be preserved while GPS metadata is stripped",
                    ));
                }
                let mut required = vec!["image.compress"];
                if *preserve_metadata {
                    required.push("image.metadata.preserve");
                }
                if *strip_gps {
                    required.push("image.metadata.strip_gps");
                }
                required
            }
            ImageCommands::Convert { .. } => vec!["image.convert"],
            ImageCommands::Resize { .. } => vec!["image.resize"],
            ImageCommands::Crop { .. } => vec!["image.crop"],
            ImageCommands::Watermark { text, .. } if text.is_some() => {
                vec!["image.watermark.text"]
            }
            ImageCommands::Watermark { .. } => vec!["image.watermark.image"],
            ImageCommands::Filter { .. } => vec!["image.filter"],
            ImageCommands::Exif { .. } => vec!["image.exif"],
            ImageCommands::Ocr { .. } => vec!["image.ocr"],
        },
        Commands::Pdf { command } => match command {
            PdfCommands::Merge { .. } => vec!["pdf.merge"],
            PdfCommands::Compress { .. } => vec!["pdf.compress"],
            PdfCommands::Split { .. } => vec!["pdf.split"],
            PdfCommands::Text { .. } => vec!["pdf.text"],
            PdfCommands::ToImage { .. } => vec!["pdf.to_image"],
        },
        Commands::Ai { command } => match command {
            AiCommands::Organize { .. } => vec!["ai.organize"],
            AiCommands::Rename { .. } => vec!["ai.rename"],
            AiCommands::AltText { .. } => vec!["ai.alt_text"],
            AiCommands::Duplicates { .. } => vec!["ai.duplicates"],
        },
        Commands::Batch { .. } => vec!["batch.run"],
        Commands::Completions { .. } => vec!["completions.generate"],
        Commands::Config { command } => match command {
            ConfigCommands::Show => vec!["config.show"],
            ConfigCommands::Init { .. } => vec!["config.init"],
            ConfigCommands::Validate { .. } => vec!["config.validate"],
        },
    };
    Ok(operation_ids)
}

#[cfg(test)]
mod tests {
    use super::{cli_capability_registry, required_operation_ids};
    use crate::{Commands, ImageCommands};
    use rtools_core::{CapabilityState, ErrorCode};

    #[test]
    fn registry_is_the_exact_sorted_truth_for_the_cli_surface() {
        let registry = cli_capability_registry().unwrap();
        let actual: Vec<(&str, CapabilityState)> = registry
            .list()
            .into_iter()
            .map(|capability| (capability.operation_id.as_str(), capability.state))
            .collect();

        assert_eq!(
            actual,
            [
                ("ai.alt_text", CapabilityState::Unavailable),
                ("ai.duplicates", CapabilityState::Experimental),
                ("ai.ocr", CapabilityState::Unavailable),
                ("ai.organize", CapabilityState::Unavailable),
                ("ai.rename", CapabilityState::Unavailable),
                ("batch.run", CapabilityState::Unavailable),
                ("completions.generate", CapabilityState::Available),
                ("config.init", CapabilityState::Available),
                ("config.show", CapabilityState::Available),
                ("config.validate", CapabilityState::Available),
                ("image.compress", CapabilityState::Available),
                ("image.convert", CapabilityState::Available),
                ("image.crop", CapabilityState::Available),
                ("image.exif", CapabilityState::Available),
                ("image.filter", CapabilityState::Available),
                ("image.metadata.preserve", CapabilityState::Unavailable),
                ("image.metadata.strip_gps", CapabilityState::Unavailable,),
                ("image.ocr", CapabilityState::Unavailable),
                ("image.resize", CapabilityState::Available),
                ("image.watermark.image", CapabilityState::Available),
                ("image.watermark.text", CapabilityState::Unavailable),
                ("pdf.compress", CapabilityState::Experimental),
                ("pdf.merge", CapabilityState::Experimental),
                ("pdf.ocr", CapabilityState::Unavailable),
                ("pdf.split", CapabilityState::Experimental),
                ("pdf.text", CapabilityState::Unavailable),
                ("pdf.to_image", CapabilityState::Unavailable),
            ]
        );
    }

    #[test]
    fn metadata_flags_select_separate_capabilities_and_reject_conflicts() {
        let preserve = Commands::Image {
            command: ImageCommands::Compress {
                input: vec!["input.png".into()],
                output: None,
                quality: 85,
                format: None,
                preserve_metadata: true,
                strip_gps: false,
            },
        };
        assert_eq!(
            required_operation_ids(&preserve).unwrap(),
            ["image.compress", "image.metadata.preserve"]
        );

        let conflict = Commands::Image {
            command: ImageCommands::Compress {
                input: vec!["input.png".into()],
                output: None,
                quality: 85,
                format: None,
                preserve_metadata: true,
                strip_gps: true,
            },
        };
        assert_eq!(
            required_operation_ids(&conflict).unwrap_err().code(),
            ErrorCode::InvalidInput
        );
    }

    #[test]
    fn text_and_image_watermarks_map_to_distinct_runtime_capabilities() {
        let text = Commands::Image {
            command: ImageCommands::Watermark {
                input: vec!["input.png".into()],
                text: Some("copyright".to_string()),
                image: None,
                position: "bottom-right".to_string(),
                opacity: 0.5,
                output: None,
            },
        };
        let image = Commands::Image {
            command: ImageCommands::Watermark {
                input: vec!["input.png".into()],
                text: None,
                image: Some("logo.png".into()),
                position: "bottom-right".to_string(),
                opacity: 0.5,
                output: None,
            },
        };

        assert_eq!(
            required_operation_ids(&text).unwrap(),
            ["image.watermark.text"]
        );
        assert_eq!(
            required_operation_ids(&image).unwrap(),
            ["image.watermark.image"]
        );
    }
}
