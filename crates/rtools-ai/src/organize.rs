use rtools_core::error::{RToolsError, RToolsResult};
use rtools_core::{FileInput, FileOutput, Processor};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::PathBuf;
use std::time::Instant;

/// AI organize configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrganizeConfig {
    /// Output directory for organized photos
    pub output_dir: PathBuf,
    /// Organization strategy
    pub strategy: OrganizeStrategy,
    /// Create year/month folders
    pub by_date: bool,
    /// Create folders by subject
    pub by_subject: bool,
    /// Dry run mode (preview only)
    pub dry_run: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OrganizeStrategy {
    /// Organize by date
    ByDate,
    /// Organize by subject
    BySubject,
    /// Organize by location
    ByLocation,
    /// Organize by camera
    ByCamera,
    /// Custom AI classification
    Custom,
}

impl Default for OrganizeConfig {
    fn default() -> Self {
        Self {
            output_dir: PathBuf::from("organized"),
            strategy: OrganizeStrategy::ByDate,
            by_date: true,
            by_subject: false,
            dry_run: false,
        }
    }
}

/// AI organize processor
pub struct OrganizeProcessor;

impl Processor for OrganizeProcessor {
    type Input = Vec<FileInput>;
    type Output = Vec<FileOutput>;
    type Config = OrganizeConfig;
    type Error = RToolsError;

    fn process_validated(
        &self,
        inputs: Vec<FileInput>,
        config: OrganizeConfig,
    ) -> RToolsResult<Vec<FileOutput>> {
        let _start = Instant::now();

        if inputs.is_empty() {
            return Err(RToolsError::invalid_input(
                "Organize requires at least one input file",
            ));
        }

        if !config.dry_run {
            std::fs::create_dir_all(&config.output_dir)?;
        }

        let mut outputs = Vec::new();
        let mut planned_destinations = HashSet::new();

        for input in inputs {
            let path = input
                .source
                .as_path()
                .ok_or_else(|| RToolsError::invalid_input("Organize requires file path inputs"))?;

            let target_folder = Self::get_date_folder(path)?;

            let target_dir = config.output_dir.join(&target_folder);
            if !config.dry_run {
                std::fs::create_dir_all(&target_dir)?;
            }

            let orig_file_name = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            let stem = path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            let ext = path
                .extension()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();

            // Collision-safe filename resolution
            let mut target_path = target_dir.join(&orig_file_name);
            let mut counter = 1;
            while target_path.exists() || planned_destinations.contains(&target_path) {
                let unique_name = if ext.is_empty() {
                    format!("{stem}_{counter}")
                } else {
                    format!("{stem}_{counter}.{ext}")
                };
                target_path = target_dir.join(unique_name);
                counter += 1;
            }
            planned_destinations.insert(target_path.clone());

            if !config.dry_run {
                std::fs::copy(path, &target_path)?;
            }

            outputs.push(FileOutput {
                destination: rtools_core::output::OutputDestination::File(target_path.clone()),
                name: target_path
                    .file_name()
                    .map(|s| s.to_string_lossy().to_string()),
                mime_type: None,
                stats: None,
                warnings: Vec::new(),
            });
        }

        Ok(outputs)
    }

    fn validate_config(&self, config: &OrganizeConfig) -> RToolsResult<()> {
        let operation_id = match config.strategy {
            OrganizeStrategy::ByDate => return Ok(()),
            OrganizeStrategy::BySubject => "ai.organize.subject",
            OrganizeStrategy::ByLocation => "ai.organize.location",
            OrganizeStrategy::ByCamera => "ai.organize.camera",
            OrganizeStrategy::Custom => "ai.organize.custom",
        };
        Err(RToolsError::capability_unavailable(
            operation_id,
            "This organization strategy is not implemented",
            "Use date organization",
        ))
    }

    fn name(&self) -> &'static str {
        "OrganizeProcessor"
    }
}

impl OrganizeProcessor {
    fn get_date_folder(path: &PathBuf) -> RToolsResult<PathBuf> {
        let metadata = std::fs::metadata(path)?;
        date_folder_from_modified(metadata.modified())
    }
}

fn date_folder_from_modified(
    modified: std::io::Result<std::time::SystemTime>,
) -> RToolsResult<PathBuf> {
    let modified = modified?;
    let datetime: chrono::DateTime<chrono::Local> = modified.into();

    Ok(PathBuf::from(datetime.format("%Y/%m").to_string()))
}

#[cfg(test)]
mod tests {
    use super::date_folder_from_modified;
    use rtools_core::ErrorCode;
    use std::io;
    use std::path::PathBuf;
    use std::time::{Duration, SystemTime};

    #[test]
    fn modified_time_errors_are_propagated_without_today_fallback() {
        let error = date_folder_from_modified(Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "injected mtime failure",
        )))
        .unwrap_err();

        assert_eq!(error.code(), ErrorCode::ProcessingFailed);
        assert!(matches!(
            error,
            rtools_core::RToolsError::Io(error)
                if error.kind() == io::ErrorKind::PermissionDenied
        ));
    }

    #[test]
    fn valid_modified_time_maps_to_its_year_and_month() {
        let modified = SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        let expected: chrono::DateTime<chrono::Local> = modified.into();

        assert_eq!(
            date_folder_from_modified(Ok(modified)).unwrap(),
            PathBuf::from(expected.format("%Y/%m").to_string())
        );
    }
}
