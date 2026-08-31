use wasm_bindgen::prelude::*;
use std::path::PathBuf;

// Enable logging in debug mode
#[cfg(feature = "console_error_panic_hook")]
extern crate console_error_panic_hook;

#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

#[wasm_bindgen]
pub struct RTools {
    // Placeholder for WASM state
}

#[wasm_bindgen]
impl RTools {
    #[wasm_bindgen(constructor)]
    pub fn new() -> RTools {
        RTools {}
    }

    /// Compress an image
    pub fn compress_image(&self, data: &[u8], filename: &str, quality: u8) -> Result<Vec<u8>, JsError> {
        // Save input to temp
        let temp_dir = tempfile::tempdir().map_err(|e| JsError::new(&e.to_string()))?;
        let input_path = temp_dir.path().join(filename);
        std::fs::write(&input_path, data).map_err(|e| JsError::new(&e.to_string()))?;

        let file_input = rtools_core::FileInput::from_path(input_path.clone());
        let config = rtools_image::CompressConfig {
            preset: rtools_image::compress::CompressionPreset::Custom(quality),
            format: None,
            output: None,
            preserve_metadata: true,
            strip_gps: false,
        };

        let processor = rtools_image::CompressProcessor;
        match processor.process(file_input, config) {
            Ok(output) => {
                let output_path = output.destination.as_path()
                    .ok_or_else(|| JsError::new("No output path"))?;
                let output_data = std::fs::read(output_path)
                    .map_err(|e| JsError::new(&e.to_string()))?;
                Ok(output_data)
            }
            Err(e) => Err(JsError::new(&e.to_string())),
        }
    }

    /// Convert image format
    pub fn convert_image(&self, data: &[u8], filename: &str, target_format: &str) -> Result<Vec<u8>, JsError> {
        let temp_dir = tempfile::tempdir().map_err(|e| JsError::new(&e.to_string()))?;
        let input_path = temp_dir.path().join(filename);
        std::fs::write(&input_path, data).map_err(|e| JsError::new(&e.to_string()))?;

        let file_input = rtools_core::FileInput::from_path(input_path.clone());
        let format = rtools_core::ImageFormat::from_extension(target_format)
            .ok_or_else(|| JsError::new(&format!("Unsupported format: {}", target_format)))?;

        let config = rtools_image::ConvertConfig {
            target_format: format,
            output: None,
            output_dir: None,
            quality: 85,
            preserve_metadata: true,
            strip_gps: false,
        };

        let processor = rtools_image::ConvertProcessor;
        match processor.process(file_input, config) {
            Ok(output) => {
                let output_path = output.destination.as_path()
                    .ok_or_else(|| JsError::new("No output path"))?;
                let output_data = std::fs::read(output_path)
                    .map_err(|e| JsError::new(&e.to_string()))?;
                Ok(output_data)
            }
            Err(e) => Err(JsError::new(&e.to_string())),
        }
    }

    /// Resize image
    pub fn resize_image(&self, data: &[u8], filename: &str, width: u32, height: u32) -> Result<Vec<u8>, JsError> {
        let temp_dir = tempfile::tempdir().map_err(|e| JsError::new(&e.to_string()))?;
        let input_path = temp_dir.path().join(filename);
        std::fs::write(&input_path, data).map_err(|e| JsError::new(&e.to_string()))?;

        let file_input = rtools_core::FileInput::from_path(input_path.clone());
        let config = rtools_image::ResizeConfig {
            width: Some(width),
            height: Some(height),
            maintain_aspect: true,
            algorithm: Default::default(),
            output: None,
            quality: 85,
        };

        let processor = rtools_image::ResizeProcessor;
        match processor.process(file_input, config) {
            Ok(output) => {
                let output_path = output.destination.as_path()
                    .ok_or_else(|| JsError::new("No output path"))?;
                let output_data = std::fs::read(output_path)
                    .map_err(|e| JsError::new(&e.to_string()))?;
                Ok(output_data)
            }
            Err(e) => Err(JsError::new(&e.to_string())),
        }
    }

    /// Get image metadata
    pub fn get_metadata(&self, data: &[u8], filename: &str) -> Result<JsValue, JsError> {
        let temp_dir = tempfile::tempdir().map_err(|e| JsError::new(&e.to_string()))?;
        let input_path = temp_dir.path().join(filename);
        std::fs::write(&input_path, data).map_err(|e| JsError::new(&e.to_string()))?;

        let file_input = rtools_core::FileInput::from_path(input_path);
        let config = rtools_image::MetadataConfig::default();

        let processor = rtools_image::MetadataProcessor;
        match processor.process(file_input, config) {
            Ok(metadata) => {
                let js_value = serde_wasm_bindgen::to_value(&metadata)
                    .map_err(|e| JsError::new(&e.to_string()))?;
                Ok(js_value)
            }
            Err(e) => Err(JsError::new(&e.to_string())),
        }
    }
}

// Helper module for serde
mod serde_wasm_bindgen {
    use serde::Serialize;
    use wasm_bindgen::prelude::*;

    pub fn to_value<T: Serialize + ?Sized>(value: &T) -> Result<JsValue, JsError> {
        let serializer = serde_wasm_bindgen::Serializer::new().serialize_maps_as_objects(true);
        value.serialize(&serializer).map_err(|e| JsError::new(&e.to_string()))
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
        let rtools = RTools::new();
        assert!(true);
    }
}