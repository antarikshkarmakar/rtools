use rmcp::{
    handler::ServerHandler,
    model::{CallToolResult, Content, Tool},
    schemars::JsonSchema,
    serde::{Deserialize, Serialize},
    service::ServiceExt,
    Error, RoleServer,
};
use std::path::PathBuf;
use tokio::io::{stdin, stdout};

#[derive(Clone)]
struct RToolsServer {
    config: rtools_core::AppConfig,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct CompressInput {
    /// Path to the image file
    input_path: String,
    /// Output path (optional)
    output_path: Option<String>,
    /// Quality (1-100)
    quality: Option<u8>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct ConvertInput {
    /// Path to the image file
    input_path: String,
    /// Target format (webp, png, jpg, avif)
    target_format: String,
    /// Output path (optional)
    output_path: Option<String>,
    /// Quality for lossy formats (1-100)
    quality: Option<u8>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct ResizeInput {
    /// Path to the image file
    input_path: String,
    /// Target width
    width: Option<u32>,
    /// Target height
    height: Option<u32>,
    /// Maintain aspect ratio
    maintain_aspect: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct OrganizeInput {
    /// Input directory path
    input_dir: String,
    /// Output directory path
    output_dir: String,
    /// Organization strategy (date, subject, location)
    strategy: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct RenameInput {
    /// Input directory path
    input_dir: String,
    /// Filename pattern
    pattern: Option<String>,
    /// Dry run mode
    dry_run: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct AltTextInput {
    /// Path to the image file
    input_path: String,
    /// Language
    language: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct DuplicatesInput {
    /// Input directory path
    input_dir: String,
    /// Similarity threshold (0.0-1.0)
    threshold: Option<f64>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct PdfCompressInput {
    /// Path to the PDF file
    input_path: String,
    /// Output path (optional)
    output_path: Option<String>,
    /// Compression level (light, medium, heavy)
    level: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct PdfMergeInput {
    /// List of PDF file paths
    input_paths: Vec<String>,
    /// Output path
    output_path: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct OcrInput {
    /// Path to the image/PDF file
    input_path: String,
    /// Language
    language: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct MetadataInput {
    /// Path to the image file
    input_path: String,
}

impl RToolsServer {
    fn new() -> Self {
        let config = rtools_core::AppConfig::load(None).unwrap_or_default();
        Self { config }
    }

    fn get_tools(&self) -> Vec<Tool> {
        vec![
            Tool {
                name: "compress_image".into(),
                description: Some("Compress an image with quality preservation".into()),
                input_schema: rmcp::schemars::schema_for!(CompressInput).into(),
            },
            Tool {
                name: "convert_image".into(),
                description: Some("Convert image to different format (WebP, PNG, JPG, AVIF)".into()),
                input_schema: rmcp::schemars::schema_for!(ConvertInput).into(),
            },
            Tool {
                name: "resize_image".into(),
                description: Some("Resize image by dimensions".into()),
                input_schema: rmcp::schemars::schema_for!(ResizeInput).into(),
            },
            Tool {
                name: "organize_photos".into(),
                description: Some("AI-organize photos into folders".into()),
                input_schema: rmcp::schemars::schema_for!(OrganizeInput).into(),
            },
            Tool {
                name: "rename_photos".into(),
                description: Some("AI-rename photos with descriptive names".into()),
                input_schema: rmcp::schemars::schema_for!(RenameInput).into(),
            },
            Tool {
                name: "generate_alt_text".into(),
                description: Some("Generate accessibility alt text for an image".into()),
                input_schema: rmcp::schemars::schema_for!(AltTextInput).into(),
            },
            Tool {
                name: "find_duplicates".into(),
                description: Some("Find duplicate images by visual similarity".into()),
                input_schema: rmcp::schemars::schema_for!(DuplicatesInput).into(),
            },
            Tool {
                name: "compress_pdf".into(),
                description: Some("Compress PDF file size".into()),
                input_schema: rmcp::schemars::schema_for!(PdfCompressInput).into(),
            },
            Tool {
                name: "merge_pdfs".into(),
                description: Some("Merge multiple PDF files into one".into()),
                input_schema: rmcp::schemars::schema_for!(PdfMergeInput).into(),
            },
            Tool {
                name: "extract_text".into(),
                description: Some("Extract text from image or PDF using OCR".into()),
                input_schema: rmcp::schemars::schema_for!(OcrInput).into(),
            },
            Tool {
                name: "get_metadata".into(),
                description: Some("Get image metadata including EXIF data".into()),
                input_schema: rmcp::schemars::schema_for!(MetadataInput).into(),
            },
        ]
    }

    async fn call_tool(&self, tool_name: &str, input: serde_json::Value) -> Result<CallToolResult, Error> {
        match tool_name {
            "compress_image" => {
                let input: CompressInput = serde_json::from_value(input)
                    .map_err(|e| Error::invalid_params(e.to_string()))?;
                
                let file_input = rtools_core::FileInput::from_path(PathBuf::from(&input.input_path));
                let config = rtools_image::CompressConfig {
                    preset: rtools_image::compress::CompressionPreset::Custom(input.quality.unwrap_or(85)),
                    format: None,
                    output: input.output_path.map(PathBuf::from),
                    preserve_metadata: true,
                    strip_gps: false,
                };

                let processor = rtools_image::CompressProcessor;
                match processor.process(file_input, config) {
                    Ok(output) => {
                        let mut result = format!("Compressed successfully");
                        if let Some(stats) = &output.stats {
                            result.push_str(&format!("\nInput size: {} bytes", stats.input_size));
                            result.push_str(&format!("\nOutput size: {} bytes", stats.output_size));
                            result.push_str(&format!("\nCompression ratio: {:.1}%", stats.compression_ratio * 100.0));
                        }
                        Ok(CallToolResult::success(vec![Content::text(result)]))
                    }
                    Err(e) => Ok(CallToolResult::error(vec![Content::text(e.to_string())])),
                }
            }

            "convert_image" => {
                let input: ConvertInput = serde_json::from_value(input)
                    .map_err(|e| Error::invalid_params(e.to_string()))?;
                
                let file_input = rtools_core::FileInput::from_path(PathBuf::from(&input.input_path));
                let target_format = rtools_core::ImageFormat::from_extension(&input.target_format)
                    .ok_or_else(|| Error::invalid_params(format!("Unsupported format: {}", input.target_format)))?;
                
                let config = rtools_image::ConvertConfig {
                    target_format,
                    output: input.output_path.map(PathBuf::from),
                    output_dir: None,
                    quality: input.quality.unwrap_or(85),
                    preserve_metadata: true,
                    strip_gps: false,
                };

                let processor = rtools_image::ConvertProcessor;
                match processor.process(file_input, config) {
                    Ok(output) => {
                        let result = format!("Converted to {}", input.target_format);
                        Ok(CallToolResult::success(vec![Content::text(result)]))
                    }
                    Err(e) => Ok(CallToolResult::error(vec![Content::text(e.to_string())])),
                }
            }

            "resize_image" => {
                let input: ResizeInput = serde_json::from_value(input)
                    .map_err(|e| Error::invalid_params(e.to_string()))?;
                
                let file_input = rtools_core::FileInput::from_path(PathBuf::from(&input.input_path));
                let config = rtools_image::ResizeConfig {
                    width: input.width,
                    height: input.height,
                    maintain_aspect: input.maintain_aspect.unwrap_or(true),
                    algorithm: Default::default(),
                    output: None,
                    quality: 85,
                };

                let processor = rtools_image::ResizeProcessor;
                match processor.process(file_input, config) {
                    Ok(output) => {
                        let result = format!("Resized successfully");
                        Ok(CallToolResult::success(vec![Content::text(result)]))
                    }
                    Err(e) => Ok(CallToolResult::error(vec![Content::text(e.to_string())])),
                }
            }

            "organize_photos" => {
                let input: OrganizeInput = serde_json::from_value(input)
                    .map_err(|e| Error::invalid_params(e.to_string()))?;
                
                // Collect images from directory
                let inputs = collect_images(&input.input_dir)?;
                let config = rtools_ai::organize::OrganizeConfig {
                    output_dir: PathBuf::from(&input.output_dir),
                    strategy: match input.strategy.as_deref().unwrap_or("date") {
                        "subject" => rtools_ai::organize::OrganizeStrategy::BySubject,
                        "location" => rtools_ai::organize::OrganizeStrategy::ByLocation,
                        _ => rtools_ai::organize::OrganizeStrategy::ByDate,
                    },
                    by_date: true,
                    by_subject: false,
                    dry_run: false,
                };

                let processor = rtools_ai::OrganizeProcessor;
                match processor.process(inputs, config) {
                    Ok(outputs) => {
                        let result = format!("Organized {} photos into folders", outputs.len());
                        Ok(CallToolResult::success(vec![Content::text(result)]))
                    }
                    Err(e) => Ok(CallToolResult::error(vec![Content::text(e.to_string())])),
                }
            }

            "rename_photos" => {
                let input: RenameInput = serde_json::from_value(input)
                    .map_err(|e| Error::invalid_params(e.to_string()))?;
                
                let inputs = collect_images(&input.input_dir)?;
                let config = rtools_ai::rename::RenameConfig {
                    pattern: input.pattern.unwrap_or_else(|| "{date}_{subject}_{index}".to_string()),
                    output_dir: None,
                    start_number: 1,
                    use_ai_descriptions: true,
                    dry_run: input.dry_run.unwrap_or(false),
                };

                let processor = rtools_ai::RenameProcessor;
                match processor.process(inputs, config) {
                    Ok(outputs) => {
                        let result = format!("Renamed {} photos", outputs.len());
                        Ok(CallToolResult::success(vec![Content::text(result)]))
                    }
                    Err(e) => Ok(CallToolResult::error(vec![Content::text(e.to_string())])),
                }
            }

            "generate_alt_text" => {
                let input: AltTextInput = serde_json::from_value(input)
                    .map_err(|e| Error::invalid_params(e.to_string()))?;
                
                let file_input = rtools_core::FileInput::from_path(PathBuf::from(&input.input_path));
                let config = rtools_ai::alt_text::AltTextConfig {
                    language: input.language.unwrap_or_else(|| "en".to_string()),
                    max_length: 125,
                    output_format: rtools_ai::alt_text::AltTextOutputFormat::Text,
                };

                let processor = rtools_ai::AltTextProcessor;
                match processor.process(file_input, config) {
                    Ok(result) => {
                        Ok(CallToolResult::success(vec![Content::text(result.alt_text)]))
                    }
                    Err(e) => Ok(CallToolResult::error(vec![Content::text(e.to_string())])),
                }
            }

            "find_duplicates" => {
                let input: DuplicatesInput = serde_json::from_value(input)
                    .map_err(|e| Error::invalid_params(e.to_string()))?;
                
                let inputs = collect_images(&input.input_dir)?;
                let config = rtools_ai::duplicates::DuplicatesConfig {
                    threshold: input.threshold.unwrap_or(0.9),
                    algorithm: rtools_ai::duplicates::HashAlgorithm::Perceptual,
                    action: rtools_ai::duplicates::DuplicateAction::Report,
                    dry_run: false,
                };

                let processor = rtools_ai::DuplicatesProcessor;
                match processor.process(inputs, config) {
                    Ok(result) => {
                        let mut result_text = format!("Found {} duplicate groups\n", result.groups.len());
                        result_text.push_str(&format!("Originals: {}\n", result.total_originals));
                        result_text.push_str(&format!("Duplicates: {}\n", result.total_duplicates));
                        Ok(CallToolResult::success(vec![Content::text(result_text)]))
                    }
                    Err(e) => Ok(CallToolResult::error(vec![Content::text(e.to_string())])),
                }
            }

            "compress_pdf" => {
                let input: PdfCompressInput = serde_json::from_value(input)
                    .map_err(|e| Error::invalid_params(e.to_string()))?;
                
                let file_input = rtools_core::FileInput::from_path(PathBuf::from(&input.input_path));
                let config = rtools_pdf::PdfCompressConfig {
                    level: match input.level.as_deref().unwrap_or("medium") {
                        "light" => rtools_pdf::compress::PdfCompressionLevel::Light,
                        "heavy" => rtools_pdf::compress::PdfCompressionLevel::Heavy,
                        _ => rtools_pdf::compress::PdfCompressionLevel::Medium,
                    },
                    output: input.output_path.map(PathBuf::from),
                    remove_metadata: false,
                    remove_images: false,
                };

                let processor = rtools_pdf::PdfCompressProcessor;
                match processor.process(file_input, config) {
                    Ok(output) => {
                        let mut result = "Compressed PDF successfully".to_string();
                        if let Some(stats) = &output.stats {
                            result.push_str(&format!("\nInput size: {} bytes", stats.input_size));
                            result.push_str(&format!("\nOutput size: {} bytes", stats.output_size));
                        }
                        Ok(CallToolResult::success(vec![Content::text(result)]))
                    }
                    Err(e) => Ok(CallToolResult::error(vec![Content::text(e.to_string())])),
                }
            }

            "merge_pdfs" => {
                let input: PdfMergeInput = serde_json::from_value(input)
                    .map_err(|e| Error::invalid_params(e.to_string()))?;
                
                let file_inputs: Vec<rtools_core::FileInput> = input.input_paths.iter()
                    .map(|p| rtools_core::FileInput::from_path(PathBuf::from(p)))
                    .collect();

                let config = rtools_pdf::PdfMergeConfig {
                    inputs: input.input_paths.iter().map(PathBuf::from).collect(),
                    output: PathBuf::from(&input.output_path),
                    add_page_numbers: false,
                };

                let processor = rtools_pdf::PdfMergeProcessor;
                match processor.process(file_inputs, config) {
                    Ok(output) => {
                        let result = format!("Merged PDFs into {}", input.output_path);
                        Ok(CallToolResult::success(vec![Content::text(result)]))
                    }
                    Err(e) => Ok(CallToolResult::error(vec![Content::text(e.to_string())])),
                }
            }

            "extract_text" => {
                let input: OcrInput = serde_json::from_value(input)
                    .map_err(|e| Error::invalid_params(e.to_string()))?;
                
                let file_input = rtools_core::FileInput::from_path(PathBuf::from(&input.input_path));
                let config = rtools_ai::ocr::OcrConfig {
                    language: input.language.unwrap_or_else(|| "eng".to_string()),
                    dpi: 300,
                    output_format: rtools_ai::ocr::OcrOutputFormat::Text,
                };

                let processor = rtools_ai::OcrProcessor;
                match processor.process(file_input, config) {
                    Ok(result) => {
                        Ok(CallToolResult::success(vec![Content::text(result.text)]))
                    }
                    Err(e) => Ok(CallToolResult::error(vec![Content::text(e.to_string())])),
                }
            }

            "get_metadata" => {
                let input: MetadataInput = serde_json::from_value(input)
                    .map_err(|e| Error::invalid_params(e.to_string()))?;
                
                let file_input = rtools_core::FileInput::from_path(PathBuf::from(&input.input_path));
                let config = rtools_image::MetadataConfig::default();

                let processor = rtools_image::MetadataProcessor;
                match processor.process(file_input, config) {
                    Ok(metadata) => {
                        let result = serde_json::to_string_pretty(&metadata)
                            .map_err(|e| Error::internal_error(e.to_string()))?;
                        Ok(CallToolResult::success(vec![Content::text(result)]))
                    }
                    Err(e) => Ok(CallToolResult::error(vec![Content::text(e.to_string())])),
                }
            }

            _ => Err(Error::method_not_found(format!("Unknown tool: {}", tool_name))),
        }
    }
}

fn collect_images(dir: &str) -> Result<Vec<rtools_core::FileInput>, Error> {
    let mut inputs = Vec::new();
    let valid_extensions = ["jpg", "jpeg", "png", "webp", "heic", "heif", "tiff", "bmp", "gif"];

    for entry in walkdir::WalkDir::new(dir).into_iter().filter_map(|e| e.ok()) {
        if entry.file_type().is_file() {
            if let Some(ext) = entry.path().extension().and_then(|e| e.to_str()) {
                if valid_extensions.contains(&ext.to_lowercase().as_str()) {
                    inputs.push(rtools_core::FileInput::from_path(entry.path().to_path_buf()));
                }
            }
        }
    }

    Ok(inputs)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let server = RToolsServer::new();
    let service = server.serve(stdin(), stdout()).await?;
    service.waiting().await?;

    Ok(())
}