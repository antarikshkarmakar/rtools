use base64::Engine as _;
use image::GenericImageView;
use rtools_core::{ErrorCode, FileInput, OutputPolicy, PendingOutput, Processor, ResourceLimits};
use rtools_image::crop::{AspectRatio, CropRegion, Gravity};
use rtools_image::watermark::{WatermarkPosition, WatermarkType};
use rtools_image::{
    CompressConfig, CompressProcessor, ConvertConfig, ConvertProcessor, CropConfig, CropProcessor,
    ExifConfig, ExifProcessor, FilterConfig, FilterProcessor, MetadataConfig, MetadataProcessor,
    ResizeConfig, ResizeProcessor, WatermarkConfig, WatermarkProcessor,
};
use std::io::Write;
use std::path::PathBuf;
use tempfile::TempDir;

fn decode_fixture(dir: &std::path::Path, name: &str, encoded: &str) -> PathBuf {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded.trim())
        .unwrap();
    let path = dir.join(name);
    std::fs::write(&path, bytes).unwrap();
    path
}

#[test]
fn exif_orientations_2_through_8_map_every_pixel_to_the_literal_position() {
    let image = image::DynamicImage::ImageRgba8(image::RgbaImage::from_fn(3, 2, |x, y| {
        image::Rgba([u8::try_from(y * 3 + x + 1).unwrap(), 0, 0, 255])
    }));
    let cases: &[(u32, u32, u32, &[u8])] = &[
        (2, 3, 2, &[3, 2, 1, 6, 5, 4]),
        (3, 3, 2, &[6, 5, 4, 3, 2, 1]),
        (4, 3, 2, &[4, 5, 6, 1, 2, 3]),
        (5, 2, 3, &[1, 4, 2, 5, 3, 6]),
        (6, 2, 3, &[4, 1, 5, 2, 6, 3]),
        (7, 2, 3, &[6, 3, 5, 2, 4, 1]),
        (8, 2, 3, &[3, 6, 2, 5, 1, 4]),
    ];

    for &(orientation, expected_width, expected_height, expected_pixels) in cases {
        let transformed = rtools_image::format::apply_exif_orientation(image.clone(), orientation);
        assert_eq!(
            transformed.dimensions(),
            (expected_width, expected_height),
            "orientation {orientation} dimensions"
        );
        let actual: Vec<_> = transformed
            .to_rgba8()
            .pixels()
            .map(|pixel| pixel[0])
            .collect();
        assert_eq!(actual, expected_pixels, "orientation {orientation} pixels");
    }
}

#[test]
fn bounded_decode_applies_real_jpeg_orientation_before_returning_pixels() {
    let tmp = TempDir::new().unwrap();
    let input = decode_fixture(
        tmp.path(),
        "orientation.jpg",
        include_str!("../fixtures/images/orientation-6.jpg.b64"),
    );

    let decoded = rtools_image::format::decode_bounded(&input, &ResourceLimits::default()).unwrap();

    assert_eq!(decoded.orientation_applied, Some(6));
    assert_eq!(decoded.image.dimensions(), (36, 24));
    let pixels = decoded.image.to_rgb8();
    for (x, y, expected) in [
        (6, 6, [240, 20, 240]),
        (30, 6, [240, 20, 20]),
        (6, 18, [20, 240, 240]),
        (30, 18, [20, 240, 20]),
    ] {
        let actual = pixels.get_pixel(x, y).0;
        assert!(
            actual
                .into_iter()
                .zip(expected)
                .all(|(actual, expected)| actual.abs_diff(expected) <= 8),
            "pixel ({x},{y}) was {actual:?}, expected near {expected:?}"
        );
    }
}

#[test]
fn convert_records_exact_orientation_warning_and_uses_oriented_geometry() {
    let tmp = TempDir::new().unwrap();
    let input = decode_fixture(
        tmp.path(),
        "orientation.jpg",
        include_str!("../fixtures/images/orientation-6.jpg.b64"),
    );
    let output = tmp.path().join("oriented.png");

    let result = ConvertProcessor
        .process(
            FileInput::from_path(input),
            ConvertConfig {
                target_format: rtools_core::ImageFormat::Png,
                output: Some(output.clone()),
                ..ConvertConfig::default()
            },
        )
        .unwrap();

    assert_eq!(result.warnings, ["EXIF orientation 6 applied"]);
    assert_eq!(image::open(output).unwrap().dimensions(), (36, 24));
}

#[test]
fn convert_without_exif_orientation_has_no_warnings() {
    let tmp = TempDir::new().unwrap();
    let input = create_test_image(tmp.path(), "plain.png", 3, 2);
    let result = ConvertProcessor
        .process(
            FileInput::from_path(input),
            ConvertConfig {
                target_format: rtools_core::ImageFormat::Bmp,
                output: Some(tmp.path().join("plain.bmp")),
                ..ConvertConfig::default()
            },
        )
        .unwrap();

    assert!(result.warnings.is_empty());
}

fn assert_all_single_frame_processors_reject_animation(
    tmp: &TempDir,
    input: &std::path::Path,
    case: &str,
) {
    let watermark = create_test_image(tmp.path(), "watermark.png", 1, 1);

    let operations: Vec<(
        &str,
        PathBuf,
        rtools_core::RToolsResult<rtools_core::FileOutput>,
    )> = vec![
        (
            "compress",
            tmp.path().join(format!("{case}-compress/output.png")),
            CompressProcessor.process(
                FileInput::from_path(input.to_path_buf()),
                CompressConfig {
                    format: Some(rtools_core::ImageFormat::Png),
                    output: Some(tmp.path().join(format!("{case}-compress/output.png"))),
                    ..CompressConfig::default()
                },
            ),
        ),
        (
            "convert",
            tmp.path().join(format!("{case}-convert/output.png")),
            ConvertProcessor.process(
                FileInput::from_path(input.to_path_buf()),
                ConvertConfig {
                    target_format: rtools_core::ImageFormat::Png,
                    output: Some(tmp.path().join(format!("{case}-convert/output.png"))),
                    ..ConvertConfig::default()
                },
            ),
        ),
        (
            "resize",
            tmp.path().join(format!("{case}-resize/output.png")),
            ResizeProcessor.process(
                FileInput::from_path(input.to_path_buf()),
                ResizeConfig {
                    width: Some(2),
                    output: Some(tmp.path().join(format!("{case}-resize/output.png"))),
                    ..ResizeConfig::default()
                },
            ),
        ),
        (
            "crop",
            tmp.path().join(format!("{case}-crop/output.png")),
            CropProcessor.process(
                FileInput::from_path(input.to_path_buf()),
                CropConfig {
                    output: Some(tmp.path().join(format!("{case}-crop/output.png"))),
                    ..CropConfig::default()
                },
            ),
        ),
        (
            "filter",
            tmp.path().join(format!("{case}-filter/output.png")),
            FilterProcessor.process(
                FileInput::from_path(input.to_path_buf()),
                FilterConfig {
                    output: Some(tmp.path().join(format!("{case}-filter/output.png"))),
                    ..FilterConfig::default()
                },
            ),
        ),
        (
            "watermark",
            tmp.path().join(format!("{case}-watermark/output.png")),
            WatermarkProcessor.process(
                FileInput::from_path(input.to_path_buf()),
                WatermarkConfig {
                    watermark: WatermarkType::Image {
                        image_path: watermark,
                        scale: 1.0,
                    },
                    position: WatermarkPosition::Pixels { x: 0, y: 0 },
                    output: Some(tmp.path().join(format!("{case}-watermark/output.png"))),
                    ..WatermarkConfig::default()
                },
            ),
        ),
    ];

    for (name, output, result) in operations {
        let error = result.unwrap_err();
        assert_eq!(
            error.code(),
            ErrorCode::CapabilityUnavailable,
            "{name}: {error}"
        );
        assert!(!output.exists(), "{name} created final output");
        assert!(
            !output.parent().unwrap().exists(),
            "{name} created output directory"
        );
    }
}

#[test]
fn every_single_frame_processor_rejects_a_renamed_animated_gif_before_output_artifacts() {
    let tmp = TempDir::new().unwrap();
    let input = decode_fixture(
        tmp.path(),
        "animation.dat",
        include_str!("../fixtures/images/two-frame.gif.b64"),
    );
    assert_all_single_frame_processors_reject_animation(&tmp, &input, "gif");
}

#[test]
fn every_single_frame_processor_rejects_renamed_multiframe_webp_and_apng() {
    let tmp = TempDir::new().unwrap();
    for (case, fixture) in [
        (
            "webp",
            include_str!("../fixtures/images/two-frame.webp.b64"),
        ),
        (
            "apng",
            include_str!("../fixtures/images/two-frame.apng.b64"),
        ),
    ] {
        let input = decode_fixture(tmp.path(), &format!("{case}-animation.bin"), fixture);
        assert_all_single_frame_processors_reject_animation(&tmp, &input, case);
    }
}

fn assert_all_single_frame_processors_accept(tmp: &TempDir, input: &std::path::Path, case: &str) {
    let watermark = create_test_image(tmp.path(), &format!("{case}-mark.png"), 1, 1);
    let operations: Vec<(
        &str,
        PathBuf,
        rtools_core::RToolsResult<rtools_core::FileOutput>,
    )> = vec![
        (
            "compress",
            tmp.path().join(format!("{case}-one-compress.png")),
            CompressProcessor.process(
                FileInput::from_path(input.to_path_buf()),
                CompressConfig {
                    format: Some(rtools_core::ImageFormat::Png),
                    output: Some(tmp.path().join(format!("{case}-one-compress.png"))),
                    ..CompressConfig::default()
                },
            ),
        ),
        (
            "convert",
            tmp.path().join(format!("{case}-one-convert.png")),
            ConvertProcessor.process(
                FileInput::from_path(input.to_path_buf()),
                ConvertConfig {
                    target_format: rtools_core::ImageFormat::Png,
                    output: Some(tmp.path().join(format!("{case}-one-convert.png"))),
                    ..ConvertConfig::default()
                },
            ),
        ),
        (
            "resize",
            tmp.path().join(format!("{case}-one-resize.png")),
            ResizeProcessor.process(
                FileInput::from_path(input.to_path_buf()),
                ResizeConfig {
                    width: Some(3),
                    output: Some(tmp.path().join(format!("{case}-one-resize.png"))),
                    ..ResizeConfig::default()
                },
            ),
        ),
        (
            "crop",
            tmp.path().join(format!("{case}-one-crop.png")),
            CropProcessor.process(
                FileInput::from_path(input.to_path_buf()),
                CropConfig {
                    output: Some(tmp.path().join(format!("{case}-one-crop.png"))),
                    ..CropConfig::default()
                },
            ),
        ),
        (
            "filter",
            tmp.path().join(format!("{case}-one-filter.png")),
            FilterProcessor.process(
                FileInput::from_path(input.to_path_buf()),
                FilterConfig {
                    output: Some(tmp.path().join(format!("{case}-one-filter.png"))),
                    ..FilterConfig::default()
                },
            ),
        ),
        (
            "watermark",
            tmp.path().join(format!("{case}-one-watermark.png")),
            WatermarkProcessor.process(
                FileInput::from_path(input.to_path_buf()),
                WatermarkConfig {
                    watermark: WatermarkType::Image {
                        image_path: watermark,
                        scale: 1.0,
                    },
                    position: WatermarkPosition::Pixels { x: 0, y: 0 },
                    output: Some(tmp.path().join(format!("{case}-one-watermark.png"))),
                    ..WatermarkConfig::default()
                },
            ),
        ),
    ];

    for (name, output, result) in operations {
        let result = result.unwrap_or_else(|error| panic!("{case} {name}: {error}"));
        assert_eq!(result.destination.as_path(), Some(&output));
        image::open(output).unwrap();
    }
}

#[test]
fn every_single_frame_processor_accepts_renamed_one_frame_extended_webp_and_apng() {
    let tmp = TempDir::new().unwrap();
    for (case, fixture) in [
        (
            "webp",
            include_str!("../fixtures/images/single-frame-extended.webp.b64"),
        ),
        (
            "apng",
            include_str!("../fixtures/images/single-frame.apng.b64"),
        ),
    ] {
        let input = decode_fixture(tmp.path(), &format!("{case}-one-frame.bin"), fixture);
        assert_all_single_frame_processors_accept(&tmp, &input, case);
    }
}

#[test]
fn malformed_animated_webp_and_apng_return_decode_errors_without_flattening() {
    let tmp = TempDir::new().unwrap();
    for (case, fixture, removed_bytes) in [
        (
            "webp",
            include_str!("../fixtures/images/two-frame.webp.b64"),
            8,
        ),
        (
            "apng",
            include_str!("../fixtures/images/two-frame.apng.b64"),
            24,
        ),
    ] {
        let mut bytes = base64::engine::general_purpose::STANDARD
            .decode(fixture.trim())
            .unwrap();
        bytes.truncate(bytes.len() - removed_bytes);
        let input = tmp.path().join(format!("malformed-{case}.bin"));
        std::fs::write(&input, bytes).unwrap();

        let error =
            rtools_image::format::decode_bounded(&input, &ResourceLimits::default()).unwrap_err();
        assert_eq!(error.code(), ErrorCode::ProcessingFailed, "{case}: {error}");
    }
}

#[test]
fn drop_all_validator_rejects_a_real_gps_exif_artifact() {
    let tmp = TempDir::new().unwrap();
    let gps = decode_fixture(
        tmp.path(),
        "gps.jpg",
        include_str!("../fixtures/images/gps.jpg.b64"),
    );

    let source = ExifProcessor
        .process(FileInput::from_path(gps.clone()), ExifConfig::default())
        .unwrap();
    assert!(source.gps_latitude.is_some());
    assert!(source.gps_longitude.is_some());

    let error = rtools_image::metadata::verify_drop_all_artifact(&gps, &ResourceLimits::default())
        .unwrap_err();
    assert_eq!(error.code(), ErrorCode::ProcessingFailed);
}

#[test]
fn compress_and_convert_drop_real_gps_and_orientation_metadata_before_commit() {
    let tmp = TempDir::new().unwrap();
    for operation in ["compress", "convert"] {
        let input = decode_fixture(
            tmp.path(),
            &format!("gps-{operation}.jpg"),
            include_str!("../fixtures/images/gps.jpg.b64"),
        );
        let output = tmp.path().join(format!("{operation}-drop-all.jpg"));
        let result = if operation == "compress" {
            CompressProcessor.process(
                FileInput::from_path(input),
                CompressConfig {
                    format: Some(rtools_core::ImageFormat::Jpeg),
                    output: Some(output.clone()),
                    ..CompressConfig::default()
                },
            )
        } else {
            ConvertProcessor.process(
                FileInput::from_path(input),
                ConvertConfig {
                    target_format: rtools_core::ImageFormat::Jpeg,
                    output: Some(output.clone()),
                    ..ConvertConfig::default()
                },
            )
        }
        .unwrap();

        assert_eq!(result.destination.as_path(), Some(&output));
        rtools_image::metadata::verify_drop_all_artifact(&output, &ResourceLimits::default())
            .unwrap();
        let exif = ExifProcessor
            .process(FileInput::from_path(output), ExifConfig::default())
            .unwrap();
        assert!(exif.gps_latitude.is_none());
        assert!(exif.gps_longitude.is_none());
        assert!(exif.orientation.is_none());
    }
}

#[test]
fn converting_real_gps_and_orientation_jpeg_to_tiff_keeps_only_structural_fields() {
    let tmp = TempDir::new().unwrap();
    let input = decode_fixture(
        tmp.path(),
        "gps-orientation-6.jpg",
        include_str!("../fixtures/images/gps-orientation-6.jpg.b64"),
    );
    let source = ExifProcessor
        .process(FileInput::from_path(input.clone()), ExifConfig::default())
        .unwrap();
    assert_eq!(source.orientation, Some(6));
    assert!(source.gps_latitude.is_some());
    assert!(source.gps_longitude.is_some());

    let output = tmp.path().join("drop-all.tiff");
    let result = ConvertProcessor
        .process(
            FileInput::from_path(input),
            ConvertConfig {
                target_format: rtools_core::ImageFormat::Tiff,
                output: Some(output.clone()),
                ..ConvertConfig::default()
            },
        )
        .unwrap();

    assert_eq!(result.destination.as_path(), Some(&output));
    assert_eq!(result.warnings, ["EXIF orientation 6 applied"]);
    assert_eq!(image::open(&output).unwrap().dimensions(), (16, 16));
    rtools_image::metadata::verify_drop_all_artifact(&output, &ResourceLimits::default()).unwrap();
    let exif = ExifProcessor
        .process(FileInput::from_path(output), ExifConfig::default())
        .unwrap();
    assert!(exif.orientation.is_none());
    assert!(exif.gps_latitude.is_none());
    assert!(exif.gps_longitude.is_none());
    assert!(exif.camera_make.is_none());
    assert!(exif.camera_model.is_none());
}

#[test]
fn drop_all_validator_rejects_non_structural_tiff_metadata() {
    let tmp = TempDir::new().unwrap();
    let input = decode_fixture(
        tmp.path(),
        "private-metadata.tiff",
        include_str!("../fixtures/images/private-metadata.tiff.b64"),
    );

    let error =
        rtools_image::metadata::verify_drop_all_artifact(&input, &ResourceLimits::default())
            .unwrap_err();

    assert_eq!(error.code(), ErrorCode::ProcessingFailed);
    let message = error.to_string();
    assert!(message.contains("ImageDescription"), "{message}");
    assert!(message.contains("Orientation"), "{message}");
    assert!(message.contains("Software"), "{message}");
}

#[test]
fn exif_mutation_settings_fail_validation_before_input_or_output_access() {
    let tmp = TempDir::new().unwrap();
    for config in [
        ExifConfig {
            remove_gps: true,
            ..ExifConfig::default()
        },
        ExifConfig {
            remove_all: true,
            ..ExifConfig::default()
        },
        ExifConfig {
            output: Some(tmp.path().join("forbidden.jpg")),
            ..ExifConfig::default()
        },
    ] {
        let error = ExifProcessor
            .process(FileInput::from_path(tmp.path().join("missing.jpg")), config)
            .unwrap_err();
        assert_eq!(error.code(), ErrorCode::InvalidInput);
    }
    assert!(!tmp.path().join("forbidden.jpg").exists());
}

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

fn create_solid_png(
    dir: &std::path::Path,
    name: &str,
    width: u32,
    height: u32,
    color: [u8; 4],
) -> PathBuf {
    let path = dir.join(name);
    image::RgbaImage::from_pixel(width, height, image::Rgba(color))
        .save(&path)
        .unwrap();
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
        preserve_metadata: false,
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
        preserve_metadata: false,
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
        preserve_metadata: false,
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
        preserve_metadata: false,
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

#[test]
fn watermark_resource_errors_are_structured_and_create_no_output() {
    let tmp = TempDir::new().unwrap();
    let base = create_solid_png(tmp.path(), "base.png", 40, 40, [0, 0, 0, 255]);
    let unsupported = tmp.path().join("unsupported.txt");
    std::fs::write(&unsupported, b"not an image format").unwrap();
    let corrupt = tmp.path().join("corrupt.png");
    std::fs::write(&corrupt, b"\x89PNG\r\n\x1a\ntruncated").unwrap();
    let cases = [
        (tmp.path().join("missing.png"), ErrorCode::InvalidInput),
        (unsupported, ErrorCode::UnsupportedFormat),
        (corrupt, ErrorCode::ProcessingFailed),
    ];

    for (index, (image_path, expected_code)) in cases.into_iter().enumerate() {
        let output = tmp.path().join(format!("case-{index}/output.png"));
        let error = WatermarkProcessor
            .process(
                FileInput::from_path(base.clone()),
                WatermarkConfig {
                    watermark: WatermarkType::Image {
                        image_path,
                        scale: 1.0,
                    },
                    position: WatermarkPosition::Pixels { x: 0, y: 0 },
                    output: Some(output.clone()),
                    ..WatermarkConfig::default()
                },
            )
            .unwrap_err();
        assert_eq!(error.code(), expected_code, "case {index}: {error}");
        assert!(!output.exists());
        assert!(!output.parent().unwrap().exists());
    }
}

#[test]
fn watermark_rejects_nonfinite_and_out_of_range_numbers_before_input_access() {
    let missing_input = FileInput::from_path(PathBuf::from("missing-base.png"));
    let image_path = PathBuf::from("missing-watermark.png");
    for (opacity, scale) in [
        (f64::NAN, 1.0),
        (f64::INFINITY, 1.0),
        (-0.1, 1.0),
        (1.1, 1.0),
        (0.5, f64::NAN),
        (0.5, f64::INFINITY),
        (0.5, 0.0),
        (0.5, -1.0),
    ] {
        let error = WatermarkProcessor
            .process(
                missing_input.clone(),
                WatermarkConfig {
                    watermark: WatermarkType::Image {
                        image_path: image_path.clone(),
                        scale,
                    },
                    opacity,
                    ..WatermarkConfig::default()
                },
            )
            .unwrap_err();
        assert_eq!(
            error.code(),
            ErrorCode::InvalidInput,
            "opacity={opacity}, scale={scale}"
        );
    }
}

#[test]
fn watermark_rejects_zero_or_oversized_scaled_overlay_and_out_of_bounds_placement() {
    let tmp = TempDir::new().unwrap();
    let base = create_solid_png(tmp.path(), "base.png", 10, 10, [0, 0, 0, 255]);
    let watermark = create_solid_png(tmp.path(), "mark.png", 2, 2, [255, 255, 255, 255]);
    let cases = [
        (0.1, WatermarkPosition::Pixels { x: 0, y: 0 }),
        (6.0, WatermarkPosition::Pixels { x: 0, y: 0 }),
        (1.0, WatermarkPosition::Pixels { x: 9, y: 9 }),
        (1.0, WatermarkPosition::Percentage { x: 100.0, y: 0.0 }),
        (1.0, WatermarkPosition::BottomRight),
    ];

    for (index, (scale, position)) in cases.into_iter().enumerate() {
        let output = tmp.path().join(format!("bounds-{index}/output.png"));
        let error = WatermarkProcessor
            .process(
                FileInput::from_path(base.clone()),
                WatermarkConfig {
                    watermark: WatermarkType::Image {
                        image_path: watermark.clone(),
                        scale,
                    },
                    position,
                    output: Some(output.clone()),
                    ..WatermarkConfig::default()
                },
            )
            .unwrap_err();
        assert_eq!(
            error.code(),
            ErrorCode::InvalidInput,
            "case {index}: {error}"
        );
        assert!(!output.exists());
        assert!(!output.parent().unwrap().exists());
    }
}

#[test]
fn image_watermark_places_and_blends_the_full_overlay_at_requested_opacity() {
    let tmp = TempDir::new().unwrap();
    let base = create_solid_png(tmp.path(), "base.png", 10, 10, [0, 0, 0, 255]);
    let watermark = create_solid_png(tmp.path(), "mark.png", 2, 2, [255, 255, 255, 255]);
    let output = tmp.path().join("output.png");

    WatermarkProcessor
        .process(
            FileInput::from_path(base),
            WatermarkConfig {
                watermark: WatermarkType::Image {
                    image_path: watermark,
                    scale: 1.0,
                },
                position: WatermarkPosition::Pixels { x: 2, y: 3 },
                opacity: 0.5,
                output: Some(output.clone()),
                ..WatermarkConfig::default()
            },
        )
        .unwrap();

    let pixels = image::open(output).unwrap().to_rgba8();
    assert_eq!(pixels.get_pixel(1, 3).0, [0, 0, 0, 255]);
    for (x, y) in [(2, 3), (3, 3), (2, 4), (3, 4)] {
        let pixel = pixels.get_pixel(x, y).0;
        assert!(pixel[..3].iter().all(|channel| channel.abs_diff(128) <= 1));
    }
    assert_eq!(pixels.get_pixel(4, 4).0, [0, 0, 0, 255]);
}

#[test]
fn image_watermark_uses_straight_alpha_source_over_for_translucent_pixels() {
    let tmp = TempDir::new().unwrap();
    let cases = [
        (
            "transparent",
            [0, 0, 0, 0],
            [255, 255, 255, 255],
            [255, 255, 255, 128],
        ),
        (
            "translucent",
            [20, 40, 80, 128],
            [200, 100, 50, 128],
            [92, 64, 68, 160],
        ),
    ];

    for (name, base_color, watermark_color, expected) in cases {
        let base = create_solid_png(tmp.path(), &format!("{name}-base.png"), 1, 1, base_color);
        let watermark = create_solid_png(
            tmp.path(),
            &format!("{name}-mark.png"),
            1,
            1,
            watermark_color,
        );
        let output = tmp.path().join(format!("{name}-output.png"));
        WatermarkProcessor
            .process(
                FileInput::from_path(base),
                WatermarkConfig {
                    watermark: WatermarkType::Image {
                        image_path: watermark,
                        scale: 1.0,
                    },
                    position: WatermarkPosition::Pixels { x: 0, y: 0 },
                    opacity: 0.5,
                    output: Some(output.clone()),
                    ..WatermarkConfig::default()
                },
            )
            .unwrap();
        assert_eq!(
            image::open(output).unwrap().to_rgba8().get_pixel(0, 0).0,
            expected
        );
    }

    let base = create_solid_png(tmp.path(), "vary-base.png", 2, 1, [0, 0, 0, 0]);
    let watermark = tmp.path().join("vary-mark.png");
    let mut pixels = image::RgbaImage::new(2, 1);
    pixels.put_pixel(0, 0, image::Rgba([255, 0, 0, 0]));
    pixels.put_pixel(1, 0, image::Rgba([255, 255, 255, 255]));
    pixels.save(&watermark).unwrap();
    let output = tmp.path().join("vary-output.png");
    WatermarkProcessor
        .process(
            FileInput::from_path(base),
            WatermarkConfig {
                watermark: WatermarkType::Image {
                    image_path: watermark,
                    scale: 1.0,
                },
                position: WatermarkPosition::Pixels { x: 0, y: 0 },
                opacity: 0.5,
                output: Some(output.clone()),
                ..WatermarkConfig::default()
            },
        )
        .unwrap();
    let pixels = image::open(output).unwrap().to_rgba8();
    assert_eq!(pixels.get_pixel(0, 0).0, [0, 0, 0, 0]);
    assert_eq!(pixels.get_pixel(1, 0).0, [255, 255, 255, 128]);
}

#[test]
fn metadata_flag_policies_are_identical_for_compress_and_convert_before_output_access() {
    let tmp = TempDir::new().unwrap();
    for (index, (preserve_metadata, strip_gps, expected)) in [
        (true, false, ErrorCode::CapabilityUnavailable),
        (false, true, ErrorCode::CapabilityUnavailable),
        (true, true, ErrorCode::InvalidInput),
    ]
    .into_iter()
    .enumerate()
    {
        let compress_output = tmp.path().join(format!("compress-{index}/output.png"));
        let compress_error = CompressProcessor
            .process(
                FileInput::from_path(tmp.path().join("missing.png")),
                CompressConfig {
                    format: Some(rtools_core::ImageFormat::Png),
                    output: Some(compress_output.clone()),
                    preserve_metadata,
                    strip_gps,
                    ..CompressConfig::default()
                },
            )
            .unwrap_err();
        assert_eq!(compress_error.code(), expected);
        assert!(!compress_output.parent().unwrap().exists());

        let convert_output = tmp.path().join(format!("convert-{index}/output.png"));
        let convert_error = ConvertProcessor
            .process(
                FileInput::from_path(tmp.path().join("missing.png")),
                ConvertConfig {
                    target_format: rtools_core::ImageFormat::Png,
                    output: Some(convert_output.clone()),
                    preserve_metadata,
                    strip_gps,
                    ..ConvertConfig::default()
                },
            )
            .unwrap_err();
        assert_eq!(convert_error.code(), expected);
        assert!(!convert_output.parent().unwrap().exists());
    }
}

#[test]
fn every_single_frame_processor_records_orientation_and_uses_oriented_geometry() {
    let tmp = TempDir::new().unwrap();
    let encoded = include_str!("../fixtures/images/orientation-6.jpg.b64");
    let watermark = create_solid_png(tmp.path(), "mark.png", 2, 2, [255, 255, 255, 255]);
    let input = |name: &str| decode_fixture(tmp.path(), name, encoded);
    let outputs = [
        CompressProcessor
            .process(
                FileInput::from_path(input("compress.jpg")),
                CompressConfig {
                    format: Some(rtools_core::ImageFormat::Png),
                    output: Some(tmp.path().join("compress.png")),
                    ..CompressConfig::default()
                },
            )
            .unwrap(),
        ConvertProcessor
            .process(
                FileInput::from_path(input("convert.jpg")),
                ConvertConfig {
                    target_format: rtools_core::ImageFormat::Png,
                    output: Some(tmp.path().join("convert.png")),
                    ..ConvertConfig::default()
                },
            )
            .unwrap(),
        ResizeProcessor
            .process(
                FileInput::from_path(input("resize.jpg")),
                ResizeConfig {
                    width: Some(36),
                    output: Some(tmp.path().join("resize.png")),
                    ..ResizeConfig::default()
                },
            )
            .unwrap(),
        CropProcessor
            .process(
                FileInput::from_path(input("crop.jpg")),
                CropConfig {
                    region: CropRegion::AspectRatio {
                        ratio: AspectRatio::Original,
                        gravity: Gravity::Center,
                    },
                    output: Some(tmp.path().join("crop.png")),
                    ..CropConfig::default()
                },
            )
            .unwrap(),
        FilterProcessor
            .process(
                FileInput::from_path(input("filter.jpg")),
                FilterConfig {
                    output: Some(tmp.path().join("filter.png")),
                    ..FilterConfig::default()
                },
            )
            .unwrap(),
        WatermarkProcessor
            .process(
                FileInput::from_path(input("watermark.jpg")),
                WatermarkConfig {
                    watermark: WatermarkType::Image {
                        image_path: watermark,
                        scale: 1.0,
                    },
                    position: WatermarkPosition::Pixels { x: 0, y: 0 },
                    output: Some(tmp.path().join("watermark.png")),
                    ..WatermarkConfig::default()
                },
            )
            .unwrap(),
    ];

    for output in outputs {
        assert_eq!(output.warnings, ["EXIF orientation 6 applied"]);
        assert_eq!(
            image::open(output.destination.as_path().unwrap())
                .unwrap()
                .dimensions(),
            (36, 24)
        );
    }
}

#[test]
fn absent_orientation_and_malformed_exif_leave_geometry_unchanged() {
    let tmp = TempDir::new().unwrap();
    let plain = create_test_image(tmp.path(), "plain.jpg", 7, 5);
    let plain_bytes = std::fs::read(&plain).unwrap();
    let mut malformed = Vec::with_capacity(plain_bytes.len() + 14);
    malformed.extend_from_slice(&plain_bytes[..2]);
    malformed.extend_from_slice(b"\xff\xe1\x00\x0cExif\0\0bad!");
    malformed.extend_from_slice(&plain_bytes[2..]);
    let malformed_path = tmp.path().join("malformed.jpg");
    std::fs::write(&malformed_path, malformed).unwrap();

    for path in [plain, malformed_path] {
        let decoded =
            rtools_image::format::decode_bounded(&path, &ResourceLimits::default()).unwrap();
        assert_eq!(decoded.orientation_applied, None);
        assert_eq!(decoded.image.dimensions(), (7, 5));
        assert!(decoded.warnings().is_empty());
    }
}
