# Task 8 implementation report

## Interrupted implementer handoff

- Base: `92fade7fb177fd12e50667a5090923aa889d4e66`.
- The first implementer stopped because its external model usage allowance was exhausted; no Task 8 commit exists.
- Baseline before edits: `cargo test --workspace --all-targets --locked` passed 158 tests.
- Test-first CLI run: 11 process tests ran; 10 failed for the intended missing contracts and the existing batch-nonzero test passed.
- Supporting AI dry-run tests were written before production changes. The first implementer reported all 3 focused tests green: date organization creates no directories, resolves existing destination collisions, and rename plans distinct destinations.
- Current working tree intentionally contains the uncommitted Task 8 partial implementation. Preserve and finish it; do not reset or restart.

### Current diagnostic checkpoint

`cargo check -p rtools-cli --locked --message-format short` currently fails with 19 compile errors and one unused `tracing` dependency warning. The errors are integration seams from converting command arguments and handlers: typed image arguments are still consumed as strings; typed PDF arguments are still consumed as strings; `main::run` expects `RToolsError` while old handlers return `anyhow::Error`; config dispatch has an argument-count mismatch; generate-completions still returns `()`; and `capabilities.rs` does not yet cover `Commands::Doctor`.

This is an incomplete refactor state, not a reviewed or accepted design. Continue under RED-GREEN-REFACTOR, add missing focused tests before each remaining production behavior, and replace or simplify partial code where needed.

## Completion evidence (resumed implementation)

### RED-GREEN record

- The preserved process-level RED run exercised 11 CLI contracts: 10 failed for the
  missing result-envelope, exit-status, parser, dry-run, and doctor contracts; the
  existing nonzero batch contract passed. This was the test-first evidence for the
  principal Task 8 behavior.
- Added `file_output_warnings_are_exposed_in_command_report` before the completed
  command-result adapter. Its first invocation was blocked by the handoff's 19
  incomplete-boundary compile errors (and a Rust diagnostic ICE while reporting a
  dead-code warning); after completing the typed boundary it passes and proves that
  `FileOutput.warnings` survive both nested output serialization and top-level report
  warnings.
- Added direct `PARTIAL_FAILURE -> 7` and partial-report-status tests before the
  matching report/exit completion. They pass with the final implementation. There is
  intentionally no fabricated executable batch path: `batch.run` remains unavailable
  and process contracts prove its result is nonzero.
- The focused AI dry-run tests were written before the inherited AI production edits;
  the final run is 3/3: no date-organize directories, existing destination collision
  handling, and distinct in-run rename reservations.

### Design decisions completed

- Command handlers return typed `CommandResult` values internally. The sole CLI
  boundary converts those results to `CliReport<serde_json::Value>` and retains
  `RToolsError` error variants end-to-end; serialization failures become the typed
  `Internal` variant rather than lossy `anyhow` string conversions.
- `report.rs` owns command-result stdout. JSON uses exactly one serialized envelope
  plus its newline; errors and human diagnostics go to stderr. The narrowly necessary
  raw completion-generator stream is isolated and documented in `report.rs`.
- `capabilities.rs` remains the single registry source for command availability and
  doctor output. `doctor.report` is registered; `batch.run` stays unavailable.
- Global dry-run permits only deterministic AI rename and date organization. Their
  processors produce exact source/destination manifests, reserve planned destinations
  with collision checks, and do not create date-organize directories in dry-run.
- Typed clap parsers/config adapters replace permissive string fallbacks for image,
  PDF, AI, and output-mode values. All command paths return structured results and
  preserve processor warnings.

### Changed files

- `crates/rtools-cli/src/main.rs`, `capabilities.rs`, `exit.rs`, `report.rs`
- `crates/rtools-cli/src/commands/{mod,ai,batch,config,image,pdf}.rs`
- `crates/rtools-cli/tests/{cli_contract,exif_json}.rs`
- `crates/rtools-ai/src/{organize,rename}.rs`, `crates/rtools-ai/tests/dry_run.rs`

### Final verification (fresh after the final async CLI boundary adjustment)

- `cargo fmt --all -- --check` — passed.
- `cargo test -p rtools-ai --test dry_run --locked` — 3 passed, 0 failed.
- `cargo test -p rtools-cli --test cli_contract --locked` — 11 passed, 0 failed.
- `cargo test -p rtools-cli --locked` — 20 unit + 11 contract + 2 EXIF JSON + 1
  image-warning test passed (34 total, 0 failed).
- Original reproductions:
  - `cargo run -q -p rtools-cli -- pdf text --input missing.pdf` exited nonzero and
    printed `CAPABILITY_UNAVAILABLE` without a success result.
  - `cargo run -q -p rtools-cli -- config validate --config /definitely/missing.toml`
    exited nonzero and printed `CONFIGURATION_INVALID`.
  - `cargo run -q -p rtools-cli -- --output-format json doctor | jq -e '.status == "success"'`
    printed `true` and exited zero.
- `cargo test --workspace --all-targets --locked` — 175 passed, 0 failed.
- `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` — passed.
- `cargo check -p rtools-wasm --target wasm32-unknown-unknown --locked` — passed.
- `cargo deny check` — passed (`advisories`, `bans`, `licenses`, and `sources` all
  `ok`).
- `git diff --check` — passed; only Git LF-to-CRLF informational notices appeared.

### Residual risks / intentional limits

- `batch.run` has no end-to-end partial-work implementation by design; its unavailable
  capability result is nonzero, while a direct unit test covers the required exit-7
  mapping for a genuine `PARTIAL_FAILURE`.
- Shell completion generation remains the documented raw-output exception because it
  emits shell source rather than a command result.
- `cargo deny check` reports pre-existing duplicate dependency and unmatched `ISC`
  allowance warnings, but exits successfully. No dependency or lockfile update was
  made for Task 8.
