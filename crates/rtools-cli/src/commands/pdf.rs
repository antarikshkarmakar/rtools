use crate::PdfCommands;
use rtools_core::AppConfig;
use rtools_core::FileInput;
use rtools_core::Processor;

#[allow(clippy::too_many_lines)] // Task 7 will split individual PDF command handlers.
pub async fn handle_pdf_command(cmd: PdfCommands, _config: &AppConfig) -> anyhow::Result<()> {
    std::future::ready(()).await;
    match cmd {
        PdfCommands::Merge { input, output } => {
            let processor = rtools_pdf::PdfMergeProcessor;
            let merge_config = rtools_pdf::PdfMergeConfig {
                inputs: input.clone(),
                output,
                add_page_numbers: false,
            };

            let file_inputs: Vec<FileInput> = input
                .iter()
                .map(|p| FileInput::from_path(p.clone()))
                .collect();

            match processor.process(file_inputs, merge_config) {
                Ok(output) => {
                    println!("✓ Merged {} PDFs", input.len());
                    if let Some(path) = output.destination.as_path() {
                        println!("  Output: {}", path.display());
                    }
                }
                Err(e) => {
                    return Err(e.into());
                }
            }
            Ok(())
        }

        PdfCommands::Compress {
            input,
            output,
            level,
        } => {
            let processor = rtools_pdf::PdfCompressProcessor;
            let compress_config = rtools_pdf::PdfCompressConfig {
                level: match level.as_str() {
                    "light" => rtools_pdf::compress::PdfCompressionLevel::Light,
                    "heavy" => rtools_pdf::compress::PdfCompressionLevel::Heavy,
                    _ => rtools_pdf::compress::PdfCompressionLevel::Medium,
                },
                output,
                remove_metadata: false,
            };

            let file_input = FileInput::from_path(input.clone());
            match processor.process(file_input, compress_config) {
                Ok(output) => {
                    println!("✓ Compressed: {}", input.display());
                    if let Some(stats) = &output.stats {
                        println!(
                            "  Size: {} → {} ({:.1}%)",
                            format_size(stats.input_size),
                            format_size(stats.output_size),
                            stats.compression_ratio * 100.0
                        );
                    }
                }
                Err(e) => {
                    return Err(e.into());
                }
            }
            Ok(())
        }

        PdfCommands::Split {
            input,
            pages,
            output,
        } => {
            let processor = rtools_pdf::PdfSplitProcessor;

            // Parse page range from --pages argument
            let page_range = pages.map_or(rtools_pdf::split::PageRange::All, |pages_str| {
                parse_page_range(&pages_str)
            });

            let split_config = rtools_pdf::PdfSplitConfig {
                range: page_range,
                output_dir: output,
                filename_pattern: "page_{n}.pdf".to_string(),
                as_images: false,
                image_format: Some("png".to_string()),
                image_dpi: 300,
            };

            let file_input = FileInput::from_path(input);
            match processor.process(file_input, split_config) {
                Ok(outputs) => {
                    println!("✓ Split into {} pages", outputs.len());
                }
                Err(e) => {
                    return Err(e.into());
                }
            }
            Ok(())
        }

        PdfCommands::Text { .. } => Err(rtools_core::RToolsError::capability_unavailable(
            "pdf.text",
            "PDF text extraction is not implemented in the CLI",
            "Use a verified PDF text extraction provider once one is registered",
        )
        .into()),

        PdfCommands::ToImage { .. } => Err(rtools_core::RToolsError::capability_unavailable(
            "pdf.to_image",
            "No PDF rendering provider is configured",
            "Configure a supported PDF rendering provider",
        )
        .into()),
    }
}

#[allow(clippy::cast_precision_loss)]
const fn bytes_to_f64(bytes: u64) -> f64 {
    // Displaying human-readable sizes tolerates rounding beyond f64 precision.
    bytes as f64
}

fn format_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes_to_f64(bytes) / 1024.0)
    } else {
        format!("{:.1} MB", bytes_to_f64(bytes) / (1024.0 * 1024.0))
    }
}

/// Parse page range string like "1-5,10,15-20" into `PageRange`
fn parse_page_range(s: &str) -> rtools_pdf::split::PageRange {
    let ranges: Vec<rtools_pdf::split::PageRange> = s
        .split(',')
        .filter_map(|part| {
            let part = part.trim();
            if part.contains('-') {
                let bounds: Vec<&str> = part.split('-').collect();
                if bounds.len() == 2 {
                    let start: u32 = bounds[0].parse().ok()?;
                    let end: u32 = bounds[1].parse().ok()?;
                    Some(rtools_pdf::split::PageRange::Range { start, end })
                } else {
                    None
                }
            } else {
                let page: u32 = part.parse().ok()?;
                Some(rtools_pdf::split::PageRange::Single(page))
            }
        })
        .collect();

    if ranges.len() == 1 {
        ranges.into_iter().next().unwrap()
    } else if ranges.is_empty() {
        rtools_pdf::split::PageRange::All
    } else {
        rtools_pdf::split::PageRange::Multiple(ranges)
    }
}

#[cfg(test)]
mod tests {
    use super::handle_pdf_command;
    use crate::PdfCommands;
    use rtools_core::{AppConfig, ErrorCode, RToolsError};

    #[tokio::test]
    async fn pdf_text_does_not_print_success_without_extracting_text() {
        let error = handle_pdf_command(
            PdfCommands::Text {
                input: "input.pdf".into(),
                output: None,
            },
            &AppConfig::default(),
        )
        .await
        .unwrap_err();
        assert_unavailable(&error, "pdf.text");
    }

    #[tokio::test]
    async fn pdf_to_image_does_not_print_success_without_rendering_pages() {
        let error = handle_pdf_command(
            PdfCommands::ToImage {
                input: "input.pdf".into(),
                output: "pages".into(),
                format: "png".to_string(),
                dpi: 300,
            },
            &AppConfig::default(),
        )
        .await
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
        .await
        .unwrap_err();

        assert!(error.downcast_ref::<RToolsError>().is_some());
    }

    fn assert_unavailable(error: &anyhow::Error, operation: &str) {
        let error = error.downcast_ref::<RToolsError>().unwrap();
        assert_eq!(error.code(), ErrorCode::CapabilityUnavailable);
        assert!(matches!(
            error,
            RToolsError::CapabilityUnavailable { operation_id, .. }
                if operation_id == operation
        ));
    }
}
