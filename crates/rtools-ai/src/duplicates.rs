use rtools_core::error::{RToolsError, RToolsResult};
use rtools_core::{FileInput, Processor};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Duplicates detection configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DuplicatesConfig {
    /// Similarity threshold (0.0-1.0)
    pub threshold: f64,
    /// Hash algorithm
    pub algorithm: HashAlgorithm,
    /// Action to take on duplicates
    pub action: DuplicateAction,
    /// Dry run mode
    pub dry_run: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HashAlgorithm {
    /// Average hash
    Average,
    /// Perceptual hash
    Perceptual,
    /// Difference hash
    Difference,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DuplicateAction {
    /// Just report duplicates
    Report,
    /// Move duplicates to a folder
    Move { destination: PathBuf },
    /// Delete duplicates
    Delete,
    /// Create symlinks
    Symlink,
}

impl Default for DuplicatesConfig {
    fn default() -> Self {
        Self {
            threshold: 0.9,
            algorithm: HashAlgorithm::Perceptual,
            action: DuplicateAction::Report,
            dry_run: false,
        }
    }
}

/// Duplicates detection processor
pub struct DuplicatesProcessor;

impl Processor for DuplicatesProcessor {
    type Input = Vec<FileInput>;
    type Output = DuplicatesResult;
    type Config = DuplicatesConfig;
    type Error = RToolsError;

    fn process(&self, inputs: Vec<FileInput>, config: DuplicatesConfig) -> RToolsResult<DuplicatesResult> {
        let start = std::time::Instant::now();

        // Group files by hash
        let mut hashes: std::collections::HashMap<u64, Vec<PathBuf>> = std::collections::HashMap::new();

        for input in inputs {
            let path = input.source.as_path().ok_or_else(|| {
                RToolsError::invalid_input("Duplicates requires file path inputs")
            })?;

            // Calculate hash
            let hash = calculate_image_hash(path, &config.algorithm)?;
            hashes.entry(hash).or_default().push(path.clone());
        }

        // Find duplicates
        let duplicate_groups: Vec<DuplicateGroup> = hashes
            .into_iter()
            .filter(|(_, files)| files.len() > 1)
            .map(|(hash, files)| DuplicateGroup {
                hash,
                files,
                is_original: true,
            })
            .collect();

        let elapsed = start.elapsed();
        let total_duplicates: usize = duplicate_groups.iter().map(|g| g.files.len() - 1).sum();

        Ok(DuplicatesResult {
            groups: duplicate_groups,
            total_originals: inputs.len() - total_duplicates,
            total_duplicates,
            processing_time_ms: elapsed.as_millis() as u64,
        })
    }

    fn validate_config(&self, config: &DuplicatesConfig) -> RToolsResult<()> {
        if config.threshold < 0.0 || config.threshold > 1.0 {
            return Err(RToolsError::invalid_input("Threshold must be between 0.0 and 1.0"));
        }
        Ok(())
    }

    fn name(&self) -> &str {
        "DuplicatesProcessor"
    }
}

/// Duplicates result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DuplicatesResult {
    pub groups: Vec<DuplicateGroup>,
    pub total_originals: usize,
    pub total_duplicates: usize,
    pub processing_time_ms: u64,
}

/// Duplicate group
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DuplicateGroup {
    pub hash: u64,
    pub files: Vec<PathBuf>,
    pub is_original: bool,
}

/// Calculate image hash
fn calculate_image_hash(path: &PathBuf, algorithm: &HashAlgorithm) -> RToolsResult<u64> {
    let img = image::open(path)?;
    let gray = img.to_luma8();
    let (width, height) = gray.dimensions();

    match algorithm {
        HashAlgorithm::Average => {
            // Simple average hash
            let pixels: Vec<u8> = gray.pixels().copied().collect();
            let avg: u8 = pixels.iter().sum::<u32>() as u8 / pixels.len() as u8;
            let mut hash = 0u64;
            for (i, pixel) in pixels.iter().enumerate() {
                if *pixel > avg {
                    hash |= 1 << (i % 64);
                }
            }
            Ok(hash)
        }
        HashAlgorithm::Perceptual => {
            // pHash placeholder
            let pixels: Vec<u8> = gray.pixels().copied().collect();
            let mut hash = 0u64;
            for (i, pixel) in pixels.iter().enumerate().step_by(width as usize) {
                if *pixel > 128 {
                    hash |= 1 << (i % 64);
                }
            }
            Ok(hash)
        }
        HashAlgorithm::Difference => {
            // dHash placeholder
            let mut hash = 0u64;
            for y in 0..height {
                for x in 0..width - 1 {
                    let left = gray.get_pixel((x, y))[0];
                    let right = gray.get_pixel((x + 1, y))[0];
                    if left > right {
                        hash |= 1 << ((y * width + x) % 64);
                    }
                }
            }
            Ok(hash)
        }
    }
}