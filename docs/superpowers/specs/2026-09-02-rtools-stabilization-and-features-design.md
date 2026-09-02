# rTools Stabilization and Feature Program Design

Date: 2026-09-02
Status: Approved

## Purpose

Turn rTools from a promising prototype into a trustworthy local-first toolkit whose CLI, REST API, MCP server, and WASM bindings report the same capabilities and never claim success for work they did not perform.

The program fixes correctness, security, data-loss, configuration, and release-engineering defects before adding transactional pipelines, capability discovery, privacy verification, duplicate review, and asynchronous API jobs.

## Goals

- Make every advertised operation either functional or explicitly unavailable.
- Prevent path traversal, accidental overwrite, silent failure, and irreversible duplicate deletion.
- Give every interface the same validation, error, warning, capability, and result semantics.
- Make dry-run, batch processing, and destructive operations manifest-driven and recoverable.
- Enforce resource limits for uploads, decoded images, PDFs, batches, and processing time.
- Preserve or remove metadata according to explicit, verifiable policy.
- Establish reproducible builds, cross-platform CI, and meaningful tests for every interface.
- Add the approved feature set without replacing working processor implementations unnecessarily.

## Non-Goals

- Shipping large captioning or vision models as mandatory dependencies.
- Building a graphical desktop application.
- Providing permanent cloud storage or multi-tenant account management.
- Preserving source compatibility for behavior that is unsafe or falsely reports success.
- Implementing every aspirational item in `SPEC.md` as part of this program.

## Guiding Decisions

### Compatibility-first migration

Existing crates and public types remain in place where safe. New contracts are introduced centrally, then CLI, API, MCP, and WASM adapters migrate incrementally. Compatibility aliases may remain for one release, but ignored parameters and placeholder successes are removed immediately.

### Capability truth

Each operation has a stable identifier and one of three states:

- `Available`: functional in the current environment.
- `Unavailable`: cannot run, with a machine-readable reason and remediation.
- `Experimental`: functional but carrying explicit limitations.

The same registry drives `rtools doctor`, API readiness, MCP tool registration, and documentation verification. An unavailable AI or OCR provider disables its dependent operations instead of returning placeholder content.

### Safe outputs by default

Every writing operation uses an explicit output policy:

- `FailIfExists`
- `UniqueName`
- `Overwrite`
- `Transactional`

The default is `FailIfExists` for single outputs and `Transactional` for batches and filesystem mutations. Processors write to sibling temporary files, reopen and validate them, then atomically rename them into place when the platform supports it. Cross-filesystem commits fall back to a verified copy-and-replace procedure and emit a warning that atomicity was unavailable.

## Architecture

### `rtools-core`

`rtools-core` owns contracts shared by every processor and adapter:

- `CapabilityRegistry` and provider diagnostics.
- validated processor configuration behavior.
- `OutputPolicy` and safe output resolution.
- `OperationManifest`, manifest entries, execution state, and rollback metadata.
- structured error codes, warnings, and per-item failure records.
- resource-limit types for bytes, decoded pixels, pages, items, and duration.
- shared operation/result envelopes used by CLI JSON, HTTP JSON, and MCP content.

Every processor validates configuration and input before side effects. Validation is not an optional adapter responsibility.

### Processor crates

`rtools-image`, `rtools-pdf`, and `rtools-ai` remain the implementation layer. They do not parse CLI flags, HTTP multipart fields, or MCP requests.

Image processors apply EXIF orientation before geometry changes, preserve metadata only when explicitly requested and supported, verify metadata removal, warn before flattening animation, and enforce decoded-pixel limits.

PDF processors reject invalid ranges, report unsupported or signature-sensitive structures, and never label text extraction or file copying as OCR. Configuration fields that cannot affect an implementation are removed or rejected.

AI and OCR processors depend on provider traits. Initial lightweight providers may wrap configured local executables such as Tesseract. Large model providers remain optional. Provider absence is represented through the capability registry.

### Interface adapters

The adapters translate transport-specific requests into validated core operations and translate shared results back to their transport. They do not recreate validation or processing logic.

- CLI maps errors to stable non-zero exit codes and offers human and JSON output.
- REST runs operations as isolated jobs and serves authenticated results.
- MCP registers only available or explicitly experimental tools.
- WASM registers only bounded in-memory operations supported in browsers.

## Processing Lifecycle

Every operation follows this sequence:

1. Resolve the operation and verify capability availability.
2. Validate configuration, input type, path policy, and resource limits.
3. Construct an operation manifest containing all intended filesystem effects.
4. Return the manifest without side effects when dry-run is active.
5. Process into request-scoped temporary outputs.
6. Reopen and validate each generated artifact.
7. Commit outputs according to the selected output policy.
8. Record committed entries and rollback information.
9. Return outputs, statistics, warnings, and per-item failures.

An operation may return partial failure only when its contract explicitly permits it. The result identifies every successful and failed item; adapters must not convert partial or total failure into unconditional success.

## Destructive and Filesystem Operations

Rename, organize, duplicate handling, and batch recipes always begin with a manifest. Dry-run returns that manifest.

Duplicate detection separates analysis from action. Analysis groups candidates transitively and ranks the suggested keeper using file integrity, dimensions, metadata completeness, format quality, size, and a documented sharpness proxy. Actions move rejected candidates into a recoverable quarantine. Permanent deletion is a separate operation that requires the identifier of a previously committed quarantine manifest.

Ignored filesystem errors are prohibited. Move collisions, permission failures, missing sources, and rollback failures are preserved as structured per-item failures.

## REST API Design

Client filenames are display metadata only. The server generates storage names and verifies every resolved path remains inside its request or job directory.

Processing is job-based:

- create a job with files and operation parameters;
- query state and progress;
- cancel a queued or running job;
- download result artifacts through an authenticated endpoint;
- expire jobs and artifacts according to configured retention.

CPU-heavy and blocking filesystem work executes through bounded blocking workers. Each job has isolated input and output directories. No global output filename is shared between requests.

Authentication, TLS, CORS, upload limits, concurrency, and retention settings are enforced. Invalid or incomplete security configuration causes startup failure rather than silent fallback. Health indicates process liveness; readiness includes capability and provider status.

## CLI Design

The CLI provides:

- reliable exit codes;
- functional global `--dry-run`;
- `--output-format human|json`;
- `rtools doctor` capability and dependency diagnostics;
- manifest inspection, execution, and rollback commands;
- typed batch recipes and per-step results;
- shell-friendly progress that does not corrupt JSON output.

Invalid enum values, paths, ranges, or missing configuration files fail instead of falling back to unrelated defaults. `config show` displays the effective merged configuration with secrets redacted. Explicit configuration has higher precedence than discovered defaults.

## MCP Design

MCP schemas include numeric ranges, required values, supported enums, and operation limitations. Tool results return actual output paths or manifests, warnings, and statistics.

Destructive execution requires a confirmation token derived from a previously returned manifest. Merely requesting analysis never mutates files. Long-running tools report progress and honor cancellation when supported by the MCP transport.

## WASM Design

WASM remains an in-memory image toolkit. It enforces input-byte and decoded-pixel limits, validates crop and resize dimensions, and uses reliable format detection. Filesystem, PDF, and native-provider operations are omitted from its registry. Unsupported formats return errors instead of silently changing to PNG.

## Added Features

### Transactional batch recipes

A recipe is an ordered list of typed operations. The output of each step becomes the next step's input. The initial implementation is sequential and deterministic; parallel execution is introduced only for independent items after transactional behavior is proven.

Example: orient, resize, strip private metadata, encode WebP, verify, and emit a manifest.

### Privacy-safe export

Privacy export applies a declared metadata-removal policy, writes the output, reopens it, and returns a verification report listing removed and remaining sensitive fields. Image GPS/EXIF and PDF document metadata are covered initially.

### Quality-aware duplicate review

Duplicate analysis emits JSON and an optional local HTML report. The report explains grouping distance, keeper recommendation, and quality signals. Review does not mutate files.

### Capability diagnostics

`rtools doctor` and corresponding API/MCP diagnostics show build capabilities, configured providers, external executable versions, writable directories, resource limits, and actionable remediation.

## Error Model

Errors have a stable code, user-facing message, optional source details, operation identifier, and optional item path. Initial codes include:

- `INVALID_INPUT`
- `CAPABILITY_UNAVAILABLE`
- `UNSUPPORTED_FORMAT`
- `RESOURCE_LIMIT_EXCEEDED`
- `OUTPUT_EXISTS`
- `PATH_POLICY_VIOLATION`
- `PROCESSING_FAILED`
- `PARTIAL_FAILURE`
- `AUTHENTICATION_REQUIRED`
- `CONFIGURATION_INVALID`
- `CANCELLED`
- `ROLLBACK_FAILED`

CLI maps codes to documented exit statuses. REST maps them to appropriate HTTP statuses and a consistent JSON envelope. MCP marks tool failures as failures rather than successful text content. Internal causes are logged without leaking secrets or sensitive paths to remote clients.

## Testing Strategy

Implementation uses red-green-refactor. Required layers are:

- processor unit tests for validation, algorithms, metadata behavior, and output policies;
- filesystem integration tests for collisions, atomicity, rollback, Unicode paths, traversal, permissions, and partial failure;
- CLI integration tests for exit codes, dry-run, JSON output, and invalid arguments;
- REST tests for multipart parsing, authentication, path attacks, request isolation, concurrency, expiry, and cancellation;
- MCP contract tests for schemas, capability filtering, outputs, and destructive confirmation;
- PDF fixtures covering bookmarks, forms, annotations, encrypted inputs, ranges, and signature warnings;
- WASM browser tests for format detection, bounds, and memory limits.

Security, data-loss, race, silent-failure, and assumption-violation attacks are repeated at each milestone boundary.

## Delivery Milestones

### Milestone 1: Release safety

- Pin the supported Rust toolchain and repair lint/format gates.
- Track `Cargo.lock` and add Windows/Linux CI.
- Add core validation, structured errors, resource limits, and output policies.
- Remove placeholder successes and hide unavailable capabilities.
- Fix image metadata, orientation, output collision, and CLI truthfulness defects.
- Add core, image, and CLI regression tests.

### Milestone 2: REST API security and jobs

- Eliminate client-controlled storage paths.
- Enforce configured security and limits.
- Add isolated jobs, bounded workers, downloads, expiry, progress, and cancellation.
- Add REST integration and concurrency tests.

### Milestone 3: PDF correctness

- Make merge, split, compression, metadata, and text extraction behavior truthful and tested.
- Disable unavailable OCR, rendering, encryption, and redaction until providers exist.
- Add fixture-based structural and warning tests.

### Milestone 4: Provider-backed AI and OCR

- Add provider discovery and diagnostics.
- Implement configured local OCR first.
- Add optional captioning and classification providers without mandatory model downloads.
- Replace filename-based or constant placeholder results with provider results.

### Milestone 5: Transactional features

- Add manifests, rollback, quarantine, and duplicate ranking.
- Add typed batch recipes and privacy verification.
- Add capability-derived documentation checks and optional duplicate HTML reports.

## Release Gates

Every milestone must satisfy:

- `cargo test --workspace --all-targets` passes;
- `cargo fmt --all -- --check` passes;
- Clippy passes with warnings denied on the pinned toolchain;
- dependency and license policy checks pass;
- Windows and Linux CI pass, plus the WASM target where applicable;
- no registered operation returns placeholder success;
- documentation and capability registry agree;
- adversarial review finds no unresolved release-blocking security, data-loss, race, or silent-failure issue.

## Acceptance Criteria

The program is complete when all five milestones pass their release gates, every public interface exposes only truthful capabilities, destructive operations are manifest-driven and recoverable, API jobs isolate users and outputs, privacy removal is verified, and the added features operate through the same shared processor contracts.
