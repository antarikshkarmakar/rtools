use crate::destination::{
    destination_or_case_alias_exists, insert_unique_destination, move_no_replace,
    portable_destination_key,
};
use rtools_core::error::{RToolsError, RToolsResult};
use rtools_core::{FileInput, FileOutput, Processor};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static RENAME_STAGE_SEQUENCE: AtomicU64 = AtomicU64::new(1);

/// AI rename configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenameConfig {
    /// Filename pattern
    pub pattern: String,
    /// Output directory (None = rename in place)
    pub output_dir: Option<PathBuf>,
    /// Starting number for sequence
    pub start_number: u32,
    /// Use AI-generated descriptions
    pub use_ai_descriptions: bool,
    /// Dry run mode
    pub dry_run: bool,
}

impl Default for RenameConfig {
    fn default() -> Self {
        Self {
            pattern: "{date}_{name}_{index}".to_string(),
            output_dir: None,
            start_number: 1,
            use_ai_descriptions: false,
            dry_run: false,
        }
    }
}

/// AI rename processor
pub struct RenameProcessor;

impl Processor for RenameProcessor {
    type Input = Vec<FileInput>;
    type Output = Vec<FileOutput>;
    type Config = RenameConfig;
    type Error = RToolsError;

    fn process_validated(
        &self,
        inputs: Vec<FileInput>,
        config: RenameConfig,
    ) -> RToolsResult<Vec<FileOutput>> {
        rename_with_mover(&inputs, &config, &mut FilesystemRenameMover)
    }

    fn validate_config(&self, config: &RenameConfig) -> RToolsResult<()> {
        if config.pattern.is_empty() {
            return Err(RToolsError::invalid_input("Pattern cannot be empty"));
        }
        if config.use_ai_descriptions || config.pattern.contains("{subject}") {
            return Err(RToolsError::capability_unavailable(
                "ai.rename.ai",
                "AI-assisted rename descriptions are not implemented",
                "Disable AI descriptions and use deterministic filename tokens",
            ));
        }
        validate_deterministic_pattern(&config.pattern)?;
        Ok(())
    }

    fn name(&self) -> &'static str {
        "RenameProcessor"
    }
}

trait RenameMover {
    fn move_no_replace(&mut self, source: &Path, destination: &Path) -> RToolsResult<()>;
}

struct FilesystemRenameMover;

impl RenameMover for FilesystemRenameMover {
    fn move_no_replace(&mut self, source: &Path, destination: &Path) -> RToolsResult<()> {
        move_no_replace(source, destination).map(|_| ())
    }
}

#[derive(Debug)]
struct RenamePlan {
    source: PathBuf,
    destination: PathBuf,
    stage: Option<PathBuf>,
}

fn rename_with_mover(
    inputs: &[FileInput],
    config: &RenameConfig,
    mover: &mut impl RenameMover,
) -> RToolsResult<Vec<FileOutput>> {
    if inputs.is_empty() {
        return Err(RToolsError::invalid_input(
            "Rename requires at least one input file",
        ));
    }
    let input_paths = inputs
        .iter()
        .map(|input| {
            input
                .source
                .as_path()
                .cloned()
                .ok_or_else(|| RToolsError::invalid_input("Rename requires file path inputs"))
        })
        .collect::<RToolsResult<Vec<_>>>()?;
    let mut input_identities = HashSet::with_capacity(input_paths.len());
    for path in &input_paths {
        let canonical = std::fs::canonicalize(path)?;
        if !std::fs::metadata(&canonical)?.is_file() {
            return Err(RToolsError::invalid_input(format!(
                "Rename source is not a regular file: {}",
                path.display()
            )));
        }
        if !input_identities.insert(portable_destination_key(&canonical)?) {
            return Err(RToolsError::invalid_input(
                "Rename input list contains duplicate portable source identities",
            ));
        }
    }
    let mut planned_destinations = HashSet::new();
    let mut plans = Vec::with_capacity(inputs.len());

    for (idx, path) in input_paths.iter().enumerate() {
        let offset = u32::try_from(idx).map_err(|_| {
            RToolsError::invalid_input("Rename sequence index exceeds the u32 range")
        })?;
        let index = config
            .start_number
            .checked_add(offset)
            .ok_or_else(|| RToolsError::invalid_input("Rename sequence exceeds the u32 range"))?;
        let new_name =
            render_filename_for_input(&config.pattern, path, inputs[idx].name.as_deref(), index)?;
        let output_dir = config
            .output_dir
            .as_deref()
            .unwrap_or_else(|| path.parent().unwrap_or_else(|| std::path::Path::new(".")));
        if !config.dry_run && !std::fs::metadata(output_dir).is_ok_and(|metadata| metadata.is_dir())
        {
            return Err(RToolsError::OutputDirectoryNotFound(
                output_dir.display().to_string(),
            ));
        }
        let new_path = output_dir.join(&new_name);

        if !insert_unique_destination(&mut planned_destinations, &new_path)?
            || (new_path != *path
                && (destination_or_case_alias_exists(&new_path)?
                    || input_identities.contains(&portable_destination_key(&new_path)?)))
        {
            return Err(RToolsError::output_exists(new_path.display().to_string()));
        }
        plans.push(RenamePlan {
            source: path.clone(),
            destination: new_path,
            stage: None,
        });
    }

    if !config.dry_run {
        execute_transaction(&mut plans, mover)?;
    }

    Ok(plans
        .into_iter()
        .map(|plan| FileOutput {
            destination: rtools_core::output::OutputDestination::File(plan.destination.clone()),
            name: plan
                .destination
                .file_name()
                .map(|n| n.to_string_lossy().to_string()),
            mime_type: None,
            stats: None,
            warnings: Vec::new(),
        })
        .collect())
}

fn execute_transaction(plans: &mut [RenamePlan], mover: &mut impl RenameMover) -> RToolsResult<()> {
    let stages = plans
        .iter()
        .enumerate()
        .map(|(index, plan)| {
            if plan.source == plan.destination {
                Ok(None)
            } else {
                private_stage_path(&plan.source, index).map(Some)
            }
        })
        .collect::<RToolsResult<Vec<_>>>()?;

    for index in 0..plans.len() {
        let Some(stage) = stages[index].as_ref() else {
            continue;
        };
        if let Err(error) = mover.move_no_replace(&plans[index].source, stage) {
            rollback_staged(plans, index, mover)?;
            return Err(error);
        }
        plans[index].stage = Some(stage.clone());
    }

    for index in 0..plans.len() {
        let Some(stage) = plans[index].stage.as_ref() else {
            continue;
        };
        if let Err(error) = mover.move_no_replace(stage, &plans[index].destination) {
            rollback_committed_and_staged(plans, index, mover)?;
            return Err(error);
        }
        plans[index].stage = None;
    }
    Ok(())
}

fn private_stage_path(source: &Path, index: usize) -> RToolsResult<PathBuf> {
    let parent = source
        .parent()
        .ok_or_else(|| RToolsError::invalid_input("Rename source has no parent directory"))?;
    for _ in 0..64 {
        let sequence = RENAME_STAGE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let candidate = parent.join(format!(
            ".rtools-rename-stage-{}-{sequence}-{index}",
            std::process::id()
        ));
        match std::fs::symlink_metadata(&candidate) {
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(candidate),
            Err(error) => return Err(error.into()),
        }
    }
    Err(RToolsError::Internal(
        "Unable to allocate a private rename staging path".to_string(),
    ))
}

fn rollback_staged(
    plans: &[RenamePlan],
    before: usize,
    mover: &mut impl RenameMover,
) -> RToolsResult<()> {
    let mut failures = Vec::new();
    for plan in plans[..before].iter().rev() {
        if let Some(stage) = &plan.stage {
            if let Err(error) = mover.move_no_replace(stage, &plan.source) {
                failures.push(format!("{}: {error}", plan.source.display()));
            }
        }
    }
    rollback_result(&failures)
}

fn rollback_committed_and_staged(
    plans: &[RenamePlan],
    failed_index: usize,
    mover: &mut impl RenameMover,
) -> RToolsResult<()> {
    let mut failures = Vec::new();
    for plan in plans[..failed_index].iter().rev() {
        if plan.stage.is_none() && plan.source != plan.destination {
            if let Err(error) = mover.move_no_replace(&plan.destination, &plan.source) {
                failures.push(format!("{}: {error}", plan.source.display()));
            }
        }
    }
    for plan in plans[failed_index..].iter().rev() {
        if let Some(stage) = &plan.stage {
            if let Err(error) = mover.move_no_replace(stage, &plan.source) {
                failures.push(format!("{}: {error}", plan.source.display()));
            }
        }
    }
    rollback_result(&failures)
}

fn rollback_result(failures: &[String]) -> RToolsResult<()> {
    if failures.is_empty() {
        Ok(())
    } else {
        Err(RToolsError::RollbackFailed(failures.join("; ")))
    }
}

/// Validate that a rename pattern contains only supported deterministic tokens.
///
/// # Errors
///
/// Returns `INVALID_INPUT` for unknown, nested, or unbalanced tokens.
pub fn validate_deterministic_pattern(pattern: &str) -> RToolsResult<()> {
    if pattern.is_empty() {
        return Err(RToolsError::invalid_input("Pattern cannot be empty"));
    }
    let supported = ["date", "time", "datetime", "index", "name", "ext"];
    let mut characters = pattern.char_indices().peekable();
    while let Some((_, character)) = characters.next() {
        match character {
            '}' => {
                return Err(RToolsError::invalid_input(
                    "Filename pattern contains an unmatched closing brace",
                ));
            }
            '{' => {
                let token_start = characters.peek().map_or(pattern.len(), |(index, _)| *index);
                let mut token_end = None;
                for (index, token_character) in characters.by_ref() {
                    match token_character {
                        '}' => {
                            token_end = Some(index);
                            break;
                        }
                        '{' => {
                            return Err(RToolsError::invalid_input(
                                "Filename pattern contains a nested opening brace",
                            ));
                        }
                        _ => {}
                    }
                }
                let Some(token_end) = token_end else {
                    return Err(RToolsError::invalid_input(
                        "Filename pattern contains an unterminated token",
                    ));
                };
                let token = &pattern[token_start..token_end];
                if !supported.contains(&token) {
                    return Err(RToolsError::invalid_input(format!(
                        "Unsupported filename pattern token: {{{token}}}"
                    )));
                }
            }
            _ => {}
        }
    }
    Ok(())
}

/// Render and validate the final portable filename produced by a rename pattern.
///
/// # Errors
///
/// Returns an error when file metadata is unavailable or the rendered name is
/// not a portable filename.
pub fn render_filename(pattern: &str, path: &Path, index: u32) -> RToolsResult<String> {
    render_filename_for_input(pattern, path, None, index)
}

/// Render a filename using separate storage and client-visible source names.
///
/// Adapters that stage uploads under private server-generated paths use this
/// helper to keep the client name as inert filename metadata. The storage path
/// remains authoritative for filesystem access and file timestamps.
///
/// # Errors
///
/// Returns an error when file metadata is unavailable or the rendered name is
/// not one portable filename component.
pub fn render_filename_with_source_name(
    pattern: &str,
    path: &Path,
    source_name: &str,
    index: u32,
) -> RToolsResult<String> {
    render_filename_for_input(pattern, path, Some(source_name), index)
}

fn render_filename_for_input(
    pattern: &str,
    path: &Path,
    source_name: Option<&str>,
    index: u32,
) -> RToolsResult<String> {
    let metadata = std::fs::metadata(path)?;
    let modified = metadata.modified()?;
    let datetime: chrono::DateTime<chrono::Local> = modified.into();

    let name_path = source_name.map_or(path, Path::new);
    let stem = name_path
        .file_stem()
        .and_then(OsStr::to_str)
        .ok_or_else(|| {
            RToolsError::path_policy_violation(format!(
                "rename destination filename stem is not Unicode: {}",
                path.display()
            ))
        })?;
    let ext = match name_path.extension() {
        Some(extension) => extension.to_str().ok_or_else(|| {
            RToolsError::path_policy_violation(format!(
                "rename destination filename extension is not Unicode: {}",
                path.display()
            ))
        })?,
        None => "",
    };

    let token = |name| ["{", name, "}"].concat();
    let (date, time, datetime_token, index_token, name_token, extension_token) = (
        token("date"),
        token("time"),
        token("datetime"),
        token("index"),
        token("name"),
        token("ext"),
    );
    let filename = pattern
        .replace(&date, &datetime.format("%Y%m%d").to_string())
        .replace(&time, &datetime.format("%H%M%S").to_string())
        .replace(
            &datetime_token,
            &datetime.format("%Y%m%d_%H%M%S").to_string(),
        )
        .replace(&index_token, &index.to_string())
        .replace(&name_token, stem)
        .replace(&extension_token, ext);

    // Only append extension if the pattern doesn't already include {ext}
    // (which would have been replaced with the actual extension)
    let rendered = if pattern.contains(&extension_token) {
        filename
    } else {
        format!("{filename}.{ext}")
    };
    validate_portable_filename(&rendered)?;
    Ok(rendered)
}

/// Validate a fully rendered rename result as one portable filename.
///
/// # Errors
///
/// Returns `INVALID_INPUT` for paths, reserved device names, control
/// characters, or other non-portable filename syntax.
pub fn validate_portable_filename(filename: &str) -> RToolsResult<()> {
    rtools_core::validate_portable_filename_component(filename)
}

/// Validate a batch of rendered filenames and reject portable aliases.
///
/// # Errors
///
/// Returns `INVALID_INPUT` if a filename is invalid or if two names collide
/// after portable Unicode case folding.
pub fn validate_unique_portable_filenames(filenames: &[String]) -> RToolsResult<()> {
    let mut destinations = HashSet::with_capacity(filenames.len());
    for filename in filenames {
        validate_portable_filename(filename)?;
        if !insert_unique_destination(&mut destinations, Path::new(filename))? {
            return Err(RToolsError::invalid_input(
                "Rename pattern produces duplicate output filenames",
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod transaction_tests {
    use super::*;
    use rtools_core::ErrorCode;

    struct FailOnMove {
        call: usize,
        fail_on: Vec<usize>,
    }

    impl RenameMover for FailOnMove {
        fn move_no_replace(&mut self, source: &Path, destination: &Path) -> RToolsResult<()> {
            self.call += 1;
            if self.fail_on.contains(&self.call) {
                return Err(std::io::Error::other("injected rename failure").into());
            }
            crate::destination::move_no_replace(source, destination).map(|_| ())
        }
    }

    fn assert_restored_after_failure(fail_on: usize) {
        let temp = tempfile::tempdir().unwrap();
        let first = temp.path().join("first.jpg");
        let second = temp.path().join("second.jpg");
        std::fs::write(&first, b"first").unwrap();
        std::fs::write(&second, b"second").unwrap();
        let mut mover = FailOnMove {
            call: 0,
            fail_on: vec![fail_on],
        };

        let inputs = vec![
            FileInput::from_path(first.clone()),
            FileInput::from_path(second.clone()),
        ];
        let error = rename_with_mover(
            &inputs,
            &RenameConfig {
                pattern: "renamed_{index}".to_string(),
                output_dir: None,
                start_number: 1,
                use_ai_descriptions: false,
                dry_run: false,
            },
            &mut mover,
        )
        .unwrap_err();

        assert_eq!(error.code(), ErrorCode::ProcessingFailed);
        assert_eq!(std::fs::read(first).unwrap(), b"first");
        assert_eq!(std::fs::read(second).unwrap(), b"second");
        let names = std::fs::read_dir(temp.path())
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        assert_eq!(names.len(), 2, "partial or temporary artifacts: {names:?}");
    }

    #[test]
    fn staging_failure_rolls_back_all_prior_stages() {
        assert_restored_after_failure(2);
    }

    #[test]
    fn later_commit_failure_rolls_back_committed_and_staged_files() {
        assert_restored_after_failure(4);
    }

    #[test]
    fn genuine_restoration_failure_is_reported_explicitly() {
        let temp = tempfile::tempdir().unwrap();
        let first = temp.path().join("first.jpg");
        let second = temp.path().join("second.jpg");
        std::fs::write(&first, b"first").unwrap();
        std::fs::write(&second, b"second").unwrap();
        let mut mover = FailOnMove {
            call: 0,
            fail_on: vec![2, 3],
        };

        let inputs = vec![FileInput::from_path(first), FileInput::from_path(second)];
        let error = rename_with_mover(
            &inputs,
            &RenameConfig {
                pattern: "renamed_{index}".to_string(),
                output_dir: None,
                start_number: 1,
                use_ai_descriptions: false,
                dry_run: false,
            },
            &mut mover,
        )
        .unwrap_err();

        assert_eq!(error.code(), ErrorCode::RollbackFailed);
    }
}
