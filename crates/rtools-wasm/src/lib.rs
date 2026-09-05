use image::{
    AnimationDecoder, DynamicImage, ImageDecoder, ImageError, ImageFormat, ImageReader, Limits,
};
use rtools_core::ResourceLimits;
use std::io::{BufReader, Cursor};
use wasm_bindgen::prelude::*;

const MAX_DECODED_BYTES_PER_PIXEL: u64 = 16;

#[derive(Debug)]
struct WasmError {
    code: &'static str,
    message: String,
}

impl WasmError {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    fn into_js(self) -> JsError {
        JsError::new(&format!("{}: {}", self.code, self.message))
    }
}

#[wasm_bindgen]
#[derive(Default)]
pub struct RTools;

#[wasm_bindgen]
impl RTools {
    #[allow(clippy::missing_const_for_fn)]
    #[wasm_bindgen(constructor)]
    pub fn new() -> RTools {
        RTools
    }

    /// Compress an image while retaining a supported source format.
    ///
    /// # Errors
    /// Returns a stable JavaScript error when validation, decoding, or encoding fails.
    pub fn compress_image(&self, data: &[u8], quality: f64) -> Result<Vec<u8>, JsError> {
        process_compress(data, quality, &ResourceLimits::default()).map_err(WasmError::into_js)
    }

    /// Convert an image to an explicitly supported target format.
    ///
    /// # Errors
    /// Returns a stable JavaScript error when validation, decoding, or encoding fails.
    pub fn convert_image(
        &self,
        data: &[u8],
        target_format: &str,
        quality: f64,
    ) -> Result<Vec<u8>, JsError> {
        process_convert(data, target_format, quality, &ResourceLimits::default())
            .map_err(WasmError::into_js)
    }

    /// Resize an image to exact non-zero dimensions while retaining its format.
    ///
    /// # Errors
    /// Returns a stable JavaScript error when validation, decoding, or encoding fails.
    pub fn resize_image(&self, data: &[u8], width: u32, height: u32) -> Result<Vec<u8>, JsError> {
        process_resize(data, width, height, &ResourceLimits::default()).map_err(WasmError::into_js)
    }

    /// Crop a fully in-bounds, non-zero rectangle while retaining source format.
    ///
    /// # Errors
    /// Returns a stable JavaScript error when validation, decoding, or encoding fails.
    pub fn crop_image(
        &self,
        data: &[u8],
        x: u32,
        y: u32,
        width: u32,
        height: u32,
    ) -> Result<Vec<u8>, JsError> {
        process_crop(data, x, y, width, height, &ResourceLimits::default())
            .map_err(WasmError::into_js)
    }

    /// Return bounded image metadata as a JavaScript value.
    ///
    /// # Errors
    /// Returns a stable JavaScript error when validation, decoding, or serialization fails.
    pub fn get_metadata(&self, data: &[u8]) -> Result<JsValue, JsError> {
        let (image, format) =
            decode_bounded_memory(data, &ResourceLimits::default()).map_err(WasmError::into_js)?;
        let metadata = serde_json::json!({
            "width": image.width(),
            "height": image.height(),
            "format": format_name(format),
            "color_depth": image.color().bits_per_pixel(),
        });
        serde_wasm_bindgen::to_value(&metadata)
            .map_err(|error| WasmError::new("PROCESSING_FAILED", error.to_string()).into_js())
    }

    /// Generate a bounded PNG thumbnail constrained by a positive maximum size.
    ///
    /// # Errors
    /// Returns a stable JavaScript error when validation, decoding, or encoding fails.
    pub fn generate_thumbnail(&self, data: &[u8], max_size: u32) -> Result<Vec<u8>, JsError> {
        process_thumbnail(data, max_size, &ResourceLimits::default()).map_err(WasmError::into_js)
    }
}

fn process_compress(
    data: &[u8],
    quality: f64,
    limits: &ResourceLimits,
) -> Result<Vec<u8>, WasmError> {
    let quality = validate_quality(quality)?;
    let (image, format) = decode_bounded_memory(data, limits)?;
    validate_effective_quality(format, quality)?;
    encode_with_format(&image, format, quality)
}

fn process_convert(
    data: &[u8],
    target_format: &str,
    quality: f64,
    limits: &ResourceLimits,
) -> Result<Vec<u8>, WasmError> {
    let quality = validate_quality(quality)?;
    let format = parse_format(target_format)?;
    validate_effective_quality(format, quality)?;
    let (image, _) = decode_bounded_memory(data, limits)?;
    encode_with_format(&image, format, quality)
}

fn process_resize(
    data: &[u8],
    width: u32,
    height: u32,
    limits: &ResourceLimits,
) -> Result<Vec<u8>, WasmError> {
    validate_geometry(width, height, limits)?;
    let (image, format) = decode_bounded_memory(data, limits)?;
    let resized = image.resize_exact(width, height, image::imageops::FilterType::Lanczos3);
    encode_with_format(&resized, format, 100)
}

fn process_crop(
    data: &[u8],
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    limits: &ResourceLimits,
) -> Result<Vec<u8>, WasmError> {
    validate_geometry(width, height, limits)?;
    let (image, format) = decode_bounded_memory(data, limits)?;
    let right = x
        .checked_add(width)
        .ok_or_else(|| WasmError::new("INVALID_INPUT", "crop rectangle coordinates overflow"))?;
    let bottom = y
        .checked_add(height)
        .ok_or_else(|| WasmError::new("INVALID_INPUT", "crop rectangle coordinates overflow"))?;
    if right > image.width() || bottom > image.height() {
        return Err(WasmError::new(
            "INVALID_INPUT",
            "crop rectangle must be fully within the source image",
        ));
    }
    encode_with_format(&image.crop_imm(x, y, width, height), format, 100)
}

fn process_thumbnail(
    data: &[u8],
    max_size: u32,
    limits: &ResourceLimits,
) -> Result<Vec<u8>, WasmError> {
    if max_size == 0 {
        return Err(WasmError::new(
            "INVALID_INPUT",
            "thumbnail maximum size must be positive",
        ));
    }
    let (image, _) = decode_bounded_memory(data, limits)?;
    let (output_width, output_height) =
        fitted_dimensions(image.width(), image.height(), max_size, max_size);
    check_pixels(output_width, output_height, limits)?;
    let thumbnail = image.resize(max_size, max_size, image::imageops::FilterType::Lanczos3);
    encode_with_format(&thumbnail, ImageFormat::Png, 100)
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn fitted_dimensions(width: u32, height: u32, max_width: u32, max_height: u32) -> (u32, u32) {
    let ratio =
        (f64::from(max_width) / f64::from(width)).min(f64::from(max_height) / f64::from(height));
    let fitted_width = ((f64::from(width) * ratio).round() as u64).max(1);
    let fitted_height = ((f64::from(height) * ratio).round() as u64).max(1);
    (
        u32::try_from(fitted_width).unwrap_or(u32::MAX),
        u32::try_from(fitted_height).unwrap_or(u32::MAX),
    )
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn validate_quality(quality: f64) -> Result<u8, WasmError> {
    if !quality.is_finite() || quality.fract() != 0.0 || !(1.0..=100.0).contains(&quality) {
        return Err(WasmError::new(
            "INVALID_INPUT",
            "quality must be a finite integer from 1 through 100",
        ));
    }
    Ok(quality as u8)
}

fn validate_effective_quality(format: ImageFormat, quality: u8) -> Result<(), WasmError> {
    if format == ImageFormat::WebP && quality != 100 {
        Err(WasmError::new(
            "CAPABILITY_UNAVAILABLE",
            "WebP encoding is lossless-only; use quality 100",
        ))
    } else {
        Ok(())
    }
}

fn validate_geometry(width: u32, height: u32, limits: &ResourceLimits) -> Result<(), WasmError> {
    if width == 0 || height == 0 {
        return Err(WasmError::new(
            "INVALID_INPUT",
            "width and height must be positive",
        ));
    }
    check_pixels(width, height, limits)
}

fn check_pixels(width: u32, height: u32, limits: &ResourceLimits) -> Result<(), WasmError> {
    let pixels = u64::from(width)
        .checked_mul(u64::from(height))
        .ok_or_else(|| WasmError::new("RESOURCE_LIMIT_EXCEEDED", "pixel count overflow"))?;
    if pixels > limits.max_decoded_pixels {
        return Err(WasmError::new(
            "RESOURCE_LIMIT_EXCEEDED",
            format!(
                "decoded_pixels {pixels} exceeds limit {}",
                limits.max_decoded_pixels
            ),
        ));
    }
    Ok(())
}

fn decoder_limits(resource_limits: &ResourceLimits) -> Limits {
    let mut limits = Limits::default();
    let pixel_cap = resource_limits
        .max_decoded_pixels
        .saturating_mul(MAX_DECODED_BYTES_PER_PIXEL);
    limits.max_alloc = limits.max_alloc.map(|default| default.min(pixel_cap));
    limits
}

fn map_image_error(error: ImageError, allocation_limit: u64) -> WasmError {
    match error {
        ImageError::Limits(_) => WasmError::new(
            "RESOURCE_LIMIT_EXCEEDED",
            format!("image decoder allocation exceeds limit {allocation_limit}"),
        ),
        other => WasmError::new("PROCESSING_FAILED", other.to_string()),
    }
}

fn multiple_frames(
    mut frames: impl Iterator<Item = image::ImageResult<image::Frame>>,
    allocation_limit: u64,
) -> Result<bool, WasmError> {
    let Some(first) = frames.next() else {
        return Ok(false);
    };
    first.map_err(|error| map_image_error(error, allocation_limit))?;
    match frames.next() {
        Some(second) => {
            second.map_err(|error| map_image_error(error, allocation_limit))?;
            Ok(true)
        }
        None => Ok(false),
    }
}

fn reject_animation(
    data: &[u8],
    format: ImageFormat,
    limits: Limits,
    allocation_limit: u64,
) -> Result<(), WasmError> {
    let animated = match format {
        ImageFormat::Png => {
            let mut decoder = image::codecs::png::PngDecoder::new(Cursor::new(data))
                .map_err(|error| map_image_error(error, allocation_limit))?;
            decoder
                .set_limits(limits)
                .map_err(|error| map_image_error(error, allocation_limit))?;
            if decoder
                .is_apng()
                .map_err(|error| map_image_error(error, allocation_limit))?
            {
                multiple_frames(
                    decoder
                        .apng()
                        .map_err(|error| map_image_error(error, allocation_limit))?
                        .into_frames(),
                    allocation_limit,
                )?
            } else {
                false
            }
        }
        ImageFormat::WebP => {
            let mut decoder =
                image::codecs::webp::WebPDecoder::new(BufReader::new(Cursor::new(data)))
                    .map_err(|error| map_image_error(error, allocation_limit))?;
            decoder
                .set_limits(limits)
                .map_err(|error| map_image_error(error, allocation_limit))?;
            multiple_frames(decoder.into_frames(), allocation_limit)?
        }
        ImageFormat::Jpeg => false,
        _ => unreachable!("unsupported formats are rejected before animation probing"),
    };
    if animated {
        return Err(WasmError::new(
            "CAPABILITY_UNAVAILABLE",
            "animated inputs are not supported by single-frame WASM operations",
        ));
    }
    Ok(())
}

fn supported_source_format(data: &[u8]) -> Result<ImageFormat, WasmError> {
    let format = image::guess_format(data).map_err(|_| {
        WasmError::new(
            "UNSUPPORTED_FORMAT",
            "cannot determine a supported source format",
        )
    })?;
    match format {
        ImageFormat::Jpeg | ImageFormat::Png | ImageFormat::WebP => Ok(format),
        _ => Err(WasmError::new(
            "UNSUPPORTED_FORMAT",
            "WASM supports JPEG, PNG, and WebP inputs only",
        )),
    }
}

fn decode_bounded_memory(
    data: &[u8],
    limits: &ResourceLimits,
) -> Result<(DynamicImage, ImageFormat), WasmError> {
    let input_len = u64::try_from(data.len()).unwrap_or(u64::MAX);
    if input_len > limits.max_input_bytes {
        return Err(WasmError::new(
            "RESOURCE_LIMIT_EXCEEDED",
            format!(
                "input_bytes {input_len} exceeds limit {}",
                limits.max_input_bytes
            ),
        ));
    }
    let format = supported_source_format(data)?;
    let header_cap = Limits::default().max_alloc.unwrap_or(u64::MAX);
    let header = ImageReader::new(Cursor::new(data))
        .with_guessed_format()
        .map_err(|error| WasmError::new("PROCESSING_FAILED", error.to_string()))?;
    let (width, height) = header
        .into_dimensions()
        .map_err(|error| map_image_error(error, header_cap))?;
    check_pixels(width, height, limits)?;
    let decoder_limits = decoder_limits(limits);
    let allocation_limit = decoder_limits.max_alloc.unwrap_or(u64::MAX);
    reject_animation(data, format, decoder_limits.clone(), allocation_limit)?;
    let mut reader = ImageReader::new(Cursor::new(data))
        .with_guessed_format()
        .map_err(|error| WasmError::new("PROCESSING_FAILED", error.to_string()))?;
    reader.limits(decoder_limits);
    let image = reader
        .decode()
        .map_err(|error| map_image_error(error, allocation_limit))?;
    Ok((image, format))
}

fn encode_with_format(
    image: &DynamicImage,
    format: ImageFormat,
    quality: u8,
) -> Result<Vec<u8>, WasmError> {
    let mut output = Cursor::new(Vec::new());
    match format {
        ImageFormat::Jpeg => image.write_with_encoder(
            image::codecs::jpeg::JpegEncoder::new_with_quality(&mut output, quality),
        ),
        ImageFormat::Png => {
            let compression = match quality {
                1..=33 => image::codecs::png::CompressionType::Best,
                34..=66 => image::codecs::png::CompressionType::Default,
                _ => image::codecs::png::CompressionType::Fast,
            };
            image.write_with_encoder(image::codecs::png::PngEncoder::new_with_quality(
                &mut output,
                compression,
                image::codecs::png::FilterType::Adaptive,
            ))
        }
        ImageFormat::WebP => {
            image.write_with_encoder(image::codecs::webp::WebPEncoder::new_lossless(&mut output))
        }
        _ => unreachable!("only supported output formats reach the encoder"),
    }
    .map_err(|error| WasmError::new("PROCESSING_FAILED", error.to_string()))?;
    Ok(output.into_inner())
}

fn parse_format(format: &str) -> Result<ImageFormat, WasmError> {
    match format.to_ascii_lowercase().as_str() {
        "jpg" | "jpeg" => Ok(ImageFormat::Jpeg),
        "png" => Ok(ImageFormat::Png),
        "webp" => Ok(ImageFormat::WebP),
        _ => Err(WasmError::new(
            "UNSUPPORTED_FORMAT",
            format!("unsupported WASM output format: {format}"),
        )),
    }
}

const fn format_name(format: ImageFormat) -> &'static str {
    match format {
        ImageFormat::Jpeg => "jpeg",
        ImageFormat::Png => "png",
        ImageFormat::WebP => "webp",
        _ => "unsupported",
    }
}

#[allow(clippy::missing_const_for_fn)]
#[wasm_bindgen]
pub fn init() {}

#[cfg(test)]
mod tests {
    use super::*;

    fn encoded(format: ImageFormat, width: u32, height: u32) -> Vec<u8> {
        let image = DynamicImage::new_rgba8(width, height);
        let mut output = Cursor::new(Vec::new());
        image.write_to(&mut output, format).unwrap();
        output.into_inner()
    }

    fn limits(max_input_bytes: u64, max_decoded_pixels: u64) -> ResourceLimits {
        ResourceLimits {
            max_input_bytes,
            max_decoded_pixels,
            ..ResourceLimits::default()
        }
    }

    #[test]
    fn bounded_decode_rejects_input_bytes_and_declared_pixels() {
        let png = encoded(ImageFormat::Png, 2, 2);
        assert_eq!(
            decode_bounded_memory(&png, &limits(1, 100))
                .unwrap_err()
                .code,
            "RESOURCE_LIMIT_EXCEEDED"
        );
        assert_eq!(
            decode_bounded_memory(&png, &limits(10_000, 3))
                .unwrap_err()
                .code,
            "RESOURCE_LIMIT_EXCEEDED"
        );
    }

    #[test]
    fn unsupported_container_is_rejected_instead_of_becoming_png() {
        for format in [ImageFormat::Bmp, ImageFormat::Ico] {
            let input = encoded(format, 1, 1);
            assert_eq!(
                process_resize(&input, 1, 1, &ResourceLimits::default())
                    .unwrap_err()
                    .code,
                "UNSUPPORTED_FORMAT"
            );
        }
    }

    #[test]
    fn geometry_and_quality_are_strict() {
        let png = encoded(ImageFormat::Png, 2, 2);
        let defaults = ResourceLimits::default();
        assert_eq!(
            process_resize(&png, 0, 1, &defaults).unwrap_err().code,
            "INVALID_INPUT"
        );
        assert_eq!(
            process_crop(&png, 1, 1, 2, 1, &defaults).unwrap_err().code,
            "INVALID_INPUT"
        );
        assert_eq!(
            process_compress(&png, 0.0, &defaults).unwrap_err().code,
            "INVALID_INPUT"
        );
        for invalid in [101.0, 1.5, f64::NAN, f64::INFINITY] {
            assert_eq!(
                process_compress(&png, invalid, &defaults).unwrap_err().code,
                "INVALID_INPUT"
            );
        }
        assert!(process_compress(&png, 1.0, &defaults).is_ok());
        assert!(process_compress(&png, 100.0, &defaults).is_ok());
        assert_eq!(
            process_resize(&png, 3, 2, &limits(10_000, 4))
                .unwrap_err()
                .code,
            "RESOURCE_LIMIT_EXCEEDED"
        );
        assert_eq!(
            process_thumbnail(&png, 3, &limits(10_000, 4))
                .unwrap_err()
                .code,
            "RESOURCE_LIMIT_EXCEEDED"
        );
    }

    #[test]
    fn proven_formats_are_retained_and_conversion_is_explicit() {
        let defaults = ResourceLimits::default();
        for format in [ImageFormat::Jpeg, ImageFormat::Png, ImageFormat::WebP] {
            let input = encoded(format, 2, 2);
            let resized = process_resize(&input, 1, 1, &defaults).unwrap();
            assert_eq!(image::guess_format(&resized).unwrap(), format);
        }
        let png = encoded(ImageFormat::Png, 2, 2);
        let jpeg = process_convert(&png, "jpeg", 85.0, &defaults).unwrap();
        assert_eq!(image::guess_format(&jpeg).unwrap(), ImageFormat::Jpeg);
        assert_eq!(
            process_convert(&png, "bmp", 85.0, &defaults)
                .unwrap_err()
                .code,
            "UNSUPPORTED_FORMAT"
        );
    }

    #[test]
    fn unsupported_animation_container_is_rejected() {
        let mut bytes = Vec::new();
        {
            let mut encoder = image::codecs::gif::GifEncoder::new(&mut bytes);
            encoder
                .encode_frames([
                    image::Frame::new(image::RgbaImage::new(1, 1)),
                    image::Frame::new(image::RgbaImage::new(1, 1)),
                ])
                .unwrap();
        }
        assert_eq!(
            process_resize(&bytes, 1, 1, &ResourceLimits::default())
                .unwrap_err()
                .code,
            "UNSUPPORTED_FORMAT"
        );
    }

    #[test]
    fn supported_animated_container_is_rejected_without_flattening() {
        use base64::Engine as _;
        for fixture in [
            include_str!("../../rtools-tests/fixtures/images/two-frame.webp.b64"),
            include_str!("../../rtools-tests/fixtures/images/two-frame.apng.b64"),
        ] {
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(fixture.trim())
                .unwrap();
            assert_eq!(
                process_resize(&bytes, 1, 1, &ResourceLimits::default())
                    .unwrap_err()
                    .code,
                "CAPABILITY_UNAVAILABLE"
            );
        }
    }

    #[test]
    fn one_frame_extended_container_is_not_misclassified_as_animation() {
        use base64::Engine as _;
        for fixture in [
            include_str!("../../rtools-tests/fixtures/images/single-frame-extended.webp.b64"),
            include_str!("../../rtools-tests/fixtures/images/single-frame.apng.b64"),
        ] {
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(fixture.trim())
                .unwrap();
            process_resize(&bytes, 1, 1, &ResourceLimits::default()).unwrap();
        }
    }

    #[test]
    fn rtools_new_is_host_testable() {
        let _rtools = RTools::new();
    }
}
