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

fn create_png(path: &Path, width: u32, height: u32) {
    image::RgbaImage::from_pixel(width, height, image::Rgba([20, 40, 60, 255]))
        .save(path)
        .unwrap();
}

fn create_pdf(path: &Path, page_count: u32) {
    use lopdf::{dictionary, Document, Object, Stream};
    let mut document = Document::with_version("1.5");
    let pages_id = document.new_object_id();
    let catalog_id = document.new_object_id();
    let mut kids = Vec::new();
    for _ in 0..page_count {
        let content_id = document.add_object(Stream::new(dictionary! {}, Vec::new()));
        let page_object_id = document.add_object(dictionary! {
            "Type" => "Page", "Parent" => pages_id,
            "MediaBox" => vec![0.into(), 0.into(), 100.into(), 100.into()],
            "Contents" => content_id,
        });
        kids.push(Object::Reference(page_object_id));
    }
    document.objects.insert(
        pages_id,
        Object::Dictionary(
            dictionary! { "Type" => "Pages", "Kids" => kids, "Count" => page_count },
        ),
    );
    document.objects.insert(
        catalog_id,
        Object::Dictionary(dictionary! { "Type" => "Catalog", "Pages" => pages_id }),
    );
    document.trailer.set("Root", catalog_id);
    document.save(path).unwrap();
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
fn pdf_metadata_removal_cli_fails_before_missing_input_access() {
    let temp = tempfile::tempdir().unwrap();
    let output = command(temp.path())
        .args([
            "--output-format",
            "json",
            "pdf",
            "compress",
            "--input",
            "missing-with-xmp.pdf",
            "--remove-metadata",
        ])
        .output()
        .unwrap();

    assert_exit(&output, 3);
    let report = stdout_json(&output);
    assert_eq!(report["failures"][0]["code"], "CAPABILITY_UNAVAILABLE");
    assert_eq!(report["operation_id"], "pdf.compress.metadata");
}

#[test]
fn pdf_split_cli_rejects_escaping_filename_pattern_without_artifacts() {
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("input.pdf");
    let output_dir = temp.path().join("out");
    create_pdf(&input, 2);
    std::fs::create_dir(&output_dir).unwrap();

    let output = command(temp.path())
        .args(["--output-format", "json", "pdf", "split", "--input"])
        .arg(&input)
        .arg("--output")
        .arg(&output_dir)
        .args(["--filename-pattern", "../escaped_{n}.pdf"])
        .output()
        .unwrap();

    assert_exit(&output, 2);
    assert_eq!(stdout_json(&output)["failures"][0]["code"], "INVALID_INPUT");
    assert_eq!(std::fs::read_dir(&output_dir).unwrap().count(), 0);
    assert!(!temp.path().join("escaped_1.pdf").exists());
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
fn config_init_preserves_existing_bytes_and_returns_output_exists() {
    let temp = tempfile::tempdir().unwrap();
    let output_path = temp.path().join("existing.toml");
    let original = b"preserve these exact existing bytes\n\xff";
    std::fs::write(&output_path, original).unwrap();

    let output = command(temp.path())
        .args(["config", "init", "--output"])
        .arg(&output_path)
        .output()
        .unwrap();

    assert_exit(&output, 5);
    assert_eq!(std::fs::read(&output_path).unwrap(), original);
}

#[cfg(unix)]
fn create_directory_symlink(target: &Path, link: &Path) {
    std::os::unix::fs::symlink(target, link).unwrap();
}

#[cfg(windows)]
fn create_directory_symlink(target: &Path, link: &Path) {
    std::os::windows::fs::symlink_dir(target, link).unwrap_or_else(|error| {
        panic!(
            "failed to create directory symlink {} -> {}: {error}. Enable Windows Developer Mode or grant SeCreateSymbolicLinkPrivilege so this safety regression can run",
            link.display(),
            target.display()
        )
    });
}

#[cfg(any(unix, windows))]
#[test]
fn config_init_rejects_linked_ancestor_without_creating_outside() {
    let temp = tempfile::tempdir().unwrap();
    let selected = temp.path().join("selected");
    let outside = temp.path().join("outside");
    std::fs::create_dir(&selected).unwrap();
    std::fs::create_dir(&outside).unwrap();
    create_directory_symlink(&outside, &selected.join("link"));
    let outside_child = outside.join("new-child");
    let output_path = selected.join("link/new-child/rtools.toml");

    let output = command(temp.path())
        .args(["config", "init", "--output"])
        .arg(&output_path)
        .output()
        .unwrap();

    assert_exit(&output, 5);
    assert!(!outside_child.exists());
    assert!(!output_path.exists());
    assert!(std::fs::read_dir(&outside).unwrap().next().is_none());
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
fn out_of_bounds_crop_exits_invalid_without_publishing_output() {
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("crop-input.png");
    let destination = temp.path().join("crop-output.png");
    create_png(&input, 4, 4);

    let output = command(temp.path())
        .args(["image", "crop", "--input"])
        .arg(&input)
        .args(["--region", "3,0,2,1", "--output"])
        .arg(&destination)
        .output()
        .unwrap();

    assert_exit(&output, 2);
    assert!(!destination.exists());
}

#[test]
fn nonfinite_filter_strength_exits_invalid_without_publishing_output() {
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("filter-input.png");
    let destination = temp.path().join("filter-output.png");
    create_png(&input, 4, 4);

    let output = command(temp.path())
        .args(["image", "filter", "--input"])
        .arg(&input)
        .args([
            "--preset",
            "kodak-portra400",
            "--strength",
            "NaN",
            "--output",
        ])
        .arg(&destination)
        .output()
        .unwrap();

    assert_exit(&output, 2);
    assert!(!destination.exists());
}

#[test]
fn nonfinite_duplicate_threshold_exits_invalid() {
    let temp = tempfile::tempdir().unwrap();
    create_png(&temp.path().join("duplicate-input.png"), 4, 4);

    let output = command(temp.path())
        .args(["ai", "duplicates", "--input"])
        .arg(temp.path())
        .args(["--threshold", "NaN"])
        .output()
        .unwrap();

    assert_exit(&output, 2);
}

#[test]
fn zero_image_quality_exits_invalid_without_publishing_output() {
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("input.png");
    create_png(&input, 4, 4);

    for (operation, extra, destination) in [
        (
            "compress",
            Vec::<&str>::new(),
            temp.path().join("compressed.png"),
        ),
        (
            "convert",
            vec!["--format", "jpg"],
            temp.path().join("converted.jpg"),
        ),
    ] {
        let mut invocation = command(temp.path());
        invocation
            .args(["image", operation, "--input"])
            .arg(&input)
            .args(extra)
            .args(["--quality", "0", "--output"])
            .arg(&destination);
        let output = invocation.output().unwrap();
        assert_exit(&output, 2);
        assert!(!destination.exists());
    }
}

#[test]
fn compress_format_extension_mismatch_is_invalid_without_output_artifacts() {
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("compress-input.png");
    let destination = temp.path().join("claimed.png");
    create_png(&input, 4, 4);

    let output = command(temp.path())
        .args(["--output-format", "json", "image", "compress", "--input"])
        .arg(&input)
        .args(["--format", "tiff", "--output"])
        .arg(&destination)
        .output()
        .unwrap();

    assert_exit(&output, 2);
    assert_eq!(stdout_json(&output)["failures"][0]["code"], "INVALID_INPUT");
    assert!(!destination.exists());
    assert!(std::fs::read_dir(temp.path())
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .all(|name| !name.to_string_lossy().contains(".rtools.")));
}

#[test]
fn every_general_writer_rejects_qoi_without_output_artifacts() {
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("writer-input.png");
    let watermark = temp.path().join("writer-watermark.png");
    create_png(&input, 4, 4);
    create_png(&watermark, 5, 5);

    let cases: [(&str, Vec<&str>); 4] = [
        ("resize", vec!["--width", "2"]),
        ("crop", vec!["--region", "0,0,2,2"]),
        ("filter", vec!["--preset", "portra"]),
        (
            "watermark",
            vec![
                "--image",
                watermark.to_str().unwrap(),
                "--position",
                "center",
            ],
        ),
    ];
    let mut failures = Vec::new();
    for (operation, extra) in cases {
        let destination = temp.path().join(format!("{operation}.qoi"));
        let mut process = command(temp.path());
        process
            .args(["--output-format", "json", "image", operation, "--input"])
            .arg(&input)
            .args(extra)
            .arg("--output")
            .arg(&destination);
        let output = process.output().unwrap();
        if output.status.code() != Some(2) {
            failures.push(format!(
                "{operation} exit {:?}; stdout={}; stderr={}",
                output.status.code(),
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        let report = stdout_json(&output);
        if report["failures"][0]["code"] != "UNSUPPORTED_FORMAT" {
            failures.push(format!(
                "{operation} returned error code {:?}",
                report["failures"][0]["code"]
            ));
        }
        if destination.exists() {
            failures.push(format!("{operation} published a final output"));
        }
    }
    let leftovers: Vec<_> = std::fs::read_dir(temp.path())
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .filter(|name| name.to_string_lossy().contains(".rtools."))
        .collect();
    if !leftovers.is_empty() {
        failures.push(format!(
            "leftover temporary/reservation artifacts: {leftovers:?}"
        ));
    }
    assert!(failures.is_empty(), "{}", failures.join("; "));
}

#[test]
fn duplicate_cli_uses_configured_decoded_pixel_limit() {
    let temp = tempfile::tempdir().unwrap();
    create_png(&temp.path().join("oversized.png"), 3, 3);
    let config = temp.path().join("limits.toml");
    std::fs::write(&config, "[limits]\nmax_decoded_pixels = 4\n").unwrap();

    let output = command(temp.path())
        .arg("--config")
        .arg(&config)
        .args(["ai", "duplicates", "--input"])
        .arg(temp.path())
        .output()
        .unwrap();

    assert_exit(&output, 4);
}

#[test]
fn omitted_jpeg_quality_uses_image_default_quality_and_explicit_value_overrides_it() {
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("input.png");
    let configured = temp.path().join("configured.jpg");
    let explicit = temp.path().join("explicit.jpg");
    create_png(&input, 32, 32);

    let configured_run = command(temp.path())
        .env("RTOOLS_IMAGE__DEFAULT_QUALITY", "1")
        .args(["image", "compress", "--input"])
        .arg(&input)
        .args(["--format", "jpg", "--output"])
        .arg(&configured)
        .output()
        .unwrap();
    assert_exit(&configured_run, 0);

    let explicit_run = command(temp.path())
        .env("RTOOLS_IMAGE__DEFAULT_QUALITY", "1")
        .args(["image", "compress", "--input"])
        .arg(&input)
        .args(["--format", "jpg", "--quality", "85", "--output"])
        .arg(&explicit)
        .output()
        .unwrap();
    assert_exit(&explicit_run, 0);
    assert_ne!(
        std::fs::read(configured).unwrap(),
        std::fs::read(explicit).unwrap()
    );
}

#[test]
fn cli_applies_general_file_size_and_image_dimension_settings() {
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("input.png");
    create_png(&input, 2, 1);

    for (key, value) in [
        ("RTOOLS_GENERAL__MAX_FILE_SIZE", "1"),
        ("RTOOLS_IMAGE__MAX_DIMENSION", "1"),
    ] {
        let output_path = temp.path().join(format!("{value}-{key}.png"));
        let output = command(temp.path())
            .env(key, value)
            .args(["--output-format", "json", "image", "resize", "--input"])
            .arg(&input)
            .args(["--width", "1", "--height", "1", "--output"])
            .arg(&output_path)
            .output()
            .unwrap();
        assert_exit(&output, 4);
        assert_eq!(
            stdout_json(&output)["failures"][0]["code"],
            "RESOURCE_LIMIT_EXCEEDED"
        );
        assert!(!output_path.exists());
    }
}

#[test]
fn cli_rejects_requested_resize_dimension_above_configured_maximum() {
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("input.png");
    let output_path = temp.path().join("oversized-output.png");
    create_png(&input, 1, 1);

    let output = command(temp.path())
        .env("RTOOLS_IMAGE__MAX_DIMENSION", "1")
        .args(["--output-format", "json", "image", "resize", "--input"])
        .arg(&input)
        .args(["--width", "2", "--height", "1", "--output"])
        .arg(&output_path)
        .output()
        .unwrap();

    assert_exit(&output, 4);
    assert_eq!(
        stdout_json(&output)["failures"][0]["code"],
        "RESOURCE_LIMIT_EXCEEDED"
    );
    assert!(!output_path.exists());
}

#[test]
fn unsupported_behavioral_config_fails_before_file_access() {
    let temp = tempfile::tempdir().unwrap();
    for (key, value) in [
        ("RTOOLS_IMAGE__JPEG_QUALITY", "1"),
        ("RTOOLS_GENERAL__TEMP_DIR", "configured-temp"),
        ("RTOOLS_LIMITS__MAX_PDF_PAGES", "1"),
        ("RTOOLS_LIMITS__MAX_DURATION_MS", "1"),
    ] {
        let output = command(temp.path())
            .env(key, value)
            .args([
                "--output-format",
                "json",
                "image",
                "resize",
                "--input",
                "missing.png",
                "--width",
                "1",
            ])
            .output()
            .unwrap();

        assert_exit(&output, 3);
        assert_eq!(
            stdout_json(&output)["failures"][0]["code"],
            "CONFIGURATION_INVALID",
            "{key}"
        );
    }
}

#[test]
fn json_typed_parse_failure_is_one_invalid_input_report() {
    let temp = tempfile::tempdir().unwrap();
    let output = command(temp.path())
        .args([
            "--output-format",
            "json",
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
    let report = stdout_json(&output);
    assert_eq!(report["operation_id"], "cli.parse");
    assert_eq!(report["status"], "failure");
    assert_eq!(report["result"], Value::Null);
    assert_eq!(report["failures"][0]["code"], "INVALID_INPUT");
    assert!(output.stderr.is_empty(), "JSON diagnostics must stay clean");
}

#[test]
fn completions_with_missing_explicit_config_emit_no_shell_source() {
    let temp = tempfile::tempdir().unwrap();
    let missing = temp.path().join("missing.toml");
    let output = command(temp.path())
        .arg("--config")
        .arg(&missing)
        .args(["completions", "bash"])
        .output()
        .unwrap();

    assert_exit(&output, 3);
    assert!(output.stdout.is_empty(), "shell source must not be emitted");
    assert!(String::from_utf8_lossy(&output.stderr).contains("CONFIGURATION_INVALID"));
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
            ("ai.sort", "unavailable"),
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
            ("pdf.compress.level", "unavailable"),
            ("pdf.compress.metadata", "unavailable"),
            ("pdf.merge", "experimental"),
            ("pdf.merge.page_numbers", "unavailable"),
            ("pdf.ocr", "unavailable"),
            ("pdf.split", "experimental"),
            ("pdf.split.images", "unavailable"),
            ("pdf.text", "unavailable"),
            ("pdf.to_image", "unavailable"),
        ]
    );

    assert!(report["result"]["configured_limits"].is_object());
    assert!(report["result"]["writable_directories"].is_array());
    let providers = report["result"]["provider_diagnostics"]
        .as_array()
        .expect("doctor result must include structured provider diagnostics");
    let provider_ids: Vec<&str> = providers
        .iter()
        .map(|provider| provider["provider_id"].as_str().unwrap())
        .collect();
    assert_eq!(provider_ids, ["onnx-runtime", "pdfium", "tesseract"]);
    for provider in providers {
        assert_eq!(provider["state"], "unavailable");
        assert_eq!(provider["adapter_registered"], false);
        assert!(provider["operations"].is_array());
        assert!(provider["configuration"].is_object());
    }
    assert!(output.stderr.is_empty(), "JSON diagnostics must stay clean");
}

#[test]
fn doctor_human_output_includes_provider_diagnostics() {
    let temp = tempfile::tempdir().unwrap();
    let output = command(temp.path()).arg("doctor").output().unwrap();

    assert_exit(&output, 0);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Provider diagnostics:"));
    assert!(stdout.contains("tesseract"));
    assert!(stdout.contains("adapter registered: false"));
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

#[cfg(unix)]
#[test]
fn live_organize_rejects_linked_output_root_without_outside_artifacts() {
    let temp = tempfile::tempdir().unwrap();
    let input_dir = temp.path().join("photos");
    std::fs::create_dir(&input_dir).unwrap();
    let source = input_dir.join("photo.jpg");
    std::fs::write(&source, b"fixture").unwrap();
    let outside = temp.path().join("outside");
    std::fs::create_dir(&outside).unwrap();
    let linked = temp.path().join("linked");
    std::os::unix::fs::symlink(&outside, &linked).unwrap();

    let output = command(temp.path())
        .args(["--output-format", "json", "ai", "organize", "--input"])
        .arg(&input_dir)
        .arg("--output")
        .arg(&linked)
        .args(["--strategy", "date"])
        .output()
        .unwrap();

    assert_exit(&output, 5);
    assert_eq!(
        stdout_json(&output)["failures"][0]["code"],
        "PATH_POLICY_VIOLATION"
    );
    assert_eq!(std::fs::read_dir(outside).unwrap().count(), 0);
    assert_eq!(std::fs::read(source).unwrap(), b"fixture");
}
