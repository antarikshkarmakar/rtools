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
