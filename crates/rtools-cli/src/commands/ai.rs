use crate::{AiCommands, DuplicateMode, OrganizeMode};
use rtools_core::AppConfig;
use rtools_core::FileInput;
use rtools_core::Processor;
use std::path::PathBuf;

#[allow(clippy::too_many_lines)] // Task 7 will split individual AI command handlers.
pub async fn handle_ai_command(cmd: AiCommands, _config: &AppConfig) -> anyhow::Result<()> {
    std::future::ready(()).await;
    match cmd {
        AiCommands::Organize {
            input,
            output,
            strategy,
        } => {
            let processor = rtools_ai::OrganizeProcessor;
            let organize_config = rtools_ai::organize::OrganizeConfig {
                output_dir: output,
                strategy: match strategy {
                    OrganizeMode::Date => rtools_ai::organize::OrganizeStrategy::ByDate,
                    OrganizeMode::Subject => rtools_ai::organize::OrganizeStrategy::BySubject,
                    OrganizeMode::Location => rtools_ai::organize::OrganizeStrategy::ByLocation,
                    OrganizeMode::Camera => rtools_ai::organize::OrganizeStrategy::ByCamera,
                    OrganizeMode::Custom => rtools_ai::organize::OrganizeStrategy::Custom,
                },
                by_date: true,
                by_subject: false,
                dry_run: false,
            };

            // Collect all images from input directory
            let inputs = collect_image_inputs(&input);
            require_nonempty_inputs(&inputs)?;
            println!("Found {} images to organize", inputs.len());

            match processor.process(inputs, organize_config) {
                Ok(outputs) => {
                    println!("✓ Organized {} images", outputs.len());
                }
                Err(e) => {
                    return Err(e.into());
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
                use_ai_descriptions: false,
                dry_run,
            };

            let inputs = collect_image_inputs(&input);
            require_nonempty_inputs(&inputs)?;
            println!("Found {} images to rename", inputs.len());

            if dry_run {
                println!("(Dry run mode - no files will be renamed)");
            }

            match processor.process(inputs, rename_config) {
                Ok(outputs) => {
                    for output in &outputs {
                        println!(
                            "✓ Renamed to: {}",
                            output.name.as_deref().unwrap_or("unknown")
                        );
                    }
                }
                Err(e) => {
                    return Err(e.into());
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
                        return Err(e.into());
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
                action: match action {
                    DuplicateMode::Report => rtools_ai::duplicates::DuplicateAction::Report,
                    DuplicateMode::Move => rtools_ai::duplicates::DuplicateAction::Move {
                        destination: PathBuf::from("duplicates"),
                    },
                    DuplicateMode::Delete => rtools_ai::duplicates::DuplicateAction::Delete,
                    DuplicateMode::Symlink => rtools_ai::duplicates::DuplicateAction::Symlink,
                },
                dry_run: false,
            };

            let inputs = collect_image_inputs(&input);
            require_nonempty_inputs(&inputs)?;
            println!("Found {} images to check for duplicates", inputs.len());

            match processor.process(inputs, duplicates_config) {
                Ok(result) => {
                    println!("✓ Found {} duplicate groups", result.groups.len());
                    println!("  Originals: {}", result.total_originals);
                    println!("  Duplicates: {}", result.total_duplicates);
                    println!("  Time: {}ms", result.processing_time_ms);
                }
                Err(e) => {
                    return Err(e.into());
                }
            }
            Ok(())
        }
    }
}

fn require_nonempty_inputs(inputs: &[FileInput]) -> rtools_core::RToolsResult<()> {
    if inputs.is_empty() {
        Err(rtools_core::RToolsError::invalid_input(
            "No supported image files were found",
        ))
    } else {
        Ok(())
    }
}

fn collect_image_inputs(dir: &PathBuf) -> Vec<FileInput> {
    let mut inputs = Vec::new();
    let valid_extensions = [
        "jpg", "jpeg", "png", "webp", "heic", "heif", "tiff", "bmp", "gif",
    ];

    for entry in walkdir::WalkDir::new(dir)
        .into_iter()
        .filter_map(std::result::Result::ok)
    {
        if entry.file_type().is_file() {
            if let Some(ext) = entry.path().extension().and_then(|e| e.to_str()) {
                if valid_extensions.contains(&ext.to_lowercase().as_str()) {
                    inputs.push(FileInput::from_path(entry.path().to_path_buf()));
                }
            }
        }
    }

    inputs
}

#[cfg(test)]
mod tests {
    use super::handle_ai_command;
    use crate::{AiCommands, DuplicateMode};
    use rtools_core::{AppConfig, ErrorCode, RToolsError};

    #[tokio::test]
    async fn alt_text_processor_error_is_propagated() {
        let error = handle_ai_command(
            AiCommands::AltText {
                input: vec!["private.jpg".into()],
                language: "en".to_string(),
                output: None,
            },
            &AppConfig::default(),
        )
        .await
        .unwrap_err();
        let error = error.downcast_ref::<RToolsError>().unwrap();

        assert_eq!(error.code(), ErrorCode::CapabilityUnavailable);
        assert!(matches!(
            error,
            RToolsError::CapabilityUnavailable { operation_id, .. }
                if operation_id == "ai.alt_text"
        ));
    }

    #[tokio::test]
    async fn duplicate_scan_rejects_an_empty_input_directory() {
        let temp = tempfile::tempdir().unwrap();
        let error = handle_ai_command(
            AiCommands::Duplicates {
                input: temp.path().to_path_buf(),
                threshold: 0.9,
                action: DuplicateMode::Report,
            },
            &AppConfig::default(),
        )
        .await
        .unwrap_err();
        let error = error.downcast_ref::<RToolsError>().unwrap();

        assert_eq!(error.code(), ErrorCode::InvalidInput);
    }
}
