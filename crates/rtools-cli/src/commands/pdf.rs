use crate::commands::CommandResult;
use crate::{PageSelection, PdfCommands};
use rtools_core::{AppConfig, FileInput, Processor, RToolsError, RToolsResult};

#[allow(clippy::too_many_lines)]
pub fn handle_pdf_command(command: PdfCommands, config: &AppConfig) -> RToolsResult<CommandResult> {
    match command {
        PdfCommands::Merge { input, output } => {
            config
                .limits
                .check_batch_items(u64::try_from(input.len()).unwrap_or(u64::MAX))?;
            validate_pdf_input_sizes(&input, config)?;
            let processor = rtools_pdf::PdfMergeProcessor;
            let processor_config = rtools_pdf::PdfMergeConfig {
                inputs: Vec::new(),
                output,
                add_page_numbers: false,
            };
            let file_inputs = input.into_iter().map(FileInput::from_path).collect();
            let output = processor.process(file_inputs, processor_config)?;
            CommandResult::from_file_outputs("pdf.merge", "Merged PDF files", vec![output])
        }
        PdfCommands::Compress {
            input,
            output,
            level,
            remove_metadata,
        } => {
            let processor = rtools_pdf::PdfCompressProcessor;
            let processor_config = rtools_pdf::PdfCompressConfig {
                level: level.map_or_else(
                    || match config.pdf.compression_level {
                        rtools_core::config::PdfCompressionLevel::Light => {
                            rtools_pdf::compress::PdfCompressionLevel::Light
                        }
                        rtools_core::config::PdfCompressionLevel::Medium => {
                            rtools_pdf::compress::PdfCompressionLevel::Medium
                        }
                        rtools_core::config::PdfCompressionLevel::Heavy => {
                            rtools_pdf::compress::PdfCompressionLevel::Heavy
                        }
                    },
                    crate::PdfCompressionArg::into_pdf,
                ),
                output,
                remove_metadata,
            };
            processor.validate_config(&processor_config)?;
            validate_pdf_input_sizes(std::slice::from_ref(&input), config)?;
            let output = processor.process(FileInput::from_path(input), processor_config)?;
            CommandResult::from_file_outputs("pdf.compress", "Compressed PDF", vec![output])
        }
        PdfCommands::Split {
            input,
            pages,
            output,
            filename_pattern,
        } => {
            let processor = rtools_pdf::PdfSplitProcessor;
            let processor_config = rtools_pdf::PdfSplitConfig {
                range: pages.map_or(rtools_pdf::split::PageRange::All, |PageSelection(range)| {
                    range
                }),
                output_dir: output,
                filename_pattern,
                as_images: false,
                image_format: Some("png".to_string()),
                image_dpi: 300,
            };
            processor.validate_config(&processor_config)?;
            validate_pdf_input_sizes(std::slice::from_ref(&input), config)?;
            let outputs = processor.process(FileInput::from_path(input), processor_config)?;
            CommandResult::from_file_outputs(
                "pdf.split",
                format!("Split into {} page(s)", outputs.len()),
                outputs,
            )
        }
        PdfCommands::Text { .. } => Err(RToolsError::capability_unavailable(
            "pdf.text",
            "PDF text extraction is not implemented in the CLI",
            "Use a verified PDF text extraction provider once one is registered",
        )),
        PdfCommands::ToImage { .. } => Err(RToolsError::capability_unavailable(
            "pdf.to_image",
            "No PDF rendering provider is configured",
            "Configure a supported PDF rendering provider",
        )),
    }
}

fn validate_pdf_input_sizes(paths: &[std::path::PathBuf], config: &AppConfig) -> RToolsResult<()> {
    for path in paths {
        if let Ok(metadata) = std::fs::metadata(path) {
            config.limits.check_input_bytes(metadata.len())?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::handle_pdf_command;
    use crate::{PdfCommands, PdfImageFormatArg};
    use rtools_core::{AppConfig, ErrorCode, RToolsError};

    #[tokio::test]
    async fn pdf_text_propagates_the_unavailable_capability() {
        let error = handle_pdf_command(
            PdfCommands::Text {
                input: "input.pdf".into(),
                output: None,
            },
            &AppConfig::default(),
        )
        .unwrap_err();
        assert_unavailable(&error, "pdf.text");
    }

    #[tokio::test]
    async fn pdf_to_image_propagates_the_unavailable_capability() {
        let error = handle_pdf_command(
            PdfCommands::ToImage {
                input: "input.pdf".into(),
                output: "pages".into(),
                format: PdfImageFormatArg::Png,
                dpi: 300,
            },
            &AppConfig::default(),
        )
        .unwrap_err();
        assert_unavailable(&error, "pdf.to_image");
    }

    #[tokio::test]
    async fn available_pdf_processor_error_is_propagated() {
        let error = handle_pdf_command(
            PdfCommands::Merge {
                input: vec!["missing-a.pdf".into(), "missing-b.pdf".into()],
                output: "merged.pdf".into(),
            },
            &AppConfig::default(),
        )
        .unwrap_err();

        assert_eq!(error.code(), ErrorCode::InvalidInput);
        assert!(matches!(error, RToolsError::FileNotFound(path) if path == "missing-a.pdf"));
    }

    fn assert_unavailable(error: &RToolsError, operation: &str) {
        assert_eq!(error.code(), ErrorCode::CapabilityUnavailable);
        assert!(matches!(
            error,
            RToolsError::CapabilityUnavailable { operation_id, .. }
                if operation_id == operation
        ));
    }
}
