use crate::error::{RToolsError, RToolsResult};
use crate::types::ProcessStats;
use serde::{Deserialize, Serialize};
use std::ffi::OsString;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Component, Path, PathBuf};
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
///
/// Cooperating rTools writers hold an exclusive advisory lock on a `create_new`
/// reservation. Publication itself does not depend on cooperation: create-only
/// publication preserves any final-path entry installed by another writer.
/// A same-user process that deliberately removes reservation pathnames while
/// ignoring the advisory lock is outside the portable locking guarantee, but
/// ownership changes are detected and foreign reservation entries are never
/// intentionally removed.
#[derive(Debug)]
pub struct PendingOutput {
    final_path: PathBuf,
    temporary_path: PathBuf,
    reservation_path: PathBuf,
    reservation_file: Option<File>,
    reservation_retired: bool,
    owner_token: String,
    policy: OutputPolicy,
}

impl PendingOutput {
    /// Reserve an output path and an empty sibling temporary file.
    ///
    /// Every parent-directory ancestor must already exist. This constructor
    /// deliberately does not create directories from an untrusted output path,
    /// because portable path-based creation can traverse a concurrently linked
    /// ancestor before policy validation.
    ///
    /// # Errors
    ///
    /// Returns a path-policy error for an invalid parent, an output-exists
    /// error for a collision, or an I/O error when reservation fails.
    pub fn new(output: impl AsRef<Path>, policy: OutputPolicy) -> RToolsResult<Self> {
        let requested = output.as_ref();
        validate_output_parent(requested)?;

        let owner_token = owner_token();
        let (final_path, reservation_path, reservation_file) =
            reserve_destination(requested, policy, &owner_token)?;
        let temporary_path = sibling_temporary_path(&final_path, &owner_token)?;
        if let Err(error) = OpenOptions::new()
            .write(true)
            .read(true)
            .create_new(true)
            .open(&temporary_path)
        {
            drop(reservation_file);
            let _ = retire_reservation(&reservation_path, &owner_token);
            return Err(error.into());
        }

        Ok(Self {
            final_path,
            temporary_path,
            reservation_path,
            reservation_file: Some(reservation_file),
            reservation_retired: false,
            owner_token,
            policy,
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
        self.commit_internal(validate, || Ok(()), || Ok(()))
    }

    fn commit_internal<F, H, R>(
        &mut self,
        validate: F,
        pre_publish: H,
        after_retire: R,
    ) -> RToolsResult<PathBuf>
    where
        F: FnOnce(&Path) -> RToolsResult<()>,
        H: FnOnce() -> RToolsResult<()>,
        R: FnOnce() -> RToolsResult<()>,
    {
        let mut artifact = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&self.temporary_path)?;
        validate(&self.temporary_path)?;
        artifact.flush()?;
        artifact.sync_all()?;

        ensure_open_lock_owned(self.open_reservation()?, &self.owner_token)?;
        ensure_lock_owned(&self.reservation_path, &self.owner_token)?;
        self.recheck_destination()?;
        pre_publish()?;
        ensure_open_lock_owned(self.open_reservation()?, &self.owner_token)?;
        ensure_lock_owned(&self.reservation_path, &self.owner_token)?;

        // DrvFS defers the visible destination of a rename for an open locked
        // file. Close our verified handle while the canonical reservation name
        // still blocks cooperating writers, then atomically claim and verify
        // that name before publishing. Fail-if-exists and unique publication
        // remain protected by the create-only hard-link primitive even if a
        // second writer reserves the destination in the narrow interval after
        // retirement.
        drop(self.reservation_file.take());
        retire_reservation(&self.reservation_path, &self.owner_token)?;
        self.reservation_retired = true;
        after_retire()?;
        if path_entry_exists(&self.reservation_path)? {
            return Err(RToolsError::output_exists(format!(
                "output was reserved by another writer: {}",
                self.final_path.display()
            )));
        }
        self.publish_artifact()?;

        Ok(self.final_path.clone())
    }

    fn open_reservation(&self) -> RToolsResult<&File> {
        self.reservation_file.as_ref().ok_or_else(|| {
            RToolsError::Internal("output reservation handle is already closed".to_string())
        })
    }

    #[cfg(test)]
    fn commit_with_pre_publish_hook<F, H>(
        mut self,
        validate: F,
        pre_publish: H,
    ) -> RToolsResult<PathBuf>
    where
        F: FnOnce(&Path) -> RToolsResult<()>,
        H: FnOnce() -> RToolsResult<()>,
    {
        self.commit_internal(validate, pre_publish, || Ok(()))
    }

    #[cfg(test)]
    fn commit_with_retired_reservation_hook<F, H>(
        mut self,
        validate: F,
        after_retire: H,
    ) -> RToolsResult<PathBuf>
    where
        F: FnOnce(&Path) -> RToolsResult<()>,
        H: FnOnce() -> RToolsResult<()>,
    {
        self.commit_internal(validate, || Ok(()), after_retire)
    }

    fn recheck_destination(&self) -> RToolsResult<()> {
        match self.policy {
            OutputPolicy::FailIfExists | OutputPolicy::UniqueName => {
                if path_entry_exists(&self.final_path)? {
                    return Err(RToolsError::output_exists(
                        self.final_path.display().to_string(),
                    ));
                }
            }
            OutputPolicy::Overwrite => reject_directory_destination(&self.final_path)?,
        }
        Ok(())
    }

    fn publish_artifact(&self) -> RToolsResult<()> {
        match self.policy {
            OutputPolicy::FailIfExists | OutputPolicy::UniqueName => {
                publish_no_replace(&self.temporary_path, &self.final_path)
            }
            OutputPolicy::Overwrite => self.commit_overwrite(),
        }
    }

    #[cfg(not(windows))]
    fn commit_overwrite(&self) -> RToolsResult<()> {
        fs::rename(&self.temporary_path, &self.final_path)?;
        Ok(())
    }

    #[cfg(windows)]
    fn commit_overwrite(&self) -> RToolsResult<()> {
        if !path_entry_exists(&self.final_path)? {
            return publish_no_replace(&self.temporary_path, &self.final_path);
        }

        let backup = sibling_backup_path(&self.final_path, &self.owner_token)?;
        if path_entry_exists(&backup)? {
            return Err(RToolsError::path_policy_violation(format!(
                "output backup already exists: {}",
                backup.display()
            )));
        }

        replace_with_backup_using(
            &self.temporary_path,
            &self.final_path,
            &backup,
            |source, destination| fs::rename(source, destination),
            |path| fs::remove_file(path),
        )
    }
}

impl Drop for PendingOutput {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.temporary_path);
        drop(self.reservation_file.take());
        if !self.reservation_retired {
            let _ = retire_reservation(&self.reservation_path, &self.owner_token);
        }
    }
}

fn validate_output_parent(output: &Path) -> RToolsResult<()> {
    reject_ambiguous_windows_path(output)?;
    let file_name = output.file_name().ok_or_else(|| {
        RToolsError::path_policy_violation(format!("output must name a file: {}", output.display()))
    })?;
    if file_name
        .to_str()
        .is_some_and(is_reserved_windows_device_name)
    {
        return Err(RToolsError::path_policy_violation(format!(
            "output uses a reserved Windows device name: {}",
            output.display()
        )));
    }
    let parent = output
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    validate_output_ancestor_chain(parent)
}

fn reject_ambiguous_windows_path(path: &Path) -> RToolsResult<()> {
    let portable_drive_relative = path.as_os_str().to_str().is_some_and(|path_text| {
        let bytes = path_text.as_bytes();
        bytes.first().is_some_and(u8::is_ascii_alphabetic)
            && bytes.get(1) == Some(&b':')
            && !matches!(bytes.get(2), Some(b'/' | b'\\'))
    });
    if portable_drive_relative {
        return Err(RToolsError::path_policy_violation(format!(
            "Windows drive-relative output paths are not allowed: {}",
            path.display()
        )));
    }

    #[cfg(windows)]
    if !path.is_absolute()
        && (path.has_root() || matches!(path.components().next(), Some(Component::Prefix(_))))
    {
        return Err(RToolsError::path_policy_violation(format!(
            "Windows root-relative or drive-relative output paths are not allowed: {}",
            path.display()
        )));
    }

    Ok(())
}

fn validate_output_ancestor_chain(parent: &Path) -> RToolsResult<()> {
    let mut current = if parent.is_absolute() {
        PathBuf::new()
    } else {
        let current = std::env::current_dir().map_err(|error| {
            RToolsError::path_policy_violation(format!(
                "output path anchor is unavailable: {error}"
            ))
        })?;
        validate_output_directory(&current)?;
        current
    };

    for component in parent.components() {
        match component {
            Component::Prefix(prefix) => current.push(prefix.as_os_str()),
            Component::CurDir => {}
            Component::RootDir | Component::ParentDir | Component::Normal(_) => {
                current.push(component.as_os_str());
                validate_output_directory(&current)?;
            }
        }
    }
    Ok(())
}

fn validate_output_directory(path: &Path) -> RToolsResult<()> {
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        RToolsError::path_policy_violation(format!(
            "output parent ancestor is unavailable ({}): {error}",
            path.display()
        ))
    })?;
    if metadata.file_type().is_symlink() {
        return Err(RToolsError::path_policy_violation(format!(
            "output parent ancestor must not be a symlink: {}",
            path.display()
        )));
    }
    if !metadata.is_dir() {
        return Err(RToolsError::path_policy_violation(format!(
            "output parent ancestor is not a directory: {}",
            path.display()
        )));
    }
    Ok(())
}

fn is_reserved_windows_device_name(file_name: &str) -> bool {
    let stem = file_name
        .split('.')
        .next()
        .unwrap_or(file_name)
        .trim_end_matches(' ');
    if ["CON", "PRN", "AUX", "NUL"]
        .iter()
        .any(|reserved| stem.eq_ignore_ascii_case(reserved))
    {
        return true;
    }

    ["COM", "LPT"].iter().any(|prefix| {
        stem.get(..prefix.len())
            .is_some_and(|candidate| candidate.eq_ignore_ascii_case(prefix))
            && matches!(
                &stem[prefix.len()..],
                "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
            )
    })
}

fn reserve_destination(
    requested: &Path,
    policy: OutputPolicy,
    token: &str,
) -> RToolsResult<(PathBuf, PathBuf, File)> {
    match policy {
        OutputPolicy::UniqueName => {
            for suffix in 0..=MAX_UNIQUE_SUFFIX {
                let candidate = if suffix == 0 {
                    requested.to_path_buf()
                } else {
                    unique_candidate(requested, suffix)?
                };
                if path_entry_exists(&candidate)? {
                    continue;
                }
                match create_reservation(&candidate, token) {
                    Ok((reservation, file)) => return Ok((candidate, reservation, file)),
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
            if path_entry_exists(requested)? {
                return Err(RToolsError::output_exists(requested.display().to_string()));
            }
            let (reservation, file) = create_reservation(requested, token)
                .map_err(|error| map_reservation_collision(error, requested))?;
            Ok((requested.to_path_buf(), reservation, file))
        }
        OutputPolicy::Overwrite => {
            reject_directory_destination(requested)?;
            let (reservation, file) = create_reservation(requested, token)
                .map_err(|error| map_reservation_collision(error, requested))?;
            Ok((requested.to_path_buf(), reservation, file))
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

fn create_reservation(final_path: &Path, token: &str) -> RToolsResult<(PathBuf, File)> {
    let reservation = sibling_path(final_path, ".rtools.lock")?;
    let mut file = OpenOptions::new()
        .write(true)
        .read(true)
        .create_new(true)
        .open(&reservation)?;
    if let Err(error) = file.lock() {
        drop(file);
        let _ = fs::remove_file(&reservation);
        return Err(error.into());
    }
    if let Err(error) = file
        .write_all(token.as_bytes())
        .and_then(|()| file.flush())
        .and_then(|()| file.sync_all())
    {
        drop(file);
        let _ = fs::remove_file(&reservation);
        return Err(error.into());
    }
    Ok((reservation, file))
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

fn path_entry_exists(path: &Path) -> std::io::Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
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

fn ensure_open_lock_owned(file: &File, token: &str) -> RToolsResult<()> {
    let mut file = file.try_clone()?;
    file.seek(SeekFrom::Start(0))?;
    let limit = u64::try_from(token.len().saturating_add(1)).unwrap_or(u64::MAX);
    let mut contents = Vec::with_capacity(token.len().saturating_add(1));
    file.take(limit).read_to_end(&mut contents)?;
    if contents != token.as_bytes() {
        return Err(RToolsError::path_policy_violation(
            "open output reservation ownership changed",
        ));
    }
    Ok(())
}

fn retire_reservation(path: &Path, token: &str) -> RToolsResult<()> {
    retire_reservation_with_hook(path, token, |_| Ok(()))
}

fn retire_reservation_with_hook<F>(path: &Path, token: &str, after_claim: F) -> RToolsResult<()>
where
    F: FnOnce(&Path) -> RToolsResult<()>,
{
    let retirement = sibling_path(path, &format!(".rtools-{token}.retired"))?;
    if path_entry_exists(&retirement)? {
        return Err(RToolsError::path_policy_violation(format!(
            "reservation retirement path already exists: {}",
            retirement.display()
        )));
    }

    fs::rename(path, &retirement).map_err(|error| {
        RToolsError::path_policy_violation(format!(
            "unable to atomically claim output reservation {}: {error}",
            path.display()
        ))
    })?;
    after_claim(&retirement)?;

    if let Err(ownership_error) = ensure_lock_owned(&retirement, token) {
        match publish_no_replace(&retirement, path) {
            Ok(()) => {}
            Err(RToolsError::OutputExists(_)) => {
                return Err(RToolsError::path_policy_violation(format!(
                    "claimed foreign reservation remains preserved at {} because {} is occupied",
                    retirement.display(),
                    path.display()
                )));
            }
            Err(restore_error) => {
                return Err(RToolsError::rollback_failed(format!(
                    "foreign reservation at {} could not be restored to {}: {restore_error}",
                    retirement.display(),
                    path.display()
                )));
            }
        }
        return Err(ownership_error);
    }

    fs::remove_file(retirement)?;
    Ok(())
}

fn publish_no_replace(source: &Path, destination: &Path) -> RToolsResult<()> {
    publish_no_replace_using(
        source,
        destination,
        |source, destination| fs::hard_link(source, destination),
        |path| fs::remove_file(path),
    )
}

fn publish_no_replace_using<L, U>(
    source: &Path,
    destination: &Path,
    link: L,
    unlink: U,
) -> RToolsResult<()>
where
    L: FnOnce(&Path, &Path) -> std::io::Result<()>,
    U: FnOnce(&Path) -> std::io::Result<()>,
{
    if let Err(error) = link(source, destination) {
        return if error.kind() == std::io::ErrorKind::AlreadyExists {
            Err(RToolsError::output_exists(
                destination.display().to_string(),
            ))
        } else {
            Err(error.into())
        };
    }

    // The successful link is the commit point. Cleanup of the private link is
    // best-effort so a final-path writer can never have its entry rolled back
    // by pathname; PendingOutput::drop retries its owned temporary instead.
    let _ = unlink(source);
    Ok(())
}

#[cfg(any(windows, test))]
fn replace_with_backup_using<R, D>(
    temporary: &Path,
    destination: &Path,
    backup: &Path,
    mut rename: R,
    mut remove: D,
) -> RToolsResult<()>
where
    R: FnMut(&Path, &Path) -> std::io::Result<()>,
    D: FnMut(&Path) -> std::io::Result<()>,
{
    rename(destination, backup)?;
    if let Err(commit_error) = rename(temporary, destination) {
        if let Err(restore_error) = rename(backup, destination) {
            return Err(RToolsError::rollback_failed(format!(
                "output commit failed ({commit_error}); restoring {} failed: {restore_error}",
                destination.display()
            )));
        }
        return Err(commit_error.into());
    }

    if let Err(cleanup_error) = remove(backup) {
        if let Err(remove_new_error) = remove(destination) {
            return Err(RToolsError::rollback_failed(format!(
                "backup cleanup failed ({cleanup_error}); removing new output failed: {remove_new_error}"
            )));
        }
        if let Err(restore_error) = rename(backup, destination) {
            return Err(RToolsError::rollback_failed(format!(
                "backup cleanup failed ({cleanup_error}); restoring old output failed: {restore_error}"
            )));
        }
        return Err(cleanup_error.into());
    }

    Ok(())
}

#[cfg(test)]
mod path_validation_tests {
    use crate::ErrorCode;
    use std::path::Path;

    #[test]
    fn windows_drive_relative_output_is_rejected_portably_without_filesystem_side_effects() {
        let output = Path::new("Q:rtools-drive-relative-output-policy-test.pdf");

        let error = super::validate_output_parent(output).unwrap_err();

        assert_eq!(error.code(), ErrorCode::PathPolicyViolation);
    }

    #[cfg(windows)]
    #[test]
    fn windows_root_relative_output_is_rejected_without_using_the_current_drive() {
        let output = Path::new(r"\rtools-root-relative-output-policy-test.pdf");

        let error = super::validate_output_parent(output).unwrap_err();

        assert_eq!(error.code(), ErrorCode::PathPolicyViolation);
    }
}

#[cfg(test)]
mod atomic_output_tests {
    use super::{OutputPolicy, PendingOutput, RToolsError};
    use crate::ErrorCode;
    use std::fs;

    fn rtools_artifacts(directory: &std::path::Path) -> Vec<std::ffi::OsString> {
        fs::read_dir(directory)
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .filter(|name| name.to_string_lossy().contains("rtools"))
            .collect()
    }

    #[test]
    fn no_replace_publication_preserves_existing_destination() {
        let directory = tempfile::tempdir().unwrap();
        let temporary = directory.path().join("temporary.bin");
        let output = directory.path().join("result.bin");
        fs::write(&temporary, b"ours").unwrap();
        fs::write(&output, b"external").unwrap();

        let error = super::publish_no_replace(&temporary, &output).unwrap_err();

        assert_eq!(error.code(), ErrorCode::OutputExists);
        assert_eq!(fs::read(&output).unwrap(), b"external");
        assert_eq!(fs::read(&temporary).unwrap(), b"ours");
    }

    #[test]
    fn published_output_is_never_rolled_back_after_temporary_unlink_failure() {
        let directory = tempfile::tempdir().unwrap();
        let temporary = directory.path().join("temporary.bin");
        let output = directory.path().join("result.bin");
        fs::write(&temporary, b"ours").unwrap();
        let mut unlink_calls = Vec::new();

        super::publish_no_replace_using(
            &temporary,
            &output,
            |source, destination| fs::hard_link(source, destination),
            |path| {
                unlink_calls.push(path.to_owned());
                if path == temporary {
                    fs::remove_file(&output)?;
                    fs::write(&output, b"foreign")?;
                    return Err(std::io::Error::other("injected temporary unlink failure"));
                }
                fs::remove_file(path)
            },
        )
        .unwrap();

        assert_eq!(unlink_calls.len(), 1);
        assert_eq!(unlink_calls[0], temporary);
        assert_eq!(fs::read(&output).unwrap(), b"foreign");
        assert_eq!(fs::read(&temporary).unwrap(), b"ours");
    }

    #[test]
    fn normal_no_replace_publication_removes_temporary() {
        let directory = tempfile::tempdir().unwrap();
        let temporary = directory.path().join("temporary.bin");
        let output = directory.path().join("result.bin");
        fs::write(&temporary, b"ours").unwrap();

        super::publish_no_replace(&temporary, &output).unwrap();

        assert_eq!(fs::read(&output).unwrap(), b"ours");
        assert!(!temporary.exists());
    }

    #[test]
    fn destination_created_after_recheck_is_preserved_by_publication_primitive() {
        let directory = tempfile::tempdir().unwrap();
        let output = directory.path().join("result.bin");
        let pending = PendingOutput::new(&output, OutputPolicy::FailIfExists).unwrap();
        fs::write(pending.temporary_path(), b"ours").unwrap();

        let error = pending
            .commit_with_pre_publish_hook(
                |_| Ok(()),
                || {
                    fs::write(&output, b"external").map_err(RToolsError::from)?;
                    Ok(())
                },
            )
            .unwrap_err();

        assert_eq!(error.code(), ErrorCode::OutputExists);
        assert_eq!(fs::read(output).unwrap(), b"external");
        assert!(rtools_artifacts(directory.path()).is_empty());
    }

    #[test]
    fn second_writer_can_publish_after_retirement_without_being_overwritten() {
        let directory = tempfile::tempdir().unwrap();
        let output = directory.path().join("result.bin");
        let first = PendingOutput::new(&output, OutputPolicy::FailIfExists).unwrap();
        fs::write(first.temporary_path(), b"first").unwrap();

        let error = first
            .commit_with_retired_reservation_hook(
                |_| Ok(()),
                || {
                    let second = PendingOutput::new(&output, OutputPolicy::FailIfExists).unwrap();
                    fs::write(second.temporary_path(), b"second").unwrap();
                    second.commit(|_| Ok(()))?;
                    Ok(())
                },
            )
            .unwrap_err();

        assert_eq!(error.code(), ErrorCode::OutputExists);
        assert_eq!(fs::read(&output).unwrap(), b"second");
        assert!(rtools_artifacts(directory.path()).is_empty());
    }

    #[test]
    fn foreign_reservation_created_after_retirement_is_preserved() {
        let directory = tempfile::tempdir().unwrap();
        let output = directory.path().join("result.bin");
        let reservation = directory.path().join(".result.bin.rtools.lock");
        let pending = PendingOutput::new(&output, OutputPolicy::FailIfExists).unwrap();
        fs::write(pending.temporary_path(), b"ours").unwrap();

        let error = pending
            .commit_with_retired_reservation_hook(
                |_| Ok(()),
                || {
                    fs::write(&reservation, b"foreign-owner")?;
                    Ok(())
                },
            )
            .unwrap_err();

        assert_eq!(error.code(), ErrorCode::OutputExists);
        assert!(!output.exists());
        assert_eq!(fs::read(&reservation).unwrap(), b"foreign-owner");
        let artifacts = rtools_artifacts(directory.path());
        assert_eq!(
            artifacts,
            vec![reservation.file_name().unwrap().to_os_string()]
        );
    }

    #[test]
    fn ownership_swap_after_final_verification_aborts_before_publication() {
        let directory = tempfile::tempdir().unwrap();
        let output = directory.path().join("result.bin");
        let reservation = directory.path().join(".result.bin.rtools.lock");
        let pending = PendingOutput::new(&output, OutputPolicy::FailIfExists).unwrap();
        fs::write(pending.temporary_path(), b"ours").unwrap();

        let error = pending
            .commit_with_pre_publish_hook(
                |_| Ok(()),
                || {
                    fs::remove_file(&reservation)?;
                    fs::write(&reservation, b"foreign-owner")?;
                    Ok(())
                },
            )
            .unwrap_err();

        assert_eq!(error.code(), ErrorCode::PathPolicyViolation);
        assert!(!output.exists());
        assert_eq!(fs::read(reservation).unwrap(), b"foreign-owner");
    }

    #[test]
    fn retirement_claim_never_deletes_entry_swapped_after_atomic_move() {
        let directory = tempfile::tempdir().unwrap();
        let reservation = directory.path().join(".result.bin.rtools.lock");
        let saved_owner = directory.path().join("saved-owner.lock");
        fs::write(&reservation, b"owner-token").unwrap();

        let error =
            super::retire_reservation_with_hook(&reservation, "owner-token", |retirement| {
                fs::rename(retirement, &saved_owner)?;
                fs::write(retirement, b"foreign-owner")?;
                Ok(())
            })
            .unwrap_err();

        assert_eq!(error.code(), ErrorCode::PathPolicyViolation);
        assert_eq!(fs::read(&reservation).unwrap(), b"foreign-owner");
        assert_eq!(fs::read(saved_owner).unwrap(), b"owner-token");
    }

    #[test]
    fn overwrite_backup_restores_old_output_when_final_rename_fails() {
        let directory = tempfile::tempdir().unwrap();
        let temporary = directory.path().join("temporary.bin");
        let output = directory.path().join("result.bin");
        let backup = directory.path().join("backup.bin");
        fs::write(&temporary, b"new").unwrap();
        fs::write(&output, b"old").unwrap();
        let mut rename_calls = 0;

        let error = super::replace_with_backup_using(
            &temporary,
            &output,
            &backup,
            |source, destination| {
                rename_calls += 1;
                if rename_calls == 2 {
                    Err(std::io::Error::other("injected final rename failure"))
                } else {
                    fs::rename(source, destination)
                }
            },
            |path| fs::remove_file(path),
        )
        .unwrap_err();

        assert_eq!(error.code(), ErrorCode::ProcessingFailed);
        assert_eq!(fs::read(&output).unwrap(), b"old");
        assert_eq!(fs::read(&temporary).unwrap(), b"new");
        assert!(!backup.exists());
    }

    #[test]
    fn overwrite_backup_cleanup_failure_rolls_back_new_output() {
        let directory = tempfile::tempdir().unwrap();
        let temporary = directory.path().join("temporary.bin");
        let output = directory.path().join("result.bin");
        let backup = directory.path().join("backup.bin");
        fs::write(&temporary, b"new").unwrap();
        fs::write(&output, b"old").unwrap();
        let mut remove_calls = 0;

        let error = super::replace_with_backup_using(
            &temporary,
            &output,
            &backup,
            |source, destination| fs::rename(source, destination),
            |path| {
                remove_calls += 1;
                if remove_calls == 1 {
                    Err(std::io::Error::other("injected backup cleanup failure"))
                } else {
                    fs::remove_file(path)
                }
            },
        )
        .unwrap_err();

        assert_eq!(error.code(), ErrorCode::ProcessingFailed);
        assert_eq!(fs::read(&output).unwrap(), b"old");
        assert!(!temporary.exists());
        assert!(!backup.exists());
    }

    #[test]
    fn overwrite_backup_reports_rollback_failure_and_preserves_backup() {
        let directory = tempfile::tempdir().unwrap();
        let temporary = directory.path().join("temporary.bin");
        let output = directory.path().join("result.bin");
        let backup = directory.path().join("backup.bin");
        fs::write(&temporary, b"new").unwrap();
        fs::write(&output, b"old").unwrap();
        let mut rename_calls = 0;

        let error = super::replace_with_backup_using(
            &temporary,
            &output,
            &backup,
            |source, destination| {
                rename_calls += 1;
                if rename_calls >= 2 {
                    Err(std::io::Error::other("injected rename failure"))
                } else {
                    fs::rename(source, destination)
                }
            },
            |path| fs::remove_file(path),
        )
        .unwrap_err();

        assert_eq!(error.code(), ErrorCode::RollbackFailed);
        assert!(!output.exists());
        assert_eq!(fs::read(&backup).unwrap(), b"old");
        assert_eq!(fs::read(&temporary).unwrap(), b"new");
    }
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
    /// Deterministic, user-visible warnings produced while creating the output.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
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
            warnings: Vec::new(),
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
            warnings: Vec::new(),
        }
    }

    /// Create a new bytes output
    pub const fn to_bytes() -> Self {
        Self {
            destination: OutputDestination::Bytes(Vec::new()),
            name: None,
            mime_type: None,
            stats: None,
            warnings: Vec::new(),
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
