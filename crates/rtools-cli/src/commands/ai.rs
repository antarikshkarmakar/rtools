use crate::AiCommands;
use rtools_core::AppConfig;
use rtools_core::FileInput;
use rtools_core::Processor;
use std::path::PathBuf;

pub async fn handle_ai_command(cmd: AiCommands, _config: &AppConfig) -> anyhow::Result<()> {
    match cmd {
        AiCommands::Organize {
            input,
            output,
            strategy,
        } => {
            let processor = rtools_ai::OrganizeProcessor;
            let organize_config = rtools_ai::organize::OrganizeConfig {
                output_dir: output,
                strategy: match strategy.as_str() {
                    "date" => rtools_ai::organize::OrganizeStrategy::ByDate,
                    "subject" => rtools_ai::organize::OrganizeStrategy::BySubject,
                    "location" => rtools_ai::organize::OrganizeStrategy::ByLocation,
                    _ => rtools_ai::organize::OrganizeStrategy::ByDate,
                },
                by_date: true,
                by_subject: false,
                dry_run: false,
            };

            // Collect all images from input directory
            let inputs = collect_image_inputs(&input)?;
            println!("Found {} images to organize", inputs.len());

            match processor.process(inputs, organize_config) {
                Ok(outputs) => {
                    println!("✓ Organized {} images", outputs.len());
                }
                Err(e) => {
                    eprintln!("✗ Failed to organize images: {}", e);
                }
            }
            Ok(())
        }

        AiCommands::Rename {
            input,
            pattern,
            dry_run,
        } => {
            let processor = rtools_ai::RenameProcessor;
            let rename_config = rtools_ai::rename::RenameConfig {
                pattern,
                output_dir: None,
                start_number: 1,
                use_ai_descriptions: true,
                dry_run,
            };

            let inputs = collect_image_inputs(&input)?;
            println!("Found {} images to rename", inputs.len());

            if dry_run {
                println!("(Dry run mode - no files will be renamed)");
            }

            match processor.process(inputs, rename_config) {
                Ok(outputs) => {
                    for output in &outputs {
                        println!("✓ Renamed to: {}", output.name.as_deref().unwrap_or("unknown"));
                    }
                }
                Err(e) => {
                    eprintln!("✗ Failed to rename images: {}", e);
                }
            }
            Ok(())
        }

        AiCommands::AltText {
            input,
            language,
            output: _,
        } => {
            let processor = rtools_ai::AltTextProcessor;
            let alt_text_config = rtools_ai::alt_text::AltTextConfig {
                language,
                max_length: 125,
                output_format: rtools_ai::alt_text::AltTextOutputFormat::Text,
            };

            for input_path in input {
                let file_input = FileInput::from_path(input_path.clone());
                match processor.process(file_input, alt_text_config.clone()) {
                    Ok(result) => {
                        println!("✓ Alt text for: {}", input_path.display());
                        println!("  Text: {}", result.alt_text);
                    }
                    Err(e) => {
                        eprintln!("✗ Failed to generate alt text: {}", e);
                    }
                }
            }
            Ok(())
        }

        AiCommands::Duplicates {
            input,
            threshold,
            action,
        } => {
            let processor = rtools_ai::DuplicatesProcessor;
            let duplicates_config = rtools_ai::duplicates::DuplicatesConfig {
                threshold,
                algorithm: rtools_ai::duplicates::HashAlgorithm::Perceptual,
                action: match action.as_str() {
                    "move" => rtools_ai::duplicates::DuplicateAction::Move {
                        destination: PathBuf::from("duplicates"),
                    },
                    "delete" => rtools_ai::duplicates::DuplicateAction::Delete,
                    _ => rtools_ai::duplicates::DuplicateAction::Report,
                },
                dry_run: false,
            };

            let inputs = collect_image_inputs(&input)?;
            println!("Found {} images to check for duplicates", inputs.len());

            match processor.process(inputs, duplicates_config) {
                Ok(result) => {
                    println!("✓ Found {} duplicate groups", result.groups.len());
                    println!("  Originals: {}", result.total_originals);
                    println!("  Duplicates: {}", result.total_duplicates);
                    println!("  Time: {}ms", result.processing_time_ms);
                }
                Err(e) => {
                    eprintln!("✗ Failed to find duplicates: {}", e);
                }
            }
            Ok(())
        }
    }
}

fn collect_image_inputs(dir: &PathBuf) -> anyhow::Result<Vec<FileInput>> {
    let mut inputs = Vec::new();
    let valid_extensions = ["jpg", "jpeg", "png", "webp", "heic", "heif", "tiff", "bmp", "gif"];

    for entry in walkdir::WalkDir::new(dir).into_iter().filter_map(|e| e.ok()) {
        if entry.file_type().is_file() {
            if let Some(ext) = entry.path().extension().and_then(|e| e.to_str()) {
                if valid_extensions.contains(&ext.to_lowercase().as_str()) {
                    inputs.push(FileInput::from_path(entry.path().to_path_buf()));
                }
            }
        }
    }

    Ok(inputs)
}