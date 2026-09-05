#![allow(unused_crate_dependencies)]

use rtools_ai::organize::{OrganizeConfig, OrganizeStrategy};
use rtools_ai::rename::RenameConfig;
use rtools_ai::{OrganizeProcessor, RenameProcessor};
use rtools_core::{ErrorCode, FileInput, OutputDestination, Processor};

fn destination(output: &rtools_core::FileOutput) -> &std::path::Path {
    match &output.destination {
        OutputDestination::File(path) => path,
        other => panic!("expected file destination, got {other:?}"),
    }
}

#[test]
fn date_organize_dry_run_creates_no_output_directories() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("photo.jpg");
    std::fs::write(&source, b"fixture").unwrap();
    let output_dir = temp.path().join("organized");

    let outputs = OrganizeProcessor
        .process(
            vec![FileInput::from_path(source)],
            OrganizeConfig {
                output_dir: output_dir.clone(),
                strategy: OrganizeStrategy::ByDate,
                by_date: true,
                by_subject: false,
                dry_run: true,
            },
        )
        .unwrap();

    assert_eq!(outputs.len(), 1);
    assert!(destination(&outputs[0]).starts_with(&output_dir));
    assert!(!output_dir.exists());
}

#[test]
fn date_organize_dry_run_preserves_existing_collision_bytes_and_returns_output_exists() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("photo.jpg");
    std::fs::write(&source, b"source").unwrap();
    let output_dir = temp.path().join("organized");

    let initial = OrganizeProcessor
        .process(
            vec![FileInput::from_path(source.clone())],
            OrganizeConfig {
                output_dir: output_dir.clone(),
                strategy: OrganizeStrategy::ByDate,
                by_date: true,
                by_subject: false,
                dry_run: true,
            },
        )
        .unwrap();
    let occupied = destination(&initial[0]).to_path_buf();
    std::fs::create_dir_all(occupied.parent().unwrap()).unwrap();
    std::fs::write(&occupied, b"occupied").unwrap();

    let error = OrganizeProcessor
        .process(
            vec![FileInput::from_path(source)],
            OrganizeConfig {
                output_dir,
                strategy: OrganizeStrategy::ByDate,
                by_date: true,
                by_subject: false,
                dry_run: true,
            },
        )
        .unwrap_err();

    assert_eq!(error.code(), ErrorCode::OutputExists);
    assert_eq!(std::fs::read(&occupied).unwrap(), b"occupied");
    assert_eq!(
        std::fs::read_dir(occupied.parent().unwrap())
            .unwrap()
            .count(),
        1,
        "collision detection must not create a suffix artifact"
    );
}

#[test]
fn rename_dry_run_preserves_sources_and_returns_output_exists_for_in_run_collision() {
    let temp = tempfile::tempdir().unwrap();
    let first = temp.path().join("first.jpg");
    let second = temp.path().join("second.jpg");
    std::fs::write(&first, b"first").unwrap();
    std::fs::write(&second, b"second").unwrap();

    let error = RenameProcessor
        .process(
            vec![
                FileInput::from_path(first.clone()),
                FileInput::from_path(second.clone()),
            ],
            RenameConfig {
                pattern: "same".to_string(),
                output_dir: None,
                start_number: 1,
                use_ai_descriptions: false,
                dry_run: true,
            },
        )
        .unwrap_err();

    assert_eq!(error.code(), ErrorCode::OutputExists);
    assert_eq!(std::fs::read(first).unwrap(), b"first");
    assert_eq!(std::fs::read(second).unwrap(), b"second");
    assert!(!temp.path().join("same.jpg").exists());
}

#[test]
fn rename_collision_fails_before_mutating_any_source() {
    let temp = tempfile::tempdir().unwrap();
    let first = temp.path().join("first.jpg");
    let second = temp.path().join("second.jpg");
    std::fs::write(&first, b"first").unwrap();
    std::fs::write(&second, b"second").unwrap();

    let error = RenameProcessor
        .process(
            vec![
                FileInput::from_path(first.clone()),
                FileInput::from_path(second.clone()),
            ],
            RenameConfig {
                pattern: "same".to_string(),
                output_dir: None,
                start_number: 1,
                use_ai_descriptions: false,
                dry_run: false,
            },
        )
        .unwrap_err();

    assert_eq!(error.code(), ErrorCode::OutputExists);
    assert_eq!(std::fs::read(first).unwrap(), b"first");
    assert_eq!(std::fs::read(second).unwrap(), b"second");
    assert!(!temp.path().join("same.jpg").exists());
}

#[test]
fn rename_duplicate_input_fails_without_creating_an_artifact() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("photo.jpg");
    std::fs::write(&source, b"source").unwrap();

    let error = RenameProcessor
        .process(
            vec![
                FileInput::from_path(source.clone()),
                FileInput::from_path(source.clone()),
            ],
            RenameConfig {
                pattern: "{name}".to_string(),
                output_dir: None,
                start_number: 1,
                use_ai_descriptions: false,
                dry_run: true,
            },
        )
        .unwrap_err();

    assert_eq!(error.code(), ErrorCode::InvalidInput);
    assert_eq!(std::fs::read(source).unwrap(), b"source");
}

#[test]
fn rename_rejects_superscript_devices_and_long_components_before_mutation() {
    for dry_run in [true, false] {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("source.jpg");
        std::fs::write(&source, b"source").unwrap();
        let output_dir = temp.path().join("out");
        if !dry_run {
            std::fs::create_dir(&output_dir).unwrap();
        }

        for pattern in ["COM¹".to_string(), "a".repeat(256)] {
            let error = RenameProcessor
                .process(
                    vec![FileInput::from_path(source.clone())],
                    RenameConfig {
                        pattern,
                        output_dir: Some(output_dir.clone()),
                        start_number: 1,
                        use_ai_descriptions: false,
                        dry_run,
                    },
                )
                .unwrap_err();

            assert_eq!(error.code(), ErrorCode::InvalidInput);
            assert_eq!(std::fs::read(&source).unwrap(), b"source");
            if output_dir.exists() {
                assert_eq!(std::fs::read_dir(&output_dir).unwrap().count(), 0);
            }
        }
    }
}

#[test]
fn organize_dry_run_rejects_case_only_planned_destination_aliases() {
    let temp = tempfile::tempdir().unwrap();
    let upper_source_dir = temp.path().join("upper-source");
    let lower_source_dir = temp.path().join("lower-source");
    std::fs::create_dir_all(&upper_source_dir).unwrap();
    std::fs::create_dir_all(&lower_source_dir).unwrap();
    let upper_source = upper_source_dir.join("A.jpg");
    let lower_source = lower_source_dir.join("a.jpg");
    std::fs::write(&upper_source, b"upper source").unwrap();
    std::fs::write(&lower_source, b"lower source").unwrap();
    let output_dir = temp.path().join("organized");

    let error = OrganizeProcessor
        .process(
            vec![
                FileInput::from_path(upper_source.clone()),
                FileInput::from_path(lower_source.clone()),
            ],
            OrganizeConfig {
                output_dir: output_dir.clone(),
                strategy: OrganizeStrategy::ByDate,
                by_date: true,
                by_subject: false,
                dry_run: true,
            },
        )
        .unwrap_err();

    assert_eq!(error.code(), ErrorCode::OutputExists);
    assert_eq!(std::fs::read(upper_source).unwrap(), b"upper source");
    assert_eq!(std::fs::read(lower_source).unwrap(), b"lower source");
    assert!(!output_dir.exists());
}

#[test]
fn organize_rejects_case_only_planned_destination_aliases_before_copying() {
    let temp = tempfile::tempdir().unwrap();
    let upper_source_dir = temp.path().join("upper-source");
    let lower_source_dir = temp.path().join("lower-source");
    std::fs::create_dir_all(&upper_source_dir).unwrap();
    std::fs::create_dir_all(&lower_source_dir).unwrap();
    let upper_source = upper_source_dir.join("A.jpg");
    let lower_source = lower_source_dir.join("a.jpg");
    std::fs::write(&upper_source, b"upper source").unwrap();
    std::fs::write(&lower_source, b"lower source").unwrap();
    let output_dir = temp.path().join("organized");

    let error = OrganizeProcessor
        .process(
            vec![
                FileInput::from_path(upper_source.clone()),
                FileInput::from_path(lower_source.clone()),
            ],
            OrganizeConfig {
                output_dir: output_dir.clone(),
                strategy: OrganizeStrategy::ByDate,
                by_date: true,
                by_subject: false,
                dry_run: false,
            },
        )
        .unwrap_err();

    assert_eq!(error.code(), ErrorCode::OutputExists);
    assert_eq!(std::fs::read(upper_source).unwrap(), b"upper source");
    assert_eq!(std::fs::read(lower_source).unwrap(), b"lower source");
    assert!(!output_dir.exists());
}

#[test]
fn rename_dry_run_rejects_case_only_planned_destination_aliases() {
    let temp = tempfile::tempdir().unwrap();
    let upper_source_dir = temp.path().join("upper-source");
    let lower_source_dir = temp.path().join("lower-source");
    std::fs::create_dir_all(&upper_source_dir).unwrap();
    std::fs::create_dir_all(&lower_source_dir).unwrap();
    let upper_source = upper_source_dir.join("A.jpg");
    let lower_source = lower_source_dir.join("a.jpg");
    std::fs::write(&upper_source, b"upper source").unwrap();
    std::fs::write(&lower_source, b"lower source").unwrap();
    let output_dir = temp.path().join("renamed");

    let error = RenameProcessor
        .process(
            vec![
                FileInput::from_path(upper_source.clone()),
                FileInput::from_path(lower_source.clone()),
            ],
            RenameConfig {
                pattern: "{name}".to_string(),
                output_dir: Some(output_dir.clone()),
                start_number: 1,
                use_ai_descriptions: false,
                dry_run: true,
            },
        )
        .unwrap_err();

    assert_eq!(error.code(), ErrorCode::OutputExists);
    assert_eq!(std::fs::read(upper_source).unwrap(), b"upper source");
    assert_eq!(std::fs::read(lower_source).unwrap(), b"lower source");
    assert!(!output_dir.exists());
}

#[test]
fn rename_rejects_case_only_planned_destination_aliases_before_any_move() {
    let temp = tempfile::tempdir().unwrap();
    let upper_source_dir = temp.path().join("upper-source");
    let lower_source_dir = temp.path().join("lower-source");
    std::fs::create_dir_all(&upper_source_dir).unwrap();
    std::fs::create_dir_all(&lower_source_dir).unwrap();
    let upper_source = upper_source_dir.join("A.jpg");
    let lower_source = lower_source_dir.join("a.jpg");
    std::fs::write(&upper_source, b"upper source").unwrap();
    std::fs::write(&lower_source, b"lower source").unwrap();
    let output_dir = temp.path().join("renamed");
    std::fs::create_dir_all(&output_dir).unwrap();

    let error = RenameProcessor
        .process(
            vec![
                FileInput::from_path(upper_source.clone()),
                FileInput::from_path(lower_source.clone()),
            ],
            RenameConfig {
                pattern: "{name}".to_string(),
                output_dir: Some(output_dir.clone()),
                start_number: 1,
                use_ai_descriptions: false,
                dry_run: false,
            },
        )
        .unwrap_err();

    assert_eq!(error.code(), ErrorCode::OutputExists);
    assert_eq!(std::fs::read(upper_source).unwrap(), b"upper source");
    assert_eq!(std::fs::read(lower_source).unwrap(), b"lower source");
    assert_eq!(std::fs::read_dir(output_dir).unwrap().count(), 0);
}

#[test]
fn organize_dry_run_rejects_an_existing_case_only_destination_alias() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source").join("a.jpg");
    std::fs::create_dir_all(source.parent().unwrap()).unwrap();
    std::fs::write(&source, b"source").unwrap();
    let output_dir = temp.path().join("organized");

    let planned = OrganizeProcessor
        .process(
            vec![FileInput::from_path(source.clone())],
            OrganizeConfig {
                output_dir: output_dir.clone(),
                strategy: OrganizeStrategy::ByDate,
                by_date: true,
                by_subject: false,
                dry_run: true,
            },
        )
        .unwrap();
    let occupied = destination(&planned[0]).with_file_name("A.jpg");
    std::fs::create_dir_all(occupied.parent().unwrap()).unwrap();
    std::fs::write(&occupied, b"occupied").unwrap();

    let error = OrganizeProcessor
        .process(
            vec![FileInput::from_path(source.clone())],
            OrganizeConfig {
                output_dir,
                strategy: OrganizeStrategy::ByDate,
                by_date: true,
                by_subject: false,
                dry_run: true,
            },
        )
        .unwrap_err();

    assert_eq!(error.code(), ErrorCode::OutputExists);
    assert_eq!(std::fs::read(source).unwrap(), b"source");
    assert_eq!(std::fs::read(occupied).unwrap(), b"occupied");
}

#[test]
fn rename_dry_run_rejects_an_existing_case_only_destination_alias() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source").join("a.jpg");
    std::fs::create_dir_all(source.parent().unwrap()).unwrap();
    std::fs::write(&source, b"source").unwrap();
    let output_dir = temp.path().join("renamed");
    std::fs::create_dir_all(&output_dir).unwrap();
    let occupied = output_dir.join("A.jpg");
    std::fs::write(&occupied, b"occupied").unwrap();

    let error = RenameProcessor
        .process(
            vec![FileInput::from_path(source.clone())],
            RenameConfig {
                pattern: "{name}".to_string(),
                output_dir: Some(output_dir),
                start_number: 1,
                use_ai_descriptions: false,
                dry_run: true,
            },
        )
        .unwrap_err();

    assert_eq!(error.code(), ErrorCode::OutputExists);
    assert_eq!(std::fs::read(source).unwrap(), b"source");
    assert_eq!(std::fs::read(occupied).unwrap(), b"occupied");
}

#[cfg(unix)]
#[test]
fn rename_non_dry_preserves_regular_file_identity_and_modified_time() {
    use std::os::unix::fs::MetadataExt;

    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source").join("photo.jpg");
    std::fs::create_dir_all(source.parent().unwrap()).unwrap();
    std::fs::write(&source, b"source").unwrap();
    let source_metadata = std::fs::metadata(&source).unwrap();
    let output_dir = temp.path().join("renamed");
    std::fs::create_dir_all(&output_dir).unwrap();

    let outputs = RenameProcessor
        .process(
            vec![FileInput::from_path(source.clone())],
            RenameConfig {
                pattern: "{name}".to_string(),
                output_dir: Some(output_dir),
                start_number: 1,
                use_ai_descriptions: false,
                dry_run: false,
            },
        )
        .unwrap();

    let destination = destination(&outputs[0]);
    let destination_metadata = std::fs::metadata(destination).unwrap();
    assert!(!source.exists());
    assert_eq!(std::fs::read(destination).unwrap(), b"source");
    assert_eq!(destination_metadata.dev(), source_metadata.dev());
    assert_eq!(destination_metadata.ino(), source_metadata.ino());
    assert_eq!(
        destination_metadata.modified().unwrap(),
        source_metadata.modified().unwrap()
    );
}

#[cfg(unix)]
#[test]
fn organize_rejects_ambiguous_case_only_output_directories_before_copying() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source").join("photo.jpg");
    std::fs::create_dir_all(source.parent().unwrap()).unwrap();
    std::fs::write(&source, b"source").unwrap();
    let exact_output_dir = temp.path().join("out");
    let alias_output_dir = temp.path().join("Out");
    std::fs::create_dir_all(&exact_output_dir).unwrap();

    let planned = OrganizeProcessor
        .process(
            vec![FileInput::from_path(source.clone())],
            OrganizeConfig {
                output_dir: exact_output_dir.clone(),
                strategy: OrganizeStrategy::ByDate,
                by_date: true,
                by_subject: false,
                dry_run: true,
            },
        )
        .unwrap();
    let alias_destination = alias_output_dir.join(
        destination(&planned[0])
            .strip_prefix(&exact_output_dir)
            .unwrap(),
    );
    std::fs::create_dir_all(alias_destination.parent().unwrap()).unwrap();
    std::fs::write(&alias_destination, b"occupied").unwrap();

    let error = OrganizeProcessor
        .process(
            vec![FileInput::from_path(source.clone())],
            OrganizeConfig {
                output_dir: exact_output_dir.clone(),
                strategy: OrganizeStrategy::ByDate,
                by_date: true,
                by_subject: false,
                dry_run: false,
            },
        )
        .unwrap_err();

    assert_eq!(error.code(), ErrorCode::OutputExists);
    assert_eq!(std::fs::read(&source).unwrap(), b"source");
    assert_eq!(std::fs::read(&alias_destination).unwrap(), b"occupied");
    assert_eq!(std::fs::read_dir(exact_output_dir).unwrap().count(), 0);
}

#[cfg(unix)]
#[test]
fn rename_rejects_ambiguous_case_only_output_directories_before_moving() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source").join("photo.jpg");
    std::fs::create_dir_all(source.parent().unwrap()).unwrap();
    std::fs::write(&source, b"source").unwrap();
    let exact_output_dir = temp.path().join("out");
    let alias_output_dir = temp.path().join("Out");
    std::fs::create_dir_all(&exact_output_dir).unwrap();
    std::fs::create_dir_all(&alias_output_dir).unwrap();
    let alias_destination = alias_output_dir.join("same.jpg");
    std::fs::write(&alias_destination, b"occupied").unwrap();

    let error = RenameProcessor
        .process(
            vec![FileInput::from_path(source.clone())],
            RenameConfig {
                pattern: "same".to_string(),
                output_dir: Some(exact_output_dir.clone()),
                start_number: 1,
                use_ai_descriptions: false,
                dry_run: false,
            },
        )
        .unwrap_err();

    assert_eq!(error.code(), ErrorCode::OutputExists);
    assert_eq!(std::fs::read(&source).unwrap(), b"source");
    assert_eq!(std::fs::read(&alias_destination).unwrap(), b"occupied");
    assert_eq!(std::fs::read_dir(exact_output_dir).unwrap().count(), 0);
}

#[test]
fn rename_rejects_relative_and_absolute_spellings_of_one_destination_before_moving() {
    let temp = tempfile::tempdir_in(".").unwrap();
    let relative_dir = std::path::PathBuf::from(temp.path().file_name().unwrap());
    let absolute_dir = std::env::current_dir().unwrap().join(&relative_dir);
    let relative_source = relative_dir.join("first.jpg");
    let absolute_source = absolute_dir.join("second.jpg");
    std::fs::write(&relative_source, b"first").unwrap();
    std::fs::write(&absolute_source, b"second").unwrap();
    let destination = relative_dir.join("same.jpg");

    let error = RenameProcessor
        .process(
            vec![
                FileInput::from_path(relative_source.clone()),
                FileInput::from_path(absolute_source.clone()),
            ],
            RenameConfig {
                pattern: "same".to_string(),
                output_dir: None,
                start_number: 1,
                use_ai_descriptions: false,
                dry_run: false,
            },
        )
        .unwrap_err();

    assert_eq!(error.code(), ErrorCode::OutputExists);
    assert_eq!(std::fs::read(relative_source).unwrap(), b"first");
    assert_eq!(std::fs::read(absolute_source).unwrap(), b"second");
    assert!(!destination.exists());
}

#[test]
fn rename_dry_run_rejects_sigma_and_final_sigma_destination_aliases() {
    let temp = tempfile::tempdir().unwrap();
    let first = temp.path().join("first").join("Σ.jpg");
    let second = temp.path().join("second").join("ς.jpg");
    std::fs::create_dir_all(first.parent().unwrap()).unwrap();
    std::fs::create_dir_all(second.parent().unwrap()).unwrap();
    std::fs::write(&first, b"first").unwrap();
    std::fs::write(&second, b"second").unwrap();
    let output_dir = temp.path().join("out");

    let error = RenameProcessor
        .process(
            vec![
                FileInput::from_path(first.clone()),
                FileInput::from_path(second.clone()),
            ],
            RenameConfig {
                pattern: "{name}".to_string(),
                output_dir: Some(output_dir.clone()),
                start_number: 1,
                use_ai_descriptions: false,
                dry_run: true,
            },
        )
        .unwrap_err();

    assert_eq!(error.code(), ErrorCode::OutputExists);
    assert_eq!(std::fs::read(first).unwrap(), b"first");
    assert_eq!(std::fs::read(second).unwrap(), b"second");
    assert!(!output_dir.exists());
}

#[test]
// U+212A is intentionally non-NFC; normalizing it to K would erase this case-fold regression.
#[allow(clippy::unicode_not_nfc)]
fn rename_dry_run_rejects_kelvin_sign_and_k_destination_aliases() {
    let temp = tempfile::tempdir().unwrap();
    let first = temp.path().join("first").join("K.jpg");
    let second = temp.path().join("second").join("k.jpg");
    std::fs::create_dir_all(first.parent().unwrap()).unwrap();
    std::fs::create_dir_all(second.parent().unwrap()).unwrap();
    std::fs::write(&first, b"first").unwrap();
    std::fs::write(&second, b"second").unwrap();
    let output_dir = temp.path().join("out");

    let error = RenameProcessor
        .process(
            vec![
                FileInput::from_path(first.clone()),
                FileInput::from_path(second.clone()),
            ],
            RenameConfig {
                pattern: "{name}".to_string(),
                output_dir: Some(output_dir.clone()),
                start_number: 1,
                use_ai_descriptions: false,
                dry_run: true,
            },
        )
        .unwrap_err();

    assert_eq!(error.code(), ErrorCode::OutputExists);
    assert_eq!(std::fs::read(first).unwrap(), b"first");
    assert_eq!(std::fs::read(second).unwrap(), b"second");
    assert!(!output_dir.exists());
}

#[test]
fn rename_dry_run_rejects_mtavruli_and_mkhedruli_destination_aliases() {
    let temp = tempfile::tempdir().unwrap();
    let first = temp.path().join("first").join("\u{1c90}.jpg");
    let second = temp.path().join("second").join("\u{10d0}.jpg");
    std::fs::create_dir_all(first.parent().unwrap()).unwrap();
    std::fs::create_dir_all(second.parent().unwrap()).unwrap();
    std::fs::write(&first, b"first").unwrap();
    std::fs::write(&second, b"second").unwrap();
    let output_dir = temp.path().join("out");

    let error = RenameProcessor
        .process(
            vec![
                FileInput::from_path(first.clone()),
                FileInput::from_path(second.clone()),
            ],
            RenameConfig {
                pattern: "{name}".to_string(),
                output_dir: Some(output_dir.clone()),
                start_number: 1,
                use_ai_descriptions: false,
                dry_run: true,
            },
        )
        .unwrap_err();

    assert_eq!(error.code(), ErrorCode::OutputExists);
    assert_eq!(std::fs::read(first).unwrap(), b"first");
    assert_eq!(std::fs::read(second).unwrap(), b"second");
    assert!(!output_dir.exists());
}

#[test]
fn rename_dry_run_rejects_expanding_unicode_destination_aliases() {
    let temp = tempfile::tempdir().unwrap();
    let first = temp.path().join("first").join("ß.jpg");
    let second = temp.path().join("second").join("ss.jpg");
    std::fs::create_dir_all(first.parent().unwrap()).unwrap();
    std::fs::create_dir_all(second.parent().unwrap()).unwrap();
    std::fs::write(&first, b"first").unwrap();
    std::fs::write(&second, b"second").unwrap();
    let output_dir = temp.path().join("out");

    let error = RenameProcessor
        .process(
            vec![
                FileInput::from_path(first.clone()),
                FileInput::from_path(second.clone()),
            ],
            RenameConfig {
                pattern: "{name}".to_string(),
                output_dir: Some(output_dir.clone()),
                start_number: 1,
                use_ai_descriptions: false,
                dry_run: true,
            },
        )
        .unwrap_err();

    assert_eq!(error.code(), ErrorCode::OutputExists);
    assert_eq!(std::fs::read(first).unwrap(), b"first");
    assert_eq!(std::fs::read(second).unwrap(), b"second");
    assert!(!output_dir.exists());
}

#[cfg(unix)]
#[test]
fn rename_rejects_non_unicode_destination_names_without_lossy_normalization() {
    use std::os::unix::ffi::OsStringExt;

    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join(std::ffi::OsString::from_vec(
        b"non-unicode-\xff.jpg".to_vec(),
    ));
    std::fs::write(&source, b"source").unwrap();

    let error = RenameProcessor
        .process(
            vec![FileInput::from_path(source.clone())],
            RenameConfig {
                pattern: "{name}".to_string(),
                output_dir: None,
                start_number: 1,
                use_ai_descriptions: false,
                dry_run: true,
            },
        )
        .unwrap_err();

    assert_eq!(error.code(), ErrorCode::PathPolicyViolation);
    assert_eq!(std::fs::read(source).unwrap(), b"source");
}

#[test]
fn rename_rejects_duplicate_source_identity_before_any_move() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("photo.jpg");
    std::fs::write(&source, b"source").unwrap();

    let error = RenameProcessor
        .process(
            vec![
                FileInput::from_path(source.clone()),
                FileInput::from_path(source.clone()),
            ],
            RenameConfig {
                pattern: "renamed_{index}".to_string(),
                output_dir: None,
                start_number: 1,
                use_ai_descriptions: false,
                dry_run: false,
            },
        )
        .unwrap_err();

    assert_eq!(error.code(), ErrorCode::InvalidInput);
    assert_eq!(std::fs::read(&source).unwrap(), b"source");
    assert!(!temp.path().join("renamed_1.jpg").exists());
    assert!(!temp.path().join("renamed_2.jpg").exists());
    assert_eq!(std::fs::read_dir(temp.path()).unwrap().count(), 1);
}
