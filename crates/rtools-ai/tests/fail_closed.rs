#![allow(unused_crate_dependencies)]

use rtools_ai::organize::{OrganizeConfig, OrganizeStrategy};
use rtools_ai::rename::RenameConfig;
use rtools_ai::sort::{SortConfig, SortProcessor};
use rtools_ai::{OrganizeProcessor, RenameProcessor};
#[cfg(unix)]
use rtools_core::OutputDestination;
use rtools_core::{ErrorCode, FileInput, Processor, RToolsError};

fn assert_unavailable(error: &RToolsError, operation: &str) {
    assert_eq!(error.code(), ErrorCode::CapabilityUnavailable);
    assert!(matches!(
        error,
        RToolsError::CapabilityUnavailable { operation_id, .. } if operation_id == operation
    ));
}

#[test]
fn sort_fails_closed_before_mutation_in_dry_and_live_modes() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("photo.jpg");
    std::fs::write(&source, b"source").unwrap();

    for dry_run in [true, false] {
        let output = temp.path().join(if dry_run { "dry" } else { "live" });
        let error = SortProcessor
            .process(
                vec![FileInput::from_path(source.clone())],
                SortConfig {
                    output_dir: output.clone(),
                    dry_run,
                    ..SortConfig::default()
                },
            )
            .unwrap_err();

        assert_unavailable(&error, "ai.sort");
        assert!(!output.exists());
        assert_eq!(std::fs::read(&source).unwrap(), b"source");
    }
}

#[test]
fn rename_rejects_empty_inputs_before_output_mutation() {
    let temp = tempfile::tempdir().unwrap();
    let output = temp.path().join("renamed");
    let error = RenameProcessor
        .process(
            Vec::new(),
            RenameConfig {
                output_dir: Some(output.clone()),
                ..RenameConfig::default()
            },
        )
        .unwrap_err();

    assert_eq!(error.code(), ErrorCode::InvalidInput);
    assert!(!output.exists());
}

#[test]
fn organize_rejects_ignored_mode_flags_before_input_or_output_access() {
    let temp = tempfile::tempdir().unwrap();
    for (by_date, by_subject) in [(false, false), (true, true), (false, true)] {
        let output = temp.path().join(format!("out-{by_date}-{by_subject}"));
        let error = OrganizeProcessor
            .process(
                vec![FileInput::from_path(temp.path().join("missing.jpg"))],
                OrganizeConfig {
                    output_dir: output.clone(),
                    strategy: OrganizeStrategy::ByDate,
                    by_date,
                    by_subject,
                    dry_run: false,
                },
            )
            .unwrap_err();

        assert_eq!(error.code(), ErrorCode::InvalidInput);
        assert!(!output.exists());
    }
}

#[test]
fn live_organize_requires_prepared_target_directory_without_partial_creation() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("photo.jpg");
    std::fs::write(&source, b"source").unwrap();
    let output = temp.path().join("organized");
    let error = OrganizeProcessor
        .process(
            vec![FileInput::from_path(source.clone())],
            OrganizeConfig {
                output_dir: output.clone(),
                strategy: OrganizeStrategy::ByDate,
                by_date: true,
                by_subject: false,
                dry_run: false,
            },
        )
        .unwrap_err();

    assert_eq!(error.code(), ErrorCode::PathPolicyViolation);
    assert!(!output.exists());
    assert_eq!(std::fs::read(source).unwrap(), b"source");
}

#[cfg(unix)]
#[test]
fn organize_rejects_symlinked_output_ancestor_without_outside_artifacts() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("photo.jpg");
    std::fs::write(&source, b"source").unwrap();
    let outside = temp.path().join("outside");
    std::fs::create_dir(&outside).unwrap();
    let linked = temp.path().join("linked");
    std::os::unix::fs::symlink(&outside, &linked).unwrap();

    let planned = OrganizeProcessor
        .process(
            vec![FileInput::from_path(source.clone())],
            OrganizeConfig {
                output_dir: linked.clone(),
                strategy: OrganizeStrategy::ByDate,
                by_date: true,
                by_subject: false,
                dry_run: true,
            },
        )
        .unwrap();
    let relative = match &planned[0].destination {
        OutputDestination::File(path) => path.strip_prefix(&linked).unwrap(),
        other => panic!("expected file output, got {other:?}"),
    };
    let expected_outside = outside.join(relative);

    let error = OrganizeProcessor
        .process(
            vec![FileInput::from_path(source.clone())],
            OrganizeConfig {
                output_dir: linked,
                strategy: OrganizeStrategy::ByDate,
                by_date: true,
                by_subject: false,
                dry_run: false,
            },
        )
        .unwrap_err();

    assert_eq!(error.code(), ErrorCode::PathPolicyViolation);
    assert!(!expected_outside.exists());
    assert_eq!(std::fs::read_dir(&outside).unwrap().count(), 0);
    assert_eq!(std::fs::read(source).unwrap(), b"source");
}
