use crate::ImageCommands;
use rtools_core::AppConfig;
use rtools_core::FileInput;
use rtools_core::Processor;
use rtools_image::{
    CompressConfig, CompressProcessor, ConvertConfig, ConvertProcessor, CropConfig, CropProcessor,
    FilterConfig, FilterProcessor, ResizeConfig, ResizeProcessor, WatermarkConfig,
    WatermarkProcessor,
};

#[allow(clippy::too_many_lines)] // Task 4 will split individual image command handlers.
pub async fn handle_image_command(cmd: ImageCommands, config: &AppConfig) -> anyhow::Result<()> {
    std::future::ready(()).await;
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
                output_policy: rtools_core::OutputPolicy::default(),
                preserve_metadata,
                strip_gps,
                limits: config.limits.clone(),
            };

            for input_path in input {
                let file_input = FileInput::from_path(input_path.clone());
                match processor.process(file_input, compress_config.clone()) {
                    Ok(output) => {
                        println!("✓ Compressed: {}", input_path.display());
                        if let Some(stats) = &output.stats {
                            println!(
                                "  Size: {} → {} ({:.1}%)",
                                format_size(stats.input_size),
                                format_size(stats.output_size),
                                stats.compression_ratio * 100.0
                            );
                        }
                    }
                    Err(e) => {
                        return Err(e.into());
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
                .ok_or_else(|| anyhow::anyhow!("Unsupported format: {format}"))?;

            let convert_config = ConvertConfig {
                target_format,
                output,
                output_policy: rtools_core::OutputPolicy::default(),
                output_dir: None,
                quality,
                preserve_metadata: false,
                strip_gps: false,
                limits: config.limits.clone(),
            };

            for input_path in input {
                let file_input = FileInput::from_path(input_path.clone());
                match processor.process(file_input, convert_config.clone()) {
                    Ok(output) => {
                        println!(
                            "✓ Converted: {} → {}",
                            input_path.display(),
                            output
                                .destination
                                .as_path()
                                .map(|p| p.display().to_string())
                                .unwrap_or_default()
                        );
                    }
                    Err(e) => {
                        return Err(e.into());
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
                algorithm: rtools_image::resize::ResizeAlgorithm::default(),
                output,
                output_policy: rtools_core::OutputPolicy::default(),
                quality: 85,
                limits: config.limits.clone(),
            };

            for input_path in input {
                let file_input = FileInput::from_path(input_path.clone());
                match processor.process(file_input, resize_config.clone()) {
                    Ok(_output) => {
                        println!("✓ Resized: {}", input_path.display());
                    }
                    Err(e) => {
                        return Err(e.into());
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

            // Parse gravity
            let grav = match gravity.to_lowercase().as_str() {
                "north" | "n" => rtools_image::crop::Gravity::North,
                "south" | "s" => rtools_image::crop::Gravity::South,
                "east" | "e" => rtools_image::crop::Gravity::East,
                "west" | "w" => rtools_image::crop::Gravity::West,
                "northeast" | "ne" => rtools_image::crop::Gravity::NorthEast,
                "northwest" | "nw" => rtools_image::crop::Gravity::NorthWest,
                "southeast" | "se" => rtools_image::crop::Gravity::SouthEast,
                "southwest" | "sw" => rtools_image::crop::Gravity::SouthWest,
                _ => rtools_image::crop::Gravity::Center,
            };

            // Parse crop region from CLI args
            let crop_region = if let Some(ratio_str) = ratio {
                let parts: Vec<&str> = ratio_str.split(':').collect();
                if parts.len() == 2 {
                    let w: f64 = parts[0].parse().unwrap_or(1.0);
                    let h: f64 = parts[1].parse().unwrap_or(1.0);
                    rtools_image::crop::CropRegion::AspectRatio {
                        ratio: rtools_image::crop::AspectRatio::Custom(w, h),
                        gravity: grav,
                    }
                } else {
                    rtools_image::crop::CropRegion::AspectRatio {
                        ratio: rtools_image::crop::AspectRatio::Square,
                        gravity: grav,
                    }
                }
            } else if let Some(region_str) = region {
                let parts: Vec<&str> = region_str.split(',').collect();
                if parts.len() == 4 {
                    let x: u32 = parts[0].parse().unwrap_or(0);
                    let y: u32 = parts[1].parse().unwrap_or(0);
                    let w: u32 = parts[2].parse().unwrap_or(100);
                    let h: u32 = parts[3].parse().unwrap_or(100);
                    rtools_image::crop::CropRegion::Pixels {
                        x,
                        y,
                        width: w,
                        height: h,
                    }
                } else {
                    rtools_image::crop::CropRegion::AspectRatio {
                        ratio: rtools_image::crop::AspectRatio::Square,
                        gravity: grav,
                    }
                }
            } else {
                rtools_image::crop::CropRegion::AspectRatio {
                    ratio: rtools_image::crop::AspectRatio::Square,
                    gravity: grav,
                }
            };

            let crop_config = CropConfig {
                region: crop_region,
                output,
                output_policy: rtools_core::OutputPolicy::default(),
                quality: 85,
                limits: config.limits.clone(),
            };

            for input_path in input {
                let file_input = FileInput::from_path(input_path.clone());
                match processor.process(file_input, crop_config.clone()) {
                    Ok(_output) => {
                        println!("✓ Cropped: {}", input_path.display());
                    }
                    Err(e) => {
                        return Err(e.into());
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
                position: match position.to_lowercase().as_str() {
                    "topleft" | "top-left" => rtools_image::watermark::WatermarkPosition::TopLeft,
                    "topright" | "top-right" => {
                        rtools_image::watermark::WatermarkPosition::TopRight
                    }
                    "bottomleft" | "bottom-left" => {
                        rtools_image::watermark::WatermarkPosition::BottomLeft
                    }
                    "center" => rtools_image::watermark::WatermarkPosition::Center,
                    _ => rtools_image::watermark::WatermarkPosition::BottomRight,
                },
                opacity,
                output,
                output_policy: rtools_core::OutputPolicy::default(),
                quality: 85,
                limits: config.limits.clone(),
            };

            for input_path in input {
                let file_input = FileInput::from_path(input_path.clone());
                match processor.process(file_input, watermark_config.clone()) {
                    Ok(_output) => {
                        println!("✓ Watermarked: {}", input_path.display());
                    }
                    Err(e) => {
                        return Err(e.into());
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
                _ => anyhow::bail!("Unknown filter preset: {preset}"),
            };

            let filter_config = FilterConfig {
                filter,
                strength,
                output,
                output_policy: rtools_core::OutputPolicy::default(),
                quality: 85,
                limits: config.limits.clone(),
            };

            for input_path in input {
                let file_input = FileInput::from_path(input_path.clone());
                match processor.process(file_input, filter_config.clone()) {
                    Ok(_output) => {
                        println!("✓ Filtered: {}", input_path.display());
                    }
                    Err(e) => {
                        return Err(e.into());
                    }
                }
            }
            Ok(())
        }

        ImageCommands::Exif { input, format: _ } => {
            let processor = rtools_image::ExifProcessor;
            let config = rtools_image::exif::ExifConfig::default();

            for input_path in input {
                let file_input = FileInput::from_path(input_path.clone());
                match processor.process(file_input, config.clone()) {
                    Ok(exif) => {
                        println!("✓ EXIF for: {}", input_path.display());
                        if let Some(make) = &exif.camera_make {
                            println!(
                                "  Camera: {} {}",
                                make,
                                exif.camera_model.as_deref().unwrap_or("")
                            );
                        }
                        if let Some(date) = &exif.datetime_original {
                            println!("  Date: {date}");
                        }
                        if let Some(lat) = exif.gps_latitude {
                            println!("  GPS: {}, {}", lat, exif.gps_longitude.unwrap_or(0.0));
                        }
                    }
                    Err(e) => {
                        return Err(e.into());
                    }
                }
            }
            Ok(())
        }

        ImageCommands::Ocr {
            input,
            language,
            output: _,
        } => {
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
                        return Err(e.into());
                    }
                }
            }
            Ok(())
        }
    }
}

#[allow(clippy::cast_precision_loss)]
const fn bytes_to_f64(bytes: u64) -> f64 {
    // Displaying human-readable sizes tolerates rounding beyond f64 precision.
    bytes as f64
}

fn format_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes_to_f64(bytes) / 1024.0)
    } else {
        format!("{:.1} MB", bytes_to_f64(bytes) / (1024.0 * 1024.0))
    }
}

#[cfg(test)]
mod tests {
    use super::handle_image_command;
    use crate::ImageCommands;
    use rtools_core::{AppConfig, ErrorCode, RToolsError};

    #[tokio::test]
    async fn image_ocr_processor_error_is_propagated() {
        let error = handle_image_command(
            ImageCommands::Ocr {
                input: vec!["private.png".into()],
                language: "eng".to_string(),
                output: None,
            },
            &AppConfig::default(),
        )
        .await
        .unwrap_err();
        let error = error.downcast_ref::<RToolsError>().unwrap();

        assert_eq!(error.code(), ErrorCode::CapabilityUnavailable);
        assert!(matches!(
            error,
            RToolsError::CapabilityUnavailable { operation_id, .. }
                if operation_id == "ai.ocr"
        ));
    }

    #[tokio::test]
    async fn available_image_processor_error_is_propagated() {
        let error = handle_image_command(
            ImageCommands::Resize {
                input: vec!["missing.png".into()],
                width: Some(10),
                height: None,
                maintain_aspect: true,
                output: None,
            },
            &AppConfig::default(),
        )
        .await
        .unwrap_err();

        assert!(error.downcast_ref::<RToolsError>().is_some());
    }
}
