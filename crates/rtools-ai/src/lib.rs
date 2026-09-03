pub mod alt_text;
pub mod duplicates;
pub mod ocr;
pub mod organize;
pub mod rename;
pub mod sort;

#[cfg(test)]
use tempfile as _;

// Re-export main types
pub use alt_text::{AltTextConfig, AltTextProcessor};
pub use duplicates::{DuplicatesConfig, DuplicatesProcessor};
pub use ocr::{OcrConfig, OcrProcessor};
pub use organize::{OrganizeConfig, OrganizeProcessor};
pub use rename::{RenameConfig, RenameProcessor};
pub use sort::{SortConfig, SortProcessor};
