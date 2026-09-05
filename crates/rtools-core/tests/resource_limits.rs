use rtools_core::{RToolsError, ResourceLimits};
use {
    derive_more as _, dirs as _, figment as _, serde as _, serde_json as _, tempfile as _,
    thiserror as _, toml as _, tracing as _,
};

#[test]
fn rejects_file_larger_than_byte_limit() {
    let limits = ResourceLimits {
        max_input_bytes: 10,
        ..ResourceLimits::default()
    };

    let error = limits.check_input_bytes(11).unwrap_err();
    assert!(matches!(error, RToolsError::ResourceLimitExceeded { .. }));
}

#[test]
fn rejects_decoded_pixel_overflow_without_multiplication_overflow() {
    let limits = ResourceLimits {
        max_decoded_pixels: 1_000_000,
        ..ResourceLimits::default()
    };

    assert!(limits.check_decoded_pixels(u32::MAX, u32::MAX).is_err());
}

#[test]
fn rejects_either_image_axis_above_the_dimension_limit() {
    let limits = ResourceLimits {
        max_image_dimension: 10,
        max_decoded_pixels: 1_000_000,
        ..ResourceLimits::default()
    };

    for (width, height, actual) in [(11, 1, 11), (1, 12, 12)] {
        let error = limits.check_decoded_pixels(width, height).unwrap_err();
        assert!(matches!(
            error,
            RToolsError::ResourceLimitExceeded {
                resource: "image_dimension",
                actual: value,
                limit: 10,
            } if value == actual
        ));
    }
}

#[test]
fn unknown_actual_resource_limit_has_stable_resource_code() {
    let error = RToolsError::ResourceLimitExceededUnknownActual {
        resource: "image_decoder_allocation_bytes",
        limit: 512,
    };

    assert_eq!(error.code().as_str(), "RESOURCE_LIMIT_EXCEEDED");
    assert!(matches!(
        error,
        RToolsError::ResourceLimitExceededUnknownActual {
            resource: "image_decoder_allocation_bytes",
            limit: 512,
        }
    ));
}
