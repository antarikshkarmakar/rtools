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

## Fix round 1/5: review remediation

### RED-GREEN record

1. **Config initialization no-overwrite.** Added the process regression
   `config_init_preserves_existing_bytes_and_returns_output_exists`, which writes
   non-UTF-8 sentinel bytes before invoking `config init`. RED: the command exited
   zero and overwrote the existing file. GREEN: `AppConfig::save` now writes through
   the Task 4 `PendingOutput` no-replace primitive; the focused test exits 5 and the
   exact sentinel bytes remain unchanged.
2. **JSON typed parse failures.** Added
   `json_typed_parse_failure_is_one_invalid_input_report` before changing startup
   parsing. RED: malformed crop input exited with clap output on stderr and no JSON
   stdout. GREEN: valid `--output-format json` now turns a non-help/version clap
   parse error into the single `cli.parse` JSON failure envelope on stdout with exit
   2; human help, version, and diagnostics retain clap behavior.
3. **Completion configuration validation.** Added
   `completions_with_missing_explicit_config_emit_no_shell_source`. RED: a missing
   explicit config still emitted Bash source and exited zero through the raw
   completion path. GREEN: that narrow raw-output path loads explicit config before
   generator output, returns the typed configuration failure with exit 3, and emits
   no shell source.
4. **Doctor provider facts.** The prior doctor JSON regression was RED because it
   contained capabilities but no provider diagnostic records. Added deterministic
   injected-probe coverage for a found Tesseract executable and process checks for
   JSON and human output. GREEN: doctor reports sorted registry-backed providers,
   referenced operation states, secret-free configured/not-configured facts, and
   best-effort executable probe results. A found executable leaves every dependent
   operation unavailable until a verified adapter is registered.
5. **AI collision policy.** The inherited dry-run collision expectations were RED:
   organize/rename silently produced suffixed names; a new duplicate-input test also
   produced two planned artifacts. GREEN: five focused dry-run tests pass. Rename
   and date organize preflight every destination, return `OUTPUT_EXISTS` before
   mutation for on-disk or planned collisions, and date-organize dry-run creates no
   directories or files.
6. **Multi-item outcomes.** RED process runs showed EXIF/image mixed input exit 6,
   first-error aborts, discarded successful output/warnings, and `result: null` for
   all failures. GREEN: mixed EXIF and image inputs return successful results plus
   item-path failures with `partial_failure` and exit 7; all-failed image input
   retains every failure/path, returns an explicit empty result, has `failure`
   status, and uses the first stable item error as the process exit (6 for processing
   failures). The direct alt-text aggregation test covers the same handler pattern.
   The human mixed-image regression confirms output/warnings remain on stdout while
   item diagnostics render on stderr.

### Design decisions

- `AppConfig::save` is the sole config-init publication boundary, so enforcing the
  no-overwrite contract there eliminates a racy caller pre-check and covers its only
  caller.
- The raw completion generator remains the deliberately documented stdout exception;
  it is preceded by config validation and never acts as a command result renderer.
- Provider diagnostics come from the shared capability registry. The known
  executable-backed unavailable provider is Tesseract, probed with `tesseract
  --version`; missing, empty, and failed probes remain structured observations, not
  doctor failures. ONNX Runtime/PDFium configuration is reported as a boolean state,
  never as a path or secret, and discovery never enables a capability.
- Multi-input command results retain `FileOutput.warnings`, successful result data,
  and `ItemFailure` path data. The CLI boundary derives `success`, `partial_failure`,
  or `failure` from those outcomes; vector AI organize/rename remain all-or-nothing
  domain operations and use their strict preflight path instead.
- The two narrowly scoped `clippy::too_many_lines` allowances preserve the audited
  unavailable-capability catalog and the human doctor renderer as complete,
  deterministic representations of their structured data. No workspace-wide lint
  suppression was added.

### Changed files in this fix round

- `crates/rtools-core/src/config.rs`
- `crates/rtools-ai/src/{organize,rename}.rs` and `crates/rtools-ai/tests/dry_run.rs`
- `crates/rtools-cli/src/{capabilities,exit,main,report}.rs`
- `crates/rtools-cli/src/commands/{ai,image,mod}.rs`
- `crates/rtools-cli/tests/{cli_contract,exif_json,image_warnings}.rs`

### Verification (fresh after final changes)

- `cargo fmt --all -- --check` — passed.
- `cargo test -p rtools-ai --test dry_run --locked` — 5 passed, 0 failed.
- `cargo test -p rtools-cli --test cli_contract --locked` — 15 passed, 0 failed.
- `cargo test -p rtools-cli --locked` — 43 passed, 0 failed (22 unit, 15 contract,
  2 EXIF JSON, 4 image-warning).
- Original reproductions, with `test $? -ne 0` after each expected failure:
  - `cargo run -q -p rtools-cli -- pdf text --input missing.pdf` — nonzero and
    `CAPABILITY_UNAVAILABLE`, without a success result.
  - `cargo run -q -p rtools-cli -- config validate --config /definitely/missing.toml`
    — nonzero and `CONFIGURATION_INVALID`.
  - `cargo run -q -p rtools-cli -- --output-format json doctor | jq -e '.status'`
    — emitted the one JSON report and jq succeeded.
- `cargo test --workspace --all-targets --locked` — 187 passed, 0 failed.
- `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`
  — passed.
- `cargo check -p rtools-wasm --target wasm32-unknown-unknown --locked` — passed.
- `cargo deny check` — passed; advisories, bans, licenses, and sources were `ok`.
  Existing duplicate-dependency and unmatched-ISC warnings remain.
- `git diff --check` — passed; only Git's LF-to-CRLF informational notices appeared.

### Residual risks / intentional limits

- `batch.run` remains unavailable and nonzero by the binding capability decision; no
  artificial batch executor was enabled.
- For heterogeneous all-failed multi-input processing, a single process must choose
  one exit code; the first stable item error is used while every per-item failure is
  retained in the report.
- Tesseract availability is diagnostic-only until an adapter exists. The best-effort
  probe intentionally does not change registry capability state.

## Fix round 2/5: portable destination aliases

### RED-GREEN record

- Added six real-filesystem case-alias regressions before changing production code:
  two different source directories containing `A.jpg` and `a.jpg` must fail for
  organize dry-run, organize execution, rename dry-run, and rename execution; an
  already-present case-only directory entry must fail for each dry-run operation.
  RED: the focused suite reported seven failures (the six aliases plus the
  non-Unicode rule below), each returning successful plans on the case-sensitive
  host. GREEN: all alias tests return `OUTPUT_EXISTS`, preserve both source byte
  strings, create no dry-run artifacts, and leave no earlier execution artifact.
- Added the Unix-only
  `rename_rejects_non_unicode_destination_names_without_lossy_normalization`
  regression. RED: a byte-invalid filename became a replacement-character name
  through `to_string_lossy` and planned successfully. GREEN: destination identity
  construction fails closed with `PATH_POLICY_VIOLATION` and preserves source bytes.
- The first no-replace rename implementation copied through `PendingOutput` and
  removed the source after commit. A pre-commit review correctly identified this as
  a metadata/identity regression. Added
  `rename_non_dry_preserves_regular_file_identity_and_modified_time` before changing
  it. RED: the copied target inode was `13864` while the source inode was `13534`.
  GREEN: the target has the source device, inode, and modified time, and the source
  is absent only after the target exists.

### Design decisions

- `rtools-ai::destination` supplies the shared portable destination policy.
  `PortableDestinationKey` lexically eliminates `.`/normal/`..` pairs and compares
  Unicode path components with `to_lowercase`. The planner uses those keys for every
  destination and scans each existing directory component for a case-only alias,
  including aliases in an existing output-directory path. It rejects non-Unicode
  components instead of lossy string conversion, so distinct byte paths are never
  silently merged into one portable identity.
- Organize retains copy semantics but publishes each non-dry copy through Task 4's
  `PendingOutput` `FailIfExists` reservation and no-replace commit. This provides a
  final filesystem-enforced collision defense after portable planning.
- Rename deliberately does not copy. `hard_link(source, destination)` atomically
  creates the no-replace target for supported regular files, maps `AlreadyExists` to
  `OUTPUT_EXISTS`, and unlinks the source only after link creation. This preserves
  source inode/metadata identity without a copy/delete rollback hazard. Task 4's
  temporary-artifact primitive is intentionally not used for rename because it
  requires a copied/rewritten temporary artifact and would change that identity.

### Changed files in this fix round

- `crates/rtools-ai/src/{destination,lib,organize,rename}.rs`
- `crates/rtools-ai/tests/dry_run.rs`

### Verification (fresh after final hard-link correction)

- `cargo test -p rtools-ai --test dry_run --locked` — 13 passed, 0 failed.
- `cargo test -p rtools-core --test output_policy --locked` — 19 passed, 0 failed,
  including no-replace race defense and temporary/reservation cleanup.
- `cargo test -p rtools-cli --test cli_contract --locked` — 15 passed, 0 failed.
- `cargo test -p rtools-cli --locked` — 43 passed, 0 failed.
- Original reproductions, with `test $? -ne 0` after expected failures:
  - `cargo run -q -p rtools-cli -- pdf text --input missing.pdf` — nonzero and
    `CAPABILITY_UNAVAILABLE`.
  - `cargo run -q -p rtools-cli -- config validate --config /definitely/missing.toml`
    — nonzero and `CONFIGURATION_INVALID`.
  - `cargo run -q -p rtools-cli -- --output-format json doctor | jq -e '.status'`
    — emitted one JSON report and jq succeeded.
- `cargo test --workspace --all-targets --locked` — 195 passed, 0 failed.
- `cargo fmt --all -- --check` — passed.
- `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`
  — passed.
- `cargo check -p rtools-wasm --target wasm32-unknown-unknown --locked` — passed.
- `cargo deny check` — passed; advisories, bans, licenses, and sources were `ok`.
  Existing duplicate-dependency and unmatched-ISC warnings remain.
- `git diff --check` — passed before this report append; rerun after staging below.

### Residual filesystem limits

- The rename no-replace move requires a hard-link-capable filesystem and source/
  destination on the same filesystem. Unsupported file types/filesystems fail before
  source unlink; no copying fallback is used because it would lose identity.
- If source unlink fails after a successful hard link, rtools returns that error and
  leaves a safe duplicate rather than deleting either artifact.
- The portable key intentionally rejects non-Unicode components. It performs lexical
  normalization and Unicode lowercase comparison, not platform-specific Unicode
  normalization (for example, filesystem-specific composed/decomposed equivalence),
  so such platform rules remain a conservative future audit area.
