use rmcp::{
    model::{CallToolResult, ContentBlock, ListToolsResult, ServerCapabilities, ServerInfo, Tool},
    schemars::JsonSchema,
    serde::{Deserialize, Serialize},
    service::RequestContext,
    transport::io::stdio,
    ErrorData as McpError, RoleServer, ServerHandler, ServiceExt,
};
use rtools_core::Processor;
use std::fmt::Write as _;
use std::path::PathBuf;

#[derive(Clone)]
struct RToolsServer;

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct CompressInput {
    input_path: String,
    output_path: Option<String>,
    quality: Option<u8>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct ConvertInput {
    input_path: String,
    target_format: String,
    output_path: Option<String>,
    quality: Option<u8>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct ResizeInput {
    input_path: String,
    width: Option<u32>,
    height: Option<u32>,
    maintain_aspect: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct OrganizeInput {
    input_dir: String,
    output_dir: String,
    strategy: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct RenameInput {
    input_dir: String,
    pattern: Option<String>,
    dry_run: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct AltTextInput {
    input_path: String,
    language: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct DuplicatesInput {
    input_dir: String,
    threshold: Option<f64>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct PdfCompressInput {
    input_path: String,
    output_path: Option<String>,
    level: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct PdfMergeInput {
    input_paths: Vec<String>,
    output_path: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct OcrInput {
    input_path: String,
    language: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct MetadataInput {
    input_path: String,
}

impl RToolsServer {
    #[allow(clippy::too_many_lines)] // Task 7 will group tool schemas by domain.
    fn tools() -> Vec<Tool> {
        vec![
            Tool::new(
                "compress_image",
                "Compress an image with quality preservation",
                serde_json::to_value(rmcp::schemars::schema_for!(CompressInput))
                    .unwrap_or_default()
                    .as_object()
                    .cloned()
                    .unwrap_or_default(),
            ),
            Tool::new(
                "convert_image",
                "Convert image to different format (WebP, PNG, JPG, AVIF)",
                serde_json::to_value(rmcp::schemars::schema_for!(ConvertInput))
                    .unwrap_or_default()
                    .as_object()
                    .cloned()
                    .unwrap_or_default(),
            ),
            Tool::new(
                "resize_image",
                "Resize image by dimensions",
                serde_json::to_value(rmcp::schemars::schema_for!(ResizeInput))
                    .unwrap_or_default()
                    .as_object()
                    .cloned()
                    .unwrap_or_default(),
            ),
            Tool::new(
                "organize_photos",
                "AI-organize photos into folders",
                serde_json::to_value(rmcp::schemars::schema_for!(OrganizeInput))
                    .unwrap_or_default()
                    .as_object()
                    .cloned()
                    .unwrap_or_default(),
            ),
            Tool::new(
                "rename_photos",
                "AI-rename photos with descriptive names",
                serde_json::to_value(rmcp::schemars::schema_for!(RenameInput))
                    .unwrap_or_default()
                    .as_object()
                    .cloned()
                    .unwrap_or_default(),
            ),
            Tool::new(
                "generate_alt_text",
                "Generate accessibility alt text for an image",
                serde_json::to_value(rmcp::schemars::schema_for!(AltTextInput))
                    .unwrap_or_default()
                    .as_object()
                    .cloned()
                    .unwrap_or_default(),
            ),
            Tool::new(
                "find_duplicates",
                "Find duplicate images by visual similarity",
                serde_json::to_value(rmcp::schemars::schema_for!(DuplicatesInput))
                    .unwrap_or_default()
                    .as_object()
                    .cloned()
                    .unwrap_or_default(),
            ),
            Tool::new(
                "compress_pdf",
                "Compress PDF file size",
                serde_json::to_value(rmcp::schemars::schema_for!(PdfCompressInput))
                    .unwrap_or_default()
                    .as_object()
                    .cloned()
                    .unwrap_or_default(),
            ),
            Tool::new(
                "merge_pdfs",
                "Merge multiple PDF files into one",
                serde_json::to_value(rmcp::schemars::schema_for!(PdfMergeInput))
                    .unwrap_or_default()
                    .as_object()
                    .cloned()
                    .unwrap_or_default(),
            ),
            Tool::new(
                "extract_text",
                "Extract text from image or PDF using OCR",
                serde_json::to_value(rmcp::schemars::schema_for!(OcrInput))
                    .unwrap_or_default()
                    .as_object()
                    .cloned()
                    .unwrap_or_default(),
            ),
            Tool::new(
                "get_metadata",
                "Get image metadata including EXIF data",
                serde_json::to_value(rmcp::schemars::schema_for!(MetadataInput))
                    .unwrap_or_default()
                    .as_object()
                    .cloned()
                    .unwrap_or_default(),
            ),
        ]
    }

    #[allow(clippy::too_many_lines)] // Task 7 will split MCP tool dispatch by operation.
    async fn handle_tool(
        &self,
        tool_name: &str,
        input: serde_json::Value,
    ) -> Result<CallToolResult, McpError> {
        std::future::ready(()).await;
        match tool_name {
            "compress_image" => {
                let input: CompressInput = serde_json::from_value(input)
                    .map_err(|e| McpError::invalid_params(e.to_string(), None))?;
                let file_input =
                    rtools_core::FileInput::from_path(PathBuf::from(&input.input_path));
                let config = rtools_image::CompressConfig {
                    preset: rtools_image::compress::CompressionPreset::Custom(
                        input.quality.unwrap_or(85),
                    ),
                    format: None,
                    output: input.output_path.map(PathBuf::from),
                    preserve_metadata: true,
                    strip_gps: false,
                };
                let processor = rtools_image::CompressProcessor;
                match processor.process(file_input, config) {
                    Ok(output) => {
                        let mut result = "Compressed successfully".to_string();
                        if let Some(stats) = &output.stats {
                            let _ = write!(result, "\nInput: {} bytes", stats.input_size);
                            let _ = write!(result, "\nOutput: {} bytes", stats.output_size);
                            let _ =
                                write!(result, "\nRatio: {:.1}%", stats.compression_ratio * 100.0);
                        }
                        Ok(CallToolResult::success(vec![ContentBlock::text(result)]))
                    }
                    Err(e) => Ok(CallToolResult::error(vec![ContentBlock::text(
                        e.to_string(),
                    )])),
                }
            }

            "convert_image" => {
                let input: ConvertInput = serde_json::from_value(input)
                    .map_err(|e| McpError::invalid_params(e.to_string(), None))?;
                let file_input =
                    rtools_core::FileInput::from_path(PathBuf::from(&input.input_path));
                let target_format = rtools_core::ImageFormat::from_extension(&input.target_format)
                    .ok_or_else(|| {
                        McpError::invalid_params(
                            format!("Unsupported format: {}", input.target_format),
                            None,
                        )
                    })?;
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
                    Ok(_) => Ok(CallToolResult::success(vec![ContentBlock::text(format!(
                        "Converted to {}",
                        input.target_format
                    ))])),
                    Err(e) => Ok(CallToolResult::error(vec![ContentBlock::text(
                        e.to_string(),
                    )])),
                }
            }

            "resize_image" => {
                let input: ResizeInput = serde_json::from_value(input)
                    .map_err(|e| McpError::invalid_params(e.to_string(), None))?;
                let file_input =
                    rtools_core::FileInput::from_path(PathBuf::from(&input.input_path));
                let config = rtools_image::ResizeConfig {
                    width: input.width,
                    height: input.height,
                    maintain_aspect: input.maintain_aspect.unwrap_or(true),
                    algorithm: rtools_image::resize::ResizeAlgorithm::default(),
                    output: None,
                    quality: 85,
                };
                let processor = rtools_image::ResizeProcessor;
                match processor.process(file_input, config) {
                    Ok(_) => Ok(CallToolResult::success(vec![ContentBlock::text(
                        "Resized successfully",
                    )])),
                    Err(e) => Ok(CallToolResult::error(vec![ContentBlock::text(
                        e.to_string(),
                    )])),
                }
            }

            "organize_photos" => {
                let input: OrganizeInput = serde_json::from_value(input)
                    .map_err(|e| McpError::invalid_params(e.to_string(), None))?;
                let inputs = collect_images(&input.input_dir);
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
                    Ok(outputs) => Ok(CallToolResult::success(vec![ContentBlock::text(format!(
                        "Organized {} photos",
                        outputs.len()
                    ))])),
                    Err(e) => Ok(CallToolResult::error(vec![ContentBlock::text(
                        e.to_string(),
                    )])),
                }
            }

            "rename_photos" => {
                let input: RenameInput = serde_json::from_value(input)
                    .map_err(|e| McpError::invalid_params(e.to_string(), None))?;
                let inputs = collect_images(&input.input_dir);
                let config = rtools_ai::rename::RenameConfig {
                    pattern: input
                        .pattern
                        .unwrap_or_else(|| "{date}_{subject}_{index}".to_string()),
                    output_dir: None,
                    start_number: 1,
                    use_ai_descriptions: true,
                    dry_run: input.dry_run.unwrap_or(false),
                };
                let processor = rtools_ai::RenameProcessor;
                match processor.process(inputs, config) {
                    Ok(outputs) => Ok(CallToolResult::success(vec![ContentBlock::text(format!(
                        "Renamed {} photos",
                        outputs.len()
                    ))])),
                    Err(e) => Ok(CallToolResult::error(vec![ContentBlock::text(
                        e.to_string(),
                    )])),
                }
            }

            "generate_alt_text" => {
                let input: AltTextInput = serde_json::from_value(input)
                    .map_err(|e| McpError::invalid_params(e.to_string(), None))?;
                let file_input =
                    rtools_core::FileInput::from_path(PathBuf::from(&input.input_path));
                let config = rtools_ai::alt_text::AltTextConfig {
                    language: input.language.unwrap_or_else(|| "en".to_string()),
                    max_length: 125,
                    output_format: rtools_ai::alt_text::AltTextOutputFormat::Text,
                };
                let processor = rtools_ai::AltTextProcessor;
                match processor.process(file_input, config) {
                    Ok(result) => Ok(CallToolResult::success(vec![ContentBlock::text(
                        result.alt_text,
                    )])),
                    Err(e) => Ok(CallToolResult::error(vec![ContentBlock::text(
                        e.to_string(),
                    )])),
                }
            }

            "find_duplicates" => {
                let input: DuplicatesInput = serde_json::from_value(input)
                    .map_err(|e| McpError::invalid_params(e.to_string(), None))?;
                let inputs = collect_images(&input.input_dir);
                let config = rtools_ai::duplicates::DuplicatesConfig {
                    threshold: input.threshold.unwrap_or(0.9),
                    algorithm: rtools_ai::duplicates::HashAlgorithm::Perceptual,
                    action: rtools_ai::duplicates::DuplicateAction::Report,
                    dry_run: false,
                };
                let processor = rtools_ai::DuplicatesProcessor;
                match processor.process(inputs, config) {
                    Ok(result) => {
                        let mut text = format!("Found {} duplicate groups\n", result.groups.len());
                        let _ = writeln!(text, "Originals: {}", result.total_originals);
                        let _ = writeln!(text, "Duplicates: {}", result.total_duplicates);
                        Ok(CallToolResult::success(vec![ContentBlock::text(text)]))
                    }
                    Err(e) => Ok(CallToolResult::error(vec![ContentBlock::text(
                        e.to_string(),
                    )])),
                }
            }

            "compress_pdf" => {
                let input: PdfCompressInput = serde_json::from_value(input)
                    .map_err(|e| McpError::invalid_params(e.to_string(), None))?;
                let file_input =
                    rtools_core::FileInput::from_path(PathBuf::from(&input.input_path));
                let config = rtools_pdf::PdfCompressConfig {
                    level: match input.level.as_deref().unwrap_or("medium") {
                        "light" => rtools_pdf::compress::PdfCompressionLevel::Light,
                        "heavy" => rtools_pdf::compress::PdfCompressionLevel::Heavy,
                        _ => rtools_pdf::compress::PdfCompressionLevel::Medium,
                    },
                    output: input.output_path.map(PathBuf::from),
                    remove_metadata: false,
                };
                let processor = rtools_pdf::PdfCompressProcessor;
                match processor.process(file_input, config) {
                    Ok(output) => {
                        let mut text = "Compressed PDF successfully".to_string();
                        if let Some(stats) = &output.stats {
                            let _ = write!(text, "\nInput: {} bytes", stats.input_size);
                            let _ = write!(text, "\nOutput: {} bytes", stats.output_size);
                        }
                        Ok(CallToolResult::success(vec![ContentBlock::text(text)]))
                    }
                    Err(e) => Ok(CallToolResult::error(vec![ContentBlock::text(
                        e.to_string(),
                    )])),
                }
            }

            "merge_pdfs" => {
                let input: PdfMergeInput = serde_json::from_value(input)
                    .map_err(|e| McpError::invalid_params(e.to_string(), None))?;
                let file_inputs: Vec<rtools_core::FileInput> = input
                    .input_paths
                    .iter()
                    .map(|p| rtools_core::FileInput::from_path(PathBuf::from(p)))
                    .collect();
                let config = rtools_pdf::PdfMergeConfig {
                    inputs: input.input_paths.iter().map(PathBuf::from).collect(),
                    output: PathBuf::from(&input.output_path),
                    add_page_numbers: false,
                };
                let processor = rtools_pdf::PdfMergeProcessor;
                match processor.process(file_inputs, config) {
                    Ok(_) => Ok(CallToolResult::success(vec![ContentBlock::text(format!(
                        "Merged PDFs into {}",
                        input.output_path
                    ))])),
                    Err(e) => Ok(CallToolResult::error(vec![ContentBlock::text(
                        e.to_string(),
                    )])),
                }
            }

            "extract_text" => {
                let input: OcrInput = serde_json::from_value(input)
                    .map_err(|e| McpError::invalid_params(e.to_string(), None))?;
                let file_input =
                    rtools_core::FileInput::from_path(PathBuf::from(&input.input_path));
                let config = rtools_ai::ocr::OcrConfig {
                    language: input.language.unwrap_or_else(|| "eng".to_string()),
                    dpi: 300,
                    output_format: rtools_ai::ocr::OcrOutputFormat::Text,
                };
                let processor = rtools_ai::OcrProcessor;
                match processor.process(file_input, config) {
                    Ok(result) => Ok(CallToolResult::success(vec![ContentBlock::text(
                        result.text,
                    )])),
                    Err(e) => Ok(CallToolResult::error(vec![ContentBlock::text(
                        e.to_string(),
                    )])),
                }
            }

            "get_metadata" => {
                let input: MetadataInput = serde_json::from_value(input)
                    .map_err(|e| McpError::invalid_params(e.to_string(), None))?;
                let file_input =
                    rtools_core::FileInput::from_path(PathBuf::from(&input.input_path));
                let config = rtools_image::MetadataConfig::default();
                let processor = rtools_image::MetadataProcessor;
                match processor.process(file_input, config) {
                    Ok(metadata) => {
                        let result = serde_json::to_string_pretty(&metadata)
                            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
                        Ok(CallToolResult::success(vec![ContentBlock::text(result)]))
                    }
                    Err(e) => Ok(CallToolResult::error(vec![ContentBlock::text(
                        e.to_string(),
                    )])),
                }
            }

            _ => Err(McpError::invalid_params(
                format!("Unknown tool: {tool_name}"),
                None,
            )),
        }
    }
}

fn collect_images(dir: &str) -> Vec<rtools_core::FileInput> {
    let mut inputs = Vec::new();
    let valid_extensions = [
        "jpg", "jpeg", "png", "webp", "heic", "heif", "tiff", "bmp", "gif",
    ];

    for entry in walkdir::WalkDir::new(dir)
        .into_iter()
        .filter_map(std::result::Result::ok)
    {
        if entry.file_type().is_file() {
            if let Some(ext) = entry.path().extension().and_then(|e| e.to_str()) {
                if valid_extensions.contains(&ext.to_lowercase().as_str()) {
                    inputs.push(rtools_core::FileInput::from_path(
                        entry.path().to_path_buf(),
                    ));
                }
            }
        }
    }

    inputs
}

impl ServerHandler for RToolsServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
    }

    async fn list_tools(
        &self,
        _request: Option<rmcp::model::PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        Ok(ListToolsResult {
            tools: Self::tools(),
            next_cursor: None,
            ..Default::default()
        })
    }

    async fn call_tool(
        &self,
        request: rmcp::model::CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<rmcp::model::CallToolResponse, McpError> {
        let name = request.name.to_string();
        let arguments = serde_json::Value::Object(request.arguments.unwrap_or_default());
        self.handle_tool(&name, arguments).await.map(Into::into)
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let server = RToolsServer;
    let service = server.serve(stdio()).await?;
    service.waiting().await?;

    Ok(())
}
