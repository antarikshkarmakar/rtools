use image::{DynamicImage, ImageDecoder, ImageError, ImageReader, Limits};
use rtools_core::types::ImageFormat;
use rtools_core::{RToolsError, RToolsResult, ResourceLimits};
use std::fs::File;
use std::io::{Cursor, Read};
use std::path::Path;

const MAX_DECODED_BYTES_PER_PIXEL: u64 = 16;

fn decoder_limits(resource_limits: &ResourceLimits) -> Limits {
    let mut decoder_limits = Limits::default();
    decoder_limits.max_alloc = Some(
        resource_limits
            .max_decoded_pixels
            .saturating_mul(MAX_DECODED_BYTES_PER_PIXEL),
    );
    decoder_limits
}

fn map_decode_error(error: ImageError, allocation_limit: u64) -> RToolsError {
    match error {
        ImageError::Limits(_) => RToolsError::ResourceLimitExceeded {
            resource: "decoded_bytes",
            actual: allocation_limit.saturating_add(1),
            limit: allocation_limit,
        },
        error => RToolsError::image(error.to_string()),
    }
}

/// Decode an image after checking its input size and decoded pixel count.
///
/// # Errors
///
/// Returns `ResourceLimitExceeded` when the input exceeds a configured limit,
/// or an image/I/O error when the source cannot be read or decoded.
pub fn decode_bounded(path: &Path, limits: &ResourceLimits) -> RToolsResult<DynamicImage> {
    // Open once, validate that handle, and make every decoder consume the
    // resulting immutable bytes. The bounded read also catches files that grow
    // after the metadata check without reading beyond the configured limit + 1.
    let file = File::open(path)?;
    limits.check_input_bytes(file.metadata()?.len())?;
    let mut encoded = Vec::new();
    file.take(limits.max_input_bytes.saturating_add(1))
        .read_to_end(&mut encoded)?;
    limits.check_input_bytes(u64::try_from(encoded.len()).unwrap_or(u64::MAX))?;

    let decoder_limits = decoder_limits(limits);
    let allocation_limit = decoder_limits.max_alloc.unwrap_or(u64::MAX);
    let mut reader = ImageReader::new(Cursor::new(encoded.as_slice())).with_guessed_format()?;
    reader.limits(decoder_limits.clone());
    let decoder = reader
        .into_decoder()
        .map_err(|error| map_decode_error(error, allocation_limit))?;
    let (width, height) = decoder.dimensions();
    limits.check_decoded_pixels(width, height)?;
    drop(decoder);

    // `ImageReader::decode` reserves the decoder's total output bytes against
    // the finite cap before `DynamicImage::from_decoder` allocates its buffer.
    let mut reader = ImageReader::new(Cursor::new(encoded.as_slice())).with_guessed_format()?;
    reader.limits(decoder_limits);
    reader
        .decode()
        .map_err(|error| map_decode_error(error, allocation_limit))
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
