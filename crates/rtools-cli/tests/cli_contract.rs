#![allow(unused_crate_dependencies)]

use serde_json::Value;
use std::path::Path;
use std::process::{Command, Output};

fn command(current_dir: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_rtools"));
    command.env_clear().current_dir(current_dir);
    command
}

fn stdout_json(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "stdout must be exactly one JSON value: {error}; stdout={}; stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    })
}

fn assert_exit(output: &Output, expected: i32) {
    assert_eq!(
        output.status.code(),
        Some(expected),
        "stdout={}; stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn unavailable_pdf_text_returns_capability_exit_without_success_output() {
    let temp = tempfile::tempdir().unwrap();
    let output = command(temp.path())
        .args(["pdf", "text", "--input", "missing.pdf"])
        .output()
        .unwrap();

    assert_exit(&output, 3);
    assert!(!String::from_utf8_lossy(&output.stdout).contains('✓'));
}

#[test]
fn missing_explicit_config_returns_configuration_exit_code() {
    let temp = tempfile::tempdir().unwrap();
    let missing = temp.path().join("definitely-missing.toml");
    let output = command(temp.path())
        .arg("--config")
        .arg(&missing)
        .arg("doctor")
        .output()
        .unwrap();

    assert_exit(&output, 3);
}

#[test]
fn config_validate_missing_file_returns_configuration_exit_code() {
    let temp = tempfile::tempdir().unwrap();
    let missing = temp.path().join("definitely-missing.toml");
    let output = command(temp.path())
        .args(["config", "validate", "--config"])
        .arg(&missing)
        .output()
        .unwrap();

    assert_exit(&output, 3);
}

#[test]
fn malformed_crop_region_is_rejected_as_invalid_input() {
    let temp = tempfile::tempdir().unwrap();
    let output = command(temp.path())
        .args([
            "image",
            "crop",
            "--input",
            "photo.png",
            "--region",
            "x,2,3,4",
        ])
        .output()
        .unwrap();

    assert_exit(&output, 2);
}

#[test]
fn every_documented_typed_value_rejects_unknown_or_malformed_input() {
    let temp = tempfile::tempdir().unwrap();
    let cases: &[&[&str]] = &[
        &[
            "image",
            "crop",
            "--input",
            "photo.png",
            "--ratio",
            "16:nope",
        ],
        &[
            "image",
            "crop",
            "--input",
            "photo.png",
            "--gravity",
            "somewhere",
        ],
        &[
            "pdf",
            "compress",
            "--input",
            "document.pdf",
            "--level",
            "maximum-ish",
        ],
        &[
            "ai",
            "organize",
            "--input",
            "photos",
            "--output",
            "organized",
            "--strategy",
            "mystery",
        ],
        &[
            "ai",
            "duplicates",
            "--input",
            "photos",
            "--action",
            "erase-everything",
        ],
        &[
            "image",
            "convert",
            "--input",
            "photo.png",
            "--format",
            "made-up",
        ],
        &[
            "pdf",
            "to-image",
            "--input",
            "document.pdf",
            "--output",
            "pages",
            "--format",
            "made-up",
        ],
        &["--output-format", "xml", "doctor"],
    ];

    for args in cases {
        let output = command(temp.path()).args(*args).output().unwrap();
        assert_exit(&output, 2);
    }
}

#[test]
fn doctor_json_is_one_report_and_matches_the_sorted_shared_registry() {
    let temp = tempfile::tempdir().unwrap();
    let output = command(temp.path())
        .args(["--output-format", "json", "doctor"])
        .output()
        .unwrap();

    assert_exit(&output, 0);
    let report = stdout_json(&output);
    assert!(report.is_object());
    assert_eq!(report["operation_id"], "doctor.report");
    assert_eq!(report["status"], "success");
    assert_eq!(report["warnings"], serde_json::json!([]));
    assert_eq!(report["failures"], serde_json::json!([]));

    let actual: Vec<(&str, &str)> = report["result"]["capabilities"]
        .as_array()
        .expect("doctor result must contain capabilities")
        .iter()
        .map(|capability| {
            (
                capability["operation_id"].as_str().unwrap(),
                capability["state"].as_str().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        actual,
        [
            ("ai.alt_text", "unavailable"),
            ("ai.duplicates.delete", "unavailable"),
            ("ai.duplicates.move", "unavailable"),
            ("ai.duplicates.report", "experimental"),
            ("ai.duplicates.symlink", "unavailable"),
            ("ai.ocr", "unavailable"),
            ("ai.organize.camera", "unavailable"),
            ("ai.organize.custom", "unavailable"),
            ("ai.organize.date", "experimental"),
            ("ai.organize.location", "unavailable"),
            ("ai.organize.subject", "unavailable"),
            ("ai.rename.ai", "unavailable"),
            ("ai.rename.deterministic", "experimental"),
            ("batch.run", "unavailable"),
            ("completions.generate", "available"),
            ("config.init", "available"),
            ("config.show", "available"),
            ("config.validate", "available"),
            ("doctor.report", "available"),
            ("image.compress", "available"),
            ("image.convert", "available"),
            ("image.crop", "available"),
            ("image.exif.human", "available"),
            ("image.exif.json", "available"),
            ("image.filter", "available"),
            ("image.metadata.preserve", "unavailable"),
            ("image.metadata.strip_gps", "unavailable"),
            ("image.ocr", "unavailable"),
            ("image.resize", "available"),
            ("image.watermark.image", "available"),
            ("image.watermark.text", "unavailable"),
            ("pdf.compress", "experimental"),
            ("pdf.merge", "experimental"),
            ("pdf.ocr", "unavailable"),
            ("pdf.split", "experimental"),
            ("pdf.text", "unavailable"),
            ("pdf.to_image", "unavailable"),
        ]
    );

    assert!(report["result"]["configured_limits"].is_object());
    assert!(report["result"]["writable_directories"].is_array());
    assert!(output.stderr.is_empty(), "JSON diagnostics must stay clean");
}

#[test]
fn json_errors_are_one_report_with_the_stable_error_code() {
    let temp = tempfile::tempdir().unwrap();
    let output = command(temp.path())
        .args([
            "--output-format",
            "json",
            "pdf",
            "text",
            "--input",
            "missing.pdf",
        ])
        .output()
        .unwrap();

    assert_exit(&output, 3);
    let report = stdout_json(&output);
    assert_eq!(report["status"], "failure");
    assert_eq!(report["result"], Value::Null);
    assert_eq!(report["failures"].as_array().unwrap().len(), 1);
    assert_eq!(report["failures"][0]["code"], "CAPABILITY_UNAVAILABLE");
    assert!(output.stderr.is_empty(), "JSON diagnostics must stay clean");
}

#[test]
fn batch_recipe_that_would_partially_fail_never_exits_successfully() {
    let temp = tempfile::tempdir().unwrap();
    let recipe = temp.path().join("batch.toml");
    std::fs::write(
        &recipe,
        "[[operations]]\noperation = \"compress\"\ninput = [\"present.png\", \"missing.png\"]\n",
    )
    .unwrap();

    let output = command(temp.path())
        .args(["batch", "--config"])
        .arg(&recipe)
        .output()
        .unwrap();

    assert!(!output.status.success());
}

#[test]
fn unsupported_global_dry_run_fails_before_creating_an_image_output() {
    let temp = tempfile::tempdir().unwrap();
    let destination = temp.path().join("never-created.png");
    let output = command(temp.path())
        .args([
            "--output-format",
            "json",
            "--dry-run",
            "image",
            "convert",
            "--input",
            "missing.png",
            "--format",
            "png",
            "--output",
        ])
        .arg(&destination)
        .output()
        .unwrap();

    assert_exit(&output, 3);
    assert!(!destination.exists());
    let report = stdout_json(&output);
    assert_eq!(report["failures"][0]["code"], "CAPABILITY_UNAVAILABLE");
}

#[test]
fn global_dry_run_rename_returns_exact_pair_without_renaming() {
    let temp = tempfile::tempdir().unwrap();
    let input_dir = temp.path().join("photos");
    std::fs::create_dir(&input_dir).unwrap();
    let source = input_dir.join("photo.jpg");
    let destination = input_dir.join("photo_renamed.jpg");
    std::fs::write(&source, b"fixture").unwrap();

    let output = command(temp.path())
        .args([
            "--output-format",
            "json",
            "--dry-run",
            "ai",
            "rename",
            "--input",
        ])
        .arg(&input_dir)
        .args(["--pattern", "{name}_renamed"])
        .output()
        .unwrap();

    assert_exit(&output, 0);
    assert!(source.exists());
    assert!(!destination.exists());
    let report = stdout_json(&output);
    assert_eq!(report["result"]["planned"].as_array().unwrap().len(), 1);
    assert_eq!(
        report["result"]["planned"][0],
        serde_json::json!({
            "source": source,
            "destination": destination,
        })
    );
}

#[test]
fn global_dry_run_date_organize_returns_exact_pair_without_directories() {
    let temp = tempfile::tempdir().unwrap();
    let input_dir = temp.path().join("photos");
    std::fs::create_dir(&input_dir).unwrap();
    let source = input_dir.join("photo.jpg");
    std::fs::write(&source, b"fixture").unwrap();
    let output_dir = temp.path().join("organized");

    let output = command(temp.path())
        .args([
            "--output-format",
            "json",
            "--dry-run",
            "ai",
            "organize",
            "--input",
        ])
        .arg(&input_dir)
        .arg("--output")
        .arg(&output_dir)
        .args(["--strategy", "date"])
        .output()
        .unwrap();

    assert_exit(&output, 0);
    assert!(!output_dir.exists());
    let report = stdout_json(&output);
    let planned = report["result"]["planned"].as_array().unwrap();
    assert_eq!(planned.len(), 1);
    assert_eq!(planned[0]["source"], source.display().to_string());
    let destination = planned[0]["destination"].as_str().unwrap();
    assert!(destination.starts_with(&output_dir.display().to_string()));
    assert!(destination.ends_with("photo.jpg"));
    assert!(!Path::new(destination).exists());
}
