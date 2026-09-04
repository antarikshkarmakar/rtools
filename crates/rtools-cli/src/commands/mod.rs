pub mod ai;
pub mod batch;
pub mod config;
pub mod image;
pub mod pdf;

use rtools_core::{FileOutput, RToolsError, RToolsResult};
use serde::Serialize;
use serde_json::Value;

#[derive(Debug)]
pub struct CommandResult {
    pub operation_id: String,
    pub result: Value,
    pub warnings: Vec<String>,
}

impl CommandResult {
    pub fn new(operation_id: impl Into<String>, result: Value, warnings: Vec<String>) -> Self {
        Self {
            operation_id: operation_id.into(),
            result,
            warnings,
        }
    }

    pub fn from_serializable<T: Serialize>(
        operation_id: impl Into<String>,
        result: T,
        warnings: Vec<String>,
    ) -> RToolsResult<Self> {
        let result = serde_json::to_value(result).map_err(|error| {
            RToolsError::Internal(format!("Failed to serialize CLI result: {error}"))
        })?;
        Ok(Self::new(operation_id, result, warnings))
    }

    pub fn from_file_outputs(
        operation_id: impl Into<String>,
        message: impl Into<String>,
        outputs: Vec<FileOutput>,
    ) -> RToolsResult<Self> {
        let warnings = outputs
            .iter()
            .flat_map(|output| output.warnings.iter().cloned())
            .collect();
        Self::from_serializable(
            operation_id,
            FileOutputsResult {
                message: message.into(),
                outputs,
            },
            warnings,
        )
    }
}

#[derive(Serialize)]
struct FileOutputsResult {
    message: String,
    outputs: Vec<FileOutput>,
}

#[cfg(test)]
mod tests {
    use super::CommandResult;
    use rtools_core::FileOutput;

    #[test]
    fn file_output_warnings_are_exposed_in_command_report() {
        let outputs = vec![FileOutput {
            destination: rtools_core::OutputDestination::File("output.png".into()),
            name: Some("output.png".to_string()),
            mime_type: Some("image/png".to_string()),
            stats: None,
            warnings: vec!["EXIF orientation 6 applied".to_string()],
        }];

        let report =
            CommandResult::from_file_outputs("image.convert", "Converted 1 image", outputs)
                .expect("file output must serialize into the command result");

        assert_eq!(report.warnings, ["EXIF orientation 6 applied"]);
        assert_eq!(
            report.result["outputs"][0]["warnings"],
            serde_json::json!(["EXIF orientation 6 applied"])
        );
    }
}
