use crate::types::ProcessStats;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Output destination for processing results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OutputDestination {
    /// File on local filesystem
    File(PathBuf),
    /// Directory (auto-generate filename)
    Directory(PathBuf),
    /// Bytes in memory
    Bytes(Vec<u8>),
    /// Stream to stdout
    Stdout,
}

impl OutputDestination {
    /// Check if this is a file output
    pub fn is_file(&self) -> bool {
        matches!(self, OutputDestination::File(_))
    }

    /// Get the path if this is a file output
    pub fn as_path(&self) -> Option<&PathBuf> {
        match self {
            OutputDestination::File(path) | OutputDestination::Directory(path) => Some(path),
            _ => None,
        }
    }
}

/// Process output with statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileOutput {
    /// The output destination
    pub destination: OutputDestination,
    /// Output file name
    pub name: Option<String>,
    /// Output MIME type
    pub mime_type: Option<String>,
    /// Processing statistics
    pub stats: Option<ProcessStats>,
}

impl FileOutput {
    /// Create a new file output
    pub fn to_file(path: PathBuf) -> Self {
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .map(|n| n.to_string());

        Self {
            destination: OutputDestination::File(path),
            name,
            mime_type: None,
            stats: None,
        }
    }

    /// Create a new directory output
    pub fn to_directory(path: PathBuf) -> Self {
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .map(|n| n.to_string());

        Self {
            destination: OutputDestination::Directory(path),
            name,
            mime_type: None,
            stats: None,
        }
    }

    /// Create a new bytes output
    pub fn to_bytes() -> Self {
        Self {
            destination: OutputDestination::Bytes(Vec::new()),
            name: None,
            mime_type: None,
            stats: None,
        }
    }
}

/// Process result with output and optional stats
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessResult {
    /// The output
    pub output: FileOutput,
    /// Success message
    pub message: String,
    /// Processing statistics
    pub stats: ProcessStats,
}

/// Batch process result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchProcessResult {
    /// Individual results
    pub results: Vec<ProcessResult>,
    /// Failed items
    pub failures: Vec<BatchFailure>,
    /// Aggregate statistics
    pub aggregate_stats: ProcessStats,
    /// Processing duration
    pub duration_ms: u64,
}

/// Batch processing failure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchFailure {
    /// File path that failed
    pub path: PathBuf,
    /// Error message
    pub error: String,
}

/// Progress callback for batch operations
pub type ProgressCallback = Box<dyn Fn(usize, usize, &str) + Send + Sync>;