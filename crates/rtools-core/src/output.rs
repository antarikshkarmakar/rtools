use crate::error::{RToolsError, RToolsResult};
use crate::types::ProcessStats;
use serde::{Deserialize, Serialize};
use std::ffi::OsString;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

const MAX_UNIQUE_SUFFIX: u16 = 999;
static NEXT_OWNER: AtomicU64 = AtomicU64::new(1);

/// Policy used when a requested output path already exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum OutputPolicy {
    /// Return an error without changing the existing output.
    #[default]
    FailIfExists,
    /// Select the first available numeric sibling name.
    UniqueName,
    /// Replace the existing output only after the new artifact validates.
    Overwrite,
}

/// A sibling temporary artifact and its collision reservation.
#[derive(Debug)]
pub struct PendingOutput {
    final_path: PathBuf,
    temporary_path: PathBuf,
    reservation_path: PathBuf,
    owner_token: String,
    policy: OutputPolicy,
    committed: bool,
}

impl PendingOutput {
    /// Reserve an output path and an empty sibling temporary file.
    ///
    /// # Errors
    ///
    /// Returns a path-policy error for an invalid parent, an output-exists
    /// error for a collision, or an I/O error when reservation fails.
    pub fn new(output: impl AsRef<Path>, policy: OutputPolicy) -> RToolsResult<Self> {
        let requested = output.as_ref();
        validate_output_parent(requested)?;

        let owner_token = owner_token();
        let (final_path, reservation_path) = reserve_destination(requested, policy, &owner_token)?;
        let temporary_path = sibling_temporary_path(&final_path, &owner_token)?;
        if let Err(error) = OpenOptions::new()
            .write(true)
            .read(true)
            .create_new(true)
            .open(&temporary_path)
        {
            let _ = remove_lock_if_owned(&reservation_path, &owner_token);
            return Err(error.into());
        }

        Ok(Self {
            final_path,
            temporary_path,
            reservation_path,
            owner_token,
            policy,
            committed: false,
        })
    }

    /// Return the private sibling path into which the caller must encode.
    pub fn temporary_path(&self) -> &Path {
        &self.temporary_path
    }

    /// Validate, durably flush, and atomically publish the temporary artifact.
    ///
    /// # Errors
    ///
    /// Returns the validator error, a collision or reservation-ownership
    /// error, or an I/O/rollback error when the durable commit fails.
    pub fn commit<F>(mut self, validate: F) -> RToolsResult<PathBuf>
    where
        F: FnOnce(&Path) -> RToolsResult<()>,
    {
        let mut artifact = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&self.temporary_path)?;
        validate(&self.temporary_path)?;
        artifact.flush()?;
        artifact.sync_all()?;

        ensure_lock_owned(&self.reservation_path, &self.owner_token)?;
        self.recheck_destination()?;
        self.commit_rename()?;
        self.committed = true;
        remove_lock_if_owned(&self.reservation_path, &self.owner_token)?;

        Ok(self.final_path.clone())
    }

    fn recheck_destination(&self) -> RToolsResult<()> {
        match self.policy {
            OutputPolicy::FailIfExists | OutputPolicy::UniqueName => {
                if self.final_path.try_exists()? {
                    return Err(RToolsError::output_exists(
                        self.final_path.display().to_string(),
                    ));
                }
            }
            OutputPolicy::Overwrite => reject_directory_destination(&self.final_path)?,
        }
        Ok(())
    }

    #[cfg(not(windows))]
    fn commit_rename(&self) -> RToolsResult<()> {
        fs::rename(&self.temporary_path, &self.final_path)?;
        Ok(())
    }

    #[cfg(windows)]
    fn commit_rename(&self) -> RToolsResult<()> {
        if self.policy != OutputPolicy::Overwrite || !self.final_path.try_exists()? {
            fs::rename(&self.temporary_path, &self.final_path)?;
            return Ok(());
        }

        let backup = sibling_backup_path(&self.final_path, &self.owner_token)?;
        if backup.try_exists()? {
            return Err(RToolsError::path_policy_violation(format!(
                "output backup already exists: {}",
                backup.display()
            )));
        }

        fs::rename(&self.final_path, &backup)?;
        if let Err(commit_error) = fs::rename(&self.temporary_path, &self.final_path) {
            if let Err(restore_error) = fs::rename(&backup, &self.final_path) {
                return Err(RToolsError::rollback_failed(format!(
                    "output commit failed ({commit_error}); restoring {} failed: {restore_error}",
                    self.final_path.display()
                )));
            }
            return Err(commit_error.into());
        }

        if let Err(cleanup_error) = fs::remove_file(&backup) {
            if let Err(remove_new_error) = fs::remove_file(&self.final_path) {
                return Err(RToolsError::rollback_failed(format!(
                    "backup cleanup failed ({cleanup_error}); removing new output failed: {remove_new_error}"
                )));
            }
            if let Err(restore_error) = fs::rename(&backup, &self.final_path) {
                return Err(RToolsError::rollback_failed(format!(
                    "backup cleanup failed ({cleanup_error}); restoring old output failed: {restore_error}"
                )));
            }
            return Err(cleanup_error.into());
        }

        Ok(())
    }
}

impl Drop for PendingOutput {
    fn drop(&mut self) {
        if !self.committed {
            let _ = fs::remove_file(&self.temporary_path);
        }
        let _ = remove_lock_if_owned(&self.reservation_path, &self.owner_token);
    }
}

fn validate_output_parent(output: &Path) -> RToolsResult<()> {
    if output.file_name().is_none() {
        return Err(RToolsError::path_policy_violation(format!(
            "output must name a file: {}",
            output.display()
        )));
    }
    let parent = output
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let metadata = fs::metadata(parent).map_err(|error| {
        RToolsError::path_policy_violation(format!(
            "output parent is unavailable ({}): {error}",
            parent.display()
        ))
    })?;
    if !metadata.is_dir() {
        return Err(RToolsError::path_policy_violation(format!(
            "output parent is not a directory: {}",
            parent.display()
        )));
    }
    Ok(())
}

fn reserve_destination(
    requested: &Path,
    policy: OutputPolicy,
    token: &str,
) -> RToolsResult<(PathBuf, PathBuf)> {
    match policy {
        OutputPolicy::UniqueName => {
            for suffix in 0..=MAX_UNIQUE_SUFFIX {
                let candidate = if suffix == 0 {
                    requested.to_path_buf()
                } else {
                    unique_candidate(requested, suffix)?
                };
                if candidate.try_exists()? {
                    continue;
                }
                match create_reservation(&candidate, token) {
                    Ok(reservation) => return Ok((candidate, reservation)),
                    Err(RToolsError::Io(error))
                        if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                    Err(error) => return Err(error),
                }
            }
            Err(RToolsError::output_exists(format!(
                "no unique output name available for {} after {MAX_UNIQUE_SUFFIX} suffixes",
                requested.display()
            )))
        }
        OutputPolicy::FailIfExists => {
            if requested.try_exists()? {
                return Err(RToolsError::output_exists(requested.display().to_string()));
            }
            let reservation = create_reservation(requested, token)
                .map_err(|error| map_reservation_collision(error, requested))?;
            Ok((requested.to_path_buf(), reservation))
        }
        OutputPolicy::Overwrite => {
            reject_directory_destination(requested)?;
            let reservation = create_reservation(requested, token)
                .map_err(|error| map_reservation_collision(error, requested))?;
            Ok((requested.to_path_buf(), reservation))
        }
    }
}

fn map_reservation_collision(error: RToolsError, output: &Path) -> RToolsError {
    match error {
        RToolsError::Io(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            RToolsError::output_exists(format!("output is reserved: {}", output.display()))
        }
        error => error,
    }
}

fn create_reservation(final_path: &Path, token: &str) -> RToolsResult<PathBuf> {
    let reservation = sibling_path(final_path, ".rtools.lock")?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&reservation)?;
    if let Err(error) = file
        .write_all(token.as_bytes())
        .and_then(|()| file.flush())
        .and_then(|()| file.sync_all())
    {
        drop(file);
        let _ = fs::remove_file(&reservation);
        return Err(error.into());
    }
    Ok(reservation)
}

fn reject_directory_destination(path: &Path) -> RToolsResult<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_dir() => Err(RToolsError::path_policy_violation(format!(
            "output destination is a directory: {}",
            path.display()
        ))),
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

fn owner_token() -> String {
    let counter = NEXT_OWNER.fetch_add(1, Ordering::Relaxed);
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    format!("{}-{counter}-{timestamp}", std::process::id())
}

fn unique_candidate(path: &Path, suffix: u16) -> RToolsResult<PathBuf> {
    let stem = path.file_stem().ok_or_else(|| {
        RToolsError::path_policy_violation(format!("output must name a file: {}", path.display()))
    })?;
    let mut name = OsString::from(stem);
    name.push(format!("_{suffix}"));
    if let Some(extension) = path.extension() {
        name.push(".");
        name.push(extension);
    }
    Ok(path.with_file_name(name))
}

fn sibling_path(path: &Path, suffix: &str) -> RToolsResult<PathBuf> {
    let file_name = path.file_name().ok_or_else(|| {
        RToolsError::path_policy_violation(format!("output must name a file: {}", path.display()))
    })?;
    let mut name = OsString::from(".");
    name.push(file_name);
    name.push(suffix);
    Ok(path.with_file_name(name))
}

fn sibling_temporary_path(path: &Path, token: &str) -> RToolsResult<PathBuf> {
    sibling_path(path, &format!(".rtools-{token}.tmp"))
}

#[cfg(windows)]
fn sibling_backup_path(path: &Path, token: &str) -> RToolsResult<PathBuf> {
    sibling_path(path, &format!(".rtools-{token}.backup"))
}

fn ensure_lock_owned(path: &Path, token: &str) -> RToolsResult<()> {
    let file = File::open(path).map_err(|error| {
        RToolsError::path_policy_violation(format!(
            "output reservation is missing or unreadable ({}): {error}",
            path.display()
        ))
    })?;
    let limit = u64::try_from(token.len().saturating_add(1)).unwrap_or(u64::MAX);
    let mut contents = Vec::with_capacity(token.len().saturating_add(1));
    file.take(limit).read_to_end(&mut contents)?;
    if contents != token.as_bytes() {
        return Err(RToolsError::path_policy_violation(format!(
            "output reservation ownership changed: {}",
            path.display()
        )));
    }
    Ok(())
}

fn remove_lock_if_owned(path: &Path, token: &str) -> RToolsResult<()> {
    ensure_lock_owned(path, token)?;
    fs::remove_file(path)?;
    Ok(())
}

/// Resolve an explicit output path: if it points to an existing directory
/// (or ends with a path separator), treat it as a directory and join `name`
/// onto it; otherwise return it unchanged as a file path.
#[must_use]
pub fn resolve_output_path(output: &Path, name: &str) -> PathBuf {
    if output.is_dir()
        || output
            .to_string_lossy()
            .ends_with(std::path::MAIN_SEPARATOR)
    {
        output.join(name)
    } else {
        output.to_path_buf()
    }
}

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
    pub const fn is_file(&self) -> bool {
        matches!(self, OutputDestination::File(_))
    }

    /// Get the path if this is a file output
    pub const fn as_path(&self) -> Option<&PathBuf> {
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
            .map(std::string::ToString::to_string);

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
            .map(std::string::ToString::to_string);

        Self {
            destination: OutputDestination::Directory(path),
            name,
            mime_type: None,
            stats: None,
        }
    }

    /// Create a new bytes output
    pub const fn to_bytes() -> Self {
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
