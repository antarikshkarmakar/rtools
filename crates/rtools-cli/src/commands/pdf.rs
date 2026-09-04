use crate::commands::CommandResult;
use crate::{PageSelection, PdfCommands};
use rtools_core::{AppConfig, FileInput, Processor, RToolsError, RToolsResult};

#[allow(clippy::too_many_lines)]
pub fn handle_pdf_command(
    command: PdfCommands,
    _config: &AppConfig,
) -> RToolsResult<CommandResult> {
    match command {
        PdfCommands::Merge { input, output } => {
            let processor = rtools_pdf::PdfMergeProcessor;
            let processor_config = rtools_pdf::PdfMergeConfig {
                inputs: input.clone(),
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
        } => {
            let processor = rtools_pdf::PdfCompressProcessor;
            let processor_config = rtools_pdf::PdfCompressConfig {
                level: level.into_pdf(),
                output,
                remove_metadata: false,
            };
            let output = processor.process(FileInput::from_path(input), processor_config)?;
            CommandResult::from_file_outputs("pdf.compress", "Compressed PDF", vec![output])
        }
        PdfCommands::Split {
            input,
            pages,
            output,
        } => {
            let processor = rtools_pdf::PdfSplitProcessor;
            let processor_config = rtools_pdf::PdfSplitConfig {
                range: pages.map_or(rtools_pdf::split::PageRange::All, |PageSelection(range)| {
                    range
                }),
                output_dir: output,
                filename_pattern: "page_{n}.pdf".to_string(),
                as_images: false,
                image_format: Some("png".to_string()),
                image_dpi: 300,
            };
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
