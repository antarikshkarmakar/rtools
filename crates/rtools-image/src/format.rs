use image::{AnimationDecoder, DynamicImage, ImageDecoder, ImageError, ImageReader, Limits};
use rtools_core::types::ImageFormat;
use rtools_core::{RToolsError, RToolsResult, ResourceLimits};
use std::fs::File;
use std::io::{BufReader, Cursor, Read};
use std::path::Path;

const MAX_DECODED_BYTES_PER_PIXEL: u64 = 16;

/// Result of decoding one immutable, resource-bounded image snapshot.
#[derive(Debug)]
pub struct DecodedImage {
    /// Pixels after any EXIF orientation transform.
    pub image: DynamicImage,
    /// The EXIF orientation value when a transform was applied.
    pub orientation_applied: Option<u32>,
}

impl DecodedImage {
    /// Return the output warning required when orientation changed the pixels.
    pub fn warnings(&self) -> Vec<String> {
        self.orientation_applied
            .map_or_else(Vec::new, |orientation| {
                vec![format!("EXIF orientation {orientation} applied")]
            })
    }
}

/// Apply the pixel transform defined by an EXIF orientation value.
///
/// Unknown values, `1`, and absent-orientation callers remain unchanged.
pub fn apply_exif_orientation(image: DynamicImage, orientation: u32) -> DynamicImage {
    match orientation {
        2 => image.fliph(),
        3 => image.rotate180(),
        4 => image.flipv(),
        5 => image.rotate90().fliph(),
        6 => image.rotate90(),
        7 => image.rotate90().flipv(),
        8 => image.rotate270(),
        _ => image,
    }
}

fn exif_orientation(encoded: &[u8]) -> Option<u32> {
    let mut cursor = Cursor::new(encoded);
    let exif = exif::Reader::new().read_from_container(&mut cursor).ok()?;
    exif.get_field(exif::Tag::Orientation, exif::In::PRIMARY)
        .and_then(|field| match &field.value {
            exif::Value::Short(values) => values.first().copied().map(u32::from),
            _ => None,
        })
}

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

fn multiple_frames(
    mut frames: impl Iterator<Item = image::ImageResult<image::Frame>>,
    allocation_limit: u64,
) -> RToolsResult<bool> {
    if let Some(first) = frames.next() {
        first.map_err(|error| map_decode_error(error, allocation_limit))?;
    } else {
        return Ok(false);
    }
    match frames.next() {
        Some(second) => {
            second.map_err(|error| map_decode_error(error, allocation_limit))?;
            Ok(true)
        }
        None => Ok(false),
    }
}

fn reject_animated_input(
    encoded: &[u8],
    format: image::ImageFormat,
    limits: Limits,
    allocation_limit: u64,
) -> RToolsResult<()> {
    let animated = match format {
        image::ImageFormat::Gif => {
            let mut decoder =
                image::codecs::gif::GifDecoder::new(BufReader::new(Cursor::new(encoded)))
                    .map_err(|error| map_decode_error(error, allocation_limit))?;
            decoder
                .set_limits(limits)
                .map_err(|error| map_decode_error(error, allocation_limit))?;
            multiple_frames(decoder.into_frames(), allocation_limit)?
        }
        image::ImageFormat::Png => {
            let mut decoder = image::codecs::png::PngDecoder::new(Cursor::new(encoded))
                .map_err(|error| map_decode_error(error, allocation_limit))?;
            decoder
                .set_limits(limits)
                .map_err(|error| map_decode_error(error, allocation_limit))?;
            if decoder
                .is_apng()
                .map_err(|error| map_decode_error(error, allocation_limit))?
            {
                let apng = decoder
                    .apng()
                    .map_err(|error| map_decode_error(error, allocation_limit))?;
                multiple_frames(apng.into_frames(), allocation_limit)?
            } else {
                false
            }
        }
        image::ImageFormat::WebP => {
            let mut decoder =
                image::codecs::webp::WebPDecoder::new(BufReader::new(Cursor::new(encoded)))
                    .map_err(|error| map_decode_error(error, allocation_limit))?;
            decoder
                .set_limits(limits)
                .map_err(|error| map_decode_error(error, allocation_limit))?;
            multiple_frames(decoder.into_frames(), allocation_limit)?
        }
        _ => false,
    };

    if animated {
        return Err(RToolsError::capability_unavailable(
            "image.animation.single_frame",
            "Animated inputs cannot be processed by single-frame image operations",
            "Use a non-animated image or an animation-aware processor",
        ));
    }
    Ok(())
}

/// Decode an image after checking its input size and decoded pixel count.
///
/// # Errors
///
/// Returns a structured resource-limit error when the input or decoder exceeds
/// a configured limit, or an image/I/O error when the source cannot be read or
/// decoded.
pub fn decode_bounded(path: &Path, limits: &ResourceLimits) -> RToolsResult<DecodedImage> {
    // Open once, validate that handle, and make every decoder consume the
    // resulting immutable bytes. The bounded read also catches files that grow
    // after the metadata check without reading beyond the configured limit + 1.
    let encoded = read_bounded_snapshot(path, limits)?;

    let orientation = exif_orientation(&encoded);
    let header_reader = ImageReader::new(Cursor::new(encoded.as_slice())).with_guessed_format()?;
    let format = header_reader
        .format()
        .ok_or_else(|| RToolsError::unsupported_format("Cannot determine image format"))?;
    // Read only enough of the format header to obtain dimensions before
    // applying the caller-derived allocation cap. Some decoders reserve small
    // bookkeeping buffers while being constructed, and a very small pixel
    // budget must not hide a precise declared-canvas violation behind that
    // dependency-internal allocation error.
    let header_allocation_cap = Limits::default().max_alloc.unwrap_or(u64::MAX);
    let (width, height) = header_reader
        .into_dimensions()
        .map_err(|error| map_decode_error(error, header_allocation_cap))?;
    limits.check_decoded_pixels(width, height)?;

    let decoder_limits = decoder_limits(limits);
    let per_enforcement_point_cap = decoder_limits.max_alloc.unwrap_or(u64::MAX);
    reject_animated_input(
        &encoded,
        format,
        decoder_limits.clone(),
        per_enforcement_point_cap,
    )?;

    // Separately from decoder-internal checks, `ImageReader::decode` reserves
    // the output buffer bytes against the same finite per-point cap before
    // `DynamicImage::from_decoder` allocates that buffer.
    let mut reader = ImageReader::new(Cursor::new(encoded.as_slice())).with_guessed_format()?;
    reader.limits(decoder_limits);
    let image = reader
        .decode()
        .map_err(|error| map_decode_error(error, per_enforcement_point_cap))?;
    let orientation_applied = orientation.filter(|orientation| (2..=8).contains(orientation));
    let image = match orientation_applied {
        Some(orientation) => apply_exif_orientation(image, orientation),
        None => image,
    };

    Ok(DecodedImage {
        image,
        orientation_applied,
    })
}

pub(crate) fn read_bounded_snapshot(path: &Path, limits: &ResourceLimits) -> RToolsResult<Vec<u8>> {
    let file = File::open(path)?;
    limits.check_input_bytes(file.metadata()?.len())?;
    let mut encoded = Vec::new();
    file.take(limits.max_input_bytes.saturating_add(1))
        .read_to_end(&mut encoded)?;
    limits.check_input_bytes(u64::try_from(encoded.len()).unwrap_or(u64::MAX))?;
    Ok(encoded)
}

/// Identify an encoded image from its bytes under the configured input limit.
///
/// # Errors
///
/// Returns a resource-limit error for oversized input and an unsupported-format
/// error when the encoded bytes do not identify a public rTools image format.
pub fn identify_bounded_format(path: &Path, limits: &ResourceLimits) -> RToolsResult<ImageFormat> {
    let encoded = read_bounded_snapshot(path, limits)?;
    let actual = image::guess_format(&encoded)
        .map_err(|_| RToolsError::unsupported_format("Cannot determine encoded image format"))?;
    actual
        .extensions_str()
        .first()
        .and_then(|extension| ImageFormat::from_extension(extension))
        .ok_or_else(|| RToolsError::unsupported_format("Encoded image format is unsupported"))
}

/// Resolve one encoder format and one public MIME-bearing format from an
/// output path before any output reservation is created.
pub(crate) fn resolve_output_format(
    path: &Path,
    operation: &str,
) -> RToolsResult<(ImageFormat, image::ImageFormat)> {
    let public_format = ImageFormat::from_path(path).ok_or_else(|| {
        RToolsError::unsupported_format(format!("{operation} output format is unsupported"))
    })?;
    let encoder_format = image::ImageFormat::from_path(path).map_err(|_| {
        RToolsError::unsupported_format(format!("{operation} output format is unsupported"))
    })?;
    Ok((public_format, encoder_format))
}

/// Verify the encoded bytes advertise the exact format selected before
/// output reservation.
pub(crate) fn verify_image_artifact_format(
    path: &Path,
    limits: &ResourceLimits,
    expected: image::ImageFormat,
) -> RToolsResult<()> {
    let encoded = read_bounded_snapshot(path, limits)?;
    let actual = image::guess_format(&encoded)
        .map_err(|_| RToolsError::image("encoded image format validation failed"))?;
    if actual != expected {
        return Err(RToolsError::image(
            "encoded image format did not match the requested output format",
        ));
    }
    Ok(())
}

/// Reopen and fully decode a newly encoded image within the configured limits
/// before it becomes visible.
pub(crate) fn validate_image_artifact(path: &Path, limits: &ResourceLimits) -> RToolsResult<()> {
    decode_bounded(path, limits)
        .map(|_| ())
        .map_err(|error| match error {
            RToolsError::Image(message) => {
                RToolsError::image(format!("encoded image validation failed: {message}"))
            }
            error => error,
        })
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
    use super::{decoder_limits, validate_image_artifact, ResourceLimits};
    use rtools_core::RToolsError;

    fn crc32(bytes: &[u8]) -> [u8; 4] {
        let mut crc = u32::MAX;
        for byte in bytes {
            crc ^= u32::from(*byte);
            for _ in 0..8 {
                crc = if crc & 1 == 1 {
                    (crc >> 1) ^ 0xedb8_8320
                } else {
                    crc >> 1
                };
            }
        }
        (!crc).to_be_bytes()
    }

    fn write_png_header(path: &std::path::Path, width: u32, height: u32) {
        let mut header = Vec::with_capacity(58);
        header.extend_from_slice(b"\x89PNG\r\n\x1a\n");
        header.extend_from_slice(&13_u32.to_be_bytes());
        header.extend_from_slice(b"IHDR");
        header.extend_from_slice(&width.to_be_bytes());
        header.extend_from_slice(&height.to_be_bytes());
        header.extend_from_slice(&[8, 6, 0, 0, 0]);
        header.extend_from_slice(&crc32(&header[12..]));
        header.extend_from_slice(&1_u32.to_be_bytes());
        header.extend_from_slice(b"IDAT");
        header.push(0);
        header.extend_from_slice(&crc32(b"IDAT\0"));
        header.extend_from_slice(&0_u32.to_be_bytes());
        header.extend_from_slice(b"IEND");
        header.extend_from_slice(&crc32(b"IEND"));
        std::fs::write(path, header).unwrap();
    }

    #[test]
    fn generated_artifact_validation_checks_declared_canvas_before_decode() {
        let tmp = tempfile::tempdir().unwrap();
        let artifact = tmp.path().join("declared-canvas.png");
        write_png_header(&artifact, 64, 64);
        let limits = ResourceLimits {
            max_decoded_pixels: 4,
            ..ResourceLimits::default()
        };

        let error = validate_image_artifact(&artifact, &limits).unwrap_err();

        assert!(
            matches!(
                error,
                RToolsError::ResourceLimitExceeded {
                    resource: "decoded_pixels",
                    actual: 4_096,
                    limit: 4,
                }
            ),
            "unexpected error: {error:?}"
        );
    }

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
