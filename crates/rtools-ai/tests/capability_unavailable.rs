use rtools_ai::{AltTextConfig, AltTextProcessor, OcrConfig, OcrProcessor};
use rtools_core::{ErrorCode, FileInput, Processor, RToolsError};
use std::path::PathBuf;
use {chrono as _, icu_casemap as _, image as _, serde as _, tempfile as _};

#[test]
fn alt_text_returns_capability_error_instead_of_filename_caption() {
    let error = AltTextProcessor
        .process(
            FileInput::from_path(PathBuf::from("private-secret-vacation.jpg")),
            AltTextConfig::default(),
        )
        .unwrap_err();

    assert_unavailable(error, "ai.alt_text");
}

#[test]
fn image_ocr_returns_capability_error_instead_of_sample_confidence() {
    let error = OcrProcessor
        .process(
            FileInput::from_path(PathBuf::from("private-secret-scan.png")),
            OcrConfig::default(),
        )
        .unwrap_err();

    assert_unavailable(error, "ai.ocr");
}

fn assert_unavailable(error: RToolsError, expected_operation_id: &str) {
    assert_eq!(error.code(), ErrorCode::CapabilityUnavailable);
    let rendered = error.to_string();
    assert!(!rendered.contains("private-secret"));
    match error {
        RToolsError::CapabilityUnavailable { operation_id, .. } => {
            assert_eq!(operation_id, expected_operation_id);
        }
        other => panic!("expected capability error, got {other:?}"),
    }
}
