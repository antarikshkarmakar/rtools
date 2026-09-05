use crate::{
    AiCommands, Commands, ConfigCommands, DuplicateMode, ExifOutputFormat, ImageCommands,
    OrganizeMode, PdfCommands,
};
use rtools_core::{Capability, CapabilityRegistry, ProviderDiagnostic, RToolsError, RToolsResult};

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
        "doctor.report",
        "image.compress",
        "image.convert",
        "image.crop",
        "image.exif.human",
        "image.exif.json",
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
            "ai.duplicates.report",
            "Duplicate ranking has limited release-safety coverage",
        ),
        (
            "ai.organize.date",
            "Date organization relies on filesystem modification timestamps when EXIF dates are unavailable",
        ),
        (
            "ai.rename.deterministic",
            "Deterministic rename behavior has limited release-safety coverage",
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

// This table is intentionally kept together as the audited, single source of
// unavailable-operation capability metadata.
#[allow(clippy::too_many_lines)]
fn register_unavailable(registry: &mut CapabilityRegistry) -> RToolsResult<()> {
    for (operation_id, reason, remediation, provider_id) in [
        (
            "ai.alt_text",
            "No image captioning provider is configured",
            "Configure a supported image captioning provider",
            Some("onnx-runtime"),
        ),
        (
            "ai.ocr",
            "No image OCR provider is configured",
            "Configure a supported image OCR provider",
            Some("tesseract"),
        ),
        (
            "ai.duplicates.delete",
            "Deleting duplicate files is not implemented safely",
            "Use report-only duplicate detection",
            None,
        ),
        (
            "ai.duplicates.move",
            "Moving duplicate files is not implemented safely",
            "Use report-only duplicate detection",
            None,
        ),
        (
            "ai.duplicates.symlink",
            "Replacing duplicate files with symlinks is not implemented safely",
            "Use report-only duplicate detection",
            None,
        ),
        (
            "ai.organize.camera",
            "Camera-based classification is not implemented",
            "Use date organization",
            None,
        ),
        (
            "ai.organize.custom",
            "Custom classification is not implemented",
            "Use date organization",
            None,
        ),
        (
            "ai.organize.location",
            "Location classification is not implemented",
            "Use date organization",
            None,
        ),
        (
            "ai.organize.subject",
            "Subject classification is not implemented",
            "Use date organization",
            None,
        ),
        (
            "ai.rename.ai",
            "AI-generated filename descriptions are not implemented",
            "Disable AI descriptions and use deterministic filename tokens",
            None,
        ),
        (
            "ai.sort",
            "File sorting is not implemented",
            "Use date organization or sort files manually",
            None,
        ),
        (
            "batch.run",
            "Batch recipe execution is not implemented",
            "Run operations individually until typed batch execution is available",
            None,
        ),
        (
            "image.metadata.preserve",
            "Image metadata preservation is not implemented",
            "Disable metadata preservation until verified metadata export is available",
            None,
        ),
        (
            "image.metadata.strip_gps",
            "Selective GPS metadata removal is not implemented",
            "Use the default drop-all metadata policy until selective removal is available",
            None,
        ),
        (
            "image.ocr",
            "No image OCR provider is configured",
            "Configure a supported image OCR provider",
            Some("tesseract"),
        ),
        (
            "image.watermark.text",
            "Text rendering is not implemented for image watermarks",
            "Use an image watermark or configure a supported text rendering provider",
            None,
        ),
        (
            "pdf.compress.level",
            "Only medium PDF compression is implemented",
            "Use compression level medium",
            None,
        ),
        (
            "pdf.ocr",
            "No searchable PDF OCR provider is configured",
            "Configure a supported searchable PDF OCR provider",
            Some("tesseract"),
        ),
        (
            "pdf.split.images",
            "PDF split image output is not implemented",
            "Use PDF output with the default image settings",
            None,
        ),
        (
            "pdf.text",
            "PDF text extraction is not implemented in the CLI",
            "Use a verified PDF text extraction provider once one is registered",
            None,
        ),
        (
            "pdf.to_image",
            "No PDF rendering provider is configured",
            "Configure a supported PDF rendering provider",
            Some("pdfium"),
        ),
    ] {
        let capability = Capability::unavailable(operation_id, reason, remediation);
        let capability = match provider_id {
            Some(provider_id) => {
                capability.with_provider_diagnostic(unavailable_provider(provider_id))
            }
            None => capability,
        };
        registry.register(capability)?;
    }
    Ok(())
}

fn unavailable_provider(provider_id: &str) -> ProviderDiagnostic {
    match provider_id {
        "onnx-runtime" => ProviderDiagnostic::unavailable(
            provider_id,
            "No ONNX Runtime adapter is registered",
            "Register a verified ONNX Runtime adapter before enabling dependent operations",
        ),
        "pdfium" => ProviderDiagnostic::unavailable(
            provider_id,
            "No PDFium adapter is registered",
            "Register a verified PDFium adapter before enabling PDF rendering",
        ),
        "tesseract" => ProviderDiagnostic::unavailable(
            provider_id,
            "No Tesseract adapter is registered",
            "Register a verified Tesseract adapter before enabling OCR",
        ),
        _ => unreachable!("all registered provider identifiers have diagnostics"),
    }
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
            ImageCommands::Exif { format, .. } => match format {
                ExifOutputFormat::Human => vec!["image.exif.human"],
                ExifOutputFormat::Json => vec!["image.exif.json"],
            },
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
            AiCommands::Organize { strategy, .. } => match strategy {
                OrganizeMode::Date => vec!["ai.organize.date"],
                OrganizeMode::Subject => vec!["ai.organize.subject"],
                OrganizeMode::Location => vec!["ai.organize.location"],
                OrganizeMode::Camera => vec!["ai.organize.camera"],
                OrganizeMode::Custom => vec!["ai.organize.custom"],
            },
            AiCommands::Rename { pattern, .. } => {
                if pattern.contains("{subject}") {
                    vec!["ai.rename.ai"]
                } else {
                    rtools_ai::rename::validate_deterministic_pattern(pattern)?;
                    vec!["ai.rename.deterministic"]
                }
            }
            AiCommands::AltText { .. } => vec!["ai.alt_text"],
            AiCommands::Duplicates { action, .. } => match action {
                DuplicateMode::Report => vec!["ai.duplicates.report"],
                DuplicateMode::Move => vec!["ai.duplicates.move"],
                DuplicateMode::Delete => vec!["ai.duplicates.delete"],
                DuplicateMode::Symlink => vec!["ai.duplicates.symlink"],
            },
        },
        Commands::Batch { .. } => vec!["batch.run"],
        Commands::Completions { .. } => vec!["completions.generate"],
        Commands::Config { command } => match command {
            ConfigCommands::Show => vec!["config.show"],
            ConfigCommands::Init { .. } => vec!["config.init"],
            ConfigCommands::Validate { .. } => vec!["config.validate"],
        },
        Commands::Doctor => vec!["doctor.report"],
    };
    Ok(operation_ids)
}

#[cfg(test)]
mod tests {
    use super::{cli_capability_registry, required_operation_ids};
    use crate::exit;
    use crate::{
        Commands, DuplicateMode, ExifOutputFormat, ImageCommands, OrganizeMode,
        WatermarkPositionArg,
    };
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
                ("ai.duplicates.delete", CapabilityState::Unavailable),
                ("ai.duplicates.move", CapabilityState::Unavailable),
                ("ai.duplicates.report", CapabilityState::Experimental),
                ("ai.duplicates.symlink", CapabilityState::Unavailable),
                ("ai.ocr", CapabilityState::Unavailable),
                ("ai.organize.camera", CapabilityState::Unavailable),
                ("ai.organize.custom", CapabilityState::Unavailable),
                ("ai.organize.date", CapabilityState::Experimental),
                ("ai.organize.location", CapabilityState::Unavailable),
                ("ai.organize.subject", CapabilityState::Unavailable),
                ("ai.rename.ai", CapabilityState::Unavailable),
                ("ai.rename.deterministic", CapabilityState::Experimental),
                ("ai.sort", CapabilityState::Unavailable),
                ("batch.run", CapabilityState::Unavailable),
                ("completions.generate", CapabilityState::Available),
                ("config.init", CapabilityState::Available),
                ("config.show", CapabilityState::Available),
                ("config.validate", CapabilityState::Available),
                ("doctor.report", CapabilityState::Available),
                ("image.compress", CapabilityState::Available),
                ("image.convert", CapabilityState::Available),
                ("image.crop", CapabilityState::Available),
                ("image.exif.human", CapabilityState::Available),
                ("image.exif.json", CapabilityState::Available),
                ("image.filter", CapabilityState::Available),
                ("image.metadata.preserve", CapabilityState::Unavailable),
                ("image.metadata.strip_gps", CapabilityState::Unavailable,),
                ("image.ocr", CapabilityState::Unavailable),
                ("image.resize", CapabilityState::Available),
                ("image.watermark.image", CapabilityState::Available),
                ("image.watermark.text", CapabilityState::Unavailable),
                ("pdf.compress", CapabilityState::Experimental),
                ("pdf.compress.level", CapabilityState::Unavailable),
                ("pdf.merge", CapabilityState::Experimental),
                ("pdf.ocr", CapabilityState::Unavailable),
                ("pdf.split", CapabilityState::Experimental),
                ("pdf.split.images", CapabilityState::Unavailable),
                ("pdf.text", CapabilityState::Unavailable),
                ("pdf.to_image", CapabilityState::Unavailable),
            ]
        );
    }

    #[test]
    fn every_registered_unavailable_capability_maps_to_a_nonzero_exit_status() {
        let registry = cli_capability_registry().unwrap();

        for capability in registry
            .list()
            .into_iter()
            .filter(|capability| capability.state == CapabilityState::Unavailable)
        {
            let error = registry
                .require_available(&capability.operation_id)
                .unwrap_err();
            assert_eq!(error.code(), ErrorCode::CapabilityUnavailable);
            assert_ne!(
                exit::numeric_exit_code(error.code()),
                0,
                "{} returned a success process status",
                capability.operation_id
            );
        }
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
                position: WatermarkPositionArg::BottomRight,
                opacity: 0.5,
                output: None,
            },
        };
        let image = Commands::Image {
            command: ImageCommands::Watermark {
                input: vec!["input.png".into()],
                text: None,
                image: Some("logo.png".into()),
                position: WatermarkPositionArg::BottomRight,
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

    #[test]
    fn selectable_ai_and_exif_modes_map_to_distinct_capabilities() {
        let organize_subject = Commands::Ai {
            command: crate::AiCommands::Organize {
                input: "photos".into(),
                output: "out".into(),
                strategy: OrganizeMode::Subject,
            },
        };
        assert_eq!(
            required_operation_ids(&organize_subject).unwrap(),
            ["ai.organize.subject"]
        );

        let rename = Commands::Ai {
            command: crate::AiCommands::Rename {
                input: "photos".into(),
                pattern: "{date}_{name}_{index}".to_string(),
                dry_run: true,
            },
        };
        assert_eq!(
            required_operation_ids(&rename).unwrap(),
            ["ai.rename.deterministic"]
        );
        let ai_rename = Commands::Ai {
            command: crate::AiCommands::Rename {
                input: "photos".into(),
                pattern: "{date}_{subject}_{index}".to_string(),
                dry_run: true,
            },
        };
        assert_eq!(
            required_operation_ids(&ai_rename).unwrap(),
            ["ai.rename.ai"]
        );

        let unknown_rename = Commands::Ai {
            command: crate::AiCommands::Rename {
                input: "photos".into(),
                pattern: "{date}_{mystery}_{index}".to_string(),
                dry_run: true,
            },
        };
        assert_eq!(
            required_operation_ids(&unknown_rename).unwrap_err().code(),
            ErrorCode::InvalidInput
        );

        for (action, operation_id) in [
            (DuplicateMode::Report, "ai.duplicates.report"),
            (DuplicateMode::Move, "ai.duplicates.move"),
            (DuplicateMode::Delete, "ai.duplicates.delete"),
            (DuplicateMode::Symlink, "ai.duplicates.symlink"),
        ] {
            let command = Commands::Ai {
                command: crate::AiCommands::Duplicates {
                    input: "photos".into(),
                    threshold: 0.9,
                    action,
                },
            };
            assert_eq!(required_operation_ids(&command).unwrap(), [operation_id]);
        }

        for (format, operation_id) in [
            (ExifOutputFormat::Human, "image.exif.human"),
            (ExifOutputFormat::Json, "image.exif.json"),
        ] {
            let command = Commands::Image {
                command: ImageCommands::Exif {
                    input: vec!["photo.jpg".into()],
                    format,
                },
            };
            assert_eq!(required_operation_ids(&command).unwrap(), [operation_id]);
        }
    }

    #[test]
    fn every_nested_cli_enum_value_has_a_registered_capability() {
        use clap::ValueEnum as _;

        let registry = cli_capability_registry().unwrap();
        let mut operation_ids = Vec::new();
        for strategy in OrganizeMode::value_variants() {
            let command = Commands::Ai {
                command: crate::AiCommands::Organize {
                    input: "photos".into(),
                    output: "out".into(),
                    strategy: *strategy,
                },
            };
            operation_ids.extend(required_operation_ids(&command).unwrap());
        }
        for action in DuplicateMode::value_variants() {
            let command = Commands::Ai {
                command: crate::AiCommands::Duplicates {
                    input: "photos".into(),
                    threshold: 0.9,
                    action: *action,
                },
            };
            operation_ids.extend(required_operation_ids(&command).unwrap());
        }
        for format in ExifOutputFormat::value_variants() {
            let command = Commands::Image {
                command: ImageCommands::Exif {
                    input: vec!["photo.jpg".into()],
                    format: *format,
                },
            };
            operation_ids.extend(required_operation_ids(&command).unwrap());
        }

        operation_ids.sort_unstable();
        operation_ids.dedup();
        assert_eq!(operation_ids.len(), 11);
        assert!(operation_ids
            .iter()
            .all(|operation_id| registry.lookup(operation_id).is_some()));
    }

    #[test]
    fn doctor_command_is_registered_and_advertised_in_help() {
        use clap::CommandFactory as _;

        let help = crate::Cli::command().render_long_help().to_string();
        assert_eq!(
            required_operation_ids(&Commands::Doctor).unwrap(),
            ["doctor.report"]
        );
        assert!(help.contains("doctor"));
    }
}
