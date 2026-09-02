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

    fn process(
        &self,
        inputs: Vec<FileInput>,
        config: DuplicatesConfig,
    ) -> RToolsResult<DuplicatesResult> {
        let start = std::time::Instant::now();

        let mut file_hashes: Vec<(PathBuf, u64)> = Vec::new();

        for input in &inputs {
            let path = input.source.as_path().ok_or_else(|| {
                RToolsError::invalid_input("Duplicates requires file path inputs")
            })?;

            let hash = calculate_image_hash(path, &config.algorithm)?;
            file_hashes.push((path.clone(), hash));
        }

        let max_distance = max_hamming_distance(config.threshold);

        // Group files by perceptual distance
        let mut visited = vec![false; file_hashes.len()];
        let mut duplicate_groups: Vec<DuplicateGroup> = Vec::new();

        for i in 0..file_hashes.len() {
            if visited[i] {
                continue;
            }

            let mut group_files = vec![file_hashes[i].0.clone()];
            visited[i] = true;

            for j in (i + 1)..file_hashes.len() {
                if !visited[j] {
                    let dist = (file_hashes[i].1 ^ file_hashes[j].1).count_ones();
                    if dist <= max_distance {
                        group_files.push(file_hashes[j].0.clone());
                        visited[j] = true;
                    }
                }
            }

            if group_files.len() > 1 {
                duplicate_groups.push(DuplicateGroup {
                    hash: file_hashes[i].1,
                    files: group_files,
                    is_original: true,
                });
            }
        }

        // Apply action if needed
        if !config.dry_run {
            for group in &duplicate_groups {
                for duplicate in group.files.iter().skip(1) {
                    match &config.action {
                        DuplicateAction::Delete => {
                            let _ = std::fs::remove_file(duplicate);
                        }
                        DuplicateAction::Move { destination } => {
                            let _ = std::fs::create_dir_all(destination);
                            if let Some(file_name) = duplicate.file_name() {
                                let target = destination.join(file_name);
                                let _ = std::fs::rename(duplicate, target);
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        let elapsed = start.elapsed();
        let total_duplicates: usize = duplicate_groups.iter().map(|g| g.files.len() - 1).sum();

        Ok(DuplicatesResult {
            groups: duplicate_groups,
            total_originals: inputs.len().saturating_sub(total_duplicates),
            total_duplicates,
            processing_time_ms: u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX),
        })
    }

    fn validate_config(&self, config: &DuplicatesConfig) -> RToolsResult<()> {
        if config.threshold < 0.0 || config.threshold > 1.0 {
            return Err(RToolsError::invalid_input(
                "Threshold must be between 0.0 and 1.0",
            ));
        }
        Ok(())
    }

    fn name(&self) -> &'static str {
        "DuplicatesProcessor"
    }
}

/// Converts a similarity threshold to the inclusive 64-bit hash distance.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn max_hamming_distance(threshold: f64) -> u32 {
    // The clamped, rounded value is in 0..=64. This preserves the original
    // `f64::round` half-way behavior before the validated integer conversion.
    let distance = (1.0 - threshold.clamp(0.0, 1.0)) * 64.0;
    distance.round().clamp(0.0, 64.0) as u32
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

/// Calculate robust 64-bit image perceptual hash (aHash / dHash)
fn calculate_image_hash(path: &PathBuf, algorithm: &HashAlgorithm) -> RToolsResult<u64> {
    let img = image::open(path).map_err(|e| {
        RToolsError::image(format!(
            "Failed to open image for hashing {}: {}",
            path.display(),
            e
        ))
    })?;

    match algorithm {
        HashAlgorithm::Average => {
            // Resize to 8x8 grayscale
            let resized = img
                .resize_exact(8, 8, image::imageops::FilterType::Triangle)
                .to_luma8();
            let pixels: &[u8] = &resized;
            let sum: u64 = pixels.iter().map(|&p| u64::from(p)).sum();
            let avg = u8::try_from(sum / 64).expect("an average of u8 pixels fits in u8");

            let mut hash = 0u64;
            for (i, &pixel) in pixels.iter().enumerate() {
                if pixel >= avg {
                    hash |= 1u64 << i;
                }
            }
            Ok(hash)
        }
        HashAlgorithm::Difference | HashAlgorithm::Perceptual => {
            // Resize to 9x8 grayscale for gradient tracking (dHash)
            let resized = img
                .resize_exact(9, 8, image::imageops::FilterType::Triangle)
                .to_luma8();
            let mut hash = 0u64;
            for y in 0..8 {
                for x in 0..8 {
                    let left = resized.get_pixel(x, y)[0];
                    let right = resized.get_pixel(x + 1, y)[0];
                    if left > right {
                        hash |= 1u64 << (y * 8 + x);
                    }
                }
            }
            Ok(hash)
        }
    }
}
