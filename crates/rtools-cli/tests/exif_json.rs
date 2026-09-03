use image::{ImageBuffer, Rgba};
use serde_json::Value;
use std::process::Command;
use tempfile::tempdir;
use {
    anyhow as _, clap as _, clap_complete as _, rtools_ai as _, rtools_core as _,
    rtools_image as _, rtools_pdf as _, serde as _, tokio as _, toml as _, tracing as _,
    tracing_subscriber as _, walkdir as _,
};

#[test]
fn multiple_exif_inputs_emit_one_machine_parseable_json_document() {
    let temp = tempdir().expect("temp dir");
    let first = temp.path().join("first.png");
    let second = temp.path().join("second.png");
    ImageBuffer::from_pixel(2, 2, Rgba([10_u8, 20, 30, 255]))
        .save(&first)
        .expect("first PNG");
    ImageBuffer::from_pixel(2, 2, Rgba([40_u8, 50, 60, 255]))
        .save(&second)
        .expect("second PNG");

    let output = Command::new(env!("CARGO_BIN_EXE_rtools"))
        .arg("image")
        .arg("exif")
        .arg("--input")
        .arg(&first)
        .arg(&second)
        .arg("--format")
        .arg("json")
        .output()
        .expect("CLI must run");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let document: Value = serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "stdout must be exactly one JSON value: {error}; stdout={}",
            String::from_utf8_lossy(&output.stdout)
        )
    });
    let results = document
        .get("results")
        .and_then(Value::as_array)
        .expect("JSON contract must contain a results array");
    assert_eq!(results.len(), 2);
    assert_eq!(results[0]["path"], first.display().to_string());
    assert_eq!(results[1]["path"], second.display().to_string());
    assert!(results.iter().all(|result| result.get("exif").is_some()));
}

#[test]
fn exif_json_failure_emits_no_partial_document() {
    let temp = tempdir().expect("temp dir");
    let valid = temp.path().join("valid.png");
    let missing = temp.path().join("missing.png");
    ImageBuffer::from_pixel(2, 2, Rgba([10_u8, 20, 30, 255]))
        .save(&valid)
        .expect("valid PNG");

    let output = Command::new(env!("CARGO_BIN_EXE_rtools"))
        .arg("image")
        .arg("exif")
        .arg("--input")
        .arg(&valid)
        .arg(&missing)
        .arg("--format")
        .arg("json")
        .output()
        .expect("CLI must run");

    assert!(!output.status.success());
    assert!(
        output.stdout.is_empty(),
        "stdout must contain no partial JSON: {}",
        String::from_utf8_lossy(&output.stdout)
    );
}
