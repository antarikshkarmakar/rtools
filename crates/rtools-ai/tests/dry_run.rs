#![allow(unused_crate_dependencies)]

use rtools_ai::organize::{OrganizeConfig, OrganizeStrategy};
use rtools_ai::rename::RenameConfig;
use rtools_ai::{OrganizeProcessor, RenameProcessor};
use rtools_core::{FileInput, OutputDestination, Processor};

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
fn date_organize_dry_run_resolves_existing_destination_collision() {
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

    let planned = OrganizeProcessor
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
        .unwrap();

    assert_eq!(destination(&planned[0]).file_name().unwrap(), "photo_1.jpg");
    assert_eq!(std::fs::read(occupied).unwrap(), b"occupied");
    assert!(!destination(&planned[0]).exists());
}

#[test]
fn rename_dry_run_reserves_each_planned_destination() {
    let temp = tempfile::tempdir().unwrap();
    let first = temp.path().join("first.jpg");
    let second = temp.path().join("second.jpg");
    std::fs::write(&first, b"first").unwrap();
    std::fs::write(&second, b"second").unwrap();

    let outputs = RenameProcessor
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
        .unwrap();

    assert_eq!(destination(&outputs[0]), temp.path().join("same.jpg"));
    assert_eq!(destination(&outputs[1]), temp.path().join("same_1.jpg"));
    assert_eq!(std::fs::read(first).unwrap(), b"first");
    assert_eq!(std::fs::read(second).unwrap(), b"second");
    assert!(!temp.path().join("same.jpg").exists());
    assert!(!temp.path().join("same_1.jpg").exists());
}
