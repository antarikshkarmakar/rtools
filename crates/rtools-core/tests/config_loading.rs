use rtools_core::{config::ConfigLocations, AppConfig, ErrorCode};
use std::{fs, process::Command};
use tempfile::tempdir;
use {
    derive_more as _, dirs as _, figment as _, serde as _, serde_json as _, thiserror as _,
    toml as _, tracing as _,
};

#[test]
fn missing_explicit_config_is_an_error() {
    let sandbox = tempdir().unwrap();
    let missing = sandbox.path().join("missing.toml");

    let error = AppConfig::load(Some(&missing)).unwrap_err();

    assert_eq!(error.code(), ErrorCode::ConfigurationInvalid);
}

#[test]
fn loading_config_does_not_create_the_temporary_directory() {
    let sandbox = tempdir().unwrap();
    let config_path = sandbox.path().join("explicit.toml");
    let configured_temp_dir = sandbox.path().join("not-created-by-load");
    fs::write(
        &config_path,
        format!(
            "[general]\ntemp_dir = {:?}\n",
            configured_temp_dir.to_string_lossy()
        ),
    )
    .unwrap();

    AppConfig::load(Some(&config_path)).unwrap();

    assert!(!configured_temp_dir.exists());
}

#[test]
fn semantically_invalid_config_is_rejected() {
    let sandbox = tempdir().unwrap();
    let config_path = sandbox.path().join("explicit.toml");
    fs::write(&config_path, "[general]\nparallel_jobs = 0\n").unwrap();

    let error = AppConfig::load(Some(&config_path)).unwrap_err();

    assert_eq!(error.code(), ErrorCode::ConfigurationInvalid);
}

#[test]
fn precedence_is_default_then_system_user_project_explicit() {
    let sandbox = tempdir().unwrap();
    let system = sandbox.path().join("system.toml");
    let user = sandbox.path().join("user.toml");
    let project = sandbox.path().join("project.toml");
    let explicit = sandbox.path().join("explicit.toml");
    fs::write(
        &system,
        "[general]\nlog_level = \"error\"\nmax_file_size = 100\n\n[image]\ndefault_quality = 10\n\n[pdf]\nocr_dpi = 100\n",
    )
    .unwrap();
    fs::write(
        &user,
        "[general]\nlog_level = \"warn\"\nmax_file_size = 200\n\n[image]\ndefault_quality = 20\n",
    )
    .unwrap();
    fs::write(
        &project,
        "[general]\nlog_level = \"debug\"\nmax_file_size = 300\n",
    )
    .unwrap();
    fs::write(&explicit, "[general]\nlog_level = \"trace\"\n").unwrap();
    let locations = ConfigLocations {
        system: Some(system),
        user: Some(user),
        project: Some(project),
    };

    let config = AppConfig::load_from_locations(Some(&explicit), &locations).unwrap();

    assert_eq!(config.general.log_level, "trace");
    assert_eq!(config.general.max_file_size, 300);
    assert_eq!(config.image.default_quality, 20);
    assert_eq!(config.pdf.ocr_dpi, 100);
    assert_eq!(config.image.jpeg_quality, 85);
}

#[test]
fn missing_discovered_files_are_skipped() {
    let sandbox = tempdir().unwrap();
    let locations = ConfigLocations {
        system: Some(sandbox.path().join("missing-system.toml")),
        user: Some(sandbox.path().join("missing-user.toml")),
        project: Some(sandbox.path().join("missing-project.toml")),
    };

    let config = AppConfig::load_from_locations(None, &locations).unwrap();

    assert_eq!(config.general.log_level, "info");
}

#[test]
fn invalid_discovered_file_is_an_error() {
    let sandbox = tempdir().unwrap();
    let project = sandbox.path().join("project.toml");
    fs::write(&project, "[general\nlog_level = ???").unwrap();
    let locations = ConfigLocations {
        project: Some(project),
        ..ConfigLocations::default()
    };

    let error = AppConfig::load_from_locations(None, &locations).unwrap_err();

    assert_eq!(error.code(), ErrorCode::ConfigurationInvalid);
}

#[test]
fn unreadable_discovered_location_is_an_error() {
    let sandbox = tempdir().unwrap();
    let locations = ConfigLocations {
        project: Some(sandbox.path().to_path_buf()),
        ..ConfigLocations::default()
    };

    let error = AppConfig::load_from_locations(None, &locations).unwrap_err();

    assert_eq!(error.code(), ErrorCode::ConfigurationInvalid);
}

#[test]
fn invalid_explicit_file_is_an_error() {
    let sandbox = tempdir().unwrap();
    let explicit = sandbox.path().join("explicit.toml");
    fs::write(&explicit, "[api]\nport = \"not-a-port\"\n").unwrap();

    let error =
        AppConfig::load_from_locations(Some(&explicit), &ConfigLocations::default()).unwrap_err();

    assert_eq!(error.code(), ErrorCode::ConfigurationInvalid);
}

#[test]
fn unreadable_explicit_location_is_an_error() {
    let sandbox = tempdir().unwrap();
    let explicit = sandbox.path().to_path_buf();

    let error =
        AppConfig::load_from_locations(Some(&explicit), &ConfigLocations::default()).unwrap_err();

    assert_eq!(error.code(), ErrorCode::ConfigurationInvalid);
}

#[test]
fn environment_scalar_and_nested_values_have_highest_precedence() {
    if std::env::var_os("RTOOLS_TEST_ENV_PROBE").is_some() {
        let sandbox = tempdir().unwrap();
        let explicit = sandbox.path().join("explicit.toml");
        fs::write(
            &explicit,
            "[general]\nlog_level = \"debug\"\n\n[api]\nport = 8082\n\n[image]\nwebp_lossless = false\n",
        )
        .unwrap();
        let config =
            AppConfig::load_from_locations(Some(&explicit), &ConfigLocations::default()).unwrap();
        assert_eq!(config.general.log_level, "error");
        assert_eq!(config.api.port, 9091);
        assert!(config.image.webp_lossless);
        return;
    }

    run_isolated_environment_test(
        "environment_scalar_and_nested_values_have_highest_precedence",
        &[
            ("RTOOLS_TEST_ENV_PROBE", "1"),
            ("RTOOLS_GENERAL__LOG_LEVEL", "error"),
            ("RTOOLS_API__PORT", "9091"),
            ("RTOOLS_IMAGE__WEBP_LOSSLESS", "true"),
        ],
    );
}

#[test]
fn invalid_environment_value_is_an_error() {
    if std::env::var_os("RTOOLS_TEST_INVALID_ENV_PROBE").is_some() {
        let error = AppConfig::load_from_locations(None, &ConfigLocations::default()).unwrap_err();
        assert_eq!(error.code(), ErrorCode::ConfigurationInvalid);
        return;
    }

    run_isolated_environment_test(
        "invalid_environment_value_is_an_error",
        &[
            ("RTOOLS_TEST_INVALID_ENV_PROBE", "1"),
            ("RTOOLS_API__PORT", "not-a-port"),
        ],
    );
}

fn run_isolated_environment_test(test_name: &str, environment: &[(&str, &str)]) {
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .arg("--exact")
        .arg(test_name)
        .arg("--nocapture")
        .env_clear();
    for (key, value) in environment {
        command.env(key, value);
    }

    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "isolated test failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
