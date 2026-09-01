use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct RTools;

#[wasm_bindgen]
impl RTools {
    #[wasm_bindgen(constructor)]
    pub fn new() -> RTools {
        RTools
    }

    pub fn compress_image(&self, data: &[u8], quality: u8) -> Result<Vec<u8>, JsError> {
        let img = image::load_from_memory(data).map_err(|e| JsError::new(&e.to_string()))?;
        let format = sniff_output_format(data)?;

        encode_with_format(img, format, quality)
    }

    pub fn convert_image(
        &self,
        data: &[u8],
        target_format: &str,
        quality: u8,
    ) -> Result<Vec<u8>, JsError> {
        let img = image::load_from_memory(data).map_err(|e| JsError::new(&e.to_string()))?;
        let format = parse_format(target_format)?;

        encode_with_format(img, format, quality)
    }

    pub fn resize_image(
        &self,
        data: &[u8],
        width: u32,
        height: u32,
    ) -> Result<Vec<u8>, JsError> {
        let img = image::load_from_memory(data).map_err(|e| JsError::new(&e.to_string()))?;
        let resized = img.resize(width, height, image::imageops::FilterType::Lanczos3);
        let format = sniff_output_format(data)?;

        encode_with_format(resized, format, 100)
    }

    pub fn crop_image(
        &self,
        data: &[u8],
        x: u32,
        y: u32,
        width: u32,
        height: u32,
    ) -> Result<Vec<u8>, JsError> {
        let mut img = image::load_from_memory(data).map_err(|e| JsError::new(&e.to_string()))?;
        let cropped = img.crop(x, y, width, height);
        let format = sniff_output_format(data)?;

        encode_with_format(cropped, format, 100)
    }

    pub fn get_metadata(&self, data: &[u8]) -> Result<JsValue, JsError> {
        let img = image::load_from_memory(data).map_err(|e| JsError::new(&e.to_string()))?;

        let metadata = serde_json::json!({
            "width": img.width(),
            "height": img.height(),
            "format": format!("{:?}", img.color()),
            "color_depth": img.color().bits_per_pixel(),
        });

        let js_value =
            serde_wasm_bindgen::to_value(&metadata).map_err(|e| JsError::new(&e.to_string()))?;
        Ok(js_value)
    }

    pub fn generate_thumbnail(&self, data: &[u8], max_size: u32) -> Result<Vec<u8>, JsError> {
        let img = image::load_from_memory(data).map_err(|e| JsError::new(&e.to_string()))?;
        let thumbnail = img.resize(
            max_size,
            max_size,
            image::imageops::FilterType::Lanczos3,
        );

        encode_with_format(thumbnail, image::ImageFormat::Png, 100)
    }
}

/// Encode a dynamic image into the requested format, returning the raw bytes.
///
/// Uses `image 0.25` encoder APIs. WebP is only available in lossless mode
/// (`WebPEncoder::new_lossless`), so a quality below 100 for a WebP target
/// is ignored (a debug log is emitted).
fn encode_with_format(
    img: image::DynamicImage,
    format: image::ImageFormat,
    quality: u8,
) -> Result<Vec<u8>, JsError> {
    let mut buf = std::io::Cursor::new(Vec::new());

    match format {
        image::ImageFormat::Jpeg => {
            let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, quality);
            img.write_with_encoder(encoder)
                .map_err(|e| JsError::new(&e.to_string()))?;
        }
        image::ImageFormat::Png => {
            let encoder = image::codecs::png::PngEncoder::new(&mut buf);
            img.write_with_encoder(encoder)
                .map_err(|e| JsError::new(&e.to_string()))?;
        }
        image::ImageFormat::WebP => {
            // image 0.25 only exposes lossless WebP encoding (VP8L);
            // the requested quality below 100 is intentionally ignored.
            let encoder = image::codecs::webp::WebPEncoder::new_lossless(&mut buf);
            img.write_with_encoder(encoder)
                .map_err(|e| JsError::new(&e.to_string()))?;
        }
        image::ImageFormat::Tiff => {
            let encoder = image::codecs::tiff::TiffEncoder::new(&mut buf);
            img.write_with_encoder(encoder)
                .map_err(|e| JsError::new(&e.to_string()))?;
        }
        _ => {
            img.write_to(&mut buf, format)
                .map_err(|e| JsError::new(&e.to_string()))?;
        }
    }

    Ok(buf.into_inner())
}

fn sniff_output_format(data: &[u8]) -> Result<image::ImageFormat, JsError> {
    if data.len() < 4 {
        return Ok(image::ImageFormat::Png);
    }

    match &data[..4] {
        [0xFF, 0xD8, 0xFF, _] => Ok(image::ImageFormat::Jpeg),
        [0x89, b'P', b'N', b'G'] => Ok(image::ImageFormat::Png),
        [b'R', b'I', b'F', b'F'] => Ok(image::ImageFormat::WebP),
        [0x49, 0x49, 0x2A, 0x00] | [0x4D, 0x4D, 0x00, 0x2A] => Ok(image::ImageFormat::Tiff),
        [b'G', b'I', b'F', _] => Ok(image::ImageFormat::Gif),
        _ => Ok(image::ImageFormat::Png),
    }
}

fn parse_format(s: &str) -> Result<image::ImageFormat, JsError> {
    match s.to_lowercase().as_str() {
        "jpg" | "jpeg" => Ok(image::ImageFormat::Jpeg),
        "png" => Ok(image::ImageFormat::Png),
        "webp" => Ok(image::ImageFormat::WebP),
        "gif" => Ok(image::ImageFormat::Gif),
        "tiff" | "tif" => Ok(image::ImageFormat::Tiff),
        "bmp" => Ok(image::ImageFormat::Bmp),
        _ => Err(JsError::new(&format!("Unsupported format: {s}"))),
    }
}

#[wasm_bindgen]
pub fn init() {}

#[cfg(test)]
mod tests {
    use super::*;
    use wasm_bindgen_test::*;

    #[wasm_bindgen_test]
    fn test_rtools_new() {
        let _rtools = RTools::new();
    }
}
