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

fn file_output_content(output: &rtools_core::FileOutput) -> Result<ContentBlock, McpError> {
    serde_json::to_string(output)
        .map(ContentBlock::text)
        .map_err(|error| McpError::internal_error(error.to_string(), None))
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
                    output_policy: rtools_core::OutputPolicy::default(),
                    preserve_metadata: false,
                    strip_gps: false,
                    limits: rtools_core::ResourceLimits::default(),
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
                        Ok(CallToolResult::success(vec![
                            ContentBlock::text(result),
                            file_output_content(&output)?,
                        ]))
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
                    output_policy: rtools_core::OutputPolicy::default(),
                    output_dir: None,
                    quality: input.quality.unwrap_or(85),
                    preserve_metadata: false,
                    strip_gps: false,
                    limits: rtools_core::ResourceLimits::default(),
                };
                let processor = rtools_image::ConvertProcessor;
                match processor.process(file_input, config) {
                    Ok(output) => Ok(CallToolResult::success(vec![
                        ContentBlock::text(format!("Converted to {}", input.target_format)),
                        file_output_content(&output)?,
                    ])),
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
                    output_policy: rtools_core::OutputPolicy::default(),
                    quality: 85,
                    limits: rtools_core::ResourceLimits::default(),
                };
                let processor = rtools_image::ResizeProcessor;
                match processor.process(file_input, config) {
                    Ok(output) => Ok(CallToolResult::success(vec![
                        ContentBlock::text("Resized successfully"),
                        file_output_content(&output)?,
                    ])),
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
                        "date" => rtools_ai::organize::OrganizeStrategy::ByDate,
                        "subject" => rtools_ai::organize::OrganizeStrategy::BySubject,
                        "location" => rtools_ai::organize::OrganizeStrategy::ByLocation,
                        "camera" => rtools_ai::organize::OrganizeStrategy::ByCamera,
                        "custom" => rtools_ai::organize::OrganizeStrategy::Custom,
                        other => {
                            return Err(McpError::invalid_params(
                                format!("Unsupported organization strategy: {other}"),
                                None,
                            ));
                        }
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
                        .unwrap_or_else(|| "{date}_{name}_{index}".to_string()),
                    output_dir: None,
                    start_number: 1,
                    use_ai_descriptions: false,
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

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine as _;
    use image::ImageFormat;
    use std::io::Cursor;

    fn write_png(path: &std::path::Path) {
        let mut bytes = Cursor::new(Vec::new());
        image::DynamicImage::new_rgba8(2, 2)
            .write_to(&mut bytes, ImageFormat::Png)
            .expect("test PNG must encode");
        std::fs::write(path, bytes.into_inner()).expect("test PNG must write");
    }

    fn is_error(result: &CallToolResult) -> bool {
        result.is_error == Some(true)
    }

    fn serialized_file_output(result: &CallToolResult) -> serde_json::Value {
        let result = serde_json::to_value(result).expect("tool result must serialize");
        result["content"]
            .as_array()
            .expect("tool content must be an array")
            .iter()
            .filter_map(|block| block["text"].as_str())
            .find_map(|text| {
                serde_json::from_str::<serde_json::Value>(text)
                    .ok()
                    .filter(|document| document.get("destination").is_some())
            })
            .expect("tool result must contain a serialized FileOutput")
    }

    #[tokio::test]
    async fn image_tools_serialize_orientation_warnings_and_oriented_outputs() {
        let temp = tempfile::tempdir().expect("temp dir");
        let fixture = base64::engine::general_purpose::STANDARD
            .decode(include_str!("../../rtools-tests/fixtures/images/orientation-6.jpg.b64").trim())
            .unwrap();

        for tool in ["compress_image", "convert_image", "resize_image"] {
            let directory = temp.path().join(tool);
            std::fs::create_dir(&directory).unwrap();
            let input = directory.join("orientation.jpg");
            std::fs::write(&input, &fixture).unwrap();
            let explicit_output = directory.join("output.png");
            let arguments = match tool {
                "compress_image" => serde_json::json!({
                    "input_path": input,
                    "output_path": directory.join("output.jpg"),
                    "quality": 85
                }),
                "convert_image" => serde_json::json!({
                    "input_path": input,
                    "output_path": explicit_output,
                    "target_format": "png",
                    "quality": 85
                }),
                "resize_image" => serde_json::json!({
                    "input_path": input,
                    "width": 36,
                    "height": null,
                    "maintain_aspect": true
                }),
                _ => unreachable!(),
            };
            let result = RToolsServer
                .handle_tool(tool, arguments)
                .await
                .expect("tool dispatch must complete");
            assert!(!is_error(&result), "{result:?}");
            let output = serialized_file_output(&result);
            assert_eq!(
                output["warnings"],
                serde_json::json!(["EXIF orientation 6 applied"]),
                "{tool}: {output}"
            );
            let path = output["destination"]["File"]
                .as_str()
                .map(PathBuf::from)
                .expect("serialized destination must be a file");
            let image = image::open(path).unwrap();
            assert_eq!((image.width(), image.height()), (36, 24), "{tool}");
        }
    }

    #[tokio::test]
    async fn image_tools_use_safe_metadata_defaults() {
        let temp = tempfile::tempdir().expect("temp dir");
        let input = temp.path().join("input.png");
        write_png(&input);
        for (tool, output, extra) in [
            (
                "compress_image",
                temp.path().join("compressed.png"),
                serde_json::json!({}),
            ),
            (
                "convert_image",
                temp.path().join("converted.webp"),
                serde_json::json!({"target_format": "webp"}),
            ),
        ] {
            let mut arguments = serde_json::json!({
                "input_path": input,
                "output_path": output,
            });
            arguments
                .as_object_mut()
                .expect("arguments are an object")
                .extend(extra.as_object().expect("extra is an object").clone());
            let result = RToolsServer
                .handle_tool(tool, arguments)
                .await
                .expect("tool dispatch must complete");
            assert!(!is_error(&result), "{result:?}");
            assert!(output.exists());
        }
    }

    #[tokio::test]
    async fn unsupported_ai_modes_and_empty_duplicates_propagate_errors() {
        let temp = tempfile::tempdir().expect("temp dir");
        let input = temp.path().join("input.png");
        write_png(&input);

        let organize_output = temp.path().join("organized");
        let organize = RToolsServer
            .handle_tool(
                "organize_photos",
                serde_json::json!({
                    "input_dir": temp.path(),
                    "output_dir": organize_output,
                    "strategy": "subject",
                }),
            )
            .await
            .expect("tool dispatch must complete");
        assert!(is_error(&organize), "{organize:?}");
        assert!(!organize_output.exists());

        let rename = RToolsServer
            .handle_tool(
                "rename_photos",
                serde_json::json!({
                    "input_dir": temp.path(),
                    "pattern": "{subject}_{index}",
                    "dry_run": true,
                }),
            )
            .await
            .expect("tool dispatch must complete");
        assert!(is_error(&rename), "{rename:?}");
        assert!(input.exists());

        let empty = tempfile::tempdir().expect("empty temp dir");
        let duplicates = RToolsServer
            .handle_tool(
                "find_duplicates",
                serde_json::json!({"input_dir": empty.path()}),
            )
            .await
            .expect("tool dispatch must complete");
        assert!(is_error(&duplicates), "{duplicates:?}");
    }
}
