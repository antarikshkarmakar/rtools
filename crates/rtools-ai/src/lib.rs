pub mod alt_text;
pub mod duplicates;
pub mod organize;
pub mod ocr;
pub mod rename;
pub mod sort;

// Re-export main types
pub use alt_text::{AltTextConfig, AltTextProcessor};
pub use duplicates::{DuplicatesConfig, DuplicatesProcessor};
pub use organize::{OrganizeConfig, OrganizeProcessor};
pub use ocr::{OcrConfig, OcrProcessor};
pub use rename::{RenameConfig, RenameProcessor};
pub use sort::{SortConfig, SortProcessor};