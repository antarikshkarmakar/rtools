use rtools_core::error::{RToolsError, RToolsResult};
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

    fn process(
        &self,
        inputs: Vec<FileInput>,
        config: OrganizeConfig,
    ) -> RToolsResult<Vec<FileOutput>> {
        let _start = Instant::now();

        std::fs::create_dir_all(&config.output_dir)?;

        let mut outputs = Vec::new();

        for input in inputs {
            let path = input
                .source
                .as_path()
                .ok_or_else(|| RToolsError::invalid_input("Organize requires file path inputs"))?;

            let target_folder = match config.strategy {
                OrganizeStrategy::ByDate => Self::get_date_folder(path)?,
                OrganizeStrategy::BySubject => PathBuf::from("subject"),
                OrganizeStrategy::ByLocation => PathBuf::from("location"),
                OrganizeStrategy::ByCamera => PathBuf::from("camera"),
                OrganizeStrategy::Custom => PathBuf::from("custom"),
            };

            let target_dir = config.output_dir.join(&target_folder);
            std::fs::create_dir_all(&target_dir)?;

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
            while target_path.exists() && !config.dry_run {
                let unique_name = if ext.is_empty() {
                    format!("{stem}_{counter}")
                } else {
                    format!("{stem}_{counter}.{ext}")
                };
                target_path = target_dir.join(unique_name);
                counter += 1;
            }

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
            });
        }

        Ok(outputs)
    }

    fn validate_config(&self, _config: &OrganizeConfig) -> RToolsResult<()> {
        Ok(())
    }

    fn name(&self) -> &'static str {
        "OrganizeProcessor"
    }
}

impl OrganizeProcessor {
    fn get_date_folder(path: &PathBuf) -> RToolsResult<PathBuf> {
        let metadata = std::fs::metadata(path)?;
        let modified = metadata
            .modified()
            .unwrap_or_else(|_| std::time::SystemTime::now());
        let datetime: chrono::DateTime<chrono::Local> = modified.into();

        Ok(PathBuf::from(datetime.format("%Y/%m").to_string()))
    }
}
