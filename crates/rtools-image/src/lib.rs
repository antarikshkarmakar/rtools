pub mod compress;
pub mod convert;
pub mod crop;
pub mod exif;
pub mod filter;
pub mod format;
pub mod metadata;
pub mod pdf2img;
pub mod resize;
pub mod watermark;

// Re-export main types
pub use compress::{CompressConfig, CompressProcessor};
pub use convert::{ConvertConfig, ConvertProcessor};
pub use crop::{CropConfig, CropProcessor};
pub use exif::{ExifConfig, ExifProcessor};
pub use filter::{FilterConfig, FilterProcessor};
pub use metadata::{MetadataConfig, MetadataProcessor};
pub use resize::{ResizeConfig, ResizeProcessor};
pub use watermark::{WatermarkConfig, WatermarkProcessor};

#[cfg(test)]
use tempfile as _;
