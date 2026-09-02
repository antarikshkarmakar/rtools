use image::{DynamicImage, ImageDecoder, ImageReader};
use rtools_core::types::ImageFormat;
use rtools_core::{RToolsError, RToolsResult, ResourceLimits};
use std::fs;
use std::path::Path;

/// Decode an image after checking its input size and decoded pixel count.
///
/// # Errors
///
/// Returns `ResourceLimitExceeded` when the input exceeds a configured limit,
/// or an image/I/O error when the source cannot be read or decoded.
pub fn decode_bounded(path: &Path, limits: &ResourceLimits) -> RToolsResult<DynamicImage> {
    // Keep this order: byte metadata check, decoder/header parse, checked pixel
    // limit, then full DynamicImage allocation. Callers invoke this before
    // creating output directories or files.
    limits.check_input_bytes(fs::metadata(path)?.len())?;

    let mut reader = ImageReader::open(path)?.with_guessed_format()?;
    // The crate's default allocation limit can reject the header before our
    // authoritative pixel limit is checked. The decoder is only used to read
    // dimensions here; `check_decoded_pixels` runs before `from_decoder`
    // allocates the full image buffer.
    reader.no_limits();
    let decoder = reader
        .into_decoder()
        .map_err(|error| RToolsError::image(error.to_string()))?;
    let (width, height) = decoder.dimensions();
    limits.check_decoded_pixels(width, height)?;

    DynamicImage::from_decoder(decoder).map_err(|error| RToolsError::image(error.to_string()))
}

/// Image format utilities
pub const fn is_lossy(format: &ImageFormat) -> bool {
    matches!(
        format,
        ImageFormat::Jpeg | ImageFormat::Webp | ImageFormat::Avif
    )
}

pub const fn supports_transparency(format: &ImageFormat) -> bool {
    matches!(
        format,
        ImageFormat::Png
            | ImageFormat::Webp
            | ImageFormat::Avif
            | ImageFormat::Gif
            | ImageFormat::Ico
    )
}
