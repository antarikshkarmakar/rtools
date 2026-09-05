use crate::destination::{
    destination_or_case_alias_exists, insert_unique_destination, move_no_replace,
};
use rtools_core::error::{RToolsError, RToolsResult};
use rtools_core::{FileInput, FileOutput, Processor};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::ffi::OsStr;
use std::path::{Component, Path, PathBuf};

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
        if inputs.is_empty() {
            return Err(RToolsError::invalid_input(
                "Rename requires at least one input file",
            ));
        }
        let input_paths =
            inputs
                .iter()
                .map(|input| {
                    input.source.as_path().cloned().ok_or_else(|| {
                        RToolsError::invalid_input("Rename requires file path inputs")
                    })
                })
                .collect::<RToolsResult<HashSet<_>>>()?;
        let mut planned_destinations = HashSet::new();
        let mut plans = Vec::with_capacity(inputs.len());

        for (idx, input) in inputs.iter().enumerate() {
            let path = input
                .source
                .as_path()
                .ok_or_else(|| RToolsError::invalid_input("Rename requires file path inputs"))?;

            let offset = u32::try_from(idx).map_err(|_| {
                RToolsError::invalid_input("Rename sequence index exceeds the u32 range")
            })?;
            let index = config.start_number.checked_add(offset).ok_or_else(|| {
                RToolsError::invalid_input("Rename sequence exceeds the u32 range")
            })?;
            let new_name = render_filename(&config.pattern, path, index)?;
            let output_dir = config
                .output_dir
                .as_deref()
                .unwrap_or_else(|| path.parent().unwrap_or_else(|| std::path::Path::new(".")));
            let new_path = output_dir.join(&new_name);

            if !insert_unique_destination(&mut planned_destinations, &new_path)?
                || (new_path != *path
                    && (destination_or_case_alias_exists(&new_path)?
                        || input_paths.contains(&new_path)))
            {
                return Err(RToolsError::output_exists(new_path.display().to_string()));
            }
            plans.push((path.clone(), new_path));
        }

        let mut outputs = Vec::with_capacity(plans.len());
        for (path, new_path) in plans {
            if !config.dry_run && new_path != path {
                move_no_replace(&path, &new_path)?;
            }

            outputs.push(FileOutput {
                destination: rtools_core::output::OutputDestination::File(new_path.clone()),
                name: new_path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string()),
                mime_type: None,
                stats: None,
                warnings: Vec::new(),
            });
        }

        Ok(outputs)
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
    let metadata = std::fs::metadata(path)?;
    let modified = metadata.modified()?;
    let datetime: chrono::DateTime<chrono::Local> = modified.into();

    let stem = path.file_stem().and_then(OsStr::to_str).ok_or_else(|| {
        RToolsError::path_policy_violation(format!(
            "rename destination filename stem is not Unicode: {}",
            path.display()
        ))
    })?;
    let ext = match path.extension() {
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
    let mut components = Path::new(filename).components();
    if !matches!(components.next(), Some(Component::Normal(_)))
        || components.next().is_some()
        || filename.is_empty()
        || matches!(filename, "." | "..")
        || filename.ends_with(['.', ' '])
        || filename
            .chars()
            .any(|character| character.is_control() || "<>:\"/\\|?*".contains(character))
    {
        return Err(RToolsError::invalid_input(
            "Rename result must be one portable filename",
        ));
    }
    let stem = filename
        .split('.')
        .next()
        .unwrap_or_default()
        .trim_end_matches([' ', '.']);
    let reserved = [
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
        "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    ];
    if reserved
        .iter()
        .any(|candidate| stem.eq_ignore_ascii_case(candidate))
    {
        return Err(RToolsError::invalid_input(
            "Rename result uses a reserved portable filename",
        ));
    }
    Ok(())
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
