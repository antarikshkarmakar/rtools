use rtools_core::{
    Capability, CapabilityRegistry, CapabilityState, ErrorCode, ProviderDiagnostic, RToolsError,
};
use {derive_more as _, dirs as _, figment as _, serde as _, tempfile as _, thiserror as _};
use {toml as _, tracing as _};

#[test]
fn unavailable_capability_carries_machine_readable_remediation() {
    let capability = Capability::unavailable(
        "image.ocr",
        "No OCR provider is configured",
        "Configure a supported OCR provider",
    );

    assert_eq!(capability.state, CapabilityState::Unavailable);
    assert_eq!(capability.operation_id, "image.ocr");
    assert_eq!(
        capability.reason.as_deref(),
        Some("No OCR provider is configured")
    );
    assert_eq!(
        capability.remediation.as_deref(),
        Some("Configure a supported OCR provider")
    );
}

#[test]
fn duplicate_operation_ids_are_configuration_errors() {
    let mut registry = CapabilityRegistry::default();
    registry
        .register(Capability::available("image.resize"))
        .unwrap();

    let error = registry
        .register(Capability::available("image.resize"))
        .unwrap_err();

    assert_eq!(error.code(), ErrorCode::ConfigurationInvalid);
}

#[test]
fn operation_ids_reject_non_lowercase_dotted_syntax() {
    let invalid = [
        "",
        ".image.resize",
        "image.resize.",
        "image..resize",
        "Image.resize",
        "image.Resize",
        "image resize",
        "image\tresize",
        "image-resize",
        "1image.resize",
        "image.$resize",
        "image",
    ];

    for operation_id in invalid {
        let mut registry = CapabilityRegistry::default();
        let error = registry
            .register(Capability::available(operation_id))
            .unwrap_err();
        assert_eq!(error.code(), ErrorCode::InvalidInput, "{operation_id:?}");
    }

    let mut registry = CapabilityRegistry::default();
    registry
        .register(Capability::available("image.metadata.strip_gps2"))
        .unwrap();
}

#[test]
fn provider_diagnostics_are_canonical_and_duplicate_ids_are_rejected() {
    let mut registry = CapabilityRegistry::default();
    registry
        .register(
            Capability::available("image.resize")
                .with_provider_diagnostic(ProviderDiagnostic::available("z-provider"))
                .with_provider_diagnostic(ProviderDiagnostic::available("a-provider")),
        )
        .unwrap();
    let provider_ids: Vec<&str> = registry
        .lookup("image.resize")
        .unwrap()
        .provider_diagnostics
        .iter()
        .map(|diagnostic| diagnostic.provider_id.as_str())
        .collect();
    assert_eq!(provider_ids, ["a-provider", "z-provider"]);

    let duplicate = Capability::available("image.crop")
        .with_provider_diagnostic(ProviderDiagnostic::available("native-image"))
        .with_provider_diagnostic(ProviderDiagnostic::experimental(
            "native-image",
            "second registration",
        ));
    let error = registry.register(duplicate).unwrap_err();
    assert_eq!(error.code(), ErrorCode::ConfigurationInvalid);
}

#[test]
fn state_diagnostics_are_validated_at_registration() {
    let invalid = [
        Capability {
            operation_id: "image.resize".to_string(),
            state: CapabilityState::Available,
            reason: Some("unexpected warning".to_string()),
            remediation: None,
            provider_diagnostics: Vec::new(),
        },
        Capability {
            operation_id: "pdf.merge".to_string(),
            state: CapabilityState::Experimental,
            reason: Some(" ".to_string()),
            remediation: None,
            provider_diagnostics: Vec::new(),
        },
        Capability {
            operation_id: "pdf.ocr".to_string(),
            state: CapabilityState::Unavailable,
            reason: Some("No OCR provider".to_string()),
            remediation: None,
            provider_diagnostics: Vec::new(),
        },
    ];

    for capability in invalid {
        let mut registry = CapabilityRegistry::default();
        let error = registry.register(capability).unwrap_err();
        assert_eq!(error.code(), ErrorCode::ConfigurationInvalid);
    }
}

#[test]
fn provider_diagnostics_follow_the_same_state_invariants() {
    let capability =
        Capability::available("image.resize").with_provider_diagnostic(ProviderDiagnostic {
            provider_id: "native-image".to_string(),
            state: CapabilityState::Unavailable,
            reason: Some("provider disabled".to_string()),
            remediation: None,
        });
    let mut registry = CapabilityRegistry::default();

    let error = registry.register(capability).unwrap_err();

    assert_eq!(error.code(), ErrorCode::ConfigurationInvalid);
}

#[test]
fn experimental_capabilities_are_executable_but_carry_caution() {
    let mut registry = CapabilityRegistry::default();
    registry
        .register(Capability::experimental(
            "pdf.merge",
            "Document structures are only partially verified",
        ))
        .unwrap();

    let capability = registry.require_available("pdf.merge").unwrap();

    assert_eq!(capability.state, CapabilityState::Experimental);
    assert_eq!(
        capability.reason.as_deref(),
        Some("Document structures are only partially verified")
    );
}

#[test]
fn unavailable_and_unknown_operations_return_structured_capability_errors() {
    let mut registry = CapabilityRegistry::default();
    registry
        .register(Capability::unavailable(
            "image.ocr",
            "No OCR provider is configured",
            "Configure a supported OCR provider",
        ))
        .unwrap();

    let unavailable = registry.require_available("image.ocr").unwrap_err();
    assert_capability_error(
        &unavailable,
        "image.ocr",
        "No OCR provider is configured",
        "Configure a supported OCR provider",
    );

    let unknown = registry.require_available("pdf.unknown").unwrap_err();
    assert_capability_error(
        &unknown,
        "pdf.unknown",
        "Operation is not registered",
        "Consult the capability registry for supported operations",
    );
}

#[test]
fn lookup_list_and_serialization_are_deterministic() {
    let mut registry = CapabilityRegistry::default();
    registry
        .register(Capability::experimental(
            "pdf.merge",
            "Partial structural verification",
        ))
        .unwrap();
    registry
        .register(
            Capability::available("image.resize")
                .with_provider_diagnostic(ProviderDiagnostic::available("native-image")),
        )
        .unwrap();
    registry
        .register(Capability::unavailable(
            "ai.ocr",
            "No OCR provider is configured",
            "Configure a supported OCR provider",
        ))
        .unwrap();

    let ids: Vec<&str> = registry
        .list()
        .into_iter()
        .map(|capability| capability.operation_id.as_str())
        .collect();
    assert_eq!(ids, ["ai.ocr", "image.resize", "pdf.merge"]);
    assert_eq!(
        registry.lookup("image.resize").unwrap().state,
        CapabilityState::Available
    );

    let json = serde_json::to_string(&registry).unwrap();
    assert!(json.find("ai.ocr").unwrap() < json.find("image.resize").unwrap());
    assert!(json.find("image.resize").unwrap() < json.find("pdf.merge").unwrap());

    let restored: CapabilityRegistry = serde_json::from_str(&json).unwrap();
    let restored_ids: Vec<&str> = restored
        .list()
        .into_iter()
        .map(|capability| capability.operation_id.as_str())
        .collect();
    assert_eq!(restored_ids, ids);
    assert_eq!(
        restored
            .lookup("image.resize")
            .unwrap()
            .provider_diagnostics,
        [ProviderDiagnostic::available("native-image")]
    );
}

fn assert_capability_error(
    error: &RToolsError,
    expected_operation_id: &str,
    expected_reason: &str,
    expected_remediation: &str,
) {
    assert_eq!(error.code(), ErrorCode::CapabilityUnavailable);
    match error {
        RToolsError::CapabilityUnavailable {
            operation_id,
            reason,
            remediation,
        } => {
            assert_eq!(operation_id, expected_operation_id);
            assert_eq!(reason, expected_reason);
            assert_eq!(remediation, expected_remediation);
        }
        other => panic!("expected capability error, got {other:?}"),
    }
}
