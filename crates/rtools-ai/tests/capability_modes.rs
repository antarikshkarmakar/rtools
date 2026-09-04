use image::{ImageBuffer, Rgba};
use rtools_ai::duplicates::{DuplicateAction, DuplicatesConfig, DuplicatesProcessor};
use rtools_ai::organize::{OrganizeConfig, OrganizeProcessor, OrganizeStrategy};
use rtools_ai::rename::{RenameConfig, RenameProcessor};
use rtools_core::{ErrorCode, FileInput, Processor, RToolsError};
use tempfile::tempdir;
use {chrono as _, serde as _, unicode_casefold as _};

#[test]
fn unsupported_organize_modes_fail_before_directory_or_input_work() {
    let temp = tempdir().unwrap();
    let cases = [
        (OrganizeStrategy::BySubject, "ai.organize.subject"),
        (OrganizeStrategy::ByLocation, "ai.organize.location"),
        (OrganizeStrategy::ByCamera, "ai.organize.camera"),
        (OrganizeStrategy::Custom, "ai.organize.custom"),
    ];

    for (index, (strategy, operation_id)) in cases.into_iter().enumerate() {
        let output = temp.path().join(format!("organized-{index}"));
        let error = OrganizeProcessor
            .process(
                vec![FileInput::from_path(temp.path().join("missing.png"))],
                OrganizeConfig {
                    output_dir: output.clone(),
                    strategy,
                    by_date: false,
                    by_subject: true,
                    dry_run: false,
                },
            )
            .unwrap_err();
        assert_unavailable(error, operation_id);
        assert!(!output.exists());
    }
}

#[test]
fn empty_date_organize_is_invalid_before_output_creation() {
    let temp = tempdir().unwrap();
    let output = temp.path().join("organized");
    let error = OrganizeProcessor
        .process(
            Vec::new(),
            OrganizeConfig {
                output_dir: output.clone(),
                strategy: OrganizeStrategy::ByDate,
                ..OrganizeConfig::default()
            },
        )
        .unwrap_err();

    assert_eq!(error.code(), ErrorCode::InvalidInput);
    assert!(!output.exists());
}

#[test]
fn ai_and_unknown_rename_tokens_fail_before_filesystem_work() {
    let temp = tempdir().unwrap();
    let output = temp.path().join("renamed");
    let input = vec![FileInput::from_path(temp.path().join("missing.png"))];

    for config in [
        RenameConfig {
            pattern: "{date}_{index}".to_string(),
            output_dir: Some(output.clone()),
            use_ai_descriptions: true,
            ..RenameConfig::default()
        },
        RenameConfig {
            pattern: "{date}_{subject}_{index}".to_string(),
            output_dir: Some(output.clone()),
            use_ai_descriptions: false,
            ..RenameConfig::default()
        },
    ] {
        let error = RenameProcessor.process(input.clone(), config).unwrap_err();
        assert_unavailable(error, "ai.rename.ai");
        assert!(!output.exists());
    }

    for pattern in [
        "{date}_{mystery}_{index}",
        "prefix}_{date}",
        "{date_{index}",
        "{date",
    ] {
        let error = RenameProcessor
            .process(
                input.clone(),
                RenameConfig {
                    pattern: pattern.to_string(),
                    output_dir: Some(output.clone()),
                    use_ai_descriptions: false,
                    ..RenameConfig::default()
                },
            )
            .unwrap_err();
        assert_eq!(error.code(), ErrorCode::InvalidInput, "{pattern}");
        assert!(!output.exists());
    }
}

#[test]
fn deterministic_rename_supports_only_declared_tokens_and_dry_run() {
    let temp = tempdir().unwrap();
    let input = temp.path().join("photo.png");
    std::fs::write(&input, b"content").unwrap();
    let token = |name| format!("{{{name}}}");
    let result = RenameProcessor
        .process(
            vec![FileInput::from_path(input.clone())],
            RenameConfig {
                pattern: format!(
                    "{}_{}_{}_{}_{}.{}",
                    token("date"),
                    token("time"),
                    token("datetime"),
                    token("name"),
                    token("index"),
                    token("ext")
                ),
                output_dir: None,
                start_number: 1,
                use_ai_descriptions: false,
                dry_run: true,
            },
        )
        .unwrap();

    assert_eq!(result.len(), 1);
    assert!(input.exists());
    let name = result[0].name.as_deref().unwrap();
    assert!(!name.contains('{'));
    assert!(name.contains("photo"));
}

#[test]
fn destructive_duplicate_actions_fail_before_reading_or_mutating_files() {
    let temp = tempdir().unwrap();
    let destination = temp.path().join("duplicates");
    let cases = [
        (
            DuplicateAction::Move {
                destination: destination.clone(),
            },
            "ai.duplicates.move",
        ),
        (DuplicateAction::Delete, "ai.duplicates.delete"),
        (DuplicateAction::Symlink, "ai.duplicates.symlink"),
    ];

    for (action, operation_id) in cases {
        let error = DuplicatesProcessor
            .process(
                vec![FileInput::from_path(temp.path().join("missing.png"))],
                DuplicatesConfig {
                    action,
                    dry_run: true,
                    ..DuplicatesConfig::default()
                },
            )
            .unwrap_err();
        assert_unavailable(error, operation_id);
        assert!(!destination.exists());
    }
}

#[test]
fn empty_duplicate_report_is_invalid_and_real_report_remains_executable() {
    let error = DuplicatesProcessor
        .process(Vec::new(), DuplicatesConfig::default())
        .unwrap_err();
    assert_eq!(error.code(), ErrorCode::InvalidInput);

    let temp = tempdir().unwrap();
    let first = temp.path().join("first.png");
    let second = temp.path().join("second.png");
    let image = ImageBuffer::from_pixel(4, 4, Rgba([20_u8, 30, 40, 255]));
    image.save(&first).unwrap();
    image.save(&second).unwrap();
    let report = DuplicatesProcessor
        .process(
            vec![FileInput::from_path(first), FileInput::from_path(second)],
            DuplicatesConfig::default(),
        )
        .unwrap();
    assert_eq!(report.groups.len(), 1);
    assert_eq!(report.total_duplicates, 1);
}

fn assert_unavailable(error: RToolsError, operation_id: &str) {
    assert_eq!(error.code(), ErrorCode::CapabilityUnavailable);
    assert!(matches!(
        error,
        RToolsError::CapabilityUnavailable {
            operation_id: actual,
            ..
        } if actual == operation_id
    ));
}
