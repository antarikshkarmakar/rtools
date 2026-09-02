use rtools_core::error::{RToolsError, RToolsResult};
use rtools_core::{FileInput, FileOutput, Processor};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Instant;

/// Sort configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SortConfig {
    /// Sort criteria
    pub criteria: SortCriteria,
    /// Sort order
    pub order: SortOrder,
    /// Output directory
    pub output_dir: PathBuf,
    /// Dry run mode
    pub dry_run: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SortCriteria {
    Date,
    Size,
    Type,
    Name,
    GpsLocation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SortOrder {
    Ascending,
    Descending,
}

impl Default for SortConfig {
    fn default() -> Self {
        Self {
            criteria: SortCriteria::Date,
            order: SortOrder::Ascending,
            output_dir: PathBuf::from("sorted"),
            dry_run: false,
        }
    }
}

/// Sort processor
pub struct SortProcessor;

impl Processor for SortProcessor {
    type Input = Vec<FileInput>;
    type Output = Vec<FileOutput>;
    type Config = SortConfig;
    type Error = RToolsError;

    fn process(&self, inputs: Vec<FileInput>, config: SortConfig) -> RToolsResult<Vec<FileOutput>> {
        let start = Instant::now();

        std::fs::create_dir_all(&config.output_dir)?;

        let mut files: Vec<(PathBuf, u64, String)> = inputs
            .iter()
            .filter_map(|input| {
                let path = input.source.as_path()?;
                let metadata = std::fs::metadata(path).ok()?;
                let sort_key = match config.criteria {
                    SortCriteria::Date => metadata.modified().ok().map_or(0, |t| {
                        t.duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs()
                    }),
                    SortCriteria::Size => metadata.len(),
                    SortCriteria::Type | SortCriteria::Name | SortCriteria::GpsLocation => 0,
                };
                let name = path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                Some((path.clone(), sort_key, name))
            })
            .collect();

        // Sort files
        match config.criteria {
            SortCriteria::Name => {
                files.sort_by(|a, b| a.2.cmp(&b.2));
            }
            _ => {
                files.sort_by_key(|f| f.1);
            }
        }

        if matches!(config.order, SortOrder::Descending) {
            files.reverse();
        }

        let mut outputs = Vec::new();

        for (idx, (path, _, _)) in files.iter().enumerate() {
            let file_name = path.file_name().unwrap_or_default();
            let target =
                config
                    .output_dir
                    .join(format!("{:04}_{}", idx + 1, file_name.to_string_lossy()));

            if !config.dry_run {
                std::fs::copy(path, &target)?;
            }

            outputs.push(FileOutput {
                destination: rtools_core::output::OutputDestination::File(target),
                name: Some(format!("{:04}_{}", idx + 1, file_name.to_string_lossy())),
                mime_type: None,
                stats: None,
            });
        }

        let _elapsed = start.elapsed();

        Ok(outputs)
    }

    fn validate_config(&self, _config: &SortConfig) -> RToolsResult<()> {
        Ok(())
    }

    fn name(&self) -> &'static str {
        "SortProcessor"
    }
}
