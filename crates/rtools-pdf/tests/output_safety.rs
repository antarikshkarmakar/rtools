use lopdf::{dictionary, Document, Object, Stream};
use rtools_core::{ErrorCode, FileInput, OutputPolicy, PendingOutput, Processor};
use rtools_pdf::compress::{PdfCompressConfig, PdfCompressProcessor, PdfCompressionLevel};
use rtools_pdf::merge::{PdfMergeConfig, PdfMergeProcessor};
use rtools_pdf::split::{PageRange, PdfSplitConfig, PdfSplitProcessor};
use serde as _;
use std::fs;
use std::path::Path;
use tempfile::tempdir;

#[test]
fn unsupported_pdf_compression_levels_fail_before_input_or_output_access() {
    let temp = tempfile::tempdir().unwrap();
    for level in [PdfCompressionLevel::Light, PdfCompressionLevel::Heavy] {
        let output = temp.path().join(format!("{level:?}.pdf"));
        let error = PdfCompressProcessor
            .process(
                FileInput::from_path(temp.path().join("missing.pdf")),
                PdfCompressConfig {
                    level,
                    output: Some(output.clone()),
                    remove_metadata: false,
                },
            )
            .unwrap_err();
        assert_eq!(error.code(), ErrorCode::CapabilityUnavailable);
        assert!(!output.exists());
    }
}

#[test]
fn pdf_metadata_removal_fails_closed_before_input_or_output_access() {
    let temp = tempfile::tempdir().unwrap();
    let output = temp.path().join("removed.pdf");
    let error = PdfCompressProcessor
        .process(
            FileInput::from_path(temp.path().join("missing-with-xmp.pdf")),
            PdfCompressConfig {
                level: PdfCompressionLevel::Medium,
                output: Some(output.clone()),
                remove_metadata: true,
            },
        )
        .unwrap_err();

    assert_eq!(error.code(), ErrorCode::CapabilityUnavailable);
    assert!(matches!(
        error,
        rtools_core::RToolsError::CapabilityUnavailable { operation_id, .. }
            if operation_id == "pdf.compress.metadata"
    ));
    assert!(!output.exists());
    assert_no_rtools_artifacts(temp.path());

    let xmp_input = temp.path().join("with-xmp.pdf");
    write_pdf_with_xmp(&xmp_input, b"PRIVATE-XMP-CANARY");
    let original = std::fs::read(&xmp_input).unwrap();
    assert!(original
        .windows(b"PRIVATE-XMP-CANARY".len())
        .any(|window| window == b"PRIVATE-XMP-CANARY"));
    let error = PdfCompressProcessor
        .process(
            FileInput::from_path(xmp_input.clone()),
            PdfCompressConfig {
                level: PdfCompressionLevel::Medium,
                output: Some(output.clone()),
                remove_metadata: true,
            },
        )
        .unwrap_err();
    assert_eq!(error.code(), ErrorCode::CapabilityUnavailable);
    assert_eq!(std::fs::read(xmp_input).unwrap(), original);
    assert!(!output.exists());
    assert_no_rtools_artifacts(temp.path());
}

#[test]
fn pdf_split_image_options_fail_before_input_or_output_access() {
    let temp = tempfile::tempdir().unwrap();
    let cases = [
        PdfSplitConfig {
            as_images: true,
            ..PdfSplitConfig::default()
        },
        PdfSplitConfig {
            image_format: Some("jpeg".to_string()),
            ..PdfSplitConfig::default()
        },
        PdfSplitConfig {
            image_dpi: 144,
            ..PdfSplitConfig::default()
        },
    ];
    for mut config in cases {
        config.output_dir = temp.path().join("missing-output");
        let error = PdfSplitProcessor
            .process(
                FileInput::from_path(temp.path().join("missing.pdf")),
                config,
            )
            .unwrap_err();
        assert_eq!(error.code(), ErrorCode::CapabilityUnavailable);
        assert!(!temp.path().join("missing-output").exists());
    }
}

#[test]
fn merge_page_numbering_fails_before_input_or_output_access() {
    let temp = tempdir().unwrap();
    let output_parent = temp.path().join("missing-output");
    let output = output_parent.join("numbered.pdf");

    let error = PdfMergeProcessor
        .process(
            vec![
                FileInput::from_path(temp.path().join("missing-first.pdf")),
                FileInput::from_path(temp.path().join("missing-second.pdf")),
            ],
            PdfMergeConfig {
                output: output.clone(),
                add_page_numbers: true,
                ..PdfMergeConfig::default()
            },
        )
        .unwrap_err();

    assert_eq!(error.code(), ErrorCode::CapabilityUnavailable);
    assert!(matches!(
        error,
        rtools_core::RToolsError::CapabilityUnavailable { operation_id, .. }
            if operation_id == "pdf.merge.page_numbers"
    ));
    assert!(!output_parent.exists());
    assert!(!output.exists());
}

#[test]
fn merge_rejects_legacy_config_inputs_before_filesystem_access() {
    let temp = tempdir().unwrap();
    let first = temp.path().join("first.pdf");
    let second = temp.path().join("second.pdf");
    let legacy = temp.path().join("legacy.pdf");
    write_pdf(&first, 1);
    write_pdf(&second, 1);
    write_pdf(&legacy, 1);
    let output = temp.path().join("merged.pdf");

    let error = PdfMergeProcessor
        .process(
            vec![FileInput::from_path(first), FileInput::from_path(second)],
            PdfMergeConfig {
                inputs: vec![legacy],
                output: output.clone(),
                add_page_numbers: false,
            },
        )
        .unwrap_err();

    assert_eq!(error.code(), ErrorCode::InvalidInput);
    assert!(!output.exists());
    assert_no_rtools_artifacts(temp.path());
}

#[test]
fn merge_rejects_mixed_path_and_memory_inputs_without_output_artifacts() {
    let temp = tempdir().unwrap();
    let first = temp.path().join("first.pdf");
    write_pdf(&first, 1);
    let output = temp.path().join("merged.pdf");

    let error = PdfMergeProcessor
        .process(
            vec![
                FileInput::from_path(first),
                FileInput::from_bytes(b"not a path".to_vec(), "second.pdf"),
            ],
            PdfMergeConfig {
                output: output.clone(),
                ..PdfMergeConfig::default()
            },
        )
        .unwrap_err();

    assert_eq!(error.code(), ErrorCode::InvalidInput);
    assert!(!output.exists());
    assert_no_rtools_artifacts(temp.path());
}

#[test]
fn merge_uses_processor_inputs_and_preserves_valid_merge_behavior() {
    let temp = tempdir().unwrap();
    let first = temp.path().join("first.pdf");
    let second = temp.path().join("second.pdf");
    write_pdf(&first, 1);
    write_pdf(&second, 2);
    let output = temp.path().join("merged.pdf");

    let result = PdfMergeProcessor
        .process(
            vec![FileInput::from_path(first), FileInput::from_path(second)],
            PdfMergeConfig {
                output: output.clone(),
                ..PdfMergeConfig::default()
            },
        )
        .unwrap();

    assert_eq!(result.destination.as_path(), Some(&output));
    rtools_pdf::validate_pdf_artifact(&output).unwrap();
    assert!(result
        .stats
        .as_ref()
        .is_some_and(|stats| stats.input_size > 0));
    assert_no_rtools_artifacts(temp.path());
}

#[test]
fn merge_commits_without_retirement_artifacts_on_the_workspace_filesystem() {
    let current_directory = std::env::current_dir().unwrap();
    let temp = tempfile::tempdir_in(current_directory).unwrap();
    let first = temp.path().join("first.pdf");
    let second = temp.path().join("second.pdf");
    let output = temp.path().join("merged.pdf");
    write_pdf(&first, 1);
    write_pdf(&second, 1);

    let result = PdfMergeProcessor
        .process(
            vec![FileInput::from_path(first), FileInput::from_path(second)],
            PdfMergeConfig {
                output: output.clone(),
                ..PdfMergeConfig::default()
            },
        )
        .unwrap();

    assert_eq!(result.destination.as_path(), Some(&output));
    rtools_pdf::validate_pdf_artifact(&output).unwrap();
    assert_no_rtools_artifacts(temp.path());
}

#[cfg(unix)]
fn create_directory_symlink(target: &Path, link: &Path) {
    std::os::unix::fs::symlink(target, link).unwrap_or_else(|error| {
        panic!(
            "failed to create directory symlink {} -> {}: {error}",
            link.display(),
            target.display()
        )
    });
}

#[cfg(windows)]
fn create_directory_symlink(target: &Path, link: &Path) {
    std::os::windows::fs::symlink_dir(target, link).unwrap_or_else(|error| {
        panic!(
            "failed to create directory symlink {} -> {}: {error}. Enable Windows Developer Mode or grant SeCreateSymbolicLinkPrivilege so CreateSymbolicLink can succeed; this safety regression must not be skipped",
            link.display(),
            target.display()
        )
    });
}

#[cfg(any(unix, windows))]
#[test]
fn merge_rejects_symlinked_missing_parent_without_creating_outside() {
    let temp = tempdir().unwrap();
    let first = temp.path().join("first.pdf");
    let second = temp.path().join("second.pdf");
    write_pdf(&first, 1);
    write_pdf(&second, 1);
    let first_bytes = fs::read(&first).unwrap();
    let second_bytes = fs::read(&second).unwrap();
    let selected = temp.path().join("selected");
    let outside = temp.path().join("outside");
    fs::create_dir(&selected).unwrap();
    fs::create_dir(&outside).unwrap();
    create_directory_symlink(&outside, &selected.join("link"));
    let outside_child = outside.join("new-child");
    let output = selected.join("link/new-child/result.pdf");

    let error = PdfMergeProcessor
        .process(
            vec![
                FileInput::from_path(first.clone()),
                FileInput::from_path(second.clone()),
            ],
            PdfMergeConfig {
                output,
                ..PdfMergeConfig::default()
            },
        )
        .unwrap_err();

    assert_eq!(error.code(), ErrorCode::PathPolicyViolation);
    assert!(!outside_child.exists());
    assert!(!outside_child.join("result.pdf").exists());
    assert_eq!(fs::read(first).unwrap(), first_bytes);
    assert_eq!(fs::read(second).unwrap(), second_bytes);
    assert_no_rtools_artifacts(&outside);
}

#[cfg(any(unix, windows))]
#[test]
fn compress_rejects_symlinked_missing_parent_without_creating_outside() {
    let temp = tempdir().unwrap();
    let input = temp.path().join("input.pdf");
    write_pdf(&input, 1);
    let input_bytes = fs::read(&input).unwrap();
    let selected = temp.path().join("selected");
    let outside = temp.path().join("outside");
    fs::create_dir(&selected).unwrap();
    fs::create_dir(&outside).unwrap();
    create_directory_symlink(&outside, &selected.join("link"));
    let outside_child = outside.join("new-child");
    let output = selected.join("link/new-child/result.pdf");

    let error = PdfCompressProcessor
        .process(
            FileInput::from_path(input.clone()),
            PdfCompressConfig {
                output: Some(output),
                ..PdfCompressConfig::default()
            },
        )
        .unwrap_err();

    assert_eq!(error.code(), ErrorCode::PathPolicyViolation);
    assert!(!outside_child.exists());
    assert!(!outside_child.join("result.pdf").exists());
    assert_eq!(fs::read(input).unwrap(), input_bytes);
    assert_no_rtools_artifacts(&outside);
}

#[cfg(any(unix, windows))]
#[test]
fn split_rejects_symlinked_missing_parent_without_creating_outside() {
    let temp = tempdir().unwrap();
    let input = temp.path().join("input.pdf");
    write_pdf(&input, 2);
    let input_bytes = fs::read(&input).unwrap();
    let selected = temp.path().join("selected");
    let outside = temp.path().join("outside");
    fs::create_dir(&selected).unwrap();
    fs::create_dir(&outside).unwrap();
    create_directory_symlink(&outside, &selected.join("link"));
    let outside_child = outside.join("new-child");
    let output_dir = selected.join("link/new-child");

    let error = PdfSplitProcessor
        .process(
            FileInput::from_path(input.clone()),
            PdfSplitConfig {
                range: PageRange::All,
                output_dir,
                ..PdfSplitConfig::default()
            },
        )
        .unwrap_err();

    assert_eq!(error.code(), ErrorCode::PathPolicyViolation);
    assert!(!outside_child.exists());
    assert!(!outside_child.join("page_1.pdf").exists());
    assert!(!outside_child.join("page_2.pdf").exists());
    assert_eq!(fs::read(input).unwrap(), input_bytes);
    assert_no_rtools_artifacts(&outside);
}

#[test]
fn merge_default_collision_preserves_existing_output() {
    let temp = tempdir().unwrap();
    let first = temp.path().join("first.pdf");
    let second = temp.path().join("second.pdf");
    write_pdf(&first, 1);
    write_pdf(&second, 1);
    let output = temp.path().join("merged.pdf");
    fs::write(&output, b"existing merge output").unwrap();

    let error = PdfMergeProcessor
        .process(
            vec![FileInput::from_path(first), FileInput::from_path(second)],
            PdfMergeConfig {
                output: output.clone(),
                ..PdfMergeConfig::default()
            },
        )
        .unwrap_err();

    assert_eq!(error.code(), ErrorCode::OutputExists);
    assert_eq!(fs::read(output).unwrap(), b"existing merge output");
    assert_no_rtools_artifacts(temp.path());
}

#[test]
fn compress_default_collision_preserves_existing_output() {
    let temp = tempdir().unwrap();
    let input = temp.path().join("input.pdf");
    write_pdf(&input, 1);
    let output = temp.path().join("compressed.pdf");
    fs::write(&output, b"existing compressed output").unwrap();

    let error = PdfCompressProcessor
        .process(
            FileInput::from_path(input),
            PdfCompressConfig {
                output: Some(output.clone()),
                ..PdfCompressConfig::default()
            },
        )
        .unwrap_err();

    assert_eq!(error.code(), ErrorCode::OutputExists);
    assert_eq!(fs::read(output).unwrap(), b"existing compressed output");
    assert_no_rtools_artifacts(temp.path());
}

#[test]
fn split_late_collision_publishes_no_pages_and_preserves_existing_output() {
    let temp = tempdir().unwrap();
    let input = temp.path().join("two-pages.pdf");
    write_pdf(&input, 2);
    let output_dir = temp.path().join("split");
    fs::create_dir(&output_dir).unwrap();
    let late_output = output_dir.join("page_2.pdf");
    fs::write(&late_output, b"existing second page").unwrap();

    let error = PdfSplitProcessor
        .process(
            FileInput::from_path(input),
            PdfSplitConfig {
                range: PageRange::All,
                output_dir: output_dir.clone(),
                ..PdfSplitConfig::default()
            },
        )
        .unwrap_err();

    assert_eq!(error.code(), ErrorCode::OutputExists);
    assert!(!output_dir.join("page_1.pdf").exists());
    assert_eq!(fs::read(late_output).unwrap(), b"existing second page");
    assert_no_rtools_artifacts(&output_dir);
}

#[test]
fn split_late_competing_reservation_publishes_no_pages() {
    let temp = tempdir().unwrap();
    let input = temp.path().join("two-pages.pdf");
    write_pdf(&input, 2);
    let output_dir = temp.path().join("split");
    fs::create_dir(&output_dir).unwrap();
    let late_output = output_dir.join("page_2.pdf");
    let competing = PendingOutput::new(&late_output, OutputPolicy::FailIfExists).unwrap();

    let error = PdfSplitProcessor
        .process(
            FileInput::from_path(input),
            PdfSplitConfig {
                range: PageRange::All,
                output_dir: output_dir.clone(),
                ..PdfSplitConfig::default()
            },
        )
        .unwrap_err();

    assert_eq!(error.code(), ErrorCode::OutputExists);
    assert!(!output_dir.join("page_1.pdf").exists());
    assert!(!late_output.exists());
    drop(competing);
    assert_no_rtools_artifacts(&output_dir);
}

fn assert_no_rtools_artifacts(directory: &Path) {
    let leftovers: Vec<_> = fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .filter(|name| name.to_string_lossy().contains("rtools"))
        .collect();
    assert!(leftovers.is_empty(), "leftover artifacts: {leftovers:?}");
}

fn write_pdf(path: &Path, page_count: u32) {
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

fn write_pdf_with_xmp(path: &Path, xmp: &[u8]) {
    write_pdf(path, 1);
    let mut document = Document::load(path).unwrap();
    let metadata_id = document.add_object(Stream::new(
        dictionary! { "Type" => "Metadata", "Subtype" => "XML" },
        xmp.to_vec(),
    ));
    let catalog_id = document
        .trailer
        .get(b"Root")
        .unwrap()
        .as_reference()
        .unwrap();
    document
        .get_object_mut(catalog_id)
        .unwrap()
        .as_dict_mut()
        .unwrap()
        .set("Metadata", metadata_id);
    document.save(path).unwrap();
}
