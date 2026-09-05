use lopdf::Document;
use rtools_core::{OutputPolicy, PendingOutput, RToolsError, RToolsResult};
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};

pub fn save_pdf(
    document: &mut Document,
    output: &Path,
    description: &str,
) -> RToolsResult<PathBuf> {
    let pending = PendingOutput::new(output, OutputPolicy::FailIfExists)?;
    encode_pdf(document, pending.temporary_path(), description)?;
    commit_pdf(pending)
}

pub fn encode_pdf(
    document: &mut Document,
    temporary_path: &Path,
    description: &str,
) -> RToolsResult<()> {
    let mut target = File::create(temporary_path)
        .map_err(|error| RToolsError::pdf(format!("Failed to save {description}: {error}")))?;
    encode_pdf_to(document, &mut target, description)
}

fn encode_pdf_to<W: Write>(
    document: &mut Document,
    target: &mut W,
    description: &str,
) -> RToolsResult<()> {
    document
        .save_to(target)
        .map_err(|error| RToolsError::pdf(format!("Failed to save {description}: {error}")))
}

/// Validate that a path contains a parseable PDF artifact.
///
/// # Errors
///
/// Returns a PDF processing error when the file cannot be loaded as a PDF.
pub fn validate_pdf_artifact(path: &Path) -> RToolsResult<()> {
    Document::load(path)
        .map(|_| ())
        .map_err(|error| RToolsError::pdf(format!("Failed to validate generated PDF: {error}")))
}

pub fn commit_pdf(pending: PendingOutput) -> RToolsResult<PathBuf> {
    pending.commit(validate_pdf_artifact)
}

#[cfg(test)]
mod tests {
    use super::{commit_pdf, encode_pdf_to};
    use lopdf::{dictionary, Document, Object, Stream};
    use rtools_core::{ErrorCode, OutputPolicy, PendingOutput};
    use std::fs::{self, File};
    use std::io::{self, Write};
    use std::path::Path;

    struct FailAfter {
        file: File,
        remaining: usize,
    }

    impl Write for FailAfter {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if self.remaining == 0 {
                return Err(io::Error::other("injected partial PDF write failure"));
            }
            let byte_count = bytes.len().min(self.remaining);
            let written = self.file.write(&bytes[..byte_count])?;
            self.remaining -= written;
            Ok(written)
        }

        fn flush(&mut self) -> io::Result<()> {
            self.file.flush()
        }
    }

    #[test]
    fn partial_encoder_failure_removes_private_pdf_artifacts() {
        let directory = tempfile::tempdir().unwrap();
        let output = directory.path().join("result.pdf");
        let pending = PendingOutput::new(&output, OutputPolicy::FailIfExists).unwrap();
        let temporary = pending.temporary_path().to_owned();
        let mut writer = FailAfter {
            file: File::create(&temporary).unwrap(),
            remaining: 16,
        };
        let mut document = one_page_document();

        let error = encode_pdf_to(&mut document, &mut writer, "test PDF").unwrap_err();
        drop(writer);
        drop(pending);

        assert_eq!(error.code(), ErrorCode::ProcessingFailed);
        assert!(!output.exists());
        assert!(!temporary.exists());
        assert_no_rtools_artifacts(directory.path());
    }

    #[test]
    fn validation_failure_removes_private_pdf_artifacts() {
        let directory = tempfile::tempdir().unwrap();
        let output = directory.path().join("result.pdf");
        let pending = PendingOutput::new(&output, OutputPolicy::FailIfExists).unwrap();
        let temporary = pending.temporary_path().to_owned();
        fs::write(&temporary, b"not a complete PDF").unwrap();

        let error = commit_pdf(pending).unwrap_err();

        assert_eq!(error.code(), ErrorCode::ProcessingFailed);
        assert!(!output.exists());
        assert!(!temporary.exists());
        assert_no_rtools_artifacts(directory.path());
    }

    fn assert_no_rtools_artifacts(directory: &Path) {
        let leftovers: Vec<_> = fs::read_dir(directory)
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .filter(|name| name.to_string_lossy().contains("rtools"))
            .collect();
        assert!(leftovers.is_empty(), "leftover artifacts: {leftovers:?}");
    }

    fn one_page_document() -> Document {
        let mut document = Document::with_version("1.5");
        let pages_id = document.new_object_id();
        let catalog_id = document.new_object_id();
        let content_id = document.add_object(Stream::new(dictionary! {}, Vec::new()));
        let single_page_id = document.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => vec![0.into(), 0.into(), 100.into(), 100.into()],
            "Contents" => content_id,
        });
        document.objects.insert(
            pages_id,
            Object::Dictionary(dictionary! {
                "Type" => "Pages",
                "Kids" => vec![Object::Reference(single_page_id)],
                "Count" => 1,
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
        document
    }
}
