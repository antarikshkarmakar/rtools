use clap::ValueEnum;
use rtools_core::RToolsError;
use serde::Serialize;
use serde_json::Value;
use std::io::{self, Write as _};

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum OutputFormat {
    Human,
    Json,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReportStatus {
    Success,
    Failure,
    PartialFailure,
}

#[derive(Debug, Serialize)]
pub struct ItemFailure {
    pub code: rtools_core::ErrorCode,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub item: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CliReport<T> {
    pub operation_id: String,
    pub status: ReportStatus,
    pub result: Option<T>,
    pub warnings: Vec<String>,
    pub failures: Vec<ItemFailure>,
}

impl CliReport<Value> {
    pub fn success(operation_id: impl Into<String>, result: Value, warnings: Vec<String>) -> Self {
        Self {
            operation_id: operation_id.into(),
            status: ReportStatus::Success,
            result: Some(result),
            warnings,
            failures: Vec::new(),
        }
    }

    pub fn failure(operation_id: impl Into<String>, error: &RToolsError) -> Self {
        Self {
            operation_id: operation_id.into(),
            status: if error.code() == rtools_core::ErrorCode::PartialFailure {
                ReportStatus::PartialFailure
            } else {
                ReportStatus::Failure
            },
            result: None,
            warnings: Vec::new(),
            failures: vec![ItemFailure {
                code: error.code(),
                message: error.to_string(),
                item: None,
            }],
        }
    }
}

pub fn render(report: &CliReport<Value>, format: OutputFormat) -> io::Result<()> {
    match format {
        OutputFormat::Json => render_json(report),
        OutputFormat::Human => render_human(report),
    }
}

pub fn render_completions(shell: clap_complete::Shell, command: &mut clap::Command) {
    // Completion scripts are intentionally raw shell source, not command-result
    // output. Keeping the sole stdout writer here preserves mechanical ownership.
    clap_complete::generate(shell, command, "rtools", &mut io::stdout());
}

pub fn render_write_error(error: &io::Error) {
    let _ = writeln!(io::stderr(), "Error [PROCESSING_FAILED]: {error}");
}

fn render_json(report: &CliReport<Value>) -> io::Result<()> {
    let mut stdout = io::stdout().lock();
    serde_json::to_writer(&mut stdout, report).map_err(io::Error::other)?;
    writeln!(stdout)
}

fn render_human(report: &CliReport<Value>) -> io::Result<()> {
    if report.status != ReportStatus::Success {
        let mut stderr = io::stderr().lock();
        for failure in &report.failures {
            writeln!(
                stderr,
                "Error [{}]: {}",
                failure.code.as_str(),
                failure.message
            )?;
        }
        return Ok(());
    }

    let mut stdout = io::stdout().lock();
    let result = report.result.as_ref().unwrap_or(&Value::Null);
    if let Some(text) = result.get("human_text").and_then(Value::as_str) {
        writeln!(stdout, "{text}")?;
    } else if report.operation_id == "doctor.report" {
        render_human_doctor(&mut stdout, result)?;
    } else if let Some(message) = result.get("message").and_then(Value::as_str) {
        writeln!(stdout, "✓ {message}")?;
    } else {
        serde_json::to_writer_pretty(&mut stdout, result).map_err(io::Error::other)?;
        writeln!(stdout)?;
    }

    if let Some(planned) = result.get("planned").and_then(Value::as_array) {
        for entry in planned {
            let source = entry.get("source").and_then(Value::as_str).unwrap_or("?");
            let destination = entry
                .get("destination")
                .and_then(Value::as_str)
                .unwrap_or("?");
            writeln!(stdout, "  {source} -> {destination}")?;
        }
    }
    for warning in &report.warnings {
        writeln!(stdout, "  Warning: {warning}")?;
    }
    Ok(())
}

fn render_human_doctor(writer: &mut impl io::Write, result: &Value) -> io::Result<()> {
    writeln!(writer, "rtools capability diagnostics")?;
    if let Some(capabilities) = result.get("capabilities").and_then(Value::as_array) {
        for capability in capabilities {
            let state = capability
                .get("state")
                .and_then(Value::as_str)
                .unwrap_or("?");
            let symbol = match state {
                "available" => "✓",
                "experimental" => "!",
                _ => "x",
            };
            let operation = capability
                .get("operation_id")
                .and_then(Value::as_str)
                .unwrap_or("?");
            writeln!(writer, "{symbol} {operation}: {state}")?;
            if let Some(reason) = capability.get("reason").and_then(Value::as_str) {
                writeln!(writer, "  Reason: {reason}")?;
            }
            if let Some(remediation) = capability.get("remediation").and_then(Value::as_str) {
                writeln!(writer, "  Remediation: {remediation}")?;
            }
        }
    }
    writeln!(writer, "Configured limits:")?;
    serde_json::to_writer_pretty(
        &mut *writer,
        result.get("configured_limits").unwrap_or(&Value::Null),
    )
    .map_err(io::Error::other)?;
    writeln!(writer)?;
    writeln!(writer, "Writable directories:")?;
    serde_json::to_writer_pretty(
        &mut *writer,
        result.get("writable_directories").unwrap_or(&Value::Null),
    )
    .map_err(io::Error::other)?;
    writeln!(writer)
}

#[cfg(test)]
mod tests {
    use super::{CliReport, ReportStatus};
    use rtools_core::RToolsError;

    #[test]
    fn partial_failure_error_is_rendered_as_a_partial_failure_report() {
        let report = CliReport::failure("batch.run", &RToolsError::batch_error("one item failed"));

        assert_eq!(report.status, ReportStatus::PartialFailure);
        assert_eq!(report.failures[0].code.as_str(), "PARTIAL_FAILURE");
    }
}
