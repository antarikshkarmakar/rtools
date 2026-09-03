use rtools_core::{ErrorCode, FileInput, OutputPolicy, PendingOutput, Processor, ResourceLimits};
use rtools_image::{
    CompressConfig, CompressProcessor, ConvertConfig, ConvertProcessor, CropConfig, CropProcessor,
    MetadataConfig, MetadataProcessor, ResizeConfig, ResizeProcessor,
};
use std::io::Write;
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
        width.wrapping_mul(73_856_093) ^ height.wrapping_mul(19_349_663),
        |acc, b| {
            acc.wrapping_mul(16_777_619)
                .wrapping_add(u32::from(b))
                .wrapping_add(1_013_904_223)
        },
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

    let path = dir.join(name);
    let writer = std::io::BufWriter::new(std::fs::File::create(&path).unwrap());
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_lowercase)
        .unwrap_or_default();
    match ext.as_str() {
        "jpg" | "jpeg" => {
            let rgb = image::DynamicImage::ImageRgba8(img).to_rgb8();
            rgb.write_with_encoder(image::codecs::jpeg::JpegEncoder::new_with_quality(
                writer, 90,
            ))
            .unwrap();
        }
        "webp" => {
            img.write_with_encoder(image::codecs::webp::WebPEncoder::new_lossless(writer))
                .unwrap();
        }
        _ => {
            img.write_with_encoder(image::codecs::png::PngEncoder::new(writer))
                .unwrap();
        }
    }
    path
}

fn write_png_header(path: &std::path::Path, width: u32, height: u32) {
    let mut header = Vec::with_capacity(33);
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

fn insert_png_text_chunk(path: &std::path::Path, text_len: usize) {
    let png = std::fs::read(path).unwrap();
    let ihdr_end = 8 + 4 + 4 + 13 + 4;
    let mut chunk_data = b"Comment\0".to_vec();
    chunk_data.extend(std::iter::repeat_n(b'x', text_len));

    let mut chunk = Vec::with_capacity(chunk_data.len() + 12);
    chunk.extend_from_slice(&u32::try_from(chunk_data.len()).unwrap().to_be_bytes());
    chunk.extend_from_slice(b"tEXt");
    chunk.extend_from_slice(&chunk_data);
    chunk.extend_from_slice(&crc32(&chunk[4..]));

    let mut with_text = Vec::with_capacity(png.len() + chunk.len());
    with_text.extend_from_slice(&png[..ihdr_end]);
    with_text.extend_from_slice(&chunk);
    with_text.extend_from_slice(&png[ihdr_end..]);
    std::fs::write(path, with_text).unwrap();
}

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

struct FailAfter<W> {
    inner: W,
    remaining: usize,
}

impl<W: Write> Write for FailAfter<W> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if self.remaining == 0 {
            return Err(std::io::Error::other("injected partial-write failure"));
        }
        let allowed = bytes.len().min(self.remaining);
        let written = self.inner.write(&bytes[..allowed])?;
        self.remaining = self.remaining.saturating_sub(written);
        Ok(written)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

#[test]
fn rejects_oversized_image_before_creating_output() {
    let tmp = TempDir::new().unwrap();
    let input = tmp.path().join("oversized.png");
    let output = tmp.path().join("created-after-decode").join("output.png");
    write_png_header(&input, 50_000, 50_000);

    let config = CompressConfig {
        format: Some(rtools_core::ImageFormat::Png),
        output: Some(output.clone()),
        limits: ResourceLimits {
            max_decoded_pixels: 1_000_000,
            ..ResourceLimits::default()
        },
        ..CompressConfig::default()
    };

    let error = CompressProcessor
        .process(FileInput::from_path(input), config)
        .unwrap_err();

    assert!(
        matches!(
            error,
            rtools_core::RToolsError::ResourceLimitExceeded {
                resource: "decoded_pixels",
                ..
            }
        ),
        "unexpected error: {error:?}"
    );
    assert!(!output.exists());
    assert!(!output.parent().unwrap().exists());
}

#[test]
fn rejects_decoder_allocation_over_limit_before_creating_output() {
    let tmp = TempDir::new().unwrap();
    let input = create_test_image(tmp.path(), "allocation-heavy.png", 1, 1);
    let output = tmp.path().join("created-after-decode").join("output.png");
    insert_png_text_chunk(&input, 1_024);

    let config = CompressConfig {
        format: Some(rtools_core::ImageFormat::Png),
        output: Some(output.clone()),
        limits: ResourceLimits {
            max_decoded_pixels: 1,
            ..ResourceLimits::default()
        },
        ..CompressConfig::default()
    };

    let error = CompressProcessor
        .process(FileInput::from_path(input), config)
        .unwrap_err();

    assert_eq!(error.code().as_str(), "RESOURCE_LIMIT_EXCEEDED");
    assert!(
        matches!(
            error,
            rtools_core::RToolsError::ResourceLimitExceededUnknownActual {
                resource: "image_decoder_allocation_bytes",
                limit: 16,
            }
        ),
        "unexpected error: {error:?}"
    );
    assert!(!output.exists());
    assert!(!output.parent().unwrap().exists());
}

#[test]
fn rejects_encoded_input_over_byte_limit_before_creating_output() {
    let tmp = TempDir::new().unwrap();
    let input = create_test_image(tmp.path(), "too-many-bytes.png", 2, 2);
    let output = tmp.path().join("created-after-decode").join("output.png");
    let input_len = std::fs::metadata(&input).unwrap().len();

    let config = CompressConfig {
        format: Some(rtools_core::ImageFormat::Png),
        output: Some(output.clone()),
        limits: ResourceLimits {
            max_input_bytes: input_len - 1,
            ..ResourceLimits::default()
        },
        ..CompressConfig::default()
    };

    let error = CompressProcessor
        .process(FileInput::from_path(input), config)
        .unwrap_err();

    assert!(
        matches!(
            error,
            rtools_core::RToolsError::ResourceLimitExceeded {
                resource: "input_bytes",
                actual,
                limit,
            } if actual == input_len && limit == input_len - 1
        ),
        "unexpected error: {error:?}"
    );
    assert!(!output.exists());
    assert!(!output.parent().unwrap().exists());
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
        output_policy: OutputPolicy::FailIfExists,
        preserve_metadata: true,
        strip_gps: false,
        limits: ResourceLimits::default(),
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
        output_policy: OutputPolicy::FailIfExists,
        preserve_metadata: true,
        strip_gps: false,
        limits: ResourceLimits::default(),
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
        output_policy: OutputPolicy::FailIfExists,
        output_dir: None,
        quality: 85,
        preserve_metadata: true,
        strip_gps: false,
        limits: ResourceLimits::default(),
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
        algorithm: rtools_image::resize::ResizeAlgorithm::default(),
        output: Some(tmp.path().join("resized.png")),
        output_policy: OutputPolicy::FailIfExists,
        quality: 85,
        limits: ResourceLimits::default(),
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
        algorithm: rtools_image::resize::ResizeAlgorithm::default(),
        output: Some(tmp.path().join("aspect_out.png")),
        output_policy: OutputPolicy::FailIfExists,
        quality: 85,
        limits: ResourceLimits::default(),
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
        output_policy: OutputPolicy::FailIfExists,
        quality: 85,
        limits: ResourceLimits::default(),
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
        output_policy: OutputPolicy::FailIfExists,
        preserve_metadata: false,
        strip_gps: false,
        limits: ResourceLimits::default(),
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
        output_policy: OutputPolicy::FailIfExists,
        preserve_metadata: false,
        strip_gps: false,
        limits: ResourceLimits::default(),
    };

    let file_input2 = FileInput::from_path(input);
    let result2 = processor.process(file_input2, config2).unwrap();
    let high_quality_size = std::fs::metadata(result2.destination.as_path().unwrap())
        .unwrap()
        .len();

    assert!(
        low_quality_size < high_quality_size,
        "Low quality ({low_quality_size}) should be smaller than high quality ({high_quality_size})"
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
        output_policy: OutputPolicy::FailIfExists,
        preserve_metadata: true,
        strip_gps: false,
        limits: ResourceLimits::default(),
    };

    let processor = CompressProcessor;
    let result = processor.process(file_input, config).unwrap();

    assert!(result.destination.as_path().unwrap().exists());
}

#[test]
fn image_fail_if_exists_preserves_original_bytes_and_leaves_no_temporary() {
    let tmp = TempDir::new().unwrap();
    let input = create_test_image(tmp.path(), "collision-input.png", 16, 16);
    let output = tmp.path().join("collision-output.png");
    std::fs::write(&output, b"original").unwrap();

    let error = CompressProcessor
        .process(
            FileInput::from_path(input),
            CompressConfig {
                format: Some(rtools_core::ImageFormat::Png),
                output: Some(output.clone()),
                ..CompressConfig::default()
            },
        )
        .unwrap_err();

    assert_eq!(error.code(), ErrorCode::OutputExists);
    assert_eq!(std::fs::read(output).unwrap(), b"original");
    let leftovers: Vec<_> = std::fs::read_dir(tmp.path())
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .filter(|name| name.to_string_lossy().contains("rtools"))
        .collect();
    assert!(leftovers.is_empty(), "leftover artifacts: {leftovers:?}");
}

#[test]
fn image_unique_name_commits_a_decodable_unicode_output() {
    let tmp = TempDir::new().unwrap();
    let input = create_test_image(tmp.path(), "unicode-input.png", 16, 16);
    let output = tmp.path().join("结果-🌌.png");
    std::fs::write(&output, b"occupied").unwrap();

    let result = CompressProcessor
        .process(
            FileInput::from_path(input),
            CompressConfig {
                format: Some(rtools_core::ImageFormat::Png),
                output: Some(output),
                output_policy: OutputPolicy::UniqueName,
                ..CompressConfig::default()
            },
        )
        .unwrap();

    let committed = result.destination.as_path().unwrap();
    assert_eq!(committed.file_name().unwrap(), "结果-🌌_1.png");
    image::ImageReader::open(committed)
        .unwrap()
        .with_guessed_format()
        .unwrap()
        .decode()
        .unwrap();
}

#[test]
fn partial_encoder_write_leaves_no_final_or_temporary_artifacts() {
    let tmp = TempDir::new().unwrap();
    let output = tmp.path().join("partial-output.png");
    let pending = PendingOutput::new(&output, OutputPolicy::FailIfExists).unwrap();
    let temporary = pending.temporary_path().to_owned();
    let file = std::fs::File::create(&temporary).unwrap();
    let writer = FailAfter {
        inner: file,
        remaining: 24,
    };
    let pixels = image::RgbaImage::from_pixel(16, 16, image::Rgba([1, 2, 3, 255]));

    let error = image::ImageEncoder::write_image(
        image::codecs::png::PngEncoder::new(writer),
        pixels.as_raw(),
        pixels.width(),
        pixels.height(),
        image::ExtendedColorType::Rgba8,
    )
    .unwrap_err();

    assert!(error.to_string().contains("injected partial-write failure"));
    assert!(std::fs::metadata(&temporary).unwrap().len() > 0);
    drop(pending);
    assert!(!output.exists());
    assert!(!temporary.exists());
    let leftovers: Vec<_> = std::fs::read_dir(tmp.path())
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .filter(|name| name.to_string_lossy().contains("rtools"))
        .collect();
    assert!(leftovers.is_empty(), "leftover artifacts: {leftovers:?}");
}

#[test]
fn legacy_image_configs_default_missing_output_policy_to_fail_if_exists() {
    macro_rules! assert_legacy_default {
        ($config:expr, $type:ty) => {{
            let mut legacy = serde_json::to_value($config).unwrap();
            legacy.as_object_mut().unwrap().remove("output_policy");
            let decoded: $type = serde_json::from_value(legacy).unwrap();
            assert_eq!(decoded.output_policy, OutputPolicy::FailIfExists);
        }};
    }

    assert_legacy_default!(CompressConfig::default(), CompressConfig);
    assert_legacy_default!(ConvertConfig::default(), ConvertConfig);
    assert_legacy_default!(ResizeConfig::default(), ResizeConfig);
    assert_legacy_default!(CropConfig::default(), CropConfig);
    assert_legacy_default!(
        rtools_image::FilterConfig::default(),
        rtools_image::FilterConfig
    );
    assert_legacy_default!(
        rtools_image::WatermarkConfig::default(),
        rtools_image::WatermarkConfig
    );
}
