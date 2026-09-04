#![allow(unused_crate_dependencies)]

use std::process::Command;

fn decode_base64(encoded: &str) -> Vec<u8> {
    let mut output = Vec::new();
    let mut accumulator = 0_u32;
    let mut bits = 0_u8;
    for byte in encoded.bytes().filter(|byte| !byte.is_ascii_whitespace()) {
        if byte == b'=' {
            break;
        }
        let value = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => panic!("invalid base64 fixture"),
        };
        accumulator = (accumulator << 6) | u32::from(value);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            output.push(u8::try_from(accumulator >> bits).unwrap());
            accumulator &= (1_u32 << bits) - 1;
        }
    }
    output
}

#[test]
fn human_image_command_displays_orientation_warning() {
    let tmp = tempfile::TempDir::new().unwrap();
    let input = tmp.path().join("orientation.jpg");
    let output = tmp.path().join("converted.png");
    let fixture = decode_base64(include_str!(
        "../../rtools-tests/fixtures/images/orientation-6.jpg.b64"
    ));
    std::fs::write(&input, fixture).unwrap();

    let result = Command::new(env!("CARGO_BIN_EXE_rtools"))
        .args([
            "image",
            "convert",
            "--input",
            input.to_str().unwrap(),
            "--format",
            "png",
            "--output",
            output.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let stdout = String::from_utf8(result.stdout).unwrap();
    assert!(
        stdout.contains("Warning: EXIF orientation 6 applied"),
        "{stdout}"
    );
    let image = image::open(output).unwrap();
    assert_eq!((image.width(), image.height()), (36, 24));
}

#[test]
fn mixed_image_inputs_keep_successful_warnings_and_report_item_failure() {
    let tmp = tempfile::TempDir::new().unwrap();
    let input = tmp.path().join("orientation.jpg");
    let missing = tmp.path().join("missing.jpg");
    let output = tmp.path().join("converted.png");
    let fixture = decode_base64(include_str!(
        "../../rtools-tests/fixtures/images/orientation-6.jpg.b64"
    ));
    std::fs::write(&input, fixture).unwrap();

    let result = Command::new(env!("CARGO_BIN_EXE_rtools"))
        .args(["--output-format", "json", "image", "convert", "--input"])
        .arg(&input)
        .arg(&missing)
        .args(["--format", "png", "--output"])
        .arg(&output)
        .output()
        .unwrap();

    assert_eq!(result.status.code(), Some(7));
    let report: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(report["status"], "partial_failure");
    assert_eq!(report["result"]["outputs"].as_array().unwrap().len(), 1);
    assert_eq!(
        report["warnings"],
        serde_json::json!(["EXIF orientation 6 applied"])
    );
    assert_eq!(
        report["result"]["outputs"][0]["warnings"],
        serde_json::json!(["EXIF orientation 6 applied"])
    );
    assert_eq!(report["failures"].as_array().unwrap().len(), 1);
    assert_eq!(report["failures"][0]["item"], missing.display().to_string());
}

#[test]
fn all_failed_image_inputs_keep_each_item_failure() {
    let tmp = tempfile::TempDir::new().unwrap();
    let first = tmp.path().join("missing-first.jpg");
    let second = tmp.path().join("missing-second.jpg");
    let output = tmp.path().join("converted.png");

    let result = Command::new(env!("CARGO_BIN_EXE_rtools"))
        .args(["--output-format", "json", "image", "convert", "--input"])
        .arg(&first)
        .arg(&second)
        .args(["--format", "png", "--output"])
        .arg(&output)
        .output()
        .unwrap();

    assert_eq!(result.status.code(), Some(6));
    let report: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(report["status"], "failure");
    assert_eq!(report["result"]["outputs"], serde_json::json!([]));
    assert_eq!(report["failures"].as_array().unwrap().len(), 2);
    assert_eq!(report["failures"][0]["item"], first.display().to_string());
    assert_eq!(report["failures"][1]["item"], second.display().to_string());
}

#[test]
fn human_mixed_image_inputs_keep_success_output_and_item_diagnostic() {
    let tmp = tempfile::TempDir::new().unwrap();
    let input = tmp.path().join("orientation.jpg");
    let missing = tmp.path().join("missing.jpg");
    let output = tmp.path().join("converted.png");
    let fixture = decode_base64(include_str!(
        "../../rtools-tests/fixtures/images/orientation-6.jpg.b64"
    ));
    std::fs::write(&input, fixture).unwrap();

    let result = Command::new(env!("CARGO_BIN_EXE_rtools"))
        .args(["image", "convert", "--input"])
        .arg(&input)
        .arg(&missing)
        .args(["--format", "png", "--output"])
        .arg(&output)
        .output()
        .unwrap();

    assert_eq!(result.status.code(), Some(7));
    let stdout = String::from_utf8_lossy(&result.stdout);
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(stdout.contains("Converted 1 image(s)"));
    assert!(stdout.contains("Warning: EXIF orientation 6 applied"));
    assert!(stderr.contains("Error [PROCESSING_FAILED]"));
    assert!(stderr.contains(&missing.display().to_string()));
}
