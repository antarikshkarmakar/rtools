use crate::PdfCommands;
use rtools_core::AppConfig;
use rtools_core::FileInput;
use rtools_core::Processor;

pub async fn handle_pdf_command(cmd: PdfCommands, config: &AppConfig) -> anyhow::Result<()> {
    match cmd {
        PdfCommands::Merge { input, output } => {
            let processor = rtools_pdf::PdfMergeProcessor;
            let merge_config = rtools_pdf::PdfMergeConfig {
                inputs: input.clone(),
                output,
                add_page_numbers: false,
            };

            let file_inputs: Vec<FileInput> = input.iter()
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
                    eprintln!("✗ Failed to merge PDFs: {}", e);
                }
            }
            Ok(())
        }

        PdfCommands::Compress { input, output, level } => {
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
                        println!("  Size: {} → {} ({:.1}%)", 
                            format_size(stats.input_size),
                            format_size(stats.output_size),
                            stats.compression_ratio * 100.0
                        );
                    }
                }
                Err(e) => {
                    eprintln!("✗ Failed to compress PDF: {}", e);
                }
            }
            Ok(())
        }

        PdfCommands::Split { input, pages, output } => {
            let processor = rtools_pdf::PdfSplitProcessor;

            // Parse page range from --pages argument
            let page_range = if let Some(pages_str) = pages {
                parse_page_range(&pages_str)
            } else {
                rtools_pdf::split::PageRange::All
            };

            let split_config = rtools_pdf::PdfSplitConfig {
                range: page_range,
                output_dir: output,
                filename_pattern: "page_{n}.pdf".to_string(),
                as_images: false,
                image_format: Some("png".to_string()),
                image_dpi: 300,
            };

            let file_input = FileInput::from_path(input.clone());
            match processor.process(file_input, split_config) {
                Ok(outputs) => {
                    println!("✓ Split into {} pages", outputs.len());
                }
                Err(e) => {
                    eprintln!("✗ Failed to split PDF: {}", e);
                }
            }
            Ok(())
        }

        PdfCommands::Text { input, output } => {
            // TODO: Implement text extraction
            println!("✓ Text extraction not yet implemented");
            Ok(())
        }

        PdfCommands::ToImage { input, output, format, dpi } => {
            // TODO: Implement PDF to image conversion
            println!("✓ PDF to image conversion not yet implemented");
            Ok(())
        }
    }
}

fn format_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

/// Parse page range string like "1-5,10,15-20" into PageRange
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