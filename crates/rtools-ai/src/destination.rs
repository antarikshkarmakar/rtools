use rtools_core::{OutputPolicy, PendingOutput, RToolsError, RToolsResult};
use std::collections::HashSet;
use std::ffi::OsStr;
use std::path::{Component, Path, PathBuf};
use unicode_casefold::{Locale, UnicodeCaseFold, Variant};

/// A lexical, case-normalized identity for destinations that must be portable
/// to case-insensitive filesystems.
///
/// Relative paths are anchored to the process working directory. Components use
/// deterministic full Unicode case folding, including expanding mappings, and
/// remove lexical `.`/`..` pairs, clamping parent traversal at a root.
/// Non-Unicode components are rejected rather than passed through
/// `to_string_lossy`, which could make distinct byte paths compare as one name.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PortableDestinationKey(Vec<PortablePathComponent>);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum PortablePathComponent {
    Prefix(String),
    RootDir,
    Normal(String),
}

pub fn portable_destination_key(path: &Path) -> RToolsResult<PortableDestinationKey> {
    portable_destination_key_with_base(path, &std::env::current_dir()?)
}

fn portable_destination_key_with_base(
    path: &Path,
    base: &Path,
) -> RToolsResult<PortableDestinationKey> {
    reject_windows_drive_relative(path)?;
    let anchored = if path.is_absolute() {
        path.to_path_buf()
    } else {
        base.join(path)
    };
    let mut normalized = Vec::new();
    for component in anchored.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(PortablePathComponent::Prefix(
                normalized_component(prefix.as_os_str(), path)?,
            )),
            Component::RootDir => normalized.push(PortablePathComponent::RootDir),
            Component::CurDir => {}
            Component::ParentDir => {
                if let Some(PortablePathComponent::Normal(_)) = normalized.last() {
                    normalized.pop();
                }
            }
            Component::Normal(component) => normalized.push(PortablePathComponent::Normal(
                normalized_component(component, path)?,
            )),
        }
    }
    Ok(PortableDestinationKey(normalized))
}

/// Return true when a destination or any existing case-only path alias exists.
///
/// Every traversed directory entry must be Unicode. This is deliberately
/// fail-closed: skipping a non-Unicode entry or rendering it lossily would make
/// a portable collision check unable to justify a safe result.
pub fn destination_or_case_alias_exists(path: &Path) -> RToolsResult<bool> {
    destination_or_case_alias_exists_with_base(path, &std::env::current_dir()?)
}

fn destination_or_case_alias_exists_with_base(path: &Path, base: &Path) -> RToolsResult<bool> {
    reject_windows_drive_relative(path)?;
    let mut current = if path.is_absolute() {
        PathBuf::new()
    } else {
        base.to_path_buf()
    };
    let mut has_normal_component = false;

    for component in path.components() {
        match component {
            Component::Prefix(prefix) => current = PathBuf::from(prefix.as_os_str()),
            Component::RootDir | Component::ParentDir => current.push(component.as_os_str()),
            Component::CurDir => {}
            Component::Normal(requested) => {
                has_normal_component = true;
                let candidate = current.join(requested);
                let requested_key = normalized_component(requested, path)?;
                let mut case_alias = None;
                let candidate_exists = path_entry_exists(&candidate)?;
                match std::fs::read_dir(&current) {
                    Ok(entries) => {
                        for entry in entries {
                            let entry = entry?;
                            let entry_name = entry.file_name();
                            if normalized_component(&entry_name, &entry.path())? == requested_key {
                                if entry_name.as_os_str() != requested {
                                    return Ok(true);
                                }
                                if !candidate_exists {
                                    case_alias = Some(entry.path());
                                }
                            }
                        }
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
                    Err(error) => return Err(error.into()),
                }

                if candidate_exists {
                    current = candidate;
                    continue;
                }

                let Some(case_alias) = case_alias else {
                    return Ok(false);
                };
                current = case_alias;
            }
        }
    }

    Ok(has_normal_component && path_entry_exists(&current)?)
}

/// Copy to a sibling temporary artifact, then publish with the Task 4
/// no-replace reservation/commit protocol.
pub fn copy_no_replace(source: &Path, destination: &Path) -> RToolsResult<PathBuf> {
    let pending = PendingOutput::new(destination, OutputPolicy::FailIfExists)?;
    std::fs::copy(source, pending.temporary_path())?;
    pending.commit(|_| Ok(()))
}

/// Atomically create a no-replace rename destination that shares the source
/// inode and metadata, then remove the source only after link creation.
///
/// Regular files on filesystems that support hard links preserve rename
/// semantics. Unsupported filesystems/types fail before source deletion. A
/// source-unlink failure leaves a safe duplicate and returns an error rather
/// than risking source loss.
pub fn move_no_replace(source: &Path, destination: &Path) -> RToolsResult<PathBuf> {
    match std::fs::hard_link(source, destination) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            return Err(RToolsError::output_exists(
                destination.display().to_string(),
            ));
        }
        Err(error) => return Err(error.into()),
    }
    std::fs::remove_file(source)?;
    Ok(destination.to_path_buf())
}

pub fn insert_unique_destination(
    destinations: &mut HashSet<PortableDestinationKey>,
    destination: &Path,
) -> RToolsResult<bool> {
    Ok(destinations.insert(portable_destination_key(destination)?))
}

fn reject_windows_drive_relative(path: &Path) -> RToolsResult<()> {
    let Some(path_text) = path.as_os_str().to_str() else {
        return Ok(());
    };
    let bytes = path_text.as_bytes();
    let has_drive_relative_prefix = bytes.first().is_some_and(u8::is_ascii_alphabetic)
        && bytes.get(1) == Some(&b':')
        && !matches!(bytes.get(2), Some(b'/' | b'\\'));
    if has_drive_relative_prefix {
        return Err(RToolsError::path_policy_violation(format!(
            "Windows drive-relative destination paths are not allowed: {}",
            path.display()
        )));
    }
    Ok(())
}

fn normalized_component(component: &OsStr, path: &Path) -> RToolsResult<String> {
    component
        .to_str()
        .map(|component| {
            component
                .case_fold_with(Variant::Full, Locale::NonTurkic)
                .collect()
        })
        .ok_or_else(|| {
            RToolsError::path_policy_violation(format!(
                "portable destination alias checks require Unicode path components: {}",
                path.display()
            ))
        })
}

fn path_entry_exists(path: &Path) -> RToolsResult<bool> {
    match std::fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        destination_or_case_alias_exists, destination_or_case_alias_exists_with_base,
        portable_destination_key_with_base, Path,
    };
    use rtools_core::ErrorCode;

    #[test]
    fn destination_scan_uses_process_cwd_for_a_normal_relative_first_component() {
        let temp = tempfile::tempdir_in(".").unwrap();
        let relative_root = Path::new(temp.path().file_name().unwrap());
        let relative_destination = relative_root.join("existing.jpg");
        std::fs::write(temp.path().join("existing.jpg"), b"existing").unwrap();

        assert!(destination_or_case_alias_exists(&relative_destination).unwrap());
    }

    #[cfg(unix)]
    #[test]
    fn destination_scan_checks_exact_directory_spelling_for_case_alias_siblings() {
        let base = tempfile::tempdir().unwrap();
        let exact_directory = base.path().join("out");
        let alias_directory = base.path().join("Out");
        std::fs::create_dir(&exact_directory).unwrap();
        std::fs::create_dir(&alias_directory).unwrap();
        let relative_destination = Path::new("out").join("missing.jpg");

        assert!(
            destination_or_case_alias_exists_with_base(&relative_destination, base.path()).unwrap()
        );
    }

    #[cfg(unix)]
    #[test]
    fn portable_destination_key_anchors_relative_paths_to_an_injected_base() {
        let base = Path::new("/workspace/base");

        assert_eq!(
            portable_destination_key_with_base(Path::new("out/../same.jpg"), base).unwrap(),
            portable_destination_key_with_base(Path::new("/workspace/base/same.jpg"), base)
                .unwrap()
        );
    }

    #[cfg(unix)]
    #[test]
    fn portable_destination_key_clamps_parent_traversal_at_a_root() {
        let base = Path::new("/workspace/base");

        assert_eq!(
            portable_destination_key_with_base(Path::new("/../../same.jpg"), base).unwrap(),
            portable_destination_key_with_base(Path::new("/same.jpg"), base).unwrap()
        );
    }

    #[test]
    fn destination_helpers_reject_windows_drive_relative_paths() {
        #[cfg(windows)]
        let base = Path::new(r"C:\workspace\base");
        #[cfg(not(windows))]
        let base = Path::new("/workspace/base");
        let drive_relative = Path::new("C:foo");

        let key_error = portable_destination_key_with_base(drive_relative, base).unwrap_err();
        let scan_error =
            destination_or_case_alias_exists_with_base(drive_relative, base).unwrap_err();

        assert_eq!(key_error.code(), ErrorCode::PathPolicyViolation);
        assert_eq!(scan_error.code(), ErrorCode::PathPolicyViolation);
    }
}
