use rtools_core::{FileInput, Processor};
use rtools_ai::{
    duplicates::{DuplicateAction, DuplicatesConfig, DuplicatesProcessor, HashAlgorithm},
    rename::{RenameConfig, RenameProcessor},
};
use tempfile::TempDir;

/// Create a test PNG filled with a deterministic pattern seeded from the file
/// name and dimensions, so that differently-named/sized images have distinct
/// content (and distinct perceptual hashes), while a byte-for-byte copy stays
/// identical.
fn create_test_image(dir: &std::path::Path, name: &str, width: u32, height: u32) -> std::path::PathBuf {
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

    let path = dir.join(name);
    use image::ImageEncoder;
    img.write_with_encoder(image::codecs::png::PngEncoder::new(std::io::BufWriter::new(
        std::fs::File::create(&path).unwrap(),
    )))
    .unwrap();
    path
}

fn create_test_images(dir: &std::path::Path, count: usize) -> Vec<std::path::PathBuf> {
    (0..count)
        .map(|i| create_test_image(dir, &format!("img_{}.png", i), 100, 100))
        .collect()
}

#[test]
fn test_rename_pattern_index() {
    let tmp = TempDir::new().unwrap();
    let images = create_test_images(tmp.path(), 3);

    let inputs: Vec<FileInput> = images.iter().map(|p| FileInput::from_path(p.clone())).collect();
    let config = RenameConfig {
        pattern: "photo_{index}".to_string(),
        output_dir: Some(tmp.path().join("renamed")),
        start_number: 1,
        use_ai_descriptions: false,
        dry_run: true,
    };

    let processor = RenameProcessor;
    let result = processor.process(inputs, config).unwrap();

    assert_eq!(result.len(), 3);
    for output in &result {
        let name = output.name.as_ref().unwrap();
        assert!(name.starts_with("photo_"));
    }
}

#[test]
fn test_rename_dry_run_no_files() {
    let tmp = TempDir::new().unwrap();
    let images = create_test_images(tmp.path(), 2);
    let original_names: Vec<String> = images
        .iter()
        .map(|p| p.file_name().unwrap().to_str().unwrap().to_string())
        .collect();

    let inputs: Vec<FileInput> = images.iter().map(|p| FileInput::from_path(p.clone())).collect();
    let config = RenameConfig {
        pattern: "{index}_renamed".to_string(),
        output_dir: None,
        start_number: 1,
        use_ai_descriptions: false,
        dry_run: true,
    };

    let processor = RenameProcessor;
    let _result = processor.process(inputs, config).unwrap();

    for name in &original_names {
        assert!(tmp.path().join(name).exists());
    }
}

#[test]
fn test_find_duplicates_identical() {
    let tmp = TempDir::new().unwrap();
    let img_path = create_test_image(tmp.path(), "original.png", 50, 50);

    let copy_path = tmp.path().join("copy.png");
    std::fs::copy(&img_path, &copy_path).unwrap();

    let inputs: Vec<FileInput> = vec![
        FileInput::from_path(img_path),
        FileInput::from_path(copy_path),
    ];

    let config = DuplicatesConfig {
        threshold: 0.9,
        algorithm: HashAlgorithm::Perceptual,
        action: DuplicateAction::Report,
        dry_run: false,
    };

    let processor = DuplicatesProcessor;
    let result = processor.process(inputs, config).unwrap();

    assert!(!result.groups.is_empty(), "Should find duplicate groups");
    assert_eq!(result.total_duplicates, 1);
}

#[test]
fn test_find_duplicates_different() {
    let tmp = TempDir::new().unwrap();
    let img1 = create_test_image(tmp.path(), "a.png", 50, 50);
    let img2 = create_test_image(tmp.path(), "b.png", 200, 200);

    let inputs: Vec<FileInput> = vec![
        FileInput::from_path(img1),
        FileInput::from_path(img2),
    ];

    let config = DuplicatesConfig {
        threshold: 0.9,
        algorithm: HashAlgorithm::Perceptual,
        action: DuplicateAction::Report,
        dry_run: false,
    };

    let processor = DuplicatesProcessor;
    let result = processor.process(inputs, config).unwrap();

    assert!(
        result.groups.is_empty(),
        "Different images should not be duplicates"
    );
}

#[test]
fn test_find_duplicates_empty_input() {
    let config = DuplicatesConfig::default();
    let processor = DuplicatesProcessor;
    let result = processor.process(vec![], config).unwrap();

    assert!(result.groups.is_empty());
    assert_eq!(result.total_originals, 0);
    assert_eq!(result.total_duplicates, 0);
}

#[test]
fn test_rename_start_number() {
    let tmp = TempDir::new().unwrap();
    let images = create_test_images(tmp.path(), 3);

    let inputs: Vec<FileInput> = images.iter().map(|p| FileInput::from_path(p.clone())).collect();
    let config = RenameConfig {
        pattern: "shot_{index}".to_string(),
        output_dir: Some(tmp.path().join("renamed")),
        start_number: 100,
        use_ai_descriptions: false,
        dry_run: true,
    };

    let processor = RenameProcessor;
    let result = processor.process(inputs, config).unwrap();

    let names: Vec<String> = result.iter().filter_map(|o| o.name.clone()).collect();
    assert!(names.contains(&"shot_100.png".to_string()));
    assert!(names.contains(&"shot_101.png".to_string()));
    assert!(names.contains(&"shot_102.png".to_string()));
}