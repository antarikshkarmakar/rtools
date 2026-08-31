use crate::ImageCommands;
use rtools_core::AppConfig;
use rtools_core::FileInput;
use rtools_core::Processor;
use rtools_image::{CompressConfig, CompressProcessor, CropConfig, CropProcessor, ConvertConfig, ConvertProcessor, FilterConfig, FilterProcessor, ResizeConfig, ResizeProcessor, WatermarkConfig, WatermarkProcessor};
use std::path::PathBuf;

pub async fn handle_image_command(cmd: ImageCommands, config: &AppConfig) -> anyhow::Result<()> {
    match cmd {
        ImageCommands::Compress {
            input,
            output,
            quality,
            format,
            preserve_metadata,
            strip_gps,
        } => {
            let processor = CompressProcessor;
            let compress_config = CompressConfig {
                preset: rtools_image::compress::CompressionPreset::Custom(quality),
                format: format.and_then(|f| rtools_core::ImageFormat::from_extension(&f)),
                output,
                preserve_metadata,
                strip_gps,
            };

            for input_path in input {
                let file_input = FileInput::from_path(input_path.clone());
                match processor.process(file_input, compress_config.clone()) {
                    Ok(output) => {
                        println!("✓ Compressed: {}", input_path.display());
                        if let Some(stats) = &output.stats {
                            println!("  Size: {} → {} ({:.1}%)", 
                                format_size(stats.input_size),
                                format_size(stats.output_size),
                                stats.compression_ratio * 100.0
                            );
                        }
                    }
                    Err(e) => {
                        eprintln!("✗ Failed to compress {}: {}", input_path.display(), e);
                    }
                }
            }
            Ok(())
        }

        ImageCommands::Convert {
            input,
            format,
            output,
            quality,
        } => {
            let processor = ConvertProcessor;
            let target_format = rtools_core::ImageFormat::from_extension(&format)
                .ok_or_else(|| anyhow::anyhow!("Unsupported format: {}", format))?;

            let convert_config = ConvertConfig {
                target_format,
                output,
                output_dir: None,
                quality,
                preserve_metadata: true,
                strip_gps: false,
            };

            for input_path in input {
                let file_input = FileInput::from_path(input_path.clone());
                match processor.process(file_input, convert_config.clone()) {
                    Ok(output) => {
                        println!("✓ Converted: {} → {}", input_path.display(), 
                            output.destination.as_path().map(|p| p.display().to_string()).unwrap_or_default());
                    }
                    Err(e) => {
                        eprintln!("✗ Failed to convert {}: {}", input_path.display(), e);
                    }
                }
            }
            Ok(())
        }

        ImageCommands::Resize {
            input,
            width,
            height,
            maintain_aspect,
            output,
        } => {
            let processor = ResizeProcessor;
            let resize_config = ResizeConfig {
                width,
                height,
                maintain_aspect,
                algorithm: Default::default(),
                output: output.and_then(|o| if o.is_dir() { None } else { Some(o) }),
                quality: 85,
            };

            for input_path in input {
                let file_input = FileInput::from_path(input_path.clone());
                match processor.process(file_input, resize_config.clone()) {
                    Ok(output) => {
                        println!("✓ Resized: {}", input_path.display());
                    }
                    Err(e) => {
                        eprintln!("✗ Failed to resize {}: {}", input_path.display(), e);
                    }
                }
            }
            Ok(())
        }

        ImageCommands::Crop {
            input,
            region,
            ratio,
            gravity,
            output,
        } => {
            let processor = CropProcessor;
            let crop_config = CropConfig {
                region: rtools_image::crop::CropRegion::AspectRatio {
                    ratio: rtools_image::crop::AspectRatio::Square,
                    gravity: rtools_image::crop::Gravity::Center,
                },
                output: output.and_then(|o| if o.is_dir() { None } else { Some(o) }),
                quality: 85,
            };

            for input_path in input {
                let file_input = FileInput::from_path(input_path.clone());
                match processor.process(file_input, crop_config.clone()) {
                    Ok(output) => {
                        println!("✓ Cropped: {}", input_path.display());
                    }
                    Err(e) => {
                        eprintln!("✗ Failed to crop {}: {}", input_path.display(), e);
                    }
                }
            }
            Ok(())
        }

        ImageCommands::Watermark {
            input,
            text,
            image: watermark_image,
            position,
            opacity,
            output,
        } => {
            let processor = WatermarkProcessor;
            let watermark_config = WatermarkConfig {
                watermark: if let Some(text) = text {
                    rtools_image::watermark::WatermarkType::Text {
                        text,
                        font_size: 24,
                        font_color: "#ffffff".to_string(),
                    }
                } else if let Some(img) = watermark_image {
                    rtools_image::watermark::WatermarkType::Image {
                        image_path: img,
                        scale: 0.2,
                    }
                } else {
                    anyhow::bail!("Either text or image watermark must be specified");
                },
                position: rtools_image::watermark::WatermarkPosition::BottomRight,
                opacity,
                output: output.and_then(|o| if o.is_dir() { None } else { Some(o) }),
                quality: 85,
            };

            for input_path in input {
                let file_input = FileInput::from_path(input_path.clone());
                match processor.process(file_input, watermark_config.clone()) {
                    Ok(output) => {
                        println!("✓ Watermarked: {}", input_path.display());
                    }
                    Err(e) => {
                        eprintln!("✗ Failed to watermark {}: {}", input_path.display(), e);
                    }
                }
            }
            Ok(())
        }

        ImageCommands::Filter {
            input,
            preset,
            strength,
            output,
        } => {
            let processor = FilterProcessor;
            let filter = match preset.to_lowercase().as_str() {
                "kodak-portra-400" | "portra" => rtools_image::filter::FilmFilter::KodakPortra400,
                "kodak-gold-200" | "gold" => rtools_image::filter::FilmFilter::KodakGold200,
                "fuji-pro-400h" | "fuji" => rtools_image::filter::FilmFilter::FujiPro400H,
                "fuji-velvia-50" | "velvia" => rtools_image::filter::FilmFilter::FujiVelvia50,
                "polaroid-sx70" | "polaroid" => rtools_image::filter::FilmFilter::PolaroidSX70,
                "trix-400" | "trix" => rtools_image::filter::FilmFilter::TriX400,
                "cinestill-800t" | "cinestill" => rtools_image::filter::FilmFilter::Cinestill800T,
                _ => anyhow::bail!("Unknown filter preset: {}", preset),
            };

            let filter_config = FilterConfig {
                filter,
                strength,
                output: output.and_then(|o| if o.is_dir() { None } else { Some(o) }),
                quality: 85,
            };

            for input_path in input {
                let file_input = FileInput::from_path(input_path.clone());
                match processor.process(file_input, filter_config.clone()) {
                    Ok(output) => {
                        println!("✓ Filtered: {}", input_path.display());
                    }
                    Err(e) => {
                        eprintln!("✗ Failed to filter {}: {}", input_path.display(), e);
                    }
                }
            }
            Ok(())
        }

        ImageCommands::Exif { input, format } => {
            let processor = rtools_image::ExifProcessor;
            let config = rtools_image::exif::ExifConfig::default();

            for input_path in input {
                let file_input = FileInput::from_path(input_path.clone());
                match processor.process(file_input, config.clone()) {
                    Ok(exif) => {
                        println!("✓ EXIF for: {}", input_path.display());
                        if let Some(make) = &exif.camera_make {
                            println!("  Camera: {} {}", make, exif.camera_model.as_deref().unwrap_or(""));
                        }
                        if let Some(date) = &exif.datetime_original {
                            println!("  Date: {}", date);
                        }
                        if let Some(lat) = exif.gps_latitude {
                            println!("  GPS: {}, {}", lat, exif.gps_longitude.unwrap_or(0.0));
                        }
                    }
                    Err(e) => {
                        eprintln!("✗ Failed to read EXIF {}: {}", input_path.display(), e);
                    }
                }
            }
            Ok(())
        }

        ImageCommands::Ocr { input, language, output } => {
            // OCR is handled by AI crate
            let processor = rtools_ai::OcrProcessor;
            let config = rtools_ai::ocr::OcrConfig {
                language,
                dpi: 300,
                output_format: rtools_ai::ocr::OcrOutputFormat::Text,
            };

            for input_path in input {
                let file_input = FileInput::from_path(input_path.clone());
                match processor.process(file_input, config.clone()) {
                    Ok(result) => {
                        println!("✓ OCR for: {}", input_path.display());
                        println!("  Text: {}", result.text);
                    }
                    Err(e) => {
                        eprintln!("✗ Failed to OCR {}: {}", input_path.display(), e);
                    }
                }
            }
            Ok(())
        }
    }
}

fn format_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}