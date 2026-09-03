use rtools_core::{ErrorCode, OutputPolicy, PendingOutput, RToolsError};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Barrier};
use {derive_more as _, dirs as _, figment as _, serde as _, serde_json as _, thiserror as _};
use {toml as _, tracing as _};

fn write_and_commit(output: &Path, policy: OutputPolicy, contents: &[u8]) -> PathBuf {
    let pending = PendingOutput::new(output, policy).unwrap();
    fs::write(pending.temporary_path(), contents).unwrap();
    pending
        .commit(|artifact| {
            if fs::metadata(artifact)?.len() == 0 {
                return Err(RToolsError::invalid_input("artifact is empty"));
            }
            Ok(())
        })
        .unwrap()
}

fn reservation_path(directory: &Path, temporary: &Path) -> PathBuf {
    fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| path != temporary && fs::metadata(path).unwrap().len() > 0)
        .expect("reservation lock should exist beside the temporary file")
}

fn assert_no_rtools_artifacts(directory: &Path) {
    let leftovers: Vec<_> = fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .filter(|name| name.to_string_lossy().contains("rtools"))
        .collect();
    assert!(leftovers.is_empty(), "leftover artifacts: {leftovers:?}");
}

#[test]
fn fail_if_exists_never_changes_existing_file() {
    let directory = tempfile::tempdir().unwrap();
    let output = directory.path().join("result.bin");
    fs::write(&output, b"original").unwrap();

    let error = PendingOutput::new(&output, OutputPolicy::FailIfExists).unwrap_err();

    assert_eq!(error.code(), ErrorCode::OutputExists);
    assert_eq!(fs::read(&output).unwrap(), b"original");
    assert_no_rtools_artifacts(directory.path());
}

#[cfg(unix)]
#[test]
fn fail_if_exists_treats_dangling_symlink_as_an_existing_entry() {
    use std::os::unix::fs::symlink;

    let directory = tempfile::tempdir().unwrap();
    let output = directory.path().join("result.bin");
    let target = Path::new("missing-target.bin");
    symlink(target, &output).unwrap();

    let error = PendingOutput::new(&output, OutputPolicy::FailIfExists).unwrap_err();

    assert_eq!(error.code(), ErrorCode::OutputExists);
    assert_eq!(fs::read_link(&output).unwrap(), target);
}

#[test]
fn dropping_pending_output_removes_sibling_temporary_and_owned_lock() {
    let directory = tempfile::tempdir().unwrap();
    let output = directory.path().join("result.bin");

    let pending = PendingOutput::new(&output, OutputPolicy::FailIfExists).unwrap();
    let temporary = pending.temporary_path().to_owned();
    assert_eq!(temporary.parent(), Some(directory.path()));
    assert!(temporary.exists());
    drop(pending);

    assert!(!temporary.exists());
    assert!(!output.exists());
    assert_no_rtools_artifacts(directory.path());
}

#[test]
fn missing_parent_is_rejected() {
    let directory = tempfile::tempdir().unwrap();
    let output = directory.path().join("missing").join("result.bin");

    let error = PendingOutput::new(&output, OutputPolicy::FailIfExists).unwrap_err();

    assert_eq!(error.code(), ErrorCode::PathPolicyViolation);
    assert!(!output.parent().unwrap().exists());
}

#[test]
fn non_directory_parent_is_rejected() {
    let directory = tempfile::tempdir().unwrap();
    let parent = directory.path().join("not-a-directory");
    fs::write(&parent, b"file").unwrap();

    let error =
        PendingOutput::new(parent.join("result.bin"), OutputPolicy::FailIfExists).unwrap_err();

    assert_eq!(error.code(), ErrorCode::PathPolicyViolation);
    assert_eq!(fs::read(parent).unwrap(), b"file");
}

#[test]
fn unicode_filename_commits_exact_bytes() {
    let directory = tempfile::tempdir().unwrap();
    let output = directory.path().join("结果-🌌.bin");

    let committed = write_and_commit(&output, OutputPolicy::FailIfExists, b"unicode");

    assert_eq!(committed, output);
    assert_eq!(fs::read(&committed).unwrap(), b"unicode");
    assert_no_rtools_artifacts(directory.path());
}

#[test]
fn concurrent_fail_if_exists_reservations_have_one_winner() {
    let directory = tempfile::tempdir().unwrap();
    let output = Arc::new(directory.path().join("result.bin"));
    let barrier = Arc::new(Barrier::new(3));

    let handles: [_; 2] = std::array::from_fn(|_| {
        let output = Arc::clone(&output);
        let barrier = Arc::clone(&barrier);
        std::thread::spawn(move || {
            barrier.wait();
            let result = PendingOutput::new(output.as_path(), OutputPolicy::FailIfExists);
            barrier.wait();
            result
        })
    });

    barrier.wait();
    barrier.wait();
    let results: Vec<_> = handles
        .into_iter()
        .map(|handle| handle.join().unwrap())
        .collect();
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter_map(|result| result.as_ref().err())
            .filter(|error| error.code() == ErrorCode::OutputExists)
            .count(),
        1
    );
    drop(results);
    assert_no_rtools_artifacts(directory.path());
}

#[test]
fn stale_reservation_blocks_new_writer_and_is_never_deleted() {
    let directory = tempfile::tempdir().unwrap();
    let output = directory.path().join("result.bin");
    let reservation = directory.path().join(".result.bin.rtools.lock");
    fs::write(&reservation, b"stale-foreign-owner").unwrap();

    let error = PendingOutput::new(&output, OutputPolicy::FailIfExists).unwrap_err();

    assert_eq!(error.code(), ErrorCode::OutputExists);
    assert!(!output.exists());
    assert_eq!(fs::read(reservation).unwrap(), b"stale-foreign-owner");
}

#[test]
fn unique_name_skips_existing_and_reserved_candidates() {
    let directory = tempfile::tempdir().unwrap();
    let output = directory.path().join("result.bin");
    fs::write(&output, b"original").unwrap();
    let first = PendingOutput::new(&output, OutputPolicy::UniqueName).unwrap();
    let second = PendingOutput::new(&output, OutputPolicy::UniqueName).unwrap();

    fs::write(first.temporary_path(), b"first").unwrap();
    fs::write(second.temporary_path(), b"second").unwrap();
    let first_path = first.commit(|_| Ok(())).unwrap();
    let second_path = second.commit(|_| Ok(())).unwrap();

    assert_eq!(first_path, directory.path().join("result_1.bin"));
    assert_eq!(second_path, directory.path().join("result_2.bin"));
    assert_eq!(fs::read(output).unwrap(), b"original");
    assert_eq!(fs::read(first_path).unwrap(), b"first");
    assert_eq!(fs::read(second_path).unwrap(), b"second");
    assert_no_rtools_artifacts(directory.path());
}

#[cfg(unix)]
#[test]
fn unique_name_preserves_dangling_symlink_and_commits_to_suffix() {
    use std::os::unix::fs::symlink;

    let directory = tempfile::tempdir().unwrap();
    let output = directory.path().join("result.bin");
    let target = Path::new("missing-target.bin");
    symlink(target, &output).unwrap();

    let committed = write_and_commit(&output, OutputPolicy::UniqueName, b"ours");

    assert_eq!(committed, directory.path().join("result_1.bin"));
    assert_eq!(fs::read_link(&output).unwrap(), target);
    assert_eq!(fs::read(committed).unwrap(), b"ours");
}

#[test]
fn unique_name_suffix_exhaustion_fails_without_fallback() {
    let directory = tempfile::tempdir().unwrap();
    let output = directory.path().join("result.bin");
    fs::write(&output, b"original").unwrap();
    for suffix in 1..=999 {
        fs::write(
            directory.path().join(format!("result_{suffix}.bin")),
            b"occupied",
        )
        .unwrap();
    }

    let error = PendingOutput::new(&output, OutputPolicy::UniqueName).unwrap_err();

    assert_eq!(error.code(), ErrorCode::OutputExists);
    assert_eq!(fs::read(output).unwrap(), b"original");
    assert_no_rtools_artifacts(directory.path());
}

#[test]
fn overwrite_preserves_old_bytes_until_commit_then_replaces_them() {
    let directory = tempfile::tempdir().unwrap();
    let output = directory.path().join("result.bin");
    fs::write(&output, b"original").unwrap();
    let pending = PendingOutput::new(&output, OutputPolicy::Overwrite).unwrap();
    fs::write(pending.temporary_path(), b"replacement").unwrap();

    assert_eq!(fs::read(&output).unwrap(), b"original");
    let committed = pending.commit(|_| Ok(())).unwrap();

    assert_eq!(committed, output);
    assert_eq!(fs::read(committed).unwrap(), b"replacement");
    assert_no_rtools_artifacts(directory.path());
}

#[test]
fn validator_failure_removes_temporary_and_preserves_existing_output() {
    let directory = tempfile::tempdir().unwrap();
    let output = directory.path().join("result.bin");
    fs::write(&output, b"original").unwrap();
    let pending = PendingOutput::new(&output, OutputPolicy::Overwrite).unwrap();
    let temporary = pending.temporary_path().to_owned();
    fs::write(&temporary, b"invalid").unwrap();

    let error = pending
        .commit(|_| Err(RToolsError::invalid_input("invalid artifact")))
        .unwrap_err();

    assert_eq!(error.code(), ErrorCode::InvalidInput);
    assert_eq!(fs::read(output).unwrap(), b"original");
    assert!(!temporary.exists());
    assert_no_rtools_artifacts(directory.path());
}

#[test]
fn destination_created_after_reservation_is_not_overwritten() {
    let directory = tempfile::tempdir().unwrap();
    let output = directory.path().join("result.bin");
    let pending = PendingOutput::new(&output, OutputPolicy::FailIfExists).unwrap();
    fs::write(pending.temporary_path(), b"ours").unwrap();
    fs::write(&output, b"external").unwrap();

    let error = pending.commit(|_| Ok(())).unwrap_err();

    assert_eq!(error.code(), ErrorCode::OutputExists);
    assert_eq!(fs::read(output).unwrap(), b"external");
    assert_no_rtools_artifacts(directory.path());
}

#[test]
fn changed_reservation_fails_commit_closed_and_preserves_foreign_lock() {
    let directory = tempfile::tempdir().unwrap();
    let output = directory.path().join("result.bin");
    let pending = PendingOutput::new(&output, OutputPolicy::FailIfExists).unwrap();
    let temporary = pending.temporary_path().to_owned();
    let reservation = reservation_path(directory.path(), &temporary);
    fs::write(&temporary, b"ours").unwrap();
    fs::write(&reservation, b"foreign-owner").unwrap();

    let error = pending.commit(|_| Ok(())).unwrap_err();

    assert_eq!(error.code(), ErrorCode::PathPolicyViolation);
    assert!(!output.exists());
    assert!(!temporary.exists());
    assert_eq!(fs::read(&reservation).unwrap(), b"foreign-owner");
}

#[test]
fn missing_reservation_fails_commit_closed_without_publishing_output() {
    let directory = tempfile::tempdir().unwrap();
    let output = directory.path().join("result.bin");
    let pending = PendingOutput::new(&output, OutputPolicy::FailIfExists).unwrap();
    let temporary = pending.temporary_path().to_owned();
    let reservation = reservation_path(directory.path(), &temporary);
    fs::write(&temporary, b"ours").unwrap();
    fs::remove_file(reservation).unwrap();

    let error = pending.commit(|_| Ok(())).unwrap_err();

    assert_eq!(error.code(), ErrorCode::PathPolicyViolation);
    assert!(!output.exists());
    assert!(!temporary.exists());
}

#[test]
fn drop_never_deletes_a_changed_reservation() {
    let directory = tempfile::tempdir().unwrap();
    let output = directory.path().join("result.bin");
    let pending = PendingOutput::new(&output, OutputPolicy::FailIfExists).unwrap();
    let temporary = pending.temporary_path().to_owned();
    let reservation = reservation_path(directory.path(), &temporary);
    fs::write(&reservation, b"foreign-owner").unwrap();

    drop(pending);

    assert!(!temporary.exists());
    assert_eq!(fs::read(reservation).unwrap(), b"foreign-owner");
}

#[test]
fn legacy_file_output_without_warnings_deserializes_to_an_empty_list() {
    let value = serde_json::json!({
        "destination": { "File": "result.png" },
        "name": "result.png",
        "mime_type": "image/png",
        "stats": null
    });

    let output: rtools_core::FileOutput = serde_json::from_value(value).unwrap();

    assert!(output.warnings.is_empty());
}

#[test]
fn empty_file_output_warnings_are_omitted_from_serialized_output() {
    let output = rtools_core::FileOutput::to_file(PathBuf::from("result.png"));
    let value = serde_json::to_value(output).unwrap();

    assert!(value.get("warnings").is_none());
}
