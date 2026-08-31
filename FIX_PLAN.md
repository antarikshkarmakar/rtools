# rTools Comprehensive Fix Implementation Plan

> **Generated from analysis of all 8 crates.**  
> **Total issues identified: 67** (12 Critical, 18 High, 22 Medium, 15 Low)

---

## Phase 1: Foundation Fixes (rtools-core + Cargo.toml)

**Goal:** Make the workspace compilable and fix all foundational type/error issues.

### 1.1 Workspace Cargo.toml Fixes
| # | Action | File | Detail |
|---|--------|------|--------|
| 1 | FIX | `Cargo.toml:22` | `figment` version `"0.10"` — verify this version exists on crates.io. If not, use `"0.12"` (latest stable). |
| 2 | REMOVE | `Cargo.toml:21` | Remove unused `config = "0.14"` dependency. |
| 3 | VERIFY | `Cargo.toml:39` | `webp = "0.3"` — verify API compatibility with `WebPEncoder` usage in rtools-image. May need `"0.6"`. |
| 4 | VERIFY | `Cargo.toml:40` | `ravif = "0.11"` — verify `Encoder::encode()` API matches usage in compress.rs. |
| 5 | VERIFY | `Cargo.toml:65` | `rmcp = "0.2"` — verify `ServerHandler` trait API. May need different version or different trait name. |

### 1.2 rtools-core/src/lib.rs
| # | Action | File | Detail |
|---|--------|------|--------|
| 6 | FIX | `lib.rs:6` | Remove `pub mod processor;` (module doesn't exist). Current lib.rs already doesn't have it — verify. |
| 7 | ADD | `lib.rs` | Re-export missing types: `ExifData`, `PageSize`, `PageSizeUnit`, `PdfOutputFormat`, `AIProcessor`, `MetadataExtractor`. |

### 1.3 rtools-core/src/types.rs
| # | Action | File | Detail |
|---|--------|------|--------|
| 8 | ADD | `types.rs` | Add `pub fn mime_type(&self) -> &'static str` method directly on `ImageFormat`. |
| 9 | REMOVE | `types.rs:36` | Remove `ImageFormat::Pdf` — PDF is not an image format. |
| 10 | FIX | `types.rs:211` | Change `ExifData.flash: Option<bool>` to `Option<u16>` to preserve flash mode info. |

### 1.4 rtools-core/src/error.rs
| # | Action | File | Detail |
|---|--------|------|--------|
| 11 | ADD | `error.rs` | Add helper constructors: `file_not_found()`, `output_directory_not_found()`, `batch_error()`. |
| 12 | ADD | `error.rs` | Add `#[from]` conversions for `toml::de::Error`, `toml::ser::Error`, `serde_json::Error`, `figment::Error`. |

### 1.5 rtools-core/src/traits.rs
| # | Action | File | Detail |
|---|--------|------|--------|
| 13 | FIX | `traits.rs` | Remove invalid imports of `ProcessInput` and `ProcessOutput`. |
| 14 | ADD | `traits.rs` | Add `Clone` bound to `type Config` and `Debug` bound to `type Input` in `BatchProcessor`. |

### 1.6 rtools-core/src/config.rs
| # | Action | File | Detail |
|---|--------|------|--------|
| 15 | FIX | `config.rs:23` | Rename field `pub mc: McpConfig` to `pub mcp: McpConfig`. Update serde alias. |
| 16 | FIX | `config.rs:267` | Add `std::fs::create_dir_all(path.parent())` before `std::fs::write()` in `save()`. |
| 17 | REMOVE | `config.rs` | Remove unused `config` crate import (it's in Cargo.toml but never used). |

### 1.7 rtools-core/Cargo.toml
| # | Action | File | Detail |
|---|--------|------|--------|
| 18 | ADD | `Cargo.toml` | Add `dirs = { workspace = true }`. |
| 19 | REMOVE | `Cargo.toml` | Remove unused deps: `rayon`, `walkdir`, `ignore`, `bytes`, `tokio`, `chrono`, `uuid`, `typed-path`, `mime`, `mime_guess`, `config`. |

**Estimated changes: ~19 items**

---

## Phase 2: Image Processing Fixes (rtools-image)

**Goal:** Fix all data loss risks, algorithmic bugs, and API inconsistencies.

### 2.1 Type Deduplication
| # | Action | File | Detail |
|---|--------|------|--------|
| 20 | DEDUP | `crop.rs`, `resize.rs` | Remove duplicate type definitions (`CropRegion`, `AspectRatio`, `Gravity`, `ResizeAlgorithm`) from rtools-image. Import from rtools-core instead. |

### 2.2 Compress — Data Loss & Quality Fixes
| # | Action | File | Detail |
|---|--------|------|--------|
| 21 | FIX | `compress.rs:132-143` | **WebP quality ignored** — change from `new_lossless()` to `new(&mut output).encode(quality)`. |
| 22 | FIX | `compress.rs:108` | **Alpha channel loss** — detect `img.color().has_alpha()` and log warning when converting RGBA→RGB for JPEG. |
| 23 | FIX | `compress.rs` | **File clobbering** — check if output exists, append `_1`, `_2` etc. or use atomic write via temp file. |
| 24 | FIX | `compress.rs` | **Duplicate `impl ImageFormat`** — remove the second `impl ImageFormat` block (the `mime_type()` method should be in rtools-core). |

### 2.3 Convert — Format & Quality Fixes
| # | Action | File | Detail |
|---|--------|------|--------|
| 25 | FIX | `convert.rs:89-100` | **WebP always lossless** — use `WebPEncoder::new(&mut output).encode(quality as f32 / 100.0)`. |
| 26 | FIX | `convert.rs:70` | **Alpha loss on JPEG** — detect alpha channel, log warning. |
| 27 | FIX | `convert.rs` | **File clobbering** — `set_extension()` pattern overwrites existing files. Use `_converted` suffix instead. |

### 2.4 Resize — Dimension Safety
| # | Action | File | Detail |
|---|--------|------|--------|
| 28 | FIX | `resize.rs:103` | **MitchellNetravali alias** — either remove or document as alias for CatmullRom. |
| 29 | FIX | `resize.rs` | **Zero-dimension clamp** — ensure `width >= 1` and `height >= 1` after calculation. |
| 30 | FIX | `resize.rs` | **MIME type wrong** — derive output MIME from output path, not input format. |

### 2.5 Crop — Validation & Safety
| # | Action | File | Detail |
|---|--------|------|--------|
| 31 | ADD | `crop.rs:171-173` | **Empty validation** — add bounds checks for Pixels (x+w<=width), Percentage (0..=100), AspectRatio (positive denominator). |

### 2.6 EXIF — GPS Parsing Accuracy
| # | Action | File | Detail |
|---|--------|------|--------|
| 32 | FIX | `exif.rs:109-113` | **GPS altitude sign** — read `GPSAltitudeRef` tag, negate altitude if below sea level. |
| 33 | FIX | `exif.rs:89-107` | **GPS hemisphere** — parse `GPSLatitudeRef`/`GPSLongitudeRef` more robustly (check for "N"/"S" prefix, not display string). |
| 34 | FIX | `exif.rs:183-184` | **DMS fallback** — when `values.len() == 2`, combine degrees + minutes, don't discard minutes. |
| 35 | ADD | `exif.rs` | **Graceful degradation** — return empty `ExifData` on unreadable EXIF instead of failing the processor. |

### 2.7 Filter — Per-Channel Transforms
| # | Action | File | Detail |
|---|--------|------|--------|
| 36 | FIX | `filter.rs` | **Uniform luminance scaling** — replace with per-channel RGB curve transforms, color temperature, and tint adjustments per filter preset. |

### 2.8 Watermark — Real Implementation
| # | Action | File | Detail |
|---|--------|------|--------|
| 37 | FIX | `watermark.rs` | **Placeholder** — implement text overlay using `imageproc::drawing::draw_text_mut()` and image overlay with alpha blending. |

### 2.9 Dead Dependencies
| # | Action | File | Detail |
|---|--------|------|--------|
| 38 | REMOVE | `Cargo.toml` | Remove `crc32fast` (unused). |

**Estimated changes: ~19 items**

---

## Phase 3: PDF Processing Fixes (rtools-pdf)

**Goal:** Fix merge/split correctness, compress safety, and stub completions.

### 3.1 Merge — Correctness
| # | Action | File | Detail |
|---|--------|------|--------|
| 39 | FIX | `merge.rs` | **Cross-document references** — ensure `renumber_objects_with` is called correctly for all objects from each source document. Verify catalog/pages construction. |

### 3.2 Split — Efficiency
| # | Action | File | Detail |
|---|--------|------|--------|
| 40 | FIX | `split.rs:87-90` | **O(n*m) clone** — build each single-page document from scratch instead of cloning the full document per page. |

### 3.3 Compress — Safety
| # | Action | File | Detail |
|---|--------|------|--------|
| 41 | FIX | `compress.rs:75-81` | **Metadata removal** — handle `Result` from `doc.trailer.remove(b"Info")` properly. |
| 42 | REMOVE | `compress.rs:26` | Remove unused `remove_images` config field, or implement it. |

### 3.4 Dead Dependencies
| # | Action | File | Detail |
|---|--------|------|--------|
| 43 | REMOVE | `Cargo.toml` | Remove unused `printpdf`, `pdfium-render`. |

**Estimated changes: ~5 items**

---

## Phase 4: AI Processing Fixes (rtools-ai)

**Goal:** Fix duplicate detection, collision handling, and rename bugs.

### 4.1 Duplicates — Hashing Correctness
| # | Action | File | Detail |
|---|--------|------|--------|
| 44 | FIX | `duplicates.rs` | **Transitive grouping** — implement union-find or true clustering instead of pivot-based greedy grouping. |
| 45 | FIX | `duplicates.rs` | **Downscale before hash** — ensure images are downscaled to 8×8 (aHash) or 9×8 (dHash) before bit comparison. Verify current implementation. |

### 4.2 Organize — Collision Safety
| # | Action | File | Detail |
|---|--------|------|--------|
| 46 | FIX | `organize.rs:85-95` | **Dry-run collision reporting** — report potential collisions even in dry-run mode. |

### 4.3 Rename — Data Loss Fix
| # | Action | File | Detail |
|---|--------|------|--------|
| 47 | FIX | `rename.rs:54-55` | **CRITICAL: No collision detection** — check if `new_path` exists before `std::fs::rename`. Append `_1`, `_2` etc. on collision. |
| 48 | FIX | `rename.rs:98` | **Double extension** — don't append extension if pattern already produces one. Check for `{ext}` in pattern. |

### 4.4 Sort — Missing Implementations
| # | Action | File | Detail |
|---|--------|------|--------|
| 49 | ADD | `sort.rs` | Implement `SortCriteria::Type` (group by extension) and `SortCriteria::Name` (alphabetical). |

### 4.5 Dead Dependencies
| # | Action | File | Detail |
|---|--------|------|--------|
| 50 | REMOVE | `Cargo.toml` | Remove unused `image_hasher`, `candle-core`, `candle-nn`, `candle-transformers`, `tokenizers`. |

**Estimated changes: ~7 items**

---

## Phase 5: CLI Fixes (rtools-cli)

**Goal:** Wire all CLI arguments to processors, fix missing dependencies.

### 5.1 Argument Wiring
| # | Action | File | Detail |
|---|--------|------|--------|
| 51 | FIX | `commands/image.rs:120-128` | **Crop args ignored** — parse `--region`, `--ratio`, `--gravity` and pass to `CropConfig`. |
| 52 | FIX | `commands/image.rs:168` | **Watermark position ignored** — parse `--position` and map to `WatermarkPosition` enum. |
| 53 | FIX | `commands/pdf.rs:68-69` | **PDF Split --pages ignored** — parse `--pages` string into `PageRange`. |

### 5.2 Dependencies
| # | Action | File | Detail |
|---|--------|------|--------|
| 54 | ADD | `Cargo.toml` | Add `walkdir = { workspace = true }`. |

### 5.3 Code Quality
| # | Action | File | Detail |
|---|--------|------|--------|
| 55 | DEDUP | `commands/image.rs`, `commands/pdf.rs` | Extract shared `format_size()` into a common utility module. |

**Estimated changes: ~5 items**

---

## Phase 6: API Fixes (rtools-api)

**Goal:** Fix tempfile lifetimes, wire request params, add download endpoint.

### 6.1 Tempfile Lifetime Fix
| # | Action | File | Detail |
|---|--------|------|--------|
| 56 | FIX | `handlers/pdf.rs:28-31`, `handlers/ai.rs:28-31,72-75,121,167-170` | **CRITICAL: TempDir dropped inside loop** — move `TempDir` creation outside the loop, or use `Arc<TempDir>` shared across iterations. |

### 6.2 Request Parameter Wiring
| # | Action | File | Detail |
|---|--------|------|--------|
| 57 | FIX | `handlers/image.rs:44-45` | Wire `CompressRequest.quality` and `.format` to processor config. |
| 58 | FIX | `handlers/image.rs:98` | Wire `ConvertRequest.format` to target format. |
| 59 | FIX | `handlers/image.rs:145-146` | Wire `ResizeRequest.width/height/maintain_aspect` to processor config. |

### 6.3 Output Delivery
| # | Action | File | Detail |
|---|--------|------|--------|
| 60 | ADD | `handlers/image.rs` | Add `/download/:id` endpoint that serves processed files from temp cache. |
| 61 | ADD | `handlers/image.rs` | Add cleanup middleware or scheduled task to delete old temp files. |

**Estimated changes: ~6 items**

---

## Phase 7: MCP Server Fixes (rtools-mcp)

**Goal:** Implement `ServerHandler` trait, register tools properly.

### 7.1 Trait Implementation
| # | Action | File | Detail |
|---|--------|------|--------|
| 62 | FIX | `main.rs` | **CRITICAL: Missing `impl ServerHandler`** — implement the `ServerHandler` trait for `RToolsServer` with proper `list_available_tools` and `call_tool` overrides. |

### 7.2 Dependencies
| # | Action | File | Detail |
|---|--------|------|--------|
| 63 | ADD | `Cargo.toml` | Add `walkdir = { workspace = true }`. |
| 64 | REMOVE | `Cargo.toml` | Remove unused `async-trait`. |

**Estimated changes: ~3 items**

---

## Phase 8: WASM Crate Rewrite (rtools-wasm)

**Goal:** Convert to in-memory byte buffer operations for browser compatibility.

### 8.1 Cargo.toml
| # | Action | File | Detail |
|---|--------|------|--------|
| 65 | FIX | `Cargo.toml` | Add `serde-wasm-bindgen = { workspace = true }`. Remove `wee_alloc`, `console_error_panic_hook` (add only if actually used). Remove `tempfile` if present. |

### 8.2 In-Memory Processing
| # | Action | File | Detail |
|---|--------|------|--------|
| 66 | REWRITE | `lib.rs` | Convert all functions to `&[u8]` → `Vec<u8>` pattern using `image::io::Reader` with `std::io::Cursor`. Remove all `std::fs` and `tempfile` usage. |
| 67 | REWRITE | `lib.rs` | Remove local `mod serde_wasm_bindgen` — use the crate directly. |

**Estimated changes: ~3 items**

---

## Phase 9: Integration Tests

**Goal:** Add tests for critical paths.

### 9.1 New Test Files
| # | Action | File | Detail |
|---|--------|------|--------|
| 68 | NEW | `tests/image_tests.rs` | Tests for compression, conversion (JPEG→WebP with quality), resize (exact + aspect ratio), EXIF GPS parsing. |
| 69 | NEW | `tests/ai_tests.rs` | Tests for duplicate hashing, collision-safe organize, batch rename double-extension bug. |

---

## Implementation Order

```
Phase 1 (Core)        ──→ Phase 2 (Image) ──→ Phase 5 (CLI)
                     ──→ Phase 3 (PDF)   ──→ Phase 6 (API)
                     ──→ Phase 4 (AI)    ──→ Phase 7 (MCP)
                                                    ──→ Phase 8 (WASM)
                                                    ──→ Phase 9 (Tests)
```

**Critical path:** Phase 1 → Phase 2 → Phase 8 (WASM depends on core types being correct)

---

## Verification Commands

```bash
# After each phase:
wsl ~/.cargo/bin/cargo check --workspace
wsl ~/.cargo/bin/cargo clippy --workspace -- -D warnings

# After all phases:
wsl ~/.cargo/bin/cargo test --workspace
wsl ~/.cargo/bin/cargo build --release --workspace

# WASM verification:
wsl ~/.cargo/bin/cargo build --release -p rtools-wasm --target wasm32-unknown-unknown
```