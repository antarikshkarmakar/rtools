use rtools_core::{ErrorCode, FileInput, Processor, RToolsError};
use rtools_pdf::ocr::{OcrOutputFormat, PdfOcrConfig, PdfOcrProcessor};
use tempfile::tempdir;
use {lopdf as _, serde as _};

#[test]
fn searchable_pdf_ocr_does_not_copy_source_or_create_output() {
    assert_pdf_ocr_unavailable(OcrOutputFormat::SearchablePdf, "searchable.pdf");
}

#[test]
fn text_pdf_ocr_does_not_mislabel_embedded_text_extraction_as_ocr() {
    assert_pdf_ocr_unavailable(OcrOutputFormat::Text, "ocr.txt");
}

fn assert_pdf_ocr_unavailable(output_format: OcrOutputFormat, output_name: &str) {
    let temp = tempdir().unwrap();
    let input = temp.path().join("source.pdf");
    let output = temp.path().join(output_name);
    std::fs::write(&input, b"private source bytes").unwrap();

    let error = PdfOcrProcessor
        .process(
            FileInput::from_path(input),
            PdfOcrConfig {
                output: Some(output.clone()),
                output_format,
                ..PdfOcrConfig::default()
            },
        )
        .unwrap_err();

    assert_eq!(error.code(), ErrorCode::CapabilityUnavailable);
    match error {
        RToolsError::CapabilityUnavailable { operation_id, .. } => {
            assert_eq!(operation_id, "pdf.ocr");
        }
        other => panic!("expected capability error, got {other:?}"),
    }
    assert!(!output.exists());
}
