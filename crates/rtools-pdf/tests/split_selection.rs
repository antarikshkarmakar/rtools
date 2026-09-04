use lopdf::{dictionary, Document, Object, Stream};
use rtools_core::{ErrorCode, FileInput, Processor};
use rtools_pdf::split::{PageRange, PdfSplitConfig, PdfSplitProcessor};
use serde as _;
use tempfile::tempdir;

#[test]
fn all_out_of_range_selection_is_invalid_before_output_creation() {
    let temp = tempdir().unwrap();
    let input = temp.path().join("two-pages.pdf");
    write_pdf(&input, 2);
    let output = temp.path().join("out");

    let error = PdfSplitProcessor
        .process(
            FileInput::from_path(input),
            PdfSplitConfig {
                range: PageRange::Range { start: 7, end: 9 },
                output_dir: output.clone(),
                ..PdfSplitConfig::default()
            },
        )
        .unwrap_err();

    assert_eq!(error.code(), ErrorCode::InvalidInput);
    assert!(!output.exists());
}

#[test]
fn mixed_range_extracts_in_range_pages_and_valid_selection_still_works() {
    let temp = tempdir().unwrap();
    let input = temp.path().join("two-pages.pdf");
    write_pdf(&input, 2);
    let output = temp.path().join("out");
    std::fs::create_dir(&output).unwrap();

    let outputs = PdfSplitProcessor
        .process(
            FileInput::from_path(input),
            PdfSplitConfig {
                range: PageRange::Multiple(vec![
                    PageRange::Single(1),
                    PageRange::Range { start: 7, end: 9 },
                ]),
                output_dir: output.clone(),
                ..PdfSplitConfig::default()
            },
        )
        .unwrap();

    assert_eq!(outputs.len(), 1);
    assert!(output.join("page_1.pdf").exists());
}

fn write_pdf(path: &std::path::Path, page_count: u32) {
    let mut document = Document::with_version("1.5");
    let pages_id = document.new_object_id();
    let catalog_id = document.new_object_id();
    let mut kids = Vec::new();
    for _ in 0..page_count {
        let content_id = document.add_object(Stream::new(dictionary! {}, Vec::new()));
        let page_object_id = document.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => vec![0.into(), 0.into(), 100.into(), 100.into()],
            "Contents" => content_id,
        });
        kids.push(Object::Reference(page_object_id));
    }
    document.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Kids" => kids,
            "Count" => page_count,
        }),
    );
    document.objects.insert(
        catalog_id,
        Object::Dictionary(dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
        }),
    );
    document.trailer.set("Root", catalog_id);
    document.save(path).unwrap();
}
