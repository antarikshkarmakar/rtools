use rtools_core::{FileInput, Processor};
use rtools_image::{
    CompressConfig, CompressProcessor, ConvertConfig, ConvertProcessor, CropConfig, CropProcessor,
    MetadataConfig, MetadataProcessor, ResizeConfig, ResizeProcessor,
};
use std::path::PathBuf;
use tempfile::TempDir;

/// Create a test image filled with a deterministic pseudo-random pattern.
///
/// The pattern is seeded from the file name and dimensions so that images
/// that differ in size or name have distinct content (and therefore distinct
/// perceptual hashes), while a byte-for-byte copy remains identical. Pixels
/// are written in the format implied by the file extension so JPEG/WebP/PNG
/// fixtures all decode correctly.
fn create_test_image(dir: &std::path::Path, name: &str, width: u32, height: u32) -> PathBuf {
    let mut state = name.bytes().fold(
        width.wrapping_mul(73856093) ^ height.wrapping_mul(19349663),
        |acc, b| acc.wrapping_mul(16777619).wrapping_add(b as u32).wrapping_add(1013904223),
    );
    if state == 0 {
        state = 0x9e37_79b9;
    }

    let mut img = image::RgbaImage::new(width, height);
    for px in img.pixels_mut() {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        let v = state % 256;
        *px = image::Rgba([
            (v & 0xFF) as u8,
            ((v * 3) & 0xFF) as u8,
            ((v * 5) & 0xFF) as u8,
            255,
        ]);
    }

    use image::ImageEncoder;

    let path = dir.join(name);
    let writer = std::io::BufWriter::new(std::fs::File::create(&path).unwrap());
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "jpg" | "jpeg" => {
            let rgb = image::DynamicImage::ImageRgba8(img).to_rgb8();
            rgb.write_with_encoder(image::codecs::jpeg::JpegEncoder::new_with_quality(writer, 90))
                .unwrap();
        }
        "webp" => {
            img.write_with_encoder(image::codecs::webp::WebPEncoder::new_lossless(writer))
                .unwrap();
        }
        _ => {
            img.write_with_encoder(image::codecs::png::PngEncoder::new(writer)).unwrap();
        }
    }
    path
}

#[test]
fn test_compress_jpeg() {
    let tmp = TempDir::new().unwrap();
    let input = create_test_image(tmp.path(), "test.jpg", 200, 200);

    let file_input = FileInput::from_path(input);
    let config = CompressConfig {
        preset: rtools_image::compress::CompressionPreset::Balanced,
        format: None,
        output: Some(tmp.path().join("out.jpg")),
        preserve_metadata: true,
        strip_gps: false,
    };

    let processor = CompressProcessor;
    let result = processor.process(file_input, config).unwrap();

    assert!(result.destination.as_path().is_some());
    let out_path = result.destination.as_path().unwrap();
    assert!(out_path.exists());
    assert!(std::fs::metadata(out_path).unwrap().len() > 0);
}

#[test]
fn test_compress_png() {
    let tmp = TempDir::new().unwrap();
    let input = create_test_image(tmp.path(), "test.png", 100, 100);

    let file_input = FileInput::from_path(input);
    let config = CompressConfig {
        preset: rtools_image::compress::CompressionPreset::Custom(90),
        format: None,
        output: Some(tmp.path().join("out.png")),
        preserve_metadata: true,
        strip_gps: false,
    };

    let processor = CompressProcessor;
    let result = processor.process(file_input, config).unwrap();

    let out_path = result.destination.as_path().unwrap();
    assert!(out_path.exists());
}

#[test]
fn test_convert_png_to_jpeg() {
    let tmp = TempDir::new().unwrap();
    let input = create_test_image(tmp.path(), "input.png", 150, 150);

    let file_input = FileInput::from_path(input);
    let config = ConvertConfig {
        target_format: rtools_core::ImageFormat::Jpeg,
        output: Some(tmp.path().join("converted.jpg")),
        output_dir: None,
        quality: 85,
        preserve_metadata: true,
        strip_gps: false,
    };

    let processor = ConvertProcessor;
    let result = processor.process(file_input, config).unwrap();

    let out_path = result.destination.as_path().unwrap();
    assert!(out_path.exists());
    assert_eq!(out_path.extension().unwrap().to_str().unwrap(), "jpg");
}

#[test]
fn test_resize_image() {
    let tmp = TempDir::new().unwrap();
    let input = create_test_image(tmp.path(), "large.png", 400, 300);

    let file_input = FileInput::from_path(input);
    let config = ResizeConfig {
        width: Some(200),
        height: Some(150),
        maintain_aspect: true,
        algorithm: Default::default(),
        output: Some(tmp.path().join("resized.png")),
        quality: 85,
    };

    let processor = ResizeProcessor;
    let result = processor.process(file_input, config).unwrap();

    let out_path = result.destination.as_path().unwrap();
    assert!(out_path.exists());

    let resized = image::open(out_path).unwrap();
    assert_eq!(resized.width(), 200);
    assert_eq!(resized.height(), 150);
}

#[test]
fn test_resize_maintain_aspect() {
    let tmp = TempDir::new().unwrap();
    let input = create_test_image(tmp.path(), "aspect.png", 400, 200);

    let file_input = FileInput::from_path(input);
    let config = ResizeConfig {
        width: Some(200),
        height: None,
        maintain_aspect: true,
        algorithm: Default::default(),
        output: Some(tmp.path().join("aspect_out.png")),
        quality: 85,
    };

    let processor = ResizeProcessor;
    let result = processor.process(file_input, config).unwrap();

    let out_path = result.destination.as_path().unwrap();
    let resized = image::open(out_path).unwrap();
    assert_eq!(resized.width(), 200);
    assert_eq!(resized.height(), 100);
}

#[test]
fn test_crop_image() {
    let tmp = TempDir::new().unwrap();
    let input = create_test_image(tmp.path(), "crop.png", 200, 200);

    let file_input = FileInput::from_path(input);
    let config = CropConfig {
        region: rtools_image::crop::CropRegion::Pixels {
            x: 50,
            y: 50,
            width: 100,
            height: 100,
        },
        output: Some(tmp.path().join("cropped.png")),
        quality: 85,
    };

    let processor = CropProcessor;
    let result = processor.process(file_input, config).unwrap();

    let out_path = result.destination.as_path().unwrap();
    assert!(out_path.exists());

    let cropped = image::open(out_path).unwrap();
    assert_eq!(cropped.width(), 100);
    assert_eq!(cropped.height(), 100);
}

#[test]
fn test_get_metadata() {
    let tmp = TempDir::new().unwrap();
    let input = create_test_image(tmp.path(), "meta.png", 320, 240);

    let file_input = FileInput::from_path(input);
    let config = MetadataConfig::default();

    let processor = MetadataProcessor;
    let result = processor.process(file_input, config).unwrap();

    assert_eq!(result.width, 320);
    assert_eq!(result.height, 240);
}

#[test]
fn test_compress_custom_quality() {
    let tmp = TempDir::new().unwrap();
    let input = create_test_image(tmp.path(), "quality.jpg", 100, 100);

    let file_input = FileInput::from_path(input.clone());
    let config = CompressConfig {
        preset: rtools_image::compress::CompressionPreset::Custom(10),
        format: None,
        output: Some(tmp.path().join("low_quality.jpg")),
        preserve_metadata: false,
        strip_gps: false,
    };

    let processor = CompressProcessor;
    let result = processor.process(file_input, config).unwrap();

    let out_path = result.destination.as_path().unwrap();
    assert!(out_path.exists());

    let low_quality_size = std::fs::metadata(out_path).unwrap().len();

    let config2 = CompressConfig {
        preset: rtools_image::compress::CompressionPreset::Custom(95),
        format: None,
        output: Some(tmp.path().join("high_quality.jpg")),
        preserve_metadata: false,
        strip_gps: false,
    };

    let file_input2 = FileInput::from_path(input);
    let result2 = processor.process(file_input2, config2).unwrap();
    let high_quality_size = std::fs::metadata(result2.destination.as_path().unwrap())
        .unwrap()
        .len();

    assert!(
        low_quality_size < high_quality_size,
        "Low quality ({}) should be smaller than high quality ({})",
        low_quality_size,
        high_quality_size
    );
}

#[test]
fn test_output_path_created() {
    let tmp = TempDir::new().unwrap();
    let input = create_test_image(tmp.path(), "create.png", 50, 50);
    let out_dir = tmp.path().join("nested").join("output");

    let file_input = FileInput::from_path(input);
    let config = CompressConfig {
        preset: rtools_image::compress::CompressionPreset::Balanced,
        format: None,
        output: Some(out_dir.join("result.png")),
        preserve_metadata: true,
        strip_gps: false,
    };

    let processor = CompressProcessor;
    let result = processor.process(file_input, config).unwrap();

    assert!(result.destination.as_path().unwrap().exists());
}