use image::{ImageBuffer, Rgba};
use rtools_core::{ErrorCode, FileInput, ImageFormat, Processor, RToolsError};
use rtools_image::compress::{CompressConfig, CompressionPreset};
use rtools_image::convert::ConvertConfig;
use rtools_image::pdf2img::{Pdf2ImgConfig, Pdf2ImgProcessor};
use rtools_image::watermark::{
    WatermarkConfig, WatermarkPosition, WatermarkProcessor, WatermarkType,
};
use rtools_image::{CompressProcessor, ConvertProcessor};
use tempfile::tempdir;
use {exif as _, serde as _, tracing as _};

#[test]
fn pdf_to_image_returns_capability_error_instead_of_empty_success() {
    let temp = tempdir().unwrap();
    let output_dir = temp.path().join("pages");
    let error = Pdf2ImgProcessor
        .process(
            FileInput::from_path(temp.path().join("input.pdf")),
            Pdf2ImgConfig {
                output_dir: output_dir.clone(),
                ..Pdf2ImgConfig::default()
            },
        )
        .unwrap_err();

    assert_unavailable(error, "pdf.to_image");
    assert!(!output_dir.exists());
}

#[test]
fn text_watermark_fails_before_output_reservation() {
    let temp = tempdir().unwrap();
    let input = temp.path().join("input.png");
    let output = temp.path().join("output.png");
    write_test_image(&input);

    let error = WatermarkProcessor
        .process(
            FileInput::from_path(input),
            WatermarkConfig {
                watermark: WatermarkType::Text {
                    text: "not-a-rectangle".to_string(),
                    font_size: 24,
                    font_color: "#ffffff".to_string(),
                },
                position: WatermarkPosition::BottomRight,
                output: Some(output.clone()),
                ..WatermarkConfig::default()
            },
        )
        .unwrap_err();

    assert_unavailable(error, "image.watermark.text");
    assert!(!output.exists());
}

#[test]
fn metadata_preserve_fails_before_compression_output() {
    let temp = tempdir().unwrap();
    let input = temp.path().join("input.png");
    let output = temp.path().join("output.png");
    write_test_image(&input);

    let error = CompressProcessor
        .process(
            FileInput::from_path(input),
            CompressConfig {
                preset: CompressionPreset::Balanced,
                output: Some(output.clone()),
                preserve_metadata: true,
                strip_gps: false,
                ..CompressConfig::default()
            },
        )
        .unwrap_err();

    assert_unavailable(error, "image.metadata.preserve");
    assert!(!output.exists());
}

#[test]
fn metadata_strip_gps_fails_before_conversion_output() {
    let temp = tempdir().unwrap();
    let input = temp.path().join("input.png");
    let output = temp.path().join("output.jpg");
    write_test_image(&input);

    let error = ConvertProcessor
        .process(
            FileInput::from_path(input),
            ConvertConfig {
                target_format: ImageFormat::Jpeg,
                output: Some(output.clone()),
                preserve_metadata: false,
                strip_gps: true,
                ..ConvertConfig::default()
            },
        )
        .unwrap_err();

    assert_unavailable(error, "image.metadata.strip_gps");
    assert!(!output.exists());
}

#[test]
fn conflicting_metadata_flags_remain_invalid_input() {
    let error = CompressProcessor
        .process(
            FileInput::from_path("unused.png".into()),
            CompressConfig {
                preserve_metadata: true,
                strip_gps: true,
                ..CompressConfig::default()
            },
        )
        .unwrap_err();

    assert_eq!(error.code(), ErrorCode::InvalidInput);
}

fn assert_unavailable(error: RToolsError, expected_operation_id: &str) {
    assert_eq!(error.code(), ErrorCode::CapabilityUnavailable);
    match error {
        RToolsError::CapabilityUnavailable { operation_id, .. } => {
            assert_eq!(operation_id, expected_operation_id);
        }
        other => panic!("expected capability error, got {other:?}"),
    }
}

fn write_test_image(path: &std::path::Path) {
    ImageBuffer::from_pixel(4, 4, Rgba([10_u8, 20, 30, 255]))
        .save(path)
        .unwrap();
}
