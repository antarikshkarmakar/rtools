use rtools_core::{validate_portable_filename_component, ErrorCode};
use {
    derive_more as _, dirs as _, figment as _, serde as _, serde_json as _, tempfile as _,
    thiserror as _, toml as _, tracing as _,
};

#[test]
fn portable_filename_component_rejects_superscript_devices_and_long_names() {
    for invalid in [
        "COM¹.png".to_string(),
        "com².txt".to_string(),
        "LPT³".to_string(),
        "a".repeat(256),
    ] {
        let error = validate_portable_filename_component(&invalid).unwrap_err();
        assert_eq!(error.code(), ErrorCode::InvalidInput, "{invalid}");
    }
    validate_portable_filename_component("résumé.png").unwrap();
}
