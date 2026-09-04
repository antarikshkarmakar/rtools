use rtools_core::{ErrorCode, RToolsError};
use std::process::ExitCode;

pub const fn numeric_exit_code(code: ErrorCode) -> u8 {
    match code {
        ErrorCode::InvalidInput | ErrorCode::UnsupportedFormat => 2,
        ErrorCode::CapabilityUnavailable
        | ErrorCode::ConfigurationInvalid
        | ErrorCode::AuthenticationRequired => 3,
        ErrorCode::ResourceLimitExceeded => 4,
        ErrorCode::OutputExists | ErrorCode::PathPolicyViolation => 5,
        ErrorCode::ProcessingFailed => 6,
        ErrorCode::PartialFailure => 7,
        ErrorCode::Cancelled | ErrorCode::RollbackFailed => 8,
    }
}

pub fn for_error(error: &RToolsError) -> ExitCode {
    ExitCode::from(numeric_exit_code(error.code()))
}

#[cfg(test)]
mod tests {
    use super::numeric_exit_code;
    use rtools_core::ErrorCode;

    #[test]
    fn every_stable_error_code_maps_to_the_documented_process_status() {
        for (code, expected) in [
            (ErrorCode::InvalidInput, 2),
            (ErrorCode::UnsupportedFormat, 2),
            (ErrorCode::CapabilityUnavailable, 3),
            (ErrorCode::ConfigurationInvalid, 3),
            (ErrorCode::AuthenticationRequired, 3),
            (ErrorCode::ResourceLimitExceeded, 4),
            (ErrorCode::OutputExists, 5),
            (ErrorCode::PathPolicyViolation, 5),
            (ErrorCode::ProcessingFailed, 6),
            (ErrorCode::PartialFailure, 7),
            (ErrorCode::Cancelled, 8),
            (ErrorCode::RollbackFailed, 8),
        ] {
            assert_eq!(numeric_exit_code(code), expected);
        }
    }

    #[test]
    fn partial_failure_maps_to_exit_seven_directly() {
        assert_eq!(numeric_exit_code(ErrorCode::PartialFailure), 7);
    }
}
