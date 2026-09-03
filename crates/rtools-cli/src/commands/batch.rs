use rtools_core::AppConfig;
use std::path::PathBuf;

pub async fn handle_batch_command(
    _config_path: PathBuf,
    _jobs: Option<usize>,
    _app_config: &AppConfig,
) -> anyhow::Result<()> {
    std::future::ready(()).await;
    Err(rtools_core::RToolsError::capability_unavailable(
        "batch.run",
        "Batch recipe execution is not implemented",
        "Run operations individually until typed batch execution is available",
    )
    .into())
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

        let error = handle_batch_command(config_path, None, &AppConfig::default())
            .await
            .unwrap_err();
        let error = error.downcast_ref::<RToolsError>().unwrap();

        assert_eq!(error.code(), ErrorCode::CapabilityUnavailable);
        assert!(matches!(
            error,
            RToolsError::CapabilityUnavailable { operation_id, .. }
                if operation_id == "batch.run"
        ));
    }
}
