pub mod compress;
pub mod encrypt;
pub mod extract;
pub mod merge;
pub mod metadata;
pub mod ocr;
pub mod redact;
pub mod split;

// Re-export main types
pub use compress::{PdfCompressConfig, PdfCompressProcessor};
pub use merge::{PdfMergeConfig, PdfMergeProcessor};
pub use metadata::{PdfMetadataConfig, PdfMetadataProcessor};
pub use ocr::{PdfOcrConfig, PdfOcrProcessor};
pub use split::{PdfSplitConfig, PdfSplitProcessor};

#[cfg(test)]
use tempfile as _;
