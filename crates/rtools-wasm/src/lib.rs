use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct RTools;

#[wasm_bindgen]
impl RTools {
    #[wasm_bindgen(constructor)]
    pub fn new() -> RTools {
        RTools
    }

    pub fn compress_image(
        &self,
        data: &[u8],
        quality: u8,
    ) -> Result<Vec<u8>, JsError> {
        let img = image::load_from_memory(data).map_err(|e| JsError::new(&e.to_string()))?;
        let format = sniff_output_format(data)?;

        let mut buf = std::io::Cursor::new(Vec::new());
        match format {
            image::ImageFormat::Jpeg => {
                let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, quality);
                img.write_with(encoder)
                    .map_err(|e| JsError::new(&e.to_string()))?;
            }
            image::ImageFormat::Png => {
                let encoder = image::codecs::png::PngEncoder::new(&mut buf);
                img.write_with(encoder)
                    .map_err(|e| JsError::new(&e.to_string()))?;
            }
            image::ImageFormat::WebP => {
                if quality >= 100 {
                    let encoder = image::codecs::webp::WebPEncoder::new(&mut buf);
                    img.write_with(encoder)
                        .map_err(|e| JsError::new(&e.to_string()))?;
                } else {
                    let encoder =
                        image::codecs::webp::WebPEncoder::new_with_quality(&mut buf, quality as f32 / 100.0);
                    img.write_with(encoder)
                        .map_err(|e| JsError::new(&e.to_string()))?;
                }
            }
            _ => {
                img.write_with(&mut buf)
                    .map_err(|e| JsError::new(&e.to_string()))?;
            }
        }

        Ok(buf.into_inner())
    }

    pub fn convert_image(
        &self,
        data: &[u8],
        target_format: &str,
        quality: u8,
    ) -> Result<Vec<u8>, JsError> {
        let img = image::load_from_memory(data).map_err(|e| JsError::new(&e.to_string()))?;
        let format = parse_format(target_format)?;

        let mut buf = std::io::Cursor::new(Vec::new());
        match format {
            image::ImageFormat::Jpeg => {
                let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, quality);
                img.write_with(encoder)
                    .map_err(|e| JsError::new(&e.to_string()))?;
            }
            image::ImageFormat::WebP => {
                if quality >= 100 {
                    let encoder = image::codecs::webp::WebPEncoder::new(&mut buf);
                    img.write_with(encoder)
                        .map_err(|e| JsError::new(&e.to_string()))?;
                } else {
                    let encoder =
                        image::codecs::webp::WebPEncoder::new_with_quality(&mut buf, quality as f32 / 100.0);
                    img.write_with(encoder)
                        .map_err(|e| JsError::new(&e.to_string()))?;
                }
            }
            image::ImageFormat::Png => {
                let encoder = image::codecs::png::PngEncoder::new(&mut buf);
                img.write_with(encoder)
                    .map_err(|e| JsError::new(&e.to_string()))?;
            }
            image::ImageFormat::Tiff => {
                let encoder = image::codecs::tiff::TiffEncoder::new(&mut buf);
                img.write_with(encoder)
                    .map_err(|e| JsError::new(&e.to_string()))?;
            }
            _ => {
                img.write_with(&mut buf)
                    .map_err(|e| JsError::new(&e.to_string()))?;
            }
        }

        Ok(buf.into_inner())
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

        let mut buf = std::io::Cursor::new(Vec::new());
        resized
            .write_with(&mut buf)
            .map_err(|e| JsError::new(&e.to_string()))?;

        Ok(buf.into_inner())
    }

    pub fn crop_image(
        &self,
        data: &[u8],
        x: u32,
        y: u32,
        width: u32,
        height: u32,
    ) -> Result<Vec<u8>, JsError> {
        let img = image::load_from_memory(data).map_err(|e| JsError::new(&e.to_string()))?;
        let cropped = img.crop(x, y, width, height);

        let mut buf = std::io::Cursor::new(Vec::new());
        cropped
            .write_with(&mut buf)
            .map_err(|e| JsError::new(&e.to_string()))?;

        Ok(buf.into_inner())
    }

    pub fn get_metadata(&self, data: &[u8]) -> Result<JsValue, JsError> {
        let img = image::load_from_memory(data).map_err(|e| JsError::new(&e.to_string()))?;

        let metadata = serde_json::json!({
            "width": img.width(),
            "height": img.height(),
            "format": format!("{:?}", img.color()),
            "color_depth": img.color().bits_per_pixel(),
        });

        let serializer = serde_wasm_bindgen::Serializer::new().serialize_maps_as_objects(true);
        let js_value = metadata
            .serialize(&serializer)
            .map_err(|e| JsError::new(&e.to_string()))?;
        Ok(js_value)
    }

    pub fn generate_thumbnail(
        &self,
        data: &[u8],
        max_size: u32,
    ) -> Result<Vec<u8>, JsError> {
        let img = image::load_from_memory(data).map_err(|e| JsError::new(&e.to_string()))?;
        let thumbnail = img.resize(max_size, max_size, image::imageops::FilterType::Lanczos3);

        let mut buf = std::io::Cursor::new(Vec::new());
        thumbnail
            .write_with(&mut buf)
            .map_err(|e| JsError::new(&e.to_string()))?;

        Ok(buf.into_inner())
    }
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
        _ => Err(JsError::new(&format!("Unsupported format: {}", s))),
    }
}

#[wasm_bindgen]
pub fn init() {
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[wasm_bindgen_test]
    fn test_rtools_new() {
        let _rtools = RTools::new();
    }
}