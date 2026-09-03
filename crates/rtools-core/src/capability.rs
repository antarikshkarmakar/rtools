use crate::{RToolsError, RToolsResult};
use serde::de::Error as _;
use serde::ser::SerializeSeq as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::BTreeMap;

/// Execution state for a registered operation or provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityState {
    /// The operation is implemented and executable.
    Available,
    /// The operation cannot currently execute.
    Unavailable,
    /// The operation executes with an explicit limitation or caution.
    Experimental,
}

/// Diagnostic information about a provider used by a capability.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderDiagnostic {
    /// Stable provider identifier.
    pub provider_id: String,
    /// Current provider state.
    pub state: CapabilityState,
    /// Reason for an unavailable or experimental state.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// Action that can make an unavailable provider usable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remediation: Option<String>,
}

impl ProviderDiagnostic {
    /// Describe an available provider.
    pub fn available(provider_id: impl Into<String>) -> Self {
        Self {
            provider_id: provider_id.into(),
            state: CapabilityState::Available,
            reason: None,
            remediation: None,
        }
    }

    /// Describe an unavailable provider.
    pub fn unavailable(
        provider_id: impl Into<String>,
        reason: impl Into<String>,
        remediation: impl Into<String>,
    ) -> Self {
        Self {
            provider_id: provider_id.into(),
            state: CapabilityState::Unavailable,
            reason: Some(reason.into()),
            remediation: Some(remediation.into()),
        }
    }

    /// Describe an experimental provider.
    pub fn experimental(provider_id: impl Into<String>, reason: impl Into<String>) -> Self {
        Self {
            provider_id: provider_id.into(),
            state: CapabilityState::Experimental,
            reason: Some(reason.into()),
            remediation: None,
        }
    }
}

/// Serializable truth record for one operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Capability {
    /// Stable lowercase dotted operation identifier.
    pub operation_id: String,
    /// Current execution state.
    pub state: CapabilityState,
    /// Reason for an unavailable or experimental state.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// Action that can make an unavailable operation usable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remediation: Option<String>,
    /// Provider-specific diagnostics used to explain this state.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub provider_diagnostics: Vec<ProviderDiagnostic>,
}

impl Capability {
    /// Describe an available operation.
    pub fn available(operation_id: impl Into<String>) -> Self {
        Self {
            operation_id: operation_id.into(),
            state: CapabilityState::Available,
            reason: None,
            remediation: None,
            provider_diagnostics: Vec::new(),
        }
    }

    /// Describe an unavailable operation.
    pub fn unavailable(
        operation_id: impl Into<String>,
        reason: impl Into<String>,
        remediation: impl Into<String>,
    ) -> Self {
        Self {
            operation_id: operation_id.into(),
            state: CapabilityState::Unavailable,
            reason: Some(reason.into()),
            remediation: Some(remediation.into()),
            provider_diagnostics: Vec::new(),
        }
    }

    /// Describe an executable operation with an explicit limitation.
    pub fn experimental(operation_id: impl Into<String>, reason: impl Into<String>) -> Self {
        Self {
            operation_id: operation_id.into(),
            state: CapabilityState::Experimental,
            reason: Some(reason.into()),
            remediation: None,
            provider_diagnostics: Vec::new(),
        }
    }

    /// Attach a provider diagnostic to this operation.
    #[must_use]
    pub fn with_provider_diagnostic(mut self, diagnostic: ProviderDiagnostic) -> Self {
        self.provider_diagnostics.push(diagnostic);
        self
    }
}

/// Deterministically ordered registry of operation capabilities.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CapabilityRegistry {
    capabilities: BTreeMap<String, Capability>,
}

impl CapabilityRegistry {
    /// Validate and register a capability.
    ///
    /// # Errors
    ///
    /// Returns `INVALID_INPUT` for an invalid operation identifier and
    /// `CONFIGURATION_INVALID` for duplicates or invalid state diagnostics.
    pub fn register(&mut self, capability: Capability) -> RToolsResult<()> {
        validate_operation_id(&capability.operation_id)?;
        validate_diagnostic_state(
            capability.state,
            capability.reason.as_deref(),
            capability.remediation.as_deref(),
            "capability",
        )?;
        for diagnostic in &capability.provider_diagnostics {
            if diagnostic.provider_id.trim().is_empty()
                || diagnostic.provider_id.chars().any(char::is_whitespace)
            {
                return Err(RToolsError::configuration_invalid(
                    "Provider identifiers must be non-empty and contain no whitespace",
                ));
            }
            validate_diagnostic_state(
                diagnostic.state,
                diagnostic.reason.as_deref(),
                diagnostic.remediation.as_deref(),
                "provider diagnostic",
            )?;
        }

        if self.capabilities.contains_key(&capability.operation_id) {
            return Err(RToolsError::configuration_invalid(
                "A capability operation identifier was registered more than once",
            ));
        }
        self.capabilities
            .insert(capability.operation_id.clone(), capability);
        Ok(())
    }

    /// Look up an operation without requiring it to be executable.
    pub fn lookup(&self, operation_id: &str) -> Option<&Capability> {
        self.capabilities.get(operation_id)
    }

    /// List all capabilities in deterministic operation-identifier order.
    pub fn list(&self) -> Vec<&Capability> {
        self.capabilities.values().collect()
    }

    /// Require an operation to be available or experimental.
    ///
    /// # Errors
    ///
    /// Returns `CAPABILITY_UNAVAILABLE` for unavailable and unknown operations.
    pub fn require_available(&self, operation_id: &str) -> RToolsResult<&Capability> {
        match self.lookup(operation_id) {
            Some(capability)
                if matches!(
                    capability.state,
                    CapabilityState::Available | CapabilityState::Experimental
                ) =>
            {
                Ok(capability)
            }
            Some(capability) => Err(RToolsError::capability_unavailable(
                &capability.operation_id,
                capability
                    .reason
                    .as_deref()
                    .unwrap_or("Operation is unavailable"),
                capability
                    .remediation
                    .as_deref()
                    .unwrap_or("Consult the capability registry for supported operations"),
            )),
            None => Err(RToolsError::capability_unavailable(
                operation_id,
                "Operation is not registered",
                "Consult the capability registry for supported operations",
            )),
        }
    }
}

impl Serialize for CapabilityRegistry {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut sequence = serializer.serialize_seq(Some(self.capabilities.len()))?;
        for capability in self.capabilities.values() {
            sequence.serialize_element(capability)?;
        }
        sequence.end()
    }
}

impl<'de> Deserialize<'de> for CapabilityRegistry {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let capabilities = Vec::<Capability>::deserialize(deserializer)?;
        let mut registry = Self::default();
        for capability in capabilities {
            registry.register(capability).map_err(D::Error::custom)?;
        }
        Ok(registry)
    }
}

fn validate_operation_id(operation_id: &str) -> RToolsResult<()> {
    let valid = !operation_id.is_empty()
        && operation_id.split('.').all(|segment| {
            let mut characters = segment.chars();
            characters
                .next()
                .is_some_and(|first| first.is_ascii_lowercase())
                && characters.all(|character| {
                    character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_'
                })
        });
    if valid {
        Ok(())
    } else {
        Err(RToolsError::invalid_input(
            "Operation identifiers must contain lowercase dotted segments beginning with a letter",
        ))
    }
}

fn validate_diagnostic_state(
    state: CapabilityState,
    reason: Option<&str>,
    remediation: Option<&str>,
    subject: &str,
) -> RToolsResult<()> {
    let has_reason = reason.is_some_and(|value| !value.trim().is_empty());
    let has_remediation = remediation.is_some_and(|value| !value.trim().is_empty());
    let valid = match state {
        CapabilityState::Available => reason.is_none() && remediation.is_none(),
        CapabilityState::Experimental => {
            has_reason && remediation.is_none_or(|value| !value.trim().is_empty())
        }
        CapabilityState::Unavailable => has_reason && has_remediation,
    };
    if valid {
        Ok(())
    } else {
        Err(RToolsError::configuration_invalid(format!(
            "Invalid {subject} reason or remediation for its state"
        )))
    }
}
