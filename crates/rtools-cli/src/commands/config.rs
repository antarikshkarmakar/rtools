use crate::ConfigCommands;
use rtools_core::AppConfig;

pub fn handle_config_command(cmd: ConfigCommands, effective: &AppConfig) -> anyhow::Result<()> {
    match cmd {
        ConfigCommands::Show => {
            println!("{}", serialize_effective_config(effective)?);
            Ok(())
        }

        ConfigCommands::Init { output } => {
            let config = AppConfig::default();
            config.save(&output)?;
            println!("✓ Configuration file created: {}", output.display());
            Ok(())
        }

        ConfigCommands::Validate {
            config: config_path,
        } => {
            AppConfig::load(Some(&config_path))?;
            println!("✓ Configuration file is valid: {}", config_path.display());
            Ok(())
        }
    }
}

fn serialize_effective_config(config: &AppConfig) -> anyhow::Result<String> {
    let mut value = toml::Value::try_from(config)?;
    redact_secrets(&mut value);
    Ok(toml::to_string_pretty(&value)?)
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
    let key = key.to_ascii_lowercase();
    matches!(
        key.as_str(),
        "api_key" | "password" | "passphrase" | "secret" | "token" | "private_key"
    ) || key.ends_with("_password")
        || key.ends_with("_passphrase")
        || key.ends_with("_secret")
        || key.ends_with("_token")
        || key.ends_with("_api_key")
        || key.ends_with("_private_key")
}

#[cfg(test)]
mod tests {
    use super::{redact_secrets, serialize_effective_config};
    use rtools_core::AppConfig;

    #[test]
    fn effective_config_serialization_redacts_api_key() {
        let mut config = AppConfig::default();
        config.api.api_key = Some("do-not-leak".to_string());

        let serialized = serialize_effective_config(&config).unwrap();

        assert!(serialized.contains("<redacted>"));
        assert!(!serialized.contains("do-not-leak"));
        assert!(serialized.contains("parallel_jobs"));
    }

    #[test]
    fn secret_redaction_is_key_aware_and_recursive() {
        let mut value: toml::Value = toml::from_str(
            "[outer]\npassword = \"first-secret\"\nmonkey = \"public-value\"\n\n[outer.child]\naccess_token = \"second-secret\"\n",
        )
        .unwrap();

        redact_secrets(&mut value);
        let serialized = toml::to_string(&value).unwrap();
        let json = serde_json::to_string(&value).unwrap();
        let debug = format!("{value:?}");

        for output in [&serialized, &json, &debug] {
            assert!(!output.contains("first-secret"));
            assert!(!output.contains("second-secret"));
        }
        assert!(serialized.contains("public-value"));
    }
}
