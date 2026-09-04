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

## Fix round 3/5: portable identity anchoring and Unicode aliases

### RED-GREEN record

1. **Exact intermediate directory aliases.** Added real-filesystem organize and
   rename regressions with both `out/` and `Out/` present. Each test puts the
   occupied final artifact below `Out/` while requesting `out/`. RED: both
   processors returned successful outputs below the exact `out/` spelling and would
   have mutated. GREEN: both return `OUTPUT_EXISTS` before mutation; source and
   occupied bytes are unchanged and `out/` remains empty.
2. **Anchored destination keys and root clamping.** Added two injected-base unit
   regressions: a relative `out/../same.jpg` key equals its absolute spelling, and
   `/../../same.jpg` equals `/same.jpg`. Also added a non-dry rename regression that
   passes one source through a relative spelling and another through its absolute
   spelling while both plan `same.jpg`. RED: the helper tests were compile-RED until
   the injected normalization seam existed; the processor reached the second
   no-replace publication after moving the first source, proving its planner had
   treated the two spellings as distinct. GREEN: both unit tests and the processor
   test return the same portable identity before any source moves.
3. **Unicode caseless aliases.** Added dry-run rename regressions for `Σ.jpg` versus
   `ς.jpg`, and for the expanding mapping `ß.jpg` versus `ss.jpg`. RED: each pair
   produced two successful plans. GREEN: each returns `OUTPUT_EXISTS`, preserves both
   source byte strings, and creates no output directory.

### Design decisions

- Every existing normal path component is now scanned for all Unicode-normalizable
  sibling names even if its exact spelling exists. A differing case-equivalent
  sibling is treated as an ambiguous portable identity and rejected before planning
  can follow the exact directory.
- `PortableDestinationKey` anchors relative paths to `current_dir` before lexical
  normalization. Its private injected-base helper makes the pure key tests isolated
  from process-wide current-directory mutation. Parent traversal pops a normal
  component and otherwise clamps at a root rather than retaining leading `..`.
- Components use Rust's deterministic Unicode `to_uppercase` transform as the
  conservative caseless key. It covers sigma/final-sigma and expanding `ß`/`SS`
  mappings required here without adding a dependency. Non-Unicode components remain
  fail-closed and are never converted with a lossy string representation.
- The approved copy publication and hard-link no-replace rename paths were not
  changed. They remain the final filesystem-enforced defenses after portable
  preflight.

### Changed files in this fix round

- `crates/rtools-ai/src/destination.rs`
- `crates/rtools-ai/tests/dry_run.rs`

### Verification (fresh after final formatting and Clippy correction)

- RED commands:
  - `cargo test -p rtools-ai --test dry_run ambiguous_case_only_output_directories --locked`
    — 0 passed, 2 failed before the directory scan correction.
  - `cargo test -p rtools-ai --test dry_run sigma_and_final --locked` and
    `cargo test -p rtools-ai --test dry_run unicode_destination_aliases --locked`
    — each failed before the Unicode key correction.
- Focused GREEN:
  - `cargo test -p rtools-ai --lib destination::tests --locked` — 2 passed, 0
    failed.
  - `cargo test -p rtools-ai --test dry_run --locked` — 18 passed, 0 failed.
  - `cargo test -p rtools-cli --test cli_contract --locked` — 15 passed, 0 failed.
  - `cargo test -p rtools-cli --locked` — 43 passed, 0 failed (22 unit, 15
    contract, 2 EXIF JSON, 4 image-warning).
- Original reproductions, with `test $? -ne 0` after each expected failure:
  - `cargo run -q -p rtools-cli -- pdf text --input missing.pdf` — nonzero and
    `CAPABILITY_UNAVAILABLE`.
  - `cargo run -q -p rtools-cli -- config validate --config /definitely/missing.toml`
    — nonzero and `CONFIGURATION_INVALID`.
  - `cargo run -q -p rtools-cli -- --output-format json doctor | jq -e '.status'`
    — emitted one JSON report and jq succeeded.
- `cargo test --workspace --all-targets --locked` — 202 passed, 0 failed.
- `cargo fmt --all -- --check` — passed after formatting the two new vector
  literals.
- `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`
  — passed after the root-clamping branch was changed from a one-arm `match` to its
  equivalent `if let` form.
- `cargo check -p rtools-wasm --target wasm32-unknown-unknown --locked` — passed.
- `cargo deny check` — passed; advisories, bans, licenses, and sources were `ok`.
  Existing duplicate-dependency and unmatched-ISC warnings remain.

### Residual filesystem limits

- The uppercase key is intentionally conservative rather than a platform-specific
  Unicode normalization implementation. Filesystem-specific composed/decomposed
  equivalence remains outside this portable preflight; non-Unicode paths still fail
  closed instead of being merged lossily.
- Directory scans and portable planning are preflight checks. The already-approved
  no-replace copy publication and hard-link rename operations remain necessary final
  defenses against races and aliases introduced after preflight.

## Fix round 4/5: explicit traversal anchors and full Unicode case folding

### RED-GREEN record

1. **Relative traversal anchor and exact directory aliases.** Added an initial
   combined real-filesystem unit regression before the production helper existed.
   `cargo test -p rtools-ai --lib
   destination_scan_anchors_a_relative_first_component_and_checks_exact_aliases
   --locked` was compile-RED with unresolved import
   `destination_or_case_alias_exists_with_base`. After anchoring traversal to an
   injected absolute base, the fixture was split into the two requested contracts:
   a normal relative first component from the real process current directory, and
   exact `out` traversal with an `Out` sibling. The final focused destination suite
   passes both.
2. **Windows drive-relative paths.** Added a target-portable test against an explicit
   absolute base before the rejection existed. `cargo test -p rtools-ai --lib
   destination_helpers_reject_windows_drive_relative_paths --locked` was RED because
   `C:foo` returned
   `Ok(PortableDestinationKey([RootDir, Normal("WORKSPACE"), Normal("BASE"),
   Normal("C:FOO")]))`. GREEN rejects it with `PATH_POLICY_VIOLATION` through both
   key creation and alias scanning before any join or traversal. The pre-existing
   rooted-parent test continues to prove lexical root clamping.
3. **Full Unicode case folding.** Added
   `rename_dry_run_rejects_kelvin_sign_and_k_destination_aliases` before replacing
   uppercase normalization. `cargo test -p rtools-ai --test dry_run
   rename_dry_run_rejects_kelvin_sign_and_k_destination_aliases --locked` was RED:
   rename returned two successful plans for `K.jpg` and `k.jpg`. After adding the
   exact direct dependency `unicode-casefold = "=0.2.0"` and using explicit
   `Variant::Full`/`Locale::NonTurkic` folding, the same command passes. The existing
   sigma/final-sigma and sharp-s expansion regressions also pass in the full suite.

### Design decisions

- `destination_or_case_alias_exists` captures `current_dir` once. Its private
  injected-base helper starts relative traversal at that explicit absolute base and
  still scans every requested existing component, including an exact spelling when
  a case-fold-equivalent sibling exists.
- Drive-letter-plus-colon paths without a following root separator are rejected
  conservatively on every target. No attempt is made to inherit Windows per-drive
  current-directory state; absolute `C:/...` and `C:\\...` spellings are not matched
  by that rejection.
- Filename identity uses the dependency's deterministic full non-Turkic Unicode case
  fold. Non-Unicode components remain fail-closed. No Unicode normalization layer was
  added, and the approved no-replace copy/hard-link publication code was unchanged.
- The two other rtools-ai integration targets import the direct dependency as `_`
  solely to satisfy the workspace `unused_crate_dependencies` lint for independent
  integration-test crates.

### Changed files in this fix round

- `Cargo.lock`, `crates/rtools-ai/Cargo.toml`
- `crates/rtools-ai/src/destination.rs`
- `crates/rtools-ai/tests/{capability_modes,capability_unavailable,dry_run}.rs`
- `.superpowers/sdd/2026-09-02-milestone-1-release-safety/task-8-report.md`

### Verification (fresh after final formatting and lint corrections)

- `cargo test -p rtools-ai --lib destination::tests --locked` — 5 passed, 0
  failed.
- `cargo test -p rtools-ai --test dry_run --locked` — 19 passed, 0 failed,
  including sigma/final-sigma, Kelvin/k, sharp-s expansion, non-Unicode fail-closed,
  relative/absolute identity, and no-overwrite execution coverage.
- `cargo test -p rtools-ai --all-targets --all-features --locked` — 34 passed, 0
  failed. Its first run exposed only integration-target unused-dependency warnings;
  the narrow `_` imports removed them.
- `cargo clippy -p rtools-ai --all-targets --all-features --locked -- -D warnings`
  — passed. Its first run rejected the intentionally non-NFC Kelvin literal; a
  function-scoped `unicode_not_nfc` allowance documents why replacing U+212A with
  `K` would erase the regression.
- `cargo test --workspace --all-targets --all-features --locked` — 203 passed, 0
  failed.
- `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`
  — passed.
- `cargo tree -i unicode-casefold --locked` — one pinned package,
  `unicode-casefold v0.2.0`, used directly by `rtools-ai`; it adds no transitive
  packages.
- `cargo deny check` — passed; advisories, bans, licenses, and sources were `ok`.
  Existing duplicate-dependency and unmatched-ISC warnings remain.
- `cargo check -p rtools-wasm --target wasm32-unknown-unknown --locked` — passed.
- `cargo fmt --all -- --check` — passed.
- `git diff --check` — passed; only Git's LF-to-CRLF informational notices appeared.

### Residual risk

- This WSL toolchain has only `x86_64-unknown-linux-gnu` and
  `wasm32-unknown-unknown` installed, so native Windows path parsing was not executed
  locally. The drive-relative contract is covered by a pure cross-target syntax test
  and rejects before platform path joining; native Windows execution remains CI-bound.
- Full case folding intentionally does not add canonical Unicode normalization.
  Filesystem-specific composed/decomposed equivalence remains a future conservative
  audit area; the final filesystem-enforced no-replace defenses remain in force.
