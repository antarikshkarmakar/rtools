use rtools_core::{OutputPolicy, PendingOutput, RToolsError, RToolsResult};
use std::collections::HashSet;
use std::ffi::OsStr;
use std::path::{Component, Path, PathBuf};

/// A lexical, case-normalized identity for destinations that must be portable
/// to case-insensitive filesystems.
///
/// Components use Unicode `to_lowercase` and remove lexical `.`/`..` pairs.
/// Non-Unicode components are rejected rather than passed through
/// `to_string_lossy`, which could make distinct byte paths compare as one name.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PortableDestinationKey(Vec<PortablePathComponent>);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum PortablePathComponent {
    Prefix(String),
    RootDir,
    ParentDir,
    Normal(String),
}

pub fn portable_destination_key(path: &Path) -> RToolsResult<PortableDestinationKey> {
    let mut normalized = Vec::new();
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(PortablePathComponent::Prefix(
                normalized_component(prefix.as_os_str(), path)?,
            )),
            Component::RootDir => normalized.push(PortablePathComponent::RootDir),
            Component::CurDir => {}
            Component::ParentDir => match normalized.last() {
                Some(PortablePathComponent::Normal(_)) => {
                    normalized.pop();
                }
                _ => normalized.push(PortablePathComponent::ParentDir),
            },
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
    let mut current = PathBuf::from(".");
    let mut has_normal_component = false;

    for component in path.components() {
        match component {
            Component::Prefix(prefix) => current = PathBuf::from(prefix.as_os_str()),
            Component::RootDir | Component::ParentDir => current.push(component.as_os_str()),
            Component::CurDir => {}
            Component::Normal(requested) => {
                has_normal_component = true;
                let candidate = current.join(requested);
                if path_entry_exists(&candidate)? {
                    current = candidate;
                    continue;
                }

                let requested_key = normalized_component(requested, path)?;
                let mut case_alias = None;
                match std::fs::read_dir(&current) {
                    Ok(entries) => {
                        for entry in entries {
                            let entry = entry?;
                            if normalized_component(&entry.file_name(), &entry.path())?
                                == requested_key
                            {
                                case_alias = Some(entry.path());
                                break;
                            }
                        }
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
                    Err(error) => return Err(error.into()),
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

fn normalized_component(component: &OsStr, path: &Path) -> RToolsResult<String> {
    component.to_str().map(str::to_lowercase).ok_or_else(|| {
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
