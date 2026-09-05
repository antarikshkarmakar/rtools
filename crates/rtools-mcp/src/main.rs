use rmcp::{
    model::{CallToolResult, ContentBlock, ListToolsResult, ServerCapabilities, ServerInfo, Tool},
    schemars::JsonSchema,
    serde::{Deserialize, Serialize},
    service::RequestContext,
    transport::io::stdio,
    ErrorData as McpError, RoleServer, ServerHandler, ServiceExt,
};
use rtools_core::{ErrorCode, Processor, RToolsError, RToolsResult};
use std::fmt::Write as _;
use std::path::PathBuf;

#[derive(Clone)]
struct RToolsServer;

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct CompressInput {
    input_path: String,
    output_path: Option<String>,
    /// Optional JPEG quality. Omit unless the effective output format is JPEG.
    #[schemars(range(min = 1, max = 100))]
    quality: Option<u8>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct ConvertInput {
    input_path: String,
    #[schemars(schema_with = "target_format_schema")]
    target_format: String,
    output_path: Option<String>,
    /// Optional JPEG quality. Valid only when `target_format` is jpg or jpeg;
    /// omit it for every other target format.
    #[schemars(range(min = 1, max = 100))]
    quality: Option<u8>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
enum TargetFormatSchema {
    Webp,
    Png,
    Jpg,
    Jpeg,
    Avif,
    Tiff,
    Tif,
    Bmp,
    Gif,
    Hdr,
}

fn target_format_schema(generator: &mut rmcp::schemars::SchemaGenerator) -> rmcp::schemars::Schema {
    TargetFormatSchema::json_schema(generator)
}

fn parse_target_format(value: &str) -> Option<rtools_core::ImageFormat> {
    match value {
        "webp" => Some(rtools_core::ImageFormat::Webp),
        "png" => Some(rtools_core::ImageFormat::Png),
        "jpg" | "jpeg" => Some(rtools_core::ImageFormat::Jpeg),
        "avif" => Some(rtools_core::ImageFormat::Avif),
        "tiff" | "tif" => Some(rtools_core::ImageFormat::Tiff),
        "bmp" => Some(rtools_core::ImageFormat::Bmp),
        "gif" => Some(rtools_core::ImageFormat::Gif),
        "hdr" => Some(rtools_core::ImageFormat::Hdr),
        _ => None,
    }
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct ResizeInput {
    input_path: String,
    #[schemars(range(min = 1, max = 32768))]
    width: Option<u32>,
    #[schemars(range(min = 1, max = 32768))]
    height: Option<u32>,
    maintain_aspect: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct OrganizeInput {
    input_dir: String,
    output_dir: String,
    strategy: Option<OrganizeStrategyInput>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
enum OrganizeStrategyInput {
    Date,
    Subject,
    Location,
    Camera,
    Custom,
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
    #[schemars(range(min = 0.0, max = 1.0))]
    threshold: Option<f64>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct PdfCompressInput {
    input_path: String,
    output_path: Option<String>,
    level: Option<PdfCompressionLevelInput>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
enum PdfCompressionLevelInput {
    Light,
    Medium,
    Heavy,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct PdfMergeInput {
    #[schemars(length(min = 2))]
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
    serde_json::to_string(&serde_json::json!({
        "name": output.name,
        "mime_type": output.mime_type,
        "stats": output.stats,
        "warnings": output.warnings,
    }))
    .map(ContentBlock::text)
    .map_err(|error| McpError::internal_error(error.to_string(), None))
}

const fn sanitized_error_message(code: ErrorCode) -> &'static str {
    match code {
        ErrorCode::InvalidInput => "The tool input is invalid.",
        ErrorCode::CapabilityUnavailable => "The requested capability is unavailable.",
        ErrorCode::UnsupportedFormat => "The requested format is unsupported.",
        ErrorCode::ResourceLimitExceeded => "A configured resource limit was exceeded.",
        ErrorCode::OutputExists => "The output already exists.",
        ErrorCode::PathPolicyViolation => "The requested path is not permitted.",
        ErrorCode::ProcessingFailed => "The tool could not process the input.",
        ErrorCode::PartialFailure => "The tool completed only part of the request.",
        ErrorCode::AuthenticationRequired => "Authentication is required.",
        ErrorCode::ConfigurationInvalid => "The server configuration is invalid.",
        ErrorCode::Cancelled => "The tool operation was cancelled.",
        ErrorCode::RollbackFailed => "The tool could not safely roll back its changes.",
    }
}

fn tool_error(default_operation_id: &'static str, error: &RToolsError) -> CallToolResult {
    let operation_id = match error {
        RToolsError::CapabilityUnavailable { operation_id, .. } => operation_id.as_str(),
        _ => default_operation_id,
    };
    tracing::warn!(
        operation_id,
        code = error.code().as_str(),
        error = %error,
        "MCP tool processing failed"
    );
    let message = sanitized_error_message(error.code());
    let mut result = CallToolResult::error(vec![ContentBlock::text(message)]);
    result.structured_content = Some(serde_json::json!({
        "code": error.code().as_str(),
        "message": message,
        "operation_id": operation_id,
    }));
    result
}

#[derive(Debug, Serialize)]
struct McpToolContract {
    tool: &'static str,
    operation_id: &'static str,
    state: &'static str,
    description: &'static str,
    adapter_contract: &'static str,
    structured_errors: bool,
}

const MCP_TOOL_CONTRACTS: &[McpToolContract] = &[
    McpToolContract {
        tool: "compress_image",
        operation_id: "image.compress",
        state: "available",
        description: "Compress an image with quality preservation",
        adapter_contract: "output extension must match input; quality=1..100 for JPEG output only; omit for non-JPEG",
        structured_errors: true,
    },
    McpToolContract {
        tool: "convert_image",
        operation_id: "image.convert",
        state: "available",
        description: "Convert an image to an explicitly selected format",
        adapter_contract: "target_format=webp|png|jpg|jpeg|avif|tiff|tif|bmp|gif|hdr; quality=1..100 for jpg|jpeg only; omit for other targets",
        structured_errors: true,
    },
    McpToolContract {
        tool: "resize_image",
        operation_id: "image.resize",
        state: "available",
        description: "Resize an image by dimensions",
        adapter_contract: "width|height=1..32768; fixed output quality 85",
        structured_errors: true,
    },
    McpToolContract {
        tool: "organize_photos",
        operation_id: "ai.organize.date",
        state: "experimental",
        description: "Organize photos by deterministic date into prepared year/month folders",
        adapter_contract: "strategy=date; prepared derived output directories required",
        structured_errors: true,
    },
    McpToolContract {
        tool: "rename_photos",
        operation_id: "ai.rename.deterministic",
        state: "experimental",
        description: "Rename photos with deterministic filename tokens",
        adapter_contract: "deterministic tokens only",
        structured_errors: true,
    },
    McpToolContract {
        tool: "generate_alt_text",
        operation_id: "ai.alt_text",
        state: "unavailable",
        description: "Unavailable: no verified image captioning provider is configured",
        adapter_contract: "no provider",
        structured_errors: true,
    },
    McpToolContract {
        tool: "find_duplicates",
        operation_id: "ai.duplicates.report",
        state: "experimental",
        description: "Find duplicate images by visual similarity",
        adapter_contract: "report only; threshold=0..1 finite",
        structured_errors: true,
    },
    McpToolContract {
        tool: "compress_pdf",
        operation_id: "pdf.compress",
        state: "experimental",
        description: "Experimentally compress a PDF using the medium level only",
        adapter_contract: "level=medium; light|heavy unavailable",
        structured_errors: true,
    },
    McpToolContract {
        tool: "merge_pdfs",
        operation_id: "pdf.merge",
        state: "experimental",
        description: "Merge two or more PDF files into one",
        adapter_contract: "input_paths minItems=2",
        structured_errors: true,
    },
    McpToolContract {
        tool: "extract_text",
        operation_id: "ai.ocr",
        state: "unavailable",
        description: "Unavailable: no verified OCR provider is configured",
        adapter_contract: "no OCR provider",
        structured_errors: true,
    },
    McpToolContract {
        tool: "get_metadata",
        operation_id: "image.exif.json",
        state: "available",
        description: "Get image metadata including EXIF data",
        adapter_contract: "read-only EXIF and file information",
        structured_errors: true,
    },
];

fn input_schema<T: JsonSchema>() -> serde_json::Map<String, serde_json::Value> {
    serde_json::to_value(rmcp::schemars::schema_for!(T))
        .unwrap_or_default()
        .as_object()
        .cloned()
        .unwrap_or_default()
}

fn runtime_contract_json() -> serde_json::Result<String> {
    serde_json::to_string_pretty(&serde_json::json!({
        "version": 1,
        "tools": MCP_TOOL_CONTRACTS,
    }))
}

impl RToolsServer {
    fn tools() -> Vec<Tool> {
        MCP_TOOL_CONTRACTS
            .iter()
            .map(|contract| {
                let schema = match contract.tool {
                    "compress_image" => input_schema::<CompressInput>(),
                    "convert_image" => input_schema::<ConvertInput>(),
                    "resize_image" => input_schema::<ResizeInput>(),
                    "organize_photos" => input_schema::<OrganizeInput>(),
                    "rename_photos" => input_schema::<RenameInput>(),
                    "generate_alt_text" => input_schema::<AltTextInput>(),
                    "find_duplicates" => input_schema::<DuplicatesInput>(),
                    "compress_pdf" => input_schema::<PdfCompressInput>(),
                    "merge_pdfs" => input_schema::<PdfMergeInput>(),
                    "extract_text" => input_schema::<OcrInput>(),
                    "get_metadata" => input_schema::<MetadataInput>(),
                    _ => unreachable!("runtime MCP contract has no schema"),
                };
                Tool::new(contract.tool, contract.description, schema)
            })
            .collect()
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
                let input_path = PathBuf::from(&input.input_path);
                let Some(input_format) = rtools_core::ImageFormat::from_path(&input_path) else {
                    return Ok(tool_error(
                        "image.compress",
                        &RToolsError::invalid_input("Unsupported image input format"),
                    ));
                };
                let target_format = if let Some(output_path) = input.output_path.as_deref() {
                    let Some(output_format) =
                        rtools_core::ImageFormat::from_path(std::path::Path::new(output_path))
                    else {
                        return Ok(tool_error(
                            "image.compress",
                            &RToolsError::invalid_input(
                                "Output path must have a supported image extension",
                            ),
                        ));
                    };
                    if output_format != input_format {
                        return Ok(tool_error(
                            "image.compress",
                            &RToolsError::invalid_input(
                                "Compression output format must match the input format",
                            ),
                        ));
                    }
                    output_format
                } else {
                    input_format
                };
                if input.quality.is_some() && target_format != rtools_core::ImageFormat::Jpeg {
                    return Ok(tool_error(
                        "image.compress",
                        &RToolsError::invalid_input(
                            "Quality is effective only for JPEG compression output",
                        ),
                    ));
                }
                let file_input = rtools_core::FileInput::from_path(input_path);
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
                    Err(error) => Ok(tool_error("image.compress", &error)),
                }
            }

            "convert_image" => {
                let input: ConvertInput = serde_json::from_value(input)
                    .map_err(|e| McpError::invalid_params(e.to_string(), None))?;
                let Some(target_format) = parse_target_format(&input.target_format) else {
                    return Ok(tool_error(
                        "image.convert",
                        &RToolsError::invalid_input("Unsupported target format"),
                    ));
                };
                if input.quality.is_some() && target_format != rtools_core::ImageFormat::Jpeg {
                    return Ok(tool_error(
                        "image.convert",
                        &RToolsError::invalid_input(
                            "Quality is effective only for JPEG conversion output",
                        ),
                    ));
                }
                let file_input =
                    rtools_core::FileInput::from_path(PathBuf::from(&input.input_path));
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
                    Err(error) => Ok(tool_error("image.convert", &error)),
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
                    Err(error) => Ok(tool_error("image.resize", &error)),
                }
            }

            "organize_photos" => {
                let input: OrganizeInput = serde_json::from_value(input)
                    .map_err(|e| McpError::invalid_params(e.to_string(), None))?;
                let config = rtools_ai::organize::OrganizeConfig {
                    output_dir: PathBuf::from(&input.output_dir),
                    strategy: match input.strategy.unwrap_or(OrganizeStrategyInput::Date) {
                        OrganizeStrategyInput::Date => {
                            rtools_ai::organize::OrganizeStrategy::ByDate
                        }
                        OrganizeStrategyInput::Subject => {
                            rtools_ai::organize::OrganizeStrategy::BySubject
                        }
                        OrganizeStrategyInput::Location => {
                            rtools_ai::organize::OrganizeStrategy::ByLocation
                        }
                        OrganizeStrategyInput::Camera => {
                            rtools_ai::organize::OrganizeStrategy::ByCamera
                        }
                        OrganizeStrategyInput::Custom => {
                            rtools_ai::organize::OrganizeStrategy::Custom
                        }
                    },
                    by_date: true,
                    by_subject: false,
                    dry_run: false,
                };
                let processor = rtools_ai::OrganizeProcessor;
                if let Err(error) = processor.validate_config(&config) {
                    return Ok(tool_error("ai.organize.date", &error));
                }
                let inputs = match collect_images(&input.input_dir) {
                    Ok(inputs) => inputs,
                    Err(error) => return Ok(tool_error("ai.organize.date", &error)),
                };
                match processor.process(inputs, config) {
                    Ok(outputs) => Ok(CallToolResult::success(vec![ContentBlock::text(format!(
                        "Organized {} photos",
                        outputs.len()
                    ))])),
                    Err(error) => Ok(tool_error("ai.organize.date", &error)),
                }
            }

            "rename_photos" => {
                let input: RenameInput = serde_json::from_value(input)
                    .map_err(|e| McpError::invalid_params(e.to_string(), None))?;
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
                if let Err(error) = processor.validate_config(&config) {
                    return Ok(tool_error("ai.rename.deterministic", &error));
                }
                let inputs = match collect_images(&input.input_dir) {
                    Ok(inputs) => inputs,
                    Err(error) => return Ok(tool_error("ai.rename.deterministic", &error)),
                };
                match processor.process(inputs, config) {
                    Ok(outputs) => Ok(CallToolResult::success(vec![ContentBlock::text(format!(
                        "Renamed {} photos",
                        outputs.len()
                    ))])),
                    Err(error) => Ok(tool_error("ai.rename.deterministic", &error)),
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
                    Err(error) => Ok(tool_error("ai.alt_text", &error)),
                }
            }

            "find_duplicates" => {
                let input: DuplicatesInput = serde_json::from_value(input)
                    .map_err(|e| McpError::invalid_params(e.to_string(), None))?;
                let inputs = match collect_images(&input.input_dir) {
                    Ok(inputs) => inputs,
                    Err(error) => return Ok(tool_error("ai.duplicates.report", &error)),
                };
                let config = rtools_ai::duplicates::DuplicatesConfig {
                    threshold: input.threshold.unwrap_or(0.9),
                    algorithm: rtools_ai::duplicates::HashAlgorithm::Perceptual,
                    action: rtools_ai::duplicates::DuplicateAction::Report,
                    dry_run: false,
                    limits: rtools_core::ResourceLimits::default(),
                };
                let processor = rtools_ai::DuplicatesProcessor;
                match processor.process(inputs, config) {
                    Ok(result) => {
                        let mut text = format!("Found {} duplicate groups\n", result.groups.len());
                        let _ = writeln!(text, "Originals: {}", result.total_originals);
                        let _ = writeln!(text, "Duplicates: {}", result.total_duplicates);
                        Ok(CallToolResult::success(vec![ContentBlock::text(text)]))
                    }
                    Err(error) => Ok(tool_error("ai.duplicates.report", &error)),
                }
            }

            "compress_pdf" => {
                let input: PdfCompressInput = serde_json::from_value(input)
                    .map_err(|e| McpError::invalid_params(e.to_string(), None))?;
                let file_input =
                    rtools_core::FileInput::from_path(PathBuf::from(&input.input_path));
                let config = rtools_pdf::PdfCompressConfig {
                    level: match input.level.unwrap_or(PdfCompressionLevelInput::Medium) {
                        PdfCompressionLevelInput::Light => {
                            rtools_pdf::compress::PdfCompressionLevel::Light
                        }
                        PdfCompressionLevelInput::Medium => {
                            rtools_pdf::compress::PdfCompressionLevel::Medium
                        }
                        PdfCompressionLevelInput::Heavy => {
                            rtools_pdf::compress::PdfCompressionLevel::Heavy
                        }
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
                    Err(error) => Ok(tool_error("pdf.compress", &error)),
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
                    inputs: Vec::new(),
                    output: PathBuf::from(&input.output_path),
                    add_page_numbers: false,
                };
                let processor = rtools_pdf::PdfMergeProcessor;
                match processor.process(file_inputs, config) {
                    Ok(_) => Ok(CallToolResult::success(vec![ContentBlock::text(
                        "Merged PDFs successfully",
                    )])),
                    Err(error) => Ok(tool_error("pdf.merge", &error)),
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
                    Err(error) => Ok(tool_error("ai.ocr", &error)),
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
                    Err(error) => Ok(tool_error("image.exif.json", &error)),
                }
            }

            _ => Err(McpError::invalid_params(
                format!("Unknown tool: {tool_name}"),
                None,
            )),
        }
    }
}

fn collect_images(dir: &str) -> RToolsResult<Vec<rtools_core::FileInput>> {
    let root = std::path::Path::new(dir);
    if !root.exists() {
        return Err(RToolsError::file_not_found(dir));
    }
    if !root.is_dir() {
        return Err(RToolsError::invalid_input(format!(
            "Image input is not a directory: {dir}"
        )));
    }
    let mut inputs = Vec::new();
    let valid_extensions = [
        "jpg", "jpeg", "png", "webp", "heic", "heif", "tiff", "bmp", "gif",
    ];

    for entry in walkdir::WalkDir::new(dir) {
        let entry = entry.map_err(|error| {
            RToolsError::invalid_input(format!("Failed to traverse image input {dir}: {error}"))
        })?;
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

    inputs.sort_by(|left, right| left.source.as_path().cmp(&right.source.as_path()));

    Ok(inputs)
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
    if let Some(argument) = std::env::args().nth(1) {
        if argument == "--print-contracts" {
            println!("{}", runtime_contract_json()?);
            return Ok(());
        }
        anyhow::bail!("unsupported argument: {argument}");
    }

    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
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
    use lopdf::{dictionary, Document, Object, Stream};
    use std::io::Cursor;

    fn write_png(path: &std::path::Path) {
        let mut bytes = Cursor::new(Vec::new());
        image::DynamicImage::new_rgba8(2, 2)
            .write_to(&mut bytes, ImageFormat::Png)
            .expect("test PNG must encode");
        std::fs::write(path, bytes.into_inner()).expect("test PNG must write");
    }

    fn write_quality_fixture(path: &std::path::Path) {
        let image = image::RgbImage::from_fn(96, 96, |x, y| {
            image::Rgb([
                u8::try_from((x * 17 + y * 29) % 256).unwrap(),
                u8::try_from((x * 47 + y * 11) % 256).unwrap(),
                u8::try_from((x * 7 + y * 53) % 256).unwrap(),
            ])
        });
        image.save(path).expect("quality fixture must encode");
    }

    fn write_pdf(path: &std::path::Path) {
        let mut document = Document::with_version("1.5");
        let pages_id = document.new_object_id();
        let catalog_id = document.new_object_id();
        let content_id = document.add_object(Stream::new(dictionary! {}, Vec::new()));
        let leaf_page_id = document.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => vec![0.into(), 0.into(), 100.into(), 100.into()],
            "Contents" => content_id,
        });
        document.objects.insert(
            pages_id,
            Object::Dictionary(dictionary! {
                "Type" => "Pages",
                "Kids" => vec![Object::Reference(leaf_page_id)],
                "Count" => 1,
            }),
        );
        document.objects.insert(
            catalog_id,
            Object::Dictionary(dictionary! {
                "Type" => "Catalog",
                "Pages" => pages_id,
            }),
        );
        document.trailer.set("Root", catalog_id);
        document.save(path).unwrap();
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

    fn write_declared_png(path: &std::path::Path, width: u32, height: u32) {
        let mut header = Vec::with_capacity(58);
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
        std::fs::write(path, header).expect("declared PNG must write");
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
                    .filter(|document| document.get("mime_type").is_some())
            })
            .expect("tool result must contain path-free file metadata")
    }

    fn tool_by_name(name: &str) -> serde_json::Value {
        let tools = serde_json::to_value(RToolsServer::tools()).expect("tools must serialize");
        tools
            .as_array()
            .unwrap()
            .iter()
            .find(|tool| tool["name"] == name)
            .cloned()
            .unwrap_or_else(|| panic!("missing live MCP tool {name}"))
    }

    #[test]
    fn mcp_contract_runtime_catalog_is_unique_and_drives_live_tools() {
        let exported: serde_json::Value =
            serde_json::from_str(&runtime_contract_json().unwrap()).unwrap();
        let rows = exported["tools"].as_array().unwrap();
        let exported_names: Vec<_> = rows
            .iter()
            .map(|row| row["tool"].as_str().unwrap())
            .collect();
        let mut unique_names = exported_names.clone();
        unique_names.sort_unstable();
        unique_names.dedup();
        assert_eq!(unique_names.len(), exported_names.len());

        let listed = serde_json::to_value(RToolsServer::tools()).unwrap();
        let listed_names: Vec<_> = listed
            .as_array()
            .unwrap()
            .iter()
            .map(|tool| tool["name"].as_str().unwrap())
            .collect();
        assert_eq!(listed_names, exported_names);
        assert!(rows.iter().all(|row| {
            row["structured_errors"] == true
                && row["operation_id"].is_string()
                && row["state"].is_string()
        }));
    }

    #[tokio::test]
    async fn mcp_contract_every_catalog_tool_has_a_dispatch_handler() {
        for contract in MCP_TOOL_CONTRACTS {
            let error = RToolsServer
                .handle_tool(contract.tool, serde_json::json!({}))
                .await
                .expect_err("empty arguments must be invalid MCP parameters");
            assert!(
                !error.message.contains("Unknown tool"),
                "{} is listed without a dispatch handler",
                contract.tool
            );
        }
    }

    #[test]
    fn mcp_contract_live_schemas_match_processor_validation() {
        for tool in ["compress_image", "convert_image"] {
            let schema = tool_by_name(tool);
            assert_eq!(schema["inputSchema"]["properties"]["quality"]["minimum"], 1);
            assert_eq!(
                schema["inputSchema"]["properties"]["quality"]["maximum"],
                100
            );
            assert!(
                schema["inputSchema"]["properties"]["quality"]["description"]
                    .as_str()
                    .is_some_and(|description| description.contains("JPEG"))
            );
        }

        let resize = tool_by_name("resize_image");
        for dimension in ["width", "height"] {
            assert_eq!(resize["inputSchema"]["properties"][dimension]["minimum"], 1);
            assert_eq!(
                resize["inputSchema"]["properties"][dimension]["maximum"],
                32_768
            );
        }

        let duplicates = tool_by_name("find_duplicates");
        assert_eq!(
            duplicates["inputSchema"]["properties"]["threshold"]["minimum"],
            0.0
        );
        assert_eq!(
            duplicates["inputSchema"]["properties"]["threshold"]["maximum"],
            1.0
        );

        let merge = tool_by_name("merge_pdfs");
        assert_eq!(
            merge["inputSchema"]["properties"]["input_paths"]["minItems"],
            2
        );

        let convert = tool_by_name("convert_image");
        assert_eq!(
            convert["inputSchema"]["properties"]["target_format"]["enum"],
            serde_json::json!([
                "webp", "png", "jpg", "jpeg", "avif", "tiff", "tif", "bmp", "gif", "hdr"
            ])
        );
    }

    #[tokio::test]
    async fn mcp_contract_all_processor_errors_are_sanitized_and_identified() {
        let temp = tempfile::tempdir().unwrap();
        let canary = temp.path().join("PRIVATE-CANARY-mcp-error");
        let missing = canary.join("missing");
        let cases = [
            (
                "compress_image",
                serde_json::json!({"input_path": missing.with_extension("png")}),
                "image.compress",
            ),
            (
                "convert_image",
                serde_json::json!({"input_path": missing.with_extension("png"), "target_format": "png"}),
                "image.convert",
            ),
            (
                "resize_image",
                serde_json::json!({"input_path": missing.with_extension("png"), "width": 2}),
                "image.resize",
            ),
            (
                "organize_photos",
                serde_json::json!({"input_dir": missing, "output_dir": canary.join("organized"), "strategy": "date"}),
                "ai.organize.date",
            ),
            (
                "rename_photos",
                serde_json::json!({"input_dir": missing, "dry_run": true}),
                "ai.rename.deterministic",
            ),
            (
                "generate_alt_text",
                serde_json::json!({"input_path": missing.with_extension("png")}),
                "ai.alt_text",
            ),
            (
                "find_duplicates",
                serde_json::json!({"input_dir": missing}),
                "ai.duplicates.report",
            ),
            (
                "compress_pdf",
                serde_json::json!({"input_path": missing.with_extension("pdf"), "level": "medium"}),
                "pdf.compress",
            ),
            (
                "merge_pdfs",
                serde_json::json!({
                    "input_paths": [missing.with_extension("pdf"), canary.join("missing-two.pdf")],
                    "output_path": canary.join("merged.pdf")
                }),
                "pdf.merge",
            ),
            (
                "extract_text",
                serde_json::json!({"input_path": missing.with_extension("png")}),
                "ai.ocr",
            ),
            (
                "get_metadata",
                serde_json::json!({"input_path": missing.with_extension("png")}),
                "image.exif.json",
            ),
        ];

        for (tool, arguments, operation_id) in cases {
            let result = RToolsServer
                .handle_tool(tool, arguments)
                .await
                .unwrap_or_else(|error| panic!("{tool} returned protocol error: {error}"));
            let serialized = serde_json::to_value(&result).unwrap();
            let document = serde_json::to_string(&serialized).unwrap();
            assert!(is_error(&result), "{tool}: {document}");
            assert_eq!(
                serialized["structuredContent"]["operation_id"], operation_id,
                "{tool}: {document}"
            );
            assert!(
                !document.contains("PRIVATE-CANARY-mcp-error"),
                "{tool} leaked a host path: {document}"
            );
        }
    }

    #[tokio::test]
    async fn mcp_contract_file_success_metadata_never_exposes_host_paths() {
        let temp = tempfile::tempdir().unwrap();
        let canary_dir = temp.path().join("PRIVATE-CANARY-mcp-success");
        std::fs::create_dir(&canary_dir).unwrap();
        let input = canary_dir.join("input.png");
        let output = canary_dir.join("output.png");
        write_png(&input);

        let result = RToolsServer
            .handle_tool(
                "compress_image",
                serde_json::json!({
                    "input_path": input,
                    "output_path": output
                }),
            )
            .await
            .expect("tool dispatch must complete");
        let document = serde_json::to_string(&result).unwrap();

        assert!(!is_error(&result), "{document}");
        assert!(
            !document.contains("PRIVATE-CANARY-mcp-success"),
            "{document}"
        );
        assert!(!document.contains("destination"), "{document}");
        assert!(output.exists());
    }

    #[tokio::test]
    async fn mcp_contract_explicit_quality_requires_a_jpeg_target_before_processing() {
        let temp = tempfile::tempdir().unwrap();
        let canary = temp.path().join("PRIVATE-CANARY-ineffective-quality");
        let mut cases = Vec::new();
        for extension in ["png", "webp", "avif", "tiff", "tif", "bmp", "gif", "hdr"] {
            cases.push((
                "compress_image",
                serde_json::json!({
                    "input_path": canary.join(format!("missing.{extension}")),
                    "output_path": canary.join(format!("compressed.{extension}")),
                    "quality": 50,
                }),
                "image.compress",
                canary.join(format!("compressed.{extension}")),
            ));
        }
        cases.push((
            "compress_image",
            serde_json::json!({
                "input_path": canary.join("unknown.bin"),
                "output_path": canary.join("compressed.bin"),
                "quality": 50,
            }),
            "image.compress",
            canary.join("compressed.bin"),
        ));
        for target in ["png", "webp", "avif", "tiff", "tif", "bmp", "gif", "hdr"] {
            cases.push((
                "convert_image",
                serde_json::json!({
                    "input_path": canary.join("missing.png"),
                    "target_format": target,
                    "output_path": canary.join(format!("converted.{target}")),
                    "quality": 50,
                }),
                "image.convert",
                canary.join(format!("converted.{target}")),
            ));
        }

        for (tool, arguments, operation_id, output) in cases {
            let result = RToolsServer
                .handle_tool(tool, arguments)
                .await
                .expect("quality validation must return a tool result");
            let serialized = serde_json::to_value(&result).unwrap();
            let document = serde_json::to_string(&serialized).unwrap();
            assert!(is_error(&result), "{tool}: {document}");
            assert_eq!(
                serialized["structuredContent"]["code"], "INVALID_INPUT",
                "{tool}: {document}"
            );
            assert_eq!(
                serialized["structuredContent"]["operation_id"], operation_id,
                "{tool}: {document}"
            );
            assert!(!document.contains("PRIVATE-CANARY"), "{tool}: {document}");
            assert!(!output.exists(), "{tool}: {}", output.display());
        }
        assert!(!canary.exists());
    }

    #[tokio::test]
    async fn mcp_contract_compress_validates_explicit_output_format_before_input_access() {
        let temp = tempfile::tempdir().unwrap();
        let canary = temp.path().join("PRIVATE-CANARY-compress-output-format");
        for output in [canary.join("output.png"), canary.join("output")] {
            let result = RToolsServer
                .handle_tool(
                    "compress_image",
                    serde_json::json!({
                        "input_path": canary.join("missing.jpg"),
                        "output_path": output,
                        "quality": 50,
                    }),
                )
                .await
                .expect("output format validation must return a tool result");
            let serialized = serde_json::to_value(&result).unwrap();
            let document = serde_json::to_string(&serialized).unwrap();
            assert!(is_error(&result), "{document}");
            assert_eq!(
                serialized["structuredContent"]["code"], "INVALID_INPUT",
                "{document}"
            );
            assert_eq!(
                serialized["structuredContent"]["operation_id"], "image.compress",
                "{document}"
            );
            assert!(!document.contains("PRIVATE-CANARY"), "{document}");
            assert!(!output.exists(), "{}", output.display());
        }
        assert!(!canary.exists());
    }

    #[tokio::test]
    async fn mcp_contract_unknown_conversion_target_is_a_structured_tool_error() {
        let temp = tempfile::tempdir().unwrap();
        let canary = temp.path().join("PRIVATE-CANARY-unknown-convert-target");
        for quality in [None, Some(50)] {
            let mut arguments = serde_json::json!({
                "input_path": canary.join("missing.png"),
                "target_format": "bogus",
                "output_path": canary.join("output.bogus"),
            });
            if let Some(quality) = quality {
                arguments["quality"] = quality.into();
            }
            let result = RToolsServer
                .handle_tool("convert_image", arguments)
                .await
                .expect("unknown target must return a structured tool result");
            let serialized = serde_json::to_value(&result).unwrap();
            let document = serde_json::to_string(&serialized).unwrap();
            assert!(is_error(&result), "{document}");
            assert_eq!(
                serialized["structuredContent"]["code"], "INVALID_INPUT",
                "{document}"
            );
            assert_eq!(
                serialized["structuredContent"]["operation_id"], "image.convert",
                "{document}"
            );
            assert!(!document.contains("PRIVATE-CANARY"), "{document}");
        }
        assert!(!canary.exists());
    }

    #[tokio::test]
    async fn mcp_contract_jpeg_quality_changes_compress_and_convert_outputs() {
        let temp = tempfile::tempdir().unwrap();
        let png_input = temp.path().join("quality.png");
        let jpeg_input = temp.path().join("quality-input.jpg");
        write_quality_fixture(&png_input);
        write_quality_fixture(&jpeg_input);

        for (tool, input, extra) in [
            ("compress_image", &jpeg_input, serde_json::json!({})),
            (
                "convert_image",
                &png_input,
                serde_json::json!({"target_format": "jpeg"}),
            ),
        ] {
            let mut lengths = Vec::new();
            for quality in [1, 100] {
                let output = temp.path().join(format!("{tool}-{quality}.jpg"));
                let mut arguments = serde_json::json!({
                    "input_path": input,
                    "output_path": output,
                    "quality": quality,
                });
                arguments
                    .as_object_mut()
                    .unwrap()
                    .extend(extra.as_object().unwrap().clone());
                let result = RToolsServer
                    .handle_tool(tool, arguments)
                    .await
                    .expect("JPEG quality request must dispatch");
                assert!(!is_error(&result), "{tool}: {result:?}");
                lengths.push(std::fs::metadata(output).unwrap().len());
            }
            assert_ne!(lengths[0], lengths[1], "{tool}: {lengths:?}");
        }
    }

    #[tokio::test]
    async fn mcp_contract_valid_merge_on_workspace_filesystem_has_no_path_or_artifact_leak() {
        let current_directory = std::env::current_dir().unwrap();
        let temp = tempfile::tempdir_in(current_directory).unwrap();
        let canary = temp.path().join("PRIVATE-CANARY-valid-merge");
        std::fs::create_dir(&canary).unwrap();
        let first = canary.join("first.pdf");
        let second = canary.join("second.pdf");
        let output = canary.join("merged.pdf");
        write_pdf(&first);
        write_pdf(&second);

        let result = RToolsServer
            .handle_tool(
                "merge_pdfs",
                serde_json::json!({
                    "input_paths": [first, second],
                    "output_path": output,
                }),
            )
            .await
            .expect("tool dispatch must complete");
        let document = serde_json::to_string(&result).unwrap();

        assert!(!is_error(&result), "{document}");
        assert!(output.exists());
        rtools_pdf::validate_pdf_artifact(&output).unwrap();
        assert!(
            !document.contains("PRIVATE-CANARY-valid-merge"),
            "{document}"
        );
        let leftovers: Vec<_> = std::fs::read_dir(temp.path())
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .filter(|name| name.to_string_lossy().contains("rtools"))
            .collect();
        assert!(leftovers.is_empty(), "leftover artifacts: {leftovers:?}");
    }

    #[tokio::test]
    async fn mcp_contract_unavailable_ai_modes_are_gated_before_directory_access() {
        let canary = "PRIVATE-CANARY-missing-ai-directory";
        for (strategy, operation_id) in [
            ("subject", "ai.organize.subject"),
            ("location", "ai.organize.location"),
            ("camera", "ai.organize.camera"),
            ("custom", "ai.organize.custom"),
        ] {
            let result = RToolsServer
                .handle_tool(
                    "organize_photos",
                    serde_json::json!({
                        "input_dir": canary,
                        "output_dir": "also-missing",
                        "strategy": strategy,
                    }),
                )
                .await
                .expect("known strategy must return a tool result");
            let serialized = serde_json::to_value(&result).unwrap();
            let document = serde_json::to_string(&serialized).unwrap();
            assert_eq!(
                serialized["structuredContent"]["code"], "CAPABILITY_UNAVAILABLE",
                "{strategy}: {document}"
            );
            assert_eq!(
                serialized["structuredContent"]["operation_id"], operation_id,
                "{strategy}: {document}"
            );
            assert!(!document.contains(canary), "{strategy}: {document}");
        }

        let rename = RToolsServer
            .handle_tool(
                "rename_photos",
                serde_json::json!({
                    "input_dir": canary,
                    "pattern": "{subject}_{index}",
                    "dry_run": true,
                }),
            )
            .await
            .expect("known subject token must return a tool result");
        let serialized = serde_json::to_value(&rename).unwrap();
        let document = serde_json::to_string(&serialized).unwrap();
        assert_eq!(
            serialized["structuredContent"]["code"],
            "CAPABILITY_UNAVAILABLE"
        );
        assert_eq!(
            serialized["structuredContent"]["operation_id"],
            "ai.rename.ai"
        );
        assert!(!document.contains(canary), "{document}");
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
            let (arguments, expected_path) = match tool {
                "compress_image" => (
                    serde_json::json!({
                        "input_path": input,
                        "output_path": directory.join("output.jpg"),
                        "quality": 85
                    }),
                    directory.join("output.jpg"),
                ),
                "convert_image" => (
                    serde_json::json!({
                        "input_path": input,
                        "output_path": explicit_output,
                        "target_format": "png"
                    }),
                    explicit_output,
                ),
                "resize_image" => (
                    serde_json::json!({
                        "input_path": input,
                        "width": 36,
                        "height": null,
                        "maintain_aspect": true
                    }),
                    directory.join("orientation_36x24.jpg"),
                ),
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
            assert!(output.get("destination").is_none(), "{tool}: {output}");
            let image = image::open(expected_path).unwrap();
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
    async fn metadata_tool_treats_valid_bmp_and_gif_as_empty_exif() {
        let temp = tempfile::tempdir().expect("temp dir");
        for (name, format) in [
            ("plain.bmp", ImageFormat::Bmp),
            ("plain.gif", ImageFormat::Gif),
        ] {
            let input = temp.path().join(name);
            image::DynamicImage::new_rgba8(2, 2)
                .save_with_format(&input, format)
                .expect("fixture must encode");

            let result = RToolsServer
                .handle_tool("get_metadata", serde_json::json!({"input_path": input}))
                .await
                .expect("tool dispatch must complete");
            assert!(!is_error(&result), "{name}: {result:?}");
            let serialized = serde_json::to_value(&result).expect("tool result must serialize");
            let text = serialized["content"]
                .as_array()
                .unwrap()
                .iter()
                .find_map(|block| block["text"].as_str())
                .expect("metadata tool must return text");
            let metadata: serde_json::Value = serde_json::from_str(text).unwrap();
            assert!(
                metadata["exif"]
                    .as_object()
                    .unwrap()
                    .values()
                    .all(serde_json::Value::is_null),
                "{name}: {metadata}"
            );
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

    #[tokio::test]
    async fn mcp_contract_missing_directory_is_a_structured_tool_error() {
        let result = RToolsServer
            .handle_tool(
                "rename_photos",
                serde_json::json!({
                    "input_dir": "definitely-missing-mcp-directory",
                    "dry_run": true,
                }),
            )
            .await
            .expect("tool dispatch must complete");
        let serialized = serde_json::to_value(&result).unwrap();

        assert!(is_error(&result), "{result:?}");
        assert_eq!(serialized["structuredContent"]["code"], "INVALID_INPUT");
    }

    #[tokio::test]
    async fn mcp_contract_pdf_compression_levels_do_not_fallback() {
        let invalid = RToolsServer
            .handle_tool(
                "compress_pdf",
                serde_json::json!({
                    "input_path": "missing.pdf",
                    "level": "mystery",
                }),
            )
            .await;
        assert!(invalid.is_err(), "unknown values are invalid parameters");

        let unavailable = RToolsServer
            .handle_tool(
                "compress_pdf",
                serde_json::json!({
                    "input_path": "missing.pdf",
                    "level": "light",
                }),
            )
            .await
            .expect("known level must reach capability validation");
        let serialized = serde_json::to_value(&unavailable).unwrap();
        assert!(is_error(&unavailable), "{unavailable:?}");
        assert_eq!(
            serialized["structuredContent"]["code"],
            "CAPABILITY_UNAVAILABLE"
        );
        assert_eq!(
            serialized["structuredContent"]["operation_id"],
            "pdf.compress.level"
        );
    }

    #[test]
    fn mcp_contract_tool_descriptions_do_not_advertise_unavailable_ai_behavior() {
        let tools = RToolsServer::tools();
        let serialized = serde_json::to_string(&tools).unwrap();

        assert!(!serialized.contains("AI-organize"), "{serialized}");
        assert!(!serialized.contains("AI-rename"), "{serialized}");
        assert!(serialized.contains("deterministic date"), "{serialized}");
        assert!(
            serialized.contains("deterministic filename"),
            "{serialized}"
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn mcp_contract_organize_rejects_linked_output_without_outside_artifacts() {
        let temp = tempfile::tempdir().unwrap();
        let input = temp.path().join("input.png");
        write_png(&input);
        let outside = temp.path().join("outside");
        std::fs::create_dir(&outside).unwrap();
        let linked = temp.path().join("linked");
        std::os::unix::fs::symlink(&outside, &linked).unwrap();

        let result = RToolsServer
            .handle_tool(
                "organize_photos",
                serde_json::json!({
                    "input_dir": temp.path(),
                    "output_dir": linked,
                    "strategy": "date",
                }),
            )
            .await
            .expect("tool dispatch must complete");
        let serialized = serde_json::to_value(&result).unwrap();

        assert!(is_error(&result), "{result:?}");
        assert_eq!(
            serialized["structuredContent"]["code"],
            "PATH_POLICY_VIOLATION"
        );
        assert_eq!(std::fs::read_dir(outside).unwrap().count(), 0);
        assert!(input.exists());
    }

    #[tokio::test]
    async fn duplicate_tool_uses_default_decoded_pixel_limit() {
        let temp = tempfile::tempdir().expect("temp dir");
        write_declared_png(&temp.path().join("oversized.png"), 10_001, 10_000);

        let result = RToolsServer
            .handle_tool(
                "find_duplicates",
                serde_json::json!({"input_dir": temp.path()}),
            )
            .await
            .expect("tool dispatch must complete");
        let document = serde_json::to_string(&result).expect("tool result must serialize");
        let serialized = serde_json::to_value(&result).unwrap();

        assert!(is_error(&result), "{result:?}");
        assert_eq!(
            serialized["structuredContent"]["code"],
            "RESOURCE_LIMIT_EXCEEDED"
        );
        assert_eq!(
            serialized["structuredContent"]["operation_id"],
            "ai.duplicates.report"
        );
        assert!(!document.contains("100010000"), "{document}");
        assert!(!document.contains("100000000"), "{document}");
    }
}
