use crate::commands::CommandResult;
use rtools_core::{AppConfig, RToolsError, RToolsResult};
use std::path::PathBuf;

pub fn handle_batch_command(
    _config_path: PathBuf,
    _jobs: Option<usize>,
    _app_config: &AppConfig,
) -> RToolsResult<CommandResult> {
    Err(RToolsError::capability_unavailable(
        "batch.run",
        "Batch recipe execution is not implemented",
        "Run operations individually until typed batch execution is available",
    ))
}

#[cfg(test)]
mod tests {
    use super::handle_batch_command;
    use rtools_core::{AppConfig, ErrorCode, RToolsError};

    #[tokio::test]
    async fn declared_batch_steps_cannot_report_success_without_execution() {
        let temp = tempfile::tempdir().unwrap();
        let config_path = temp.path().join("batch.toml");
        std::fs::write(
            &config_path,
            "[[operations]]\noperation = \"compress\"\ninput = [\"photo.png\"]\n",
        )
        .unwrap();

        let error = handle_batch_command(config_path, None, &AppConfig::default()).unwrap_err();

        assert_eq!(error.code(), ErrorCode::CapabilityUnavailable);
        assert!(matches!(
            error,
            RToolsError::CapabilityUnavailable { operation_id, .. }
                if operation_id == "batch.run"
        ));
    }
}
