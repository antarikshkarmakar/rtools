use crate::commands::CommandResult;
use crate::ConfigCommands;
use rtools_core::{AppConfig, RToolsResult};
use serde::Serialize;
use serde_json::json;

pub fn handle_config_command(
    command: ConfigCommands,
    effective: &AppConfig,
) -> RToolsResult<CommandResult> {
    match command {
        ConfigCommands::Show => {
            let config = effective_config_value(effective)?;
            let human_text = toml::to_string_pretty(&config)?;
            CommandResult::from_serializable(
                "config.show",
                ConfigShowResult { human_text, config },
                Vec::new(),
            )
        }
        ConfigCommands::Init { output } => {
            AppConfig::default().save(&output)?;
            Ok(CommandResult::new(
                "config.init",
                json!({
                    "message": "Configuration file created",
                    "path": output,
                }),
                Vec::new(),
            ))
        }
        ConfigCommands::Validate { config } => {
            AppConfig::load(Some(&config))?;
            Ok(CommandResult::new(
                "config.validate",
                json!({
                    "message": "Configuration file is valid",
                    "path": config,
                }),
                Vec::new(),
            ))
        }
    }
}

#[derive(Serialize)]
struct ConfigShowResult {
    human_text: String,
    config: toml::Value,
}

fn effective_config_value(config: &AppConfig) -> RToolsResult<toml::Value> {
    let mut value = toml::Value::try_from(config)?;
    redact_secrets(&mut value);
    Ok(value)
}

fn redact_secrets(value: &mut toml::Value) {
    match value {
        toml::Value::Table(table) => {
            for (key, child) in table {
                if is_secret_key(key) {
                    *child = toml::Value::String("<redacted>".to_string());
                } else {
                    redact_secrets(child);
                }
            }
        }
        toml::Value::Array(values) => {
            for child in values {
                redact_secrets(child);
            }
        }
        _ => {}
    }
}

fn is_secret_key(key: &str) -> bool {
    let mut normalized = String::with_capacity(key.len());
    for character in key.chars() {
        if character.is_ascii_alphanumeric() {
            normalized.push(character.to_ascii_lowercase());
        } else if !normalized.ends_with('_') {
            normalized.push('_');
        }
    }
    let key = normalized.trim_matches('_');
    matches!(
        key,
        "api_key"
            | "credential"
            | "credentials"
            | "password"
            | "passphrase"
            | "private_key"
            | "secret"
            | "secret_key"
            | "token"
    ) || key.ends_with("_password")
        || key.ends_with("_passphrase")
        || key.ends_with("_secret")
        || key.ends_with("_secret_key")
        || key.ends_with("_token")
        || key.ends_with("_api_key")
        || key.ends_with("_private_key")
        || key.ends_with("_credential")
        || key.ends_with("_credentials")
}

#[cfg(test)]
mod tests {
    use super::{effective_config_value, redact_secrets};
    use rtools_core::AppConfig;

    #[test]
    fn effective_config_serialization_redacts_api_key() {
        let mut config = AppConfig::default();
        config.api.api_key = Some("do-not-leak".to_string());

        let value = effective_config_value(&config).unwrap();
        let serialized = toml::to_string_pretty(&value).unwrap();

        assert!(serialized.contains("<redacted>"));
        assert!(!serialized.contains("do-not-leak"));
        assert!(serialized.contains("parallel_jobs"));
    }

    #[test]
    fn secret_redaction_is_key_aware_and_recursive() {
        let mut value: toml::Value = toml::from_str(
            "[outer]\npassword = \"first-secret\"\nmonkey = \"public-value\"\nsecret_key = \"third-secret\"\nclient_credentials = \"fourth-secret\"\ntokenizer = \"benign-tokenizer\"\nsecretary = \"benign-secretary\"\n\n[outer.child]\naccess_token = \"second-secret\"\n",
        )
        .unwrap();

        redact_secrets(&mut value);
        let serialized = toml::to_string(&value).unwrap();
        let json = serde_json::to_string(&value).unwrap();
        let debug = format!("{value:?}");

        for output in [&serialized, &json, &debug] {
            assert!(!output.contains("first-secret"));
            assert!(!output.contains("second-secret"));
            assert!(!output.contains("third-secret"));
            assert!(!output.contains("fourth-secret"));
        }
        assert!(serialized.contains("public-value"));
        assert!(serialized.contains("benign-tokenizer"));
        assert!(serialized.contains("benign-secretary"));
    }
}
