use image::{DynamicImage, ImageDecoder, ImageError, ImageReader, Limits};
use rtools_core::types::ImageFormat;
use rtools_core::{RToolsError, RToolsResult, ResourceLimits};
use std::fs::File;
use std::io::{Cursor, Read};
use std::path::Path;

const MAX_DECODED_BYTES_PER_PIXEL: u64 = 16;

fn decoder_limits(resource_limits: &ResourceLimits) -> Limits {
    let mut decoder_limits = Limits::default();
    let pixel_derived_cap = resource_limits
        .max_decoded_pixels
        .saturating_mul(MAX_DECODED_BYTES_PER_PIXEL);
    // Only narrow image's finite default. The dependency applies cloned limits
    // at separate decoder-internal and output-reservation enforcement points;
    // this is not an aggregate allocation budget across those points.
    decoder_limits.max_alloc = decoder_limits
        .max_alloc
        .map(|dependency_cap| dependency_cap.min(pixel_derived_cap));
    decoder_limits
}

fn map_decode_error(error: ImageError, allocation_limit: u64) -> RToolsError {
    match error {
        ImageError::Limits(_) => RToolsError::ResourceLimitExceededUnknownActual {
            resource: "image_decoder_allocation_bytes",
            limit: allocation_limit,
        },
        error => RToolsError::image(error.to_string()),
    }
}

/// Decode an image after checking its input size and decoded pixel count.
///
/// # Errors
///
/// Returns a structured resource-limit error when the input or decoder exceeds
/// a configured limit, or an image/I/O error when the source cannot be read or
/// decoded.
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
    let per_enforcement_point_cap = decoder_limits.max_alloc.unwrap_or(u64::MAX);
    let mut reader = ImageReader::new(Cursor::new(encoded.as_slice())).with_guessed_format()?;
    reader.limits(decoder_limits.clone());
    let decoder = reader
        .into_decoder()
        .map_err(|error| map_decode_error(error, per_enforcement_point_cap))?;
    let (width, height) = decoder.dimensions();
    limits.check_decoded_pixels(width, height)?;
    drop(decoder);

    // Separately from decoder-internal checks, `ImageReader::decode` reserves
    // the output buffer bytes against the same finite per-point cap before
    // `DynamicImage::from_decoder` allocates that buffer.
    let mut reader = ImageReader::new(Cursor::new(encoded.as_slice())).with_guessed_format()?;
    reader.limits(decoder_limits);
    reader
        .decode()
        .map_err(|error| map_decode_error(error, per_enforcement_point_cap))
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

#[cfg(test)]
mod tests {
    use super::{decoder_limits, ResourceLimits};

    #[test]
    fn default_resource_limits_do_not_raise_dependency_allocation_cap() {
        let dependency_cap = image::Limits::default()
            .max_alloc
            .expect("image default allocation cap must be finite");
        let effective_cap = decoder_limits(&ResourceLimits::default())
            .max_alloc
            .expect("effective allocation cap must be finite");

        assert!(
            effective_cap <= dependency_cap,
            "effective cap {effective_cap} exceeded dependency default {dependency_cap}"
        );
    }

    #[test]
    fn extreme_pixel_limit_keeps_finite_dependency_allocation_cap() {
        let dependency_cap = image::Limits::default()
            .max_alloc
            .expect("image default allocation cap must be finite");
        let effective_cap = decoder_limits(&ResourceLimits {
            max_decoded_pixels: u64::MAX,
            ..ResourceLimits::default()
        })
        .max_alloc
        .expect("effective allocation cap must be finite");

        assert!(effective_cap < u64::MAX);
        assert!(effective_cap <= dependency_cap);
    }
}
