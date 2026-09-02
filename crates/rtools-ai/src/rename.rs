use rtools_core::error::{RToolsError, RToolsResult};
use rtools_core::{FileInput, FileOutput, Processor};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

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
            pattern: "{date}_{subject}_{index}".to_string(),
            output_dir: None,
            start_number: 1,
            use_ai_descriptions: true,
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

    fn process(
        &self,
        inputs: Vec<FileInput>,
        config: RenameConfig,
    ) -> RToolsResult<Vec<FileOutput>> {
        let mut outputs = Vec::new();

        for (idx, input) in inputs.iter().enumerate() {
            let path = input
                .source
                .as_path()
                .ok_or_else(|| RToolsError::invalid_input("Rename requires file path inputs"))?;

            let index = u32::try_from(idx)
                .unwrap_or(u32::MAX)
                .saturating_add(config.start_number);
            let new_name = generate_filename(&config.pattern, path, index)?;
            let output_dir = config
                .output_dir
                .as_deref()
                .unwrap_or_else(|| path.parent().unwrap_or_else(|| std::path::Path::new(".")));
            let mut new_path = output_dir.join(&new_name);

            // Collision detection: append numeric suffix if file exists
            if new_path.exists() && new_path != *path {
                let stem = new_path.file_stem().unwrap_or_default().to_string_lossy();
                let ext = new_path.extension().unwrap_or_default().to_string_lossy();
                for i in 1..1000 {
                    let candidate = output_dir.join(format!("{stem}_{i}.{ext}"));
                    if !candidate.exists() || candidate == *path {
                        new_path = candidate;
                        break;
                    }
                }
            }

            if !config.dry_run && new_path != *path {
                std::fs::rename(path, &new_path)?;
            }

            outputs.push(FileOutput {
                destination: rtools_core::output::OutputDestination::File(new_path.clone()),
                name: new_path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string()),
                mime_type: None,
                stats: None,
            });
        }

        Ok(outputs)
    }

    fn validate_config(&self, config: &RenameConfig) -> RToolsResult<()> {
        if config.pattern.is_empty() {
            return Err(RToolsError::invalid_input("Pattern cannot be empty"));
        }
        Ok(())
    }

    fn name(&self) -> &'static str {
        "RenameProcessor"
    }
}

/// Generate filename from pattern, avoiding double extensions
fn generate_filename(pattern: &str, path: &PathBuf, index: u32) -> RToolsResult<String> {
    let metadata = std::fs::metadata(path)?;
    let modified = metadata.modified()?;
    let datetime: chrono::DateTime<chrono::Local> = modified.into();

    let stem = path.file_stem().unwrap_or_default().to_string_lossy();
    let ext = path.extension().unwrap_or_default().to_string_lossy();

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
        .replace(&name_token, &stem)
        .replace(&extension_token, &ext);

    // Only append extension if the pattern doesn't already include {ext}
    // (which would have been replaced with the actual extension)
    if pattern.contains(&extension_token) {
        Ok(filename)
    } else {
        Ok(format!("{filename}.{ext}"))
    }
}
