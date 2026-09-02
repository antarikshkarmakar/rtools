# Milestone 1 Release Safety Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task with review checkpoints.

**Goal:** Make the current rTools release honest and safe: reproducible builds, mandatory processor validation, stable errors, bounded inputs, collision-safe outputs, truthful capability reporting, corrected image behavior, and reliable CLI exit status.

**Architecture:** Add the safety contracts to `rtools-core`, then migrate the existing processor implementations without moving transport logic into them. The CLI becomes a thin adapter over those contracts. Features that do not have a real implementation are registered as unavailable and return a structured error; they never produce fabricated output or a zero exit status.

**Tech Stack:** Rust 1.95.0, Cargo workspace, `thiserror`, `serde`, `figment`, `image`, `clap`, standard-library integration tests, GitHub Actions, cargo-deny.

**Spec:** `docs/superpowers/specs/2026-09-02-rtools-stabilization-and-features-design.md`

## Global Constraints

- Preserve the user's pre-existing staged deletion of the malformed path under `crates/rtools-tests`; exclude it from every milestone commit.
- Run Cargo through WSL from `/mnt/c/GitHub/rTools` on this host.
- Use red-green-refactor for every behavioral change: add one focused failing test, observe the expected failure, implement the smallest correct change, rerun the focused test, then run the affected crate suite.
- Do not introduce silent fallback. Invalid values, unavailable providers, missing explicit configuration, collisions, and exceeded limits are errors.
- Do not advertise operations that lack a real implementation.
- Do not overwrite an existing destination unless `OutputPolicy::Overwrite` was explicitly selected.
- Keep the existing crate boundaries. CLI, REST, MCP, and WASM adapters must not duplicate processor validation.
- Commit after each task with `git commit --only` and the exact pathspecs listed in that task; never use `git add -A` or a plain `git commit` while the malformed staged deletion is present.

---

## Task 1: Establish a Reproducible Quality Baseline

**Files:**

- Create: `rust-toolchain.toml`
- Create: `.github/workflows/ci.yml`
- Create: `deny.toml`
- Modify: `.gitignore`
- Modify: `Cargo.toml`
- Modify: every `crates/*/Cargo.toml`
- Track: `Cargo.lock`
- Modify: all Rust files reported by `cargo fmt --all -- --check`

### Step 1: Pin the toolchain and repair workspace metadata

Create `rust-toolchain.toml`:

```toml
[toolchain]
channel = "1.95.0"
profile = "minimal"
components = ["clippy", "rustfmt"]
targets = ["wasm32-unknown-unknown"]
```

Add this to the top-level workspace manifest after the complete `[workspace]` table and before `[workspace.dependencies]`:

```toml
[workspace.package]
edition = "2021"
rust-version = "1.95"
repository = "https://github.com/antarikshkarmakar/rtools"
license = "MIT OR Apache-2.0"
```

In every publishable crate, inherit `edition`, `rust-version`, `repository`, and `license` with `field.workspace = true`, and correct the root README path to `../../README.md`. Preserve the existing dual-license declaration; add the standard Apache-2.0 text as `LICENSE-APACHE` and retain the existing MIT text as `LICENSE` rather than silently changing the project's declared license.

Change the Clippy group declarations in both `Cargo.toml` and `crates/rtools-tests/Cargo.toml` so explicit allows have higher priority:

```toml
all = { level = "warn", priority = -1 }
pedantic = { level = "warn", priority = -1 }
nursery = { level = "warn", priority = -1 }
cargo = { level = "warn", priority = -1 }
unwrap_used = "allow"
expect_used = "allow"
missing_docs_in_private_items = "allow"
```

Remove `Cargo.lock` from `.gitignore`, run `cargo generate-lockfile`, and stage the resulting lockfile explicitly.

### Step 2: Add dependency and license policy

Create `deny.toml` with an allow-list matching the licenses already declared by workspace crates:

```toml
[advisories]
version = 2
yanked = "deny"

[licenses]
version = 2
confidence-threshold = 0.8
allow = [
  "Apache-2.0",
  "BSD-2-Clause",
  "BSD-3-Clause",
  "ISC",
  "MIT",
  "Unicode-3.0",
  "Zlib",
]

[bans]
multiple-versions = "warn"
wildcards = "deny"
highlight = "all"

[sources]
unknown-registry = "deny"
unknown-git = "deny"
allow-registry = ["https://github.com/rust-lang/crates.io-index"]
```

Run `cargo deny check`. If a transitive package uses another recognized permissive license, inspect its SPDX expression and add only that exact license identifier; do not use a catch-all.

### Step 3: Add Windows/Linux CI

Create `.github/workflows/ci.yml`:

```yaml
name: CI

on:
  push:
  pull_request:

permissions:
  contents: read

jobs:
  quality:
    strategy:
      fail-fast: false
      matrix:
        os: [ubuntu-latest, windows-latest]
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4
      - name: Install pinned Rust toolchain
        run: rustup show active-toolchain
      - name: Format
        run: cargo fmt --all -- --check
      - name: Clippy
        run: cargo clippy --workspace --all-targets --locked -- -D warnings
      - name: Test
        run: cargo test --workspace --all-targets --locked

  policy:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: EmbarkStudios/cargo-deny-action@v2
        with:
          command: check

  wasm:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: cargo check -p rtools-wasm --target wasm32-unknown-unknown --locked
```

### Step 4: Format and clear the baseline warnings

Run:

```bash
cargo fmt --all
cargo clippy --workspace --all-targets --locked -- -D warnings
```

Fix each diagnostic without adding crate-wide lint suppression. For the two known unused imports, remove the imports rather than allowing them. If the pinned Clippy still crashes, reduce the invocation to the crate that triggers it, record the minimal reproducer in the commit message body, and keep all non-crashing crates warning-free before proceeding.

### Step 5: Verify the baseline

Run:

```bash
cargo fmt --all -- --check
cargo test --workspace --all-targets --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo check -p rtools-wasm --target wasm32-unknown-unknown --locked
cargo deny check
```

Expected: every command exits zero. No test count may decrease from the current 15.

### Step 6: Commit

```bash
git add rust-toolchain.toml .github/workflows/ci.yml deny.toml Cargo.lock LICENSE-APACHE
git commit --only -m "build: establish reproducible release gates" -- rust-toolchain.toml .github/workflows/ci.yml deny.toml .gitignore Cargo.toml Cargo.lock LICENSE-APACHE ':(glob)crates/*/Cargo.toml' ':(glob)crates/**/*.rs'
```

---

## Task 2: Make Validation Mandatory and Add Stable Error Codes

**Files:**

- Modify: `crates/rtools-core/src/error.rs`
- Modify: `crates/rtools-core/src/traits.rs`
- Modify: `crates/rtools-core/src/lib.rs`
- Create: `crates/rtools-core/tests/processor_contract.rs`
- Modify: every `impl Processor` in `crates/rtools-image/src`, `crates/rtools-pdf/src`, and `crates/rtools-ai/src`

### Step 1: Write the failing processor contract test

Create `crates/rtools-core/tests/processor_contract.rs`:

```rust
use rtools_core::{Processor, RToolsError, RToolsResult};
use std::sync::atomic::{AtomicBool, Ordering};

struct RejectingProcessor {
    ran: AtomicBool,
}

impl Processor for RejectingProcessor {
    type Input = ();
    type Output = ();
    type Config = ();
    type Error = RToolsError;

    fn process_validated(&self, _input: (), _config: ()) -> RToolsResult<()> {
        self.ran.store(true, Ordering::SeqCst);
        Ok(())
    }

    fn validate_config(&self, _config: &()) -> RToolsResult<()> {
        Err(RToolsError::configuration_invalid("rejected by test"))
    }

    fn name(&self) -> &str {
        "RejectingProcessor"
    }
}

#[test]
fn process_never_runs_when_validation_fails() {
    let processor = RejectingProcessor {
        ran: AtomicBool::new(false),
    };

    let error = processor.process((), ()).unwrap_err();

    assert_eq!(error.code().as_str(), "CONFIGURATION_INVALID");
    assert!(!processor.ran.load(Ordering::SeqCst));
}
```

Run `cargo test -p rtools-core --test processor_contract`. Expected: compile failure because `process_validated`, `configuration_invalid`, and `code` do not exist.

### Step 2: Introduce stable codes without breaking existing variants

Add a serializable `ErrorCode` enum to `error.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCode {
    InvalidInput,
    CapabilityUnavailable,
    UnsupportedFormat,
    ResourceLimitExceeded,
    OutputExists,
    PathPolicyViolation,
    ProcessingFailed,
    PartialFailure,
    AuthenticationRequired,
    ConfigurationInvalid,
    Cancelled,
    RollbackFailed,
}

impl ErrorCode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InvalidInput => "INVALID_INPUT",
            Self::CapabilityUnavailable => "CAPABILITY_UNAVAILABLE",
            Self::UnsupportedFormat => "UNSUPPORTED_FORMAT",
            Self::ResourceLimitExceeded => "RESOURCE_LIMIT_EXCEEDED",
            Self::OutputExists => "OUTPUT_EXISTS",
            Self::PathPolicyViolation => "PATH_POLICY_VIOLATION",
            Self::ProcessingFailed => "PROCESSING_FAILED",
            Self::PartialFailure => "PARTIAL_FAILURE",
            Self::AuthenticationRequired => "AUTHENTICATION_REQUIRED",
            Self::ConfigurationInvalid => "CONFIGURATION_INVALID",
            Self::Cancelled => "CANCELLED",
            Self::RollbackFailed => "ROLLBACK_FAILED",
        }
    }
}
```

Add explicit variants for capability, output collision, path policy, resource limit, and configuration invalid. Implement `RToolsError::code()` as an exhaustive match. Map legacy variants to the closest stable code so adapters can migrate incrementally.

### Step 3: Use a template method for validation

Change `Processor` to:

```rust
pub trait Processor: Send + Sync {
    type Input: Send + Sync;
    type Output: Send + Sync;
    type Config: Send + Sync + Clone;
    type Error: Into<RToolsError> + std::fmt::Display;

    fn process(&self, input: Self::Input, config: Self::Config) -> RToolsResult<Self::Output> {
        self.validate_config(&config)?;
        self.process_validated(input, config)
    }

    fn process_validated(
        &self,
        input: Self::Input,
        config: Self::Config,
    ) -> RToolsResult<Self::Output>;

    fn validate_config(&self, config: &Self::Config) -> RToolsResult<()>;

    fn estimate_output_size(
        &self,
        _input: &Self::Input,
        _config: &Self::Config,
    ) -> Option<u64> {
        None
    }

    fn name(&self) -> &str;
}
```

Rename each processor implementation's `fn process` to `fn process_validated`. Do not otherwise alter processor behavior in this step.

### Step 4: Verify validation and all processor migrations

Run:

```bash
cargo test -p rtools-core --test processor_contract
cargo test --workspace --all-targets --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
```

Expected: the contract test passes and every processor compiles through the mandatory validation path.

### Step 5: Commit

```bash
git commit --only -m "feat(core): enforce validation and stable error codes" -- crates/rtools-core crates/rtools-image/src crates/rtools-pdf/src crates/rtools-ai/src
```

---

## Task 3: Enforce Resource Limits Before Decode or Processing

**Files:**

- Create: `crates/rtools-core/src/limits.rs`
- Modify: `crates/rtools-core/src/lib.rs`
- Modify: `crates/rtools-core/src/config.rs`
- Modify: `crates/rtools-image/src/format.rs`
- Modify: image processors that decode inputs
- Modify: `crates/rtools-cli/src/commands/image.rs`
- Create: `crates/rtools-core/tests/resource_limits.rs`
- Add tests: `crates/rtools-tests/tests/image_tests.rs`

### Step 1: Write failing limit tests

Create `crates/rtools-core/tests/resource_limits.rs`:

```rust
use rtools_core::{ResourceLimits, RToolsError};

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
```

Run `cargo test -p rtools-core --test resource_limits`. Expected: compile failure because `ResourceLimits` does not exist.

### Step 2: Add checked limit types

Implement `ResourceLimits` with `max_input_bytes`, `max_decoded_pixels`, `max_pdf_pages`, `max_batch_items`, and `max_duration_ms`. Each check must use checked arithmetic and return `ResourceLimitExceeded { resource, actual, limit }`.

Use these defaults:

```rust
impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            max_input_bytes: 100 * 1024 * 1024,
            max_decoded_pixels: 100_000_000,
            max_pdf_pages: 2_000,
            max_batch_items: 10_000,
            max_duration_ms: 300_000,
        }
    }
}
```

Add `limits: ResourceLimits` to `AppConfig` with `#[serde(default)]`. Add `limits: ResourceLimits` to each image processor configuration that decodes an image, also with a default. In `commands/image.rs`, clone `AppConfig::limits` into the operation configuration before calling the processor. This makes configured limits effective instead of leaving them as dead configuration.

### Step 3: Validate image headers before full decode

Add a shared image loader that opens the reader, applies guessed format, reads dimensions, checks decoded pixels, then decodes:

```rust
pub fn decode_bounded(path: &Path, limits: &ResourceLimits) -> RToolsResult<DynamicImage> {
    let bytes = fs::metadata(path)?.len();
    limits.check_input_bytes(bytes)?;

    let reader = ImageReader::open(path)?.with_guessed_format()?;
    let (width, height) = reader.into_dimensions()?;
    limits.check_decoded_pixels(width, height)?;

    let reader = ImageReader::open(path)?.with_guessed_format()?;
    reader.decode().map_err(|error| RToolsError::image(error.to_string()))
}
```

Pass configured limits to every image processor and replace direct `image::open` calls with `decode_bounded`. Add a generated header-only fixture test whose claimed dimensions exceed the limit; assert that the output path is not created.

### Step 4: Verify

Run:

```bash
cargo test -p rtools-core --test resource_limits
cargo test -p rtools-tests --test image_tests
cargo test --workspace --all-targets --locked
```

### Step 5: Commit

```bash
git commit --only -m "feat(core): enforce processing resource limits" -- crates/rtools-core crates/rtools-image crates/rtools-cli/src/commands/image.rs crates/rtools-tests/tests/image_tests.rs
```

---

## Task 4: Add Explicit Collision Policies and Atomic File Commit

**Files:**

- Modify: `crates/rtools-core/src/output.rs`
- Modify: `crates/rtools-core/src/lib.rs`
- Create: `crates/rtools-core/tests/output_policy.rs`
- Modify: `crates/rtools-image/src/compress.rs`
- Modify: `crates/rtools-image/src/convert.rs`
- Modify: `crates/rtools-image/src/resize.rs`
- Modify: `crates/rtools-image/src/crop.rs`
- Modify: `crates/rtools-image/src/filter.rs`
- Modify: `crates/rtools-image/src/watermark.rs`
- Add tests: `crates/rtools-tests/tests/image_tests.rs`

### Step 1: Write failing collision and cleanup tests

Create `crates/rtools-core/tests/output_policy.rs` with these cases:

```rust
#[test]
fn fail_if_exists_never_changes_existing_file() {
    let directory = tempfile::tempdir().unwrap();
    let output = directory.path().join("result.bin");
    std::fs::write(&output, b"original").unwrap();

    let error = PendingOutput::new(&output, OutputPolicy::FailIfExists).unwrap_err();

    assert_eq!(error.code(), ErrorCode::OutputExists);
    assert_eq!(std::fs::read(&output).unwrap(), b"original");
}

#[test]
fn failed_write_removes_sibling_temporary_file() {
    let directory = tempfile::tempdir().unwrap();
    let output = directory.path().join("result.bin");

    let pending = PendingOutput::new(&output, OutputPolicy::FailIfExists).unwrap();
    let temporary = pending.temporary_path().to_owned();
    drop(pending);

    assert!(!temporary.exists());
    assert!(!output.exists());
}
```

Also cover `UniqueName` suffix exhaustion, explicit `Overwrite`, a missing parent, Unicode filenames, and concurrent reservations of the same destination.

Run `cargo test -p rtools-core --test output_policy`. Expected: compile failure because `OutputPolicy` and `PendingOutput` do not exist.

### Step 2: Implement output reservation and commit

Add:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputPolicy {
    FailIfExists,
    UniqueName,
    Overwrite,
}

pub struct PendingOutput {
    final_path: PathBuf,
    temporary_path: PathBuf,
    committed: bool,
}
```

`PendingOutput::new` must:

- reject a missing or non-directory parent;
- resolve `UniqueName` without ever falling back to an existing path;
- reserve a sibling lock path using `OpenOptions::create_new(true)` so two rTools writers cannot select the same final path;
- reserve a sibling temporary path using a second `OpenOptions::create_new(true)` call;
- keep the final path private until commit.

The reservation lock stores a random operation identifier and is removed only by the owning `PendingOutput`. `commit` must reopen and validate the produced artifact through a caller-supplied closure, flush and `sync_all`, recheck the destination state while the lock is held, then rename into place. For explicit overwrite on Windows, move the old file to a sibling backup, rename the new file, remove the backup after success, and restore the backup if commit fails. `Drop` removes both an uncommitted temporary file and its owned reservation. Tests must cover a stale reservation and prove that it fails closed rather than deleting another process's lock.

### Step 3: Migrate image writers

Add `output_policy: OutputPolicy` to each writing image configuration with a safe default of `FailIfExists`. Encode to `PendingOutput::temporary_path()`, reopen with `image::ImageReader` as validation, then commit. Delete the per-processor `unique_output_path` helpers, including the current 999-attempt overwrite fallback.

### Step 4: Verify the artifact behavior

Run:

```bash
cargo test -p rtools-core --test output_policy
cargo test -p rtools-tests --test image_tests
cargo test --workspace --all-targets --locked
```

Expected: collisions preserve the original bytes, failed encodes leave neither final nor temporary artifacts, and Unicode output names succeed.

### Step 5: Commit

```bash
git commit --only -m "feat(core): add collision-safe atomic outputs" -- crates/rtools-core crates/rtools-image crates/rtools-tests/tests/image_tests.rs
```

---

## Task 5: Make Configuration Loading Deterministic and Side-Effect Free

**Files:**

- Modify: `crates/rtools-core/src/config.rs`
- Create: `crates/rtools-core/tests/config_loading.rs`
- Modify: `crates/rtools-cli/src/commands/config.rs`

### Step 1: Write failing precedence and missing-file tests

Create `crates/rtools-core/tests/config_loading.rs`:

```rust
#[test]
fn missing_explicit_config_is_an_error() {
    let missing = tempfile::tempdir().unwrap().path().join("missing.toml");
    let error = AppConfig::load(Some(&missing)).unwrap_err();
    assert_eq!(error.code(), ErrorCode::ConfigurationInvalid);
}

#[test]
fn explicit_config_has_higher_precedence_than_discovered_config() {
    let sandbox = tempfile::tempdir().unwrap();
    let discovered = sandbox.path().join("rtools.toml");
    let explicit = sandbox.path().join("explicit.toml");
    std::fs::write(&discovered, "[general]\nlog_level = \"warn\"\n").unwrap();
    std::fs::write(&explicit, "[general]\nlog_level = \"debug\"\n").unwrap();
    let locations = ConfigLocations {
        system: None,
        user: None,
        project: Some(discovered),
    };

    let config = AppConfig::load_from_locations(Some(&explicit), &locations).unwrap();

    assert_eq!(config.general.log_level, "debug");
}
```

The test helper must point discovery at temporary directories through a new internal `ConfigLocations` value; it must not mutate the developer's real home directory.

Run `cargo test -p rtools-core --test config_loading`. Expected: the missing-file assertion fails because the current loader silently continues.

### Step 2: Refactor loading order

Use this precedence, from lowest to highest:

1. `AppConfig::default()`
2. system config
3. user config
4. project config
5. explicitly supplied config
6. `RTOOLS_` environment variables with `__` nesting

Return `CONFIGURATION_INVALID` when an explicit path is missing, unreadable, or invalid. Call `AppConfig::validate()` before returning. Loading must not create the configured temporary directory; directory creation belongs to application startup immediately before processing.

### Step 3: Make config commands truthful

Change `config validate` to call the same loader and semantic validation used at runtime. Change `config show` to serialize the effective merged configuration and redact secrets. A missing explicit file must print an error and produce a non-zero exit status through the CLI error mapper added in Task 7.

### Step 4: Verify

Run:

```bash
cargo test -p rtools-core --test config_loading
cargo test -p rtools-core
cargo test --workspace --all-targets --locked
```

### Step 5: Commit

```bash
git commit --only -m "fix(config): enforce deterministic validated loading" -- crates/rtools-core/src/config.rs crates/rtools-core/tests/config_loading.rs crates/rtools-cli/src/commands/config.rs
```

---

## Task 6: Replace Fabricated Success with a Shared Capability Registry

**Files:**

- Create: `crates/rtools-core/src/capability.rs`
- Modify: `crates/rtools-core/src/lib.rs`
- Create: `crates/rtools-core/tests/capability_registry.rs`
- Modify: `crates/rtools-ai/src/alt_text.rs`
- Modify: `crates/rtools-ai/src/ocr.rs`
- Modify: `crates/rtools-image/src/pdf2img.rs`
- Modify: `crates/rtools-pdf/src/ocr.rs`
- Modify: `crates/rtools-cli/src/commands/batch.rs`
- Create: `crates/rtools-cli/src/capabilities.rs`
- Modify: `crates/rtools-cli/src/main.rs`

### Step 1: Write failing registry tests

Create `crates/rtools-core/tests/capability_registry.rs`:

```rust
#[test]
fn unavailable_capability_carries_machine_readable_remediation() {
    let capability = Capability::unavailable(
        "image.ocr",
        "No OCR provider is configured",
        "Configure a supported OCR provider",
    );

    assert_eq!(capability.state, CapabilityState::Unavailable);
    assert_eq!(capability.operation_id, "image.ocr");
    assert!(!capability.remediation.unwrap().is_empty());
}

#[test]
fn duplicate_operation_ids_are_rejected() {
    let mut registry = CapabilityRegistry::default();
    registry.register(Capability::available("image.resize")).unwrap();
    assert!(registry
        .register(Capability::available("image.resize"))
        .is_err());
}
```

Run `cargo test -p rtools-core --test capability_registry`. Expected: compile failure because the registry types do not exist.

### Step 2: Implement capability contracts

Add serializable `CapabilityState::{Available, Unavailable, Experimental}`, `Capability`, `ProviderDiagnostic`, and `CapabilityRegistry`. Operation IDs are lowercase dotted strings and are validated at registration. `require_available(id)` returns `CAPABILITY_UNAVAILABLE` with reason and remediation.

### Step 3: Register current truth

Create `rtools-cli/src/capabilities.rs` that registers working image operations as available, partially proven PDF operations as experimental, and these current fabricated-success paths as unavailable:

- `image.ocr`
- `image.watermark.text`
- `image.metadata.preserve`
- `image.metadata.strip_gps`
- `pdf.to_image`
- `pdf.ocr`
- `ai.alt_text`
- `ai.ocr`
- `batch.run`

Register `image.watermark.image` as available while keeping the text variant separate. Change the underlying processors to return `RToolsError::capability_unavailable` rather than mock content, copied input, or an empty successful result. Keep the command in help for one release but label it unavailable and point users to `rtools doctor`; removal from MCP/API registration is handled in their milestones.

### Step 4: Verify no fabricated output remains

Add tests that assert:

- AI alt text never returns a filename-derived caption;
- image OCR never returns sample text or a made-up confidence score;
- searchable PDF OCR never copies the source and reports success;
- PDF-to-image never returns an empty successful vector;
- batch never exits successfully without executing declared steps.

Run:

```bash
cargo test -p rtools-core --test capability_registry
cargo test -p rtools-ai
cargo test -p rtools-image
cargo test -p rtools-pdf
cargo test --workspace --all-targets --locked
```

### Step 5: Commit

```bash
git commit --only -m "fix: replace fabricated results with capability errors" -- crates/rtools-core crates/rtools-ai crates/rtools-image/src/pdf2img.rs crates/rtools-image/src/watermark.rs crates/rtools-pdf/src/ocr.rs crates/rtools-cli/src/capabilities.rs crates/rtools-cli/src/main.rs crates/rtools-cli/src/commands/batch.rs
```

---

## Task 7: Correct Image Metadata, Orientation, and Watermark Semantics

**Files:**

- Modify: `crates/rtools-image/src/exif.rs`
- Modify: `crates/rtools-image/src/format.rs`
- Modify: `crates/rtools-image/src/compress.rs`
- Modify: `crates/rtools-image/src/convert.rs`
- Modify: `crates/rtools-image/src/resize.rs`
- Modify: `crates/rtools-image/src/crop.rs`
- Modify: `crates/rtools-image/src/watermark.rs`
- Modify: `crates/rtools-image/src/metadata.rs`
- Add tests: `crates/rtools-tests/tests/image_tests.rs`
- Add fixtures: `crates/rtools-tests/fixtures/images/*.b64`

### Step 1: Add regression fixtures and failing tests

Use an in-memory colored pixel grid to test the transform mapping for EXIF orientations 2 through 8. Add small base64 text fixtures for one orientation-tagged JPEG, one GPS-tagged JPEG, and a two-frame GIF; tests decode them into the temporary test directory. Tests must assert pixel placement after orientation, explicit metadata outcomes, and rejection of implicit animation flattening.

Add these behavioral tests:

```rust
#[test]
fn missing_watermark_image_is_an_error() {
    let config = WatermarkConfig {
        watermark: WatermarkType::Image {
            image_path: PathBuf::from("missing-logo.png"),
            scale: 1.0,
        },
        ..WatermarkConfig::default()
    };

    let error = WatermarkProcessor::new().process(input_fixture(), config).unwrap_err();
    assert_eq!(error.code(), ErrorCode::InvalidInput);
}

#[test]
fn preserve_metadata_is_rejected_before_output_creation() {
    let config = CompressConfig {
        preserve_metadata: true,
        format: Some(ImageFormat::Bmp),
        ..CompressConfig::default()
    };

    assert_eq!(
        CompressProcessor::new().process(input_fixture(), config).unwrap_err().code(),
        ErrorCode::CapabilityUnavailable
    );
}
```

Run `cargo test -p rtools-tests --test image_tests`. Expected: orientation and missing-watermark cases fail; current metadata flags do not produce a verifiable contract.

### Step 2: Apply orientation before geometry

Parse EXIF orientation once, map values 2 through 8 to the corresponding flip/rotation, and apply it immediately after bounded decode. Resize and crop dimensions must be calculated from the oriented image. Record whether orientation was applied in output warnings/statistics.

### Step 3: Make metadata behavior explicit

Use three mutually exclusive internal policies: `DropAll`, `Preserve`, and `StripGps`. Milestone 1 supports `DropAll`: reopen the encoded output through the metadata reader and verify that EXIF and GPS fields are absent before commit. `Preserve` and `StripGps` return `CAPABILITY_UNAVAILABLE` before output reservation until the verified privacy exporter in Milestone 5 supplies a read/write metadata backend. This is a deliberate truthful restriction, not a silent fallback. The current boolean combinations map to exactly one policy, and the conflicting `preserve_metadata && strip_gps` combination is invalid input.

Change `CompressConfig::default().preserve_metadata` to `false`, matching the only supported safe policy. The CLI's `--preserve-metadata` and `--strip-gps` flags remain visible for compatibility but return the capability error before writing; `rtools doctor` reports both metadata sub-capabilities as unavailable.

`ExifProcessor` remains read-only; reject `remove_gps`, `remove_all`, or an output path instead of ignoring mutation settings. Do not claim animation preservation: a multi-frame input passed to a single-frame operation returns `CAPABILITY_UNAVAILABLE` before output reservation.

### Step 4: Fix watermark truthfulness

- A missing watermark image is `INVALID_INPUT`.
- Unsupported watermark image formats are `UNSUPPORTED_FORMAT`.
- Text watermarking returns `CAPABILITY_UNAVAILABLE` until a real font-rendering backend is selected; delete the white-rectangle implementation and do not create output.
- Image watermark validation requires an existing, decodable watermark image.
- Out-of-bounds placement and opacity outside `0.0..=1.0` fail before output reservation.

### Step 5: Verify

Run:

```bash
cargo test -p rtools-tests --test image_tests
cargo test -p rtools-image
cargo test --workspace --all-targets --locked
```

Inspect generated fixture outputs for correct orientation and verify EXIF/GPS fields through the crate's metadata reader, not by file-size comparison.

### Step 6: Commit

```bash
git add crates/rtools-tests/fixtures/images/*.b64
git commit --only -m "fix(image): enforce orientation and metadata contracts" -- crates/rtools-image crates/rtools-tests/tests/image_tests.rs crates/rtools-tests/fixtures/images
```

---

## Task 8: Make CLI Output, Dry-Run, Parsing, and Exit Status Truthful

**Files:**

- Create: `crates/rtools-cli/src/report.rs`
- Create: `crates/rtools-cli/src/exit.rs`
- Modify: `crates/rtools-cli/src/main.rs`
- Modify: `crates/rtools-cli/src/commands/image.rs`
- Modify: `crates/rtools-cli/src/commands/pdf.rs`
- Modify: `crates/rtools-cli/src/commands/ai.rs`
- Modify: `crates/rtools-cli/src/commands/config.rs`
- Modify: `crates/rtools-cli/src/commands/batch.rs`
- Create: `crates/rtools-cli/tests/cli_contract.rs`

### Step 1: Write process-level failing tests

Use `std::process::Command` and `env!("CARGO_BIN_EXE_rtools")`; add no test-only CLI dependency.

```rust
#[test]
fn unavailable_pdf_text_returns_nonzero() {
    let output = command()
        .args(["pdf", "text", "--input", "missing.pdf"])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(!String::from_utf8_lossy(&output.stdout).contains('✓'));
}

#[test]
fn missing_explicit_config_returns_configuration_exit_code() {
    let output = command()
        .args(["--config", "definitely-missing.toml", "doctor"])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(3));
}

#[test]
fn malformed_crop_region_is_rejected() {
    let output = command()
        .args(["image", "crop", "--input", "photo.png", "--region", "x,2,3,4"])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(2));
}
```

Also test JSON output parses as one JSON object, batch partial failure exits non-zero, unsupported global dry-run exits non-zero without creating output, and `doctor --output-format json` matches the registry.

Run `cargo test -p rtools-cli --test cli_contract`. Expected: current PDF and config commands exit zero and malformed crop values silently fall back.

### Step 2: Add a single result envelope and output mode

Add `--output-format human|json` as a global clap value enum. Define:

```rust
#[derive(Debug, Serialize)]
pub struct CliReport<T> {
    pub operation_id: String,
    pub status: ReportStatus,
    pub result: Option<T>,
    pub warnings: Vec<String>,
    pub failures: Vec<ItemFailure>,
}
```

Only `report.rs` may write command results to stdout. Diagnostics and progress go to stderr and are disabled or JSON-safe when JSON mode is active.

### Step 3: Map errors to stable process exit codes

Use this documented mapping:

- `0`: success
- `2`: invalid input or unsupported format
- `3`: invalid configuration or unavailable capability
- `4`: resource limit or timeout
- `5`: output collision or path-policy violation
- `6`: processing failure
- `7`: partial failure
- `8`: cancelled or rollback failure

Make `main` return `ExitCode`, call an async `run(cli) -> RToolsResult<CliReport<_>>`, and render an error envelope before returning the mapped non-zero code. Remove every `println!("Error ...")` followed by `Ok(())`.

### Step 4: Replace permissive string parsing

Parse crop region, aspect ratio, gravity, PDF compression level, organization strategy, duplicate action, formats, and output mode through clap value parsers or typed `FromStr` implementations. Reject invalid input; do not call `unwrap_or_default` or substitute zero values.

### Step 5: Make dry-run honest

For existing filesystem-mutating AI rename and date organization, dry-run returns the exact planned source/destination pairs and creates no directories. For image/PDF output commands that do not yet emit a full manifest, return `CAPABILITY_UNAVAILABLE` when global `--dry-run` is supplied. The manifest-backed universal implementation is delivered in Milestone 5.

### Step 6: Add doctor

Add `rtools doctor` and render each capability's state, reason, remediation, relevant configured limits, and writable-directory checks. Human output may use symbols; JSON output must contain only the serialized report.

### Step 7: Verify the original reproductions and the contract suite

Run:

```bash
cargo test -p rtools-cli --test cli_contract
cargo run -q -p rtools-cli -- pdf text --input missing.pdf
test $? -ne 0
cargo run -q -p rtools-cli -- config validate --config /definitely/missing.toml
test $? -ne 0
cargo run -q -p rtools-cli -- --output-format json doctor | jq -e '.status'
cargo test --workspace --all-targets --locked
```

Expected: unavailable operations and missing configuration exit non-zero, no success glyph is printed for failure, and JSON is parseable without progress text.

### Step 8: Commit

```bash
git commit --only -m "fix(cli): report truthful results and exit codes" -- crates/rtools-cli
```

---

## Task 9: Close Milestone 1 with Adversarial Regression Gates

**Files:**

- Create: `docs/operations/capabilities.md`
- Create: `docs/operations/exit-codes.md`
- Modify: `README.md`
- Modify: `SPEC.md`
- Create: `scripts/verify-capabilities.ps1`
- Create: `scripts/verify-capabilities.sh`
- Modify: `.github/workflows/ci.yml`

### Step 1: Generate capability documentation from runtime truth

Have `rtools doctor --output-format json` produce deterministic, sorted capability JSON. Both verification scripts compare operation IDs and states in generated output with the checked-in capability table. The scripts fail on extra, missing, or differently classified operations.

### Step 2: Document safe defaults and exit codes

Update README and SPEC examples so unavailable OCR/AI/PDF rendering/batch operations are clearly marked unavailable. Document output collision defaults, explicit overwrite, metadata policies, resource limits, JSON reports, and the Task 8 exit-code table.

### Step 3: Run adversarial cases

Add or confirm tests for:

- a symlinked output parent escaping the selected directory;
- Unicode and reserved Windows filename handling;
- concurrent writers selecting the same output;
- truncated, malformed, and decompression-bomb image inputs;
- EXIF orientations 2 through 8;
- verified drop-all metadata output plus fail-before-write behavior for unavailable preserve and GPS-only policies;
- missing watermark resources;
- a read-only output directory;
- batch partial failure;
- missing explicit config;
- invalid enum and numeric values;
- every registered unavailable capability returning a non-zero error.

Any failing adversarial case is fixed in the owning crate with a focused regression test before continuing.

### Step 4: Add documentation verification to CI

Run `scripts/verify-capabilities.sh` in the Ubuntu job and `scripts/verify-capabilities.ps1` in the Windows job after tests.

### Step 5: Run the complete release gate

Run from WSL:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --all-targets --locked
cargo check -p rtools-wasm --target wasm32-unknown-unknown --locked
cargo deny check
bash scripts/verify-capabilities.sh
git diff --check
git status --short
```

Run from PowerShell:

```powershell
pwsh -NoProfile -File scripts/verify-capabilities.ps1
```

Expected: all gates pass. `git status --short` shows only the user's pre-existing staged deletion before the final documentation commit.

### Step 6: Commit

```bash
git add docs/operations scripts
git commit --only -m "docs: close milestone one release gates" -- README.md SPEC.md docs/operations scripts .github/workflows/ci.yml
```

### Step 7: Final milestone review

Compare the resulting diff to the approved design's Milestone 1 bullets. Confirm all of these before declaring the milestone complete:

- pinned toolchain and lockfile;
- Windows/Linux CI and WASM check;
- format, test, Clippy, dependency, and license gates;
- mandatory validation and stable error codes;
- byte/pixel/page/item/time limit types and image enforcement;
- collision-safe output reservation and commit;
- deterministic configuration precedence;
- no fabricated success paths;
- capability-driven doctor output;
- corrected orientation, metadata, watermark, and animation behavior;
- non-zero CLI errors, strict parsing, truthful dry-run, and parseable JSON;
- adversarial tests for security, data loss, races, and silent failure.

If any item is absent, reopen its owning task and add a failing regression test before changing implementation.
