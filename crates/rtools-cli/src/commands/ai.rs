use crate::commands::CommandResult;
use crate::{AiCommands, DuplicateMode, OrganizeMode};
use rtools_core::{AppConfig, FileInput, FileOutput, Processor, RToolsError, RToolsResult};
use serde::Serialize;
use std::path::PathBuf;

#[allow(clippy::too_many_lines)]
pub fn handle_ai_command(
    command: AiCommands,
    _config: &AppConfig,
    global_dry_run: bool,
) -> RToolsResult<CommandResult> {
    match command {
        AiCommands::Organize {
            input,
            output,
            strategy,
        } => {
            let source_paths = collect_image_paths(&input);
            require_nonempty_inputs(&source_paths)?;
            let processor = rtools_ai::OrganizeProcessor;
            let processor_config = rtools_ai::organize::OrganizeConfig {
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
                dry_run: global_dry_run,
            };
            let inputs = source_paths
                .iter()
                .cloned()
                .map(FileInput::from_path)
                .collect();
            let outputs = processor.process(inputs, processor_config)?;
            if global_dry_run {
                planned_result("ai.organize.date", source_paths, outputs)
            } else {
                CommandResult::from_file_outputs(
                    "ai.organize.date",
                    format!("Organized {} image(s)", outputs.len()),
                    outputs,
                )
            }
        }
        AiCommands::Rename {
            input,
            pattern,
            dry_run,
        } => {
            let source_paths = collect_image_paths(&input);
            require_nonempty_inputs(&source_paths)?;
            let dry_run = global_dry_run || dry_run;
            let processor = rtools_ai::RenameProcessor;
            let processor_config = rtools_ai::rename::RenameConfig {
                pattern,
                output_dir: None,
                start_number: 1,
                use_ai_descriptions: false,
                dry_run,
            };
            let inputs = source_paths
                .iter()
                .cloned()
                .map(FileInput::from_path)
                .collect();
            let outputs = processor.process(inputs, processor_config)?;
            if dry_run {
                planned_result("ai.rename.deterministic", source_paths, outputs)
            } else {
                CommandResult::from_file_outputs(
                    "ai.rename.deterministic",
                    format!("Renamed {} image(s)", outputs.len()),
                    outputs,
                )
            }
        }
        AiCommands::AltText {
            input,
            language,
            output: _,
        } => {
            let processor = rtools_ai::AltTextProcessor;
            let processor_config = rtools_ai::alt_text::AltTextConfig {
                language,
                max_length: 125,
                output_format: rtools_ai::alt_text::AltTextOutputFormat::Text,
            };
            let results = input
                .into_iter()
                .map(|path| processor.process(FileInput::from_path(path), processor_config.clone()))
                .collect::<RToolsResult<Vec<_>>>()?;
            CommandResult::from_serializable("ai.alt_text", results, Vec::new())
        }
        AiCommands::Duplicates {
            input,
            threshold,
            action,
        } => {
            let source_paths = collect_image_paths(&input);
            require_nonempty_inputs(&source_paths)?;
            let processor = rtools_ai::DuplicatesProcessor;
            let processor_config = rtools_ai::duplicates::DuplicatesConfig {
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
                dry_run: global_dry_run,
            };
            let inputs = source_paths.into_iter().map(FileInput::from_path).collect();
            let result = processor.process(inputs, processor_config)?;
            CommandResult::from_serializable("ai.duplicates.report", result, Vec::new())
        }
    }
}

#[derive(Serialize)]
struct DryRunResult {
    message: String,
    planned: Vec<PlannedOutput>,
}

#[derive(Serialize)]
struct PlannedOutput {
    source: PathBuf,
    destination: PathBuf,
}

fn planned_result(
    operation_id: &'static str,
    source_paths: Vec<PathBuf>,
    outputs: Vec<FileOutput>,
) -> RToolsResult<CommandResult> {
    if source_paths.len() != outputs.len() {
        return Err(RToolsError::Internal(
            "AI processor returned a different number of outputs than inputs".to_string(),
        ));
    }
    let warnings = outputs
        .iter()
        .flat_map(|output| output.warnings.iter().cloned())
        .collect();
    let planned = source_paths
        .into_iter()
        .zip(outputs)
        .map(|(source, output)| {
            let destination = output.destination.as_path().ok_or_else(|| {
                RToolsError::Internal("AI dry-run did not return a file destination".to_string())
            })?;
            Ok(PlannedOutput {
                source,
                destination: destination.clone(),
            })
        })
        .collect::<RToolsResult<Vec<_>>>()?;
    CommandResult::from_serializable(
        operation_id,
        DryRunResult {
            message: format!("Planned {} file operation(s)", planned.len()),
            planned,
        },
        warnings,
    )
}

fn require_nonempty_inputs(inputs: &[PathBuf]) -> RToolsResult<()> {
    if inputs.is_empty() {
        Err(RToolsError::invalid_input(
            "No supported image files were found",
        ))
    } else {
        Ok(())
    }
}

fn collect_image_paths(directory: &PathBuf) -> Vec<PathBuf> {
    let valid_extensions = [
        "jpg", "jpeg", "png", "webp", "heic", "heif", "tiff", "bmp", "gif",
    ];
    let mut paths = walkdir::WalkDir::new(directory)
        .into_iter()
        .filter_map(std::result::Result::ok)
        .filter(|entry| entry.file_type().is_file())
        .filter_map(|entry| {
            entry
                .path()
                .extension()
                .and_then(|extension| extension.to_str())
                .filter(|extension| valid_extensions.contains(&extension.to_lowercase().as_str()))
                .map(|_| entry.path().to_path_buf())
        })
        .collect::<Vec<_>>();
    paths.sort();
    paths
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
            false,
        )
        .unwrap_err();

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
            false,
        )
        .unwrap_err();

        assert_eq!(error.code(), ErrorCode::InvalidInput);
    }
}
