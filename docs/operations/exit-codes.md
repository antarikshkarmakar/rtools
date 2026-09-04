# Stable CLI exit codes

rTools uses the following process exit codes for CLI reports. With
`--output-format json`, stdout is exactly one JSON report carrying the same
machine-readable error code; diagnostics do not contaminate the JSON stream.

| Exit | Meaning | Structured error codes |
|---:|---|---|
| `0` | The requested operation completed successfully. | None |
| `2` | The request or typed value is invalid, or the input format is unsupported. | `INVALID_INPUT`, `UNSUPPORTED_FORMAT` |
| `3` | A capability, required configuration, or authentication is unavailable. | `CAPABILITY_UNAVAILABLE`, `CONFIGURATION_INVALID`, `AUTHENTICATION_REQUIRED` |
| `4` | A configured resource ceiling was exceeded. | `RESOURCE_LIMIT_EXCEEDED` |
| `5` | Publishing would collide with an existing output or violate path policy. | `OUTPUT_EXISTS`, `PATH_POLICY_VIOLATION` |
| `6` | Processing or report emission failed. | `PROCESSING_FAILED` |
| `7` | A real multi-item operation produced both successes and failures. | `PARTIAL_FAILURE` |
| `8` | Work was cancelled or rollback could not restore the prior state. | `CANCELLED`, `ROLLBACK_FAILED` |

`batch.run` is currently unavailable, so exit `7` does not imply that batch
recipe execution exists. The mapping is reserved and tested for commands that
truthfully return a partial result. Signals, panics, and failures before the
rTools process starts are outside this application-level table.
