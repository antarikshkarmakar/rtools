use rtools_core::error::{RToolsError, RToolsResult};
use rtools_core::types::ProcessStats;
use rtools_core::{FileInput, FileOutput, Processor};
use serde::{Deserialize, Serialize};
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

    fn process(&self, inputs: Vec<FileInput>, config: OrganizeConfig) -> RToolsResult<Vec<FileOutput>> {
        let start = Instant::now();

        // Create output directory
        std::fs::create_dir_all(&config.output_dir)?;

        let mut outputs = Vec::new();

        for input in inputs {
            let path = input.source.as_path().ok_or_else(|| {
                RToolsError::invalid_input("Organize requires file path inputs")
            })?;

            // Determine target folder based on strategy
            let target_folder = match config.strategy {
                OrganizeStrategy::ByDate => self.get_date_folder(path)?,
                OrganizeStrategy::BySubject => {
                    // TODO: Use AI to classify subject
                    PathBuf::from("unknown_subject")
                }
                OrganizeStrategy::ByLocation => {
                    // TODO: Use GPS coordinates
                    PathBuf::from("unknown_location")
                }
                OrganizeStrategy::ByCamera => {
                    // TODO: Use EXIF camera info
                    PathBuf::from("unknown_camera")
                }
                OrganizeStrategy::Custom => {
                    PathBuf::from("custom")
                }
            };

            let target_dir = config.output_dir.join(&target_folder);
            std::fs::create_dir_all(&target_dir)?;

            let file_name = path.file_name().unwrap_or_default();
            let target_path = target_dir.join(file_name);

            if !config.dry_run {
                std::fs::copy(path, &target_path)?;
            }

            outputs.push(FileOutput {
                destination: rtools_core::output::OutputDestination::File(target_path),
                name: file_name.to_str().map(|s| s.to_string()),
                mime_type: None,
                stats: None,
            });
        }

        let elapsed = start.elapsed();

        Ok(outputs)
    }

    fn validate_config(&self, config: &OrganizeConfig) -> RToolsResult<()> {
        if !config.output_dir.exists() && !config.dry_run {
            return Err(RToolsError::output_directory_not_found(
                config.output_dir.display().to_string(),
            ));
        }
        Ok(())
    }

    fn name(&self) -> &str {
        "OrganizeProcessor"
    }
}

impl OrganizeProcessor {
    fn get_date_folder(&self, path: &PathBuf) -> RToolsResult<PathBuf> {
        let metadata = std::fs::metadata(path)?;
        let modified = metadata.modified()?;
        let datetime: chrono::DateTime<chrono::Local> = modified.into();

        Ok(PathBuf::from(datetime.format("%Y/%m").to_string()))
    }
}