use rtools_core::error::{RToolsError, RToolsResult};
use rtools_core::{FileInput, FileOutput, Processor};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

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

    fn process_validated(
        &self,
        _inputs: Vec<FileInput>,
        _config: SortConfig,
    ) -> RToolsResult<Vec<FileOutput>> {
        Err(sort_unavailable())
    }

    fn validate_config(&self, _config: &SortConfig) -> RToolsResult<()> {
        Err(sort_unavailable())
    }

    fn name(&self) -> &'static str {
        "SortProcessor"
    }
}

fn sort_unavailable() -> RToolsError {
    RToolsError::capability_unavailable(
        "ai.sort",
        "File sorting is not implemented",
        "Use date organization or sort files manually",
    )
}
