# rTools - Rust Image & PDF Processing Toolkit

## Project Overview

A high-performance, local-first image and PDF processing toolkit written in Rust, accessible via:
- **MCP (Model Context Protocol)** - For AI assistant integration
- **CLI** - For direct command-line usage
- **REST API** - For service integration

**Core Philosophy**: Privacy-first, local processing, zero server uploads, WebAssembly-compatible.

---

## Milestone 1 runtime contract

The runtime capability registry is authoritative for what this build can do.
`rtools --output-format json doctor` emits that registry in deterministic
operation-ID order. The checked-in projection and its CI verifier live in
[docs/operations/capabilities.md](docs/operations/capabilities.md).

| State | Milestone 1 surface |
|---|---|
| Available | Image compress, convert, crop, resize, filter, image watermark, EXIF human/JSON inspection; configuration commands; completions; doctor |
| Experimental | PDF merge/compress/split; report-only duplicate detection; date organization; deterministic rename |
| Unavailable | OCR, alt text, AI classification/naming/sorting, PDF rendering/text/OCR, merge page numbering, light/heavy PDF compression, PDF split-to-image, text watermarking, metadata preservation or GPS-only removal, destructive duplicate modes, batch recipes |

Unavailable operations return a structured `CAPABILITY_UNAVAILABLE` error and
a nonzero process status. Provider discovery never enables an operation without
a verified adapter; Milestone 1 therefore does not claim Tesseract OCR, PDFium
rendering, ONNX inference, or fabricated batch execution.

### Output and metadata safety

- Every executable image and PDF write defaults to fail if its destination
  exists. The CLI, REST, and MCP adapters retain that default. `UniqueName` and
  `Overwrite` are explicit Rust API selections for image operations; merely
  supplying an output path never grants overwrite permission.
- Output reservation and a sibling temporary artifact precede validated atomic
  publication. Reserved Windows device names, ambiguous Windows drive/root
  relative paths, and symlinks in any output-parent ancestor are rejected.
- Reservation and publication guarantees cover cooperating rTools processes
  and ordinary filesystem races. A malicious same-account process with write
  access to the output directory can alter private namespace entries and is
  outside this boundary; use directory permissions or a separate account when
  local processes are not mutually trusted.
- PDF output parents must already exist. PDF processors never call path-based
  recursive directory creation on a requested output, because that operation
  can traverse an unvalidated linked ancestor before output policy runs. This
  includes the default `PdfSplitConfig` output directory, `output/`, which a
  caller must create from a trusted path before processing.
- Live date organization likewise requires its derived year/month directory to
  exist before processing. The public sort processor is unavailable and fails
  before reading inputs or creating an output directory.
- PDF merge takes its ordered `Processor::Input` list as the only authoritative
  source; the legacy config list and page numbering option fail closed. PDF
  compression accepts only the implemented `medium` level. PDF split emits
  PDF pages only; its image-output fields remain public for compatibility but
  non-default values fail with `CAPABILITY_UNAVAILABLE`.
- Writing image operations re-encode with a verified drop-all metadata policy.
  Preserve and GPS-only policies are unavailable and fail before any output is
  reserved. EXIF human and JSON operations are read-only.

### Limits, reports, dry-run, and configuration

`ResourceLimits` carries ceilings for input bytes, decoded pixels, PDF pages,
batch items, and duration. Milestone 1 enforces the byte and pixel ceilings on
bounded image decoding. The page/item/duration fields are stable typed policy
inputs, not evidence that unavailable batch or provider-backed operations are
implemented.

`--output-format json` produces exactly one report on stdout for success,
partial failure, or failure, including a stable error code. Global `--dry-run`
is truthful only for experimental date organization and deterministic rename:
it returns exact source/destination plans and writes nothing. Other dry runs
fail with `CAPABILITY_UNAVAILABLE`.

Configuration precedence is deterministic: defaults, system file, user file,
project file, explicit `--config`, then `RTOOLS_` environment values. A missing
explicit file is an error; conflicting parent/child environment keys and
invalid values are rejected without echoing secret values.

### Stable CLI exit codes

| Exit | Meaning |
|---:|---|
| `0` | Success |
| `2` | Invalid input or unsupported format |
| `3` | Capability, configuration, or authentication unavailable |
| `4` | Resource limit exceeded |
| `5` | Output collision or path-policy violation |
| `6` | Processing or report emission failed |
| `7` | Truthful partial failure |
| `8` | Cancellation or rollback failure |

The machine-readable mappings are detailed in
[docs/operations/exit-codes.md](docs/operations/exit-codes.md).

---

## Product roadmap (not runtime availability)

The categories below describe product direction. They are not an implementation
claim; the Milestone 1 runtime registry above decides whether an operation is
available, experimental, or unavailable.

### 1. Optimize (Compression & Conversion)
| Tool | Description |
|------|-------------|
| Compress | Reduce file size without visible quality loss |
| WebP Converter | Convert JPG/PNG to WebP for faster web delivery |
| HEIC Converter | Convert iPhone HEIC photos to JPG or PNG |
| Batch Resize | Resize multiple images by pixel dimensions or percentage |
| Crop & Ratio | Crop to standard aspect ratios or custom dimensions |
| PDF to Image | Extract pages from PDF as high-quality images |

### 2. AI-Powered
| Tool | Description |
|------|-------------|
| AI Organize | Drop 100+ photos — AI sorts them into named folders |
| AI Rename | Generate descriptive, SEO-friendly filenames automatically |
| AI Alt Text | Generate accessibility alt text for any image |
| AI Photo Sort | Sort photos by subject, scene, or custom criteria |
| Web Optimize | AI-assisted optimization for web performance |
| Transcribe (OCR) | Extract text from images |

### 3. Creative
| Tool | Description |
|------|-------------|
| Film Filters | Apply 14 analog film presets (Kodak, Fuji, Polaroid, etc.) |
| Watermark | Add text or image watermarks with custom positioning |

### 4. Organize
| Tool | Description |
|------|-------------|
| EXIF Viewer | Inspect full metadata — GPS, camera, exposure, timestamps |
| EXIF Remover | Strip GPS and privacy data before sharing |
| Find Duplicates | Detect duplicate images by visual similarity |
| Batch Rename | Rename files in bulk with custom patterns and sequences |
| Cull | Quickly review and select keepers from a shoot |
| Sort by Location | Group photos by GPS coordinates on an interactive map |
| Photo Map | Visualize where your photos were taken |

### 5. PDF Essentials (from ihatepdf.cv)
| Tool | Description |
|------|-------------|
| Merge PDFs | Combine multiple files into one document |
| Compress PDF | Reduce file sizes by up to 70% |
| Split PDF | Separate into individual pages or extract ranges |

### 6. PDF Edit & Organize
| Tool | Description |
|------|-------------|
| In-Place Text Editing | Click to change existing PDF text |
| Page Management | Rotate, delete, crop, rearrange pages |
| Overlays | Add page numbers, watermarks, headers, footers |
| OCR & Text Extraction | Extract text, make scanned PDFs searchable |

### 7. PDF Convert & Export
| Tool | Description |
|------|-------------|
| Office to PDF | Word, Excel, PowerPoint, CSV, Markdown, HTML → PDF |
| PDF to Office | PDF → Word, PowerPoint, Excel, HTML, JPG/PNG, EPUB |
| Media Conversions | Images → PDF, Dark mode, ZIP, eBook → PDF, Audio → PDF |

### 8. PDF Security & Privacy
| Tool | Description |
|------|-------------|
| Encryption | AES-256 password protection, remove passwords |
| Redact & Flatten | Black-out sensitive info, flatten forms |
| Risk Scanner | Strip metadata, add tracking fingerprints |

### 9. PDF AI Tools
| Tool | Description |
|------|-------------|
| Chat with PDF | Query documents with local LLM |
| AI PDF Summarizer | Generate summaries locally |
| Compare PDFs | Side-by-side diff with sync scrolling |
| Repair PDF | Forensic recovery of corrupted files |

### 10. Sharing & Collaboration
| Tool | Description |
|------|-------------|
| Scan to PDF | Camera/webcam scanner with auto-crop |
| P2P Share | Browser-to-browser file transfer |
| Collaborative Whiteboard | Real-time drawing canvas |

### 11. Business Utilities
| Tool | Description |
|------|-------------|
| GST Invoice Generator | Tax-compliant invoices with auto-calculation |
| POS Billing Software | Product carts, GST receipts, thermal printer output |

---

## Architecture

```
rtools/
├── crates/
│   ├── rtools-core/           # Core processing logic, traits, types
│   ├── rtools-image/          # Image processing operations
│   ├── rtools-pdf/            # PDF processing operations
│   ├── rtools-ai/             # AI/ML integrations (local models)
│   ├── rtools-cli/            # CLI interface (clap)
│   ├── rtools-api/            # REST API server (axum)
│   ├── rtools-mcp/            # MCP server implementation
│   └── rtools-wasm/           # WASM bindings for browser
├── specs/                     # Feature specifications
├── tests/                     # Integration tests
├── benches/                   # Performance benchmarks
└── Cargo.toml                 # Workspace configuration
```

---

## Core Traits (rtools-core)

```rust
// Core processing trait
pub trait Processor {
    type Input;
    type Output;
    type Config;
    type Error;
    
    fn process(&self, input: Self::Input, config: Self::Config) -> Result<Self::Output, Self::Error>;
    fn validate_config(&self, config: &Self::Config) -> Result<(), Self::Error>;
    fn estimate_output_size(&self, input: &Self::Input, config: &Self::Config) -> Option<u64>;
}

// Batch processing trait
pub trait BatchProcessor: Processor {
    fn process_batch(&self, inputs: Vec<Self::Input>, config: Self::Config) 
        -> Result<Vec<Self::Output>, Self::Error>;
    
    fn process_streaming(&self, inputs: impl Iterator<Item = Self::Input>, config: Self::Config)
        -> impl Iterator<Item = Result<Self::Output, Self::Error>>;
}

// AI-enabled trait
pub trait AIProcessor: Processor {
    type Model;
    
    fn load_model(&mut self, model: Self::Model) -> Result<(), Self::Error>;
    fn unload_model(&mut self);
    fn is_model_loaded(&self) -> bool;
}
```

---

## Interface Specifications

### CLI Interface (rtools-cli)

```bash
# Global options
rtools [GLOBAL_OPTIONS] <COMMAND> [COMMAND_OPTIONS]

# Machine-readable runtime truth
rtools --output-format json doctor

# Available image commands
# Explicit output parents must already exist; rTools does not create them.
rtools image compress --input photo.jpg --output compressed.jpg --quality 85
rtools image convert --format webp --input photo.jpg --output converted.webp
rtools image resize --input photo.jpg --output resized.jpg --width 1920 --maintain-aspect
rtools image crop --input photo.jpg --output cropped.jpg --ratio 16:9 --gravity center
rtools image watermark --input photo.jpg --image mark.png --output marked.jpg --position bottom-right --opacity 0.5
rtools image filter --input photo.jpg --output filtered.jpg --preset kodak-portra-400

# Config initialization has the same existing-parent requirement.
mkdir -p config
rtools config init --output config/rtools.toml

# Experimental deterministic operations; --dry-run writes nothing
rtools --dry-run ai organize --strategy date --input photos/ --output organized/
rtools --dry-run ai rename --input photos/ --pattern "{date}_{name}_{index}"

# Experimental PDF operations
rtools pdf merge --input file1.pdf file2.pdf --output merged.pdf
rtools pdf compress --level medium --input large.pdf --output small.pdf
rtools pdf split --pages 1-5,10-15 --input doc.pdf --output split/

# Registered but unavailable; each returns exit 3 without placeholder success
rtools image ocr --input scan.png --output text.txt
rtools ai alt-text --input image.jpg --language en
rtools pdf to-image --input doc.pdf --output pages/
rtools batch --config batch.toml --jobs 4
```

### REST API (rtools-api)

```
POST   /api/v1/image/compress    # available
POST   /api/v1/image/convert     # available
POST   /api/v1/image/resize      # available
POST   /api/v1/image/crop        # REST adapter unavailable; core/CLI available
POST   /api/v1/image/watermark   # REST adapter unavailable; core image watermark available
POST   /api/v1/image/filter      # REST adapter unavailable; core/CLI available

POST   /api/v1/ai/organize       # date mode experimental; AI modes unavailable
POST   /api/v1/ai/rename         # deterministic experimental; AI naming unavailable
POST   /api/v1/ai/alt-text       # unavailable

POST   /api/v1/pdf/merge         # experimental
POST   /api/v1/pdf/compress      # experimental
POST   /api/v1/pdf/split         # REST adapter unavailable; core/CLI experimental
POST   /api/v1/pdf/ocr           # unavailable

GET    /health
```

### MCP Interface (rtools-mcp)

Tool presence is not a capability guarantee: each call is gated by the same
operation states, and unavailable modes return structured errors. The exact
tool-to-operation contract and live schema come from the running server rather
than a copied schema fragment:

```bash
rtools-mcp --print-contracts
```

The exported contract drives MCP `tools/list`; CI compares it with
[docs/MCP.md](docs/MCP.md) and the runtime capability registry.

---

## Technology direction

This section is roadmap context. The workspace `Cargo.toml` files, not this
list, are the dependency truth for the current build.

### Core Dependencies

| Category | Crates |
|----------|--------|
| Image Processing | `image`, `imageproc`, `mozjpeg`, `webp`, `ravif`, `heic-reader` |
| PDF Processing | `pdfium-render`, `lopdf`, `printpdf`, `pdf-extract`, `tesseract` (OCR) |
| Async Runtime | `tokio`, `async-std` |
| CLI | `clap`, `clap_complete` |
| Web Framework | `axum`, `tower`, `hyper` |
| gRPC | `tonic`, `prost` |
| MCP | `rmcp` (Rust MCP SDK) |
| Serialization | `serde`, `serde_json`, `toml` |
| File I/O | `tokio-fs`, `walkdir`, `ignore` |
| Parallelism | `rayon`, `futures` |
| Error Handling | `anyhow`, `thiserror` |
| Logging | `tracing`, `tracing-subscriber` |
| Config | `config`, `figment` |
| WASM | `wasm-bindgen`, `wasm-pack` |
| AI/ML | `candle`, `ort` (ONNX Runtime), `tokenizers` |

### AI Model Strategy

- **Local-first**: Use `candle` for pure Rust inference
- **ONNX Support**: `ort` for broader model compatibility
- **Models**: 
  - CLIP for image classification/organization
  - BLIP for captioning/alt-text
  - Tesseract/OCR models for text extraction
  - Custom models for duplicate detection

---

## Configuration

```toml
# rtools.toml
[general]
parallel_jobs = 4
temp_dir = "/tmp/rtools"
log_level = "info"

[image]
default_quality = 85
webp_lossless = false
avif_enabled = true
max_dimension = 8192

[limits]
max_input_bytes = 104857600
max_decoded_pixels = 100000000
max_image_dimension = 32768
max_pdf_pages = 2000
max_batch_items = 10000
max_duration_ms = 300000

[pdf]
pdfium_path = "/usr/lib/libpdfium.so"
ocr_language = "eng"
ocr_dpi = 300

[ai]
model_dir = "/absolute/path/to/models"
device = "Cpu"
batch_size = 8

[api]
host = "127.0.0.1"
port = 8080
max_upload_size = 104857600
cors_origins = ["*"]

[mcp]
server_name = "rtools"
stdio_transport = true
```

---

## Implementation Phases

### Phase 1: Foundation (Weeks 1-2)
- [ ] Workspace setup with Cargo
- [ ] Core traits and types in `rtools-core`
- [ ] Error handling framework
- [ ] Configuration system
- [ ] Basic CLI structure with `clap`
- [ ] Logging and tracing setup
- [ ] CI/CD pipeline (GitHub Actions)

### Phase 2: Image Processing Core (Weeks 3-5)
- [ ] `rtools-image` crate
- [ ] Compress (mozjpeg, webp, avif)
- [ ] Format conversion (HEIC, WebP, AVIF, JPEG XL)
- [ ] Resize with multiple algorithms (Lanczos, Triangle, Catmull-Rom)
- [ ] Crop with gravity/anchor points
- [ ] Batch processing with `rayon`
- [ ] Unit tests for each operation

### Phase 3: PDF Processing Core (Weeks 6-8)
- [ ] `rtools-pdf` crate
- [ ] PDFium integration for rendering
- [ ] Merge, split, compress
- [ ] Page manipulation (rotate, delete, reorder)
- [ ] Text extraction and OCR (Tesseract)
- [ ] PDF to image conversion
- [ ] Office → PDF (LibreOffice headless or `docx2pdf`)

### Phase 4: AI Integration (Weeks 9-12)
- [ ] `rtools-ai` crate
- [ ] Model management (download, cache, load)
- [ ] CLIP integration for organization/sorting
- [ ] BLIP for captioning/alt-text
- [ ] OCR pipeline
- [ ] Duplicate detection (perceptual hashing + CLIP embeddings)
- [ ] Film filter LUTs

### Phase 5: CLI & API (Weeks 13-15)
- [ ] Full CLI command implementation
- [ ] REST API with `axum`
- [ ] gRPC API with `tonic`
- [ ] OpenAPI/Swagger documentation
- [ ] Authentication & rate limiting
- [ ] Batch job management

### Phase 6: MCP Server (Weeks 16-17)
- [ ] `rtools-mcp` crate
- [ ] MCP tool definitions for all operations
- [ ] Resource handling for files
- [ ] Progress notifications
- [ ] Testing with Claude/other MCP clients

### Phase 7: WASM & Polish (Weeks 18-20)
- [ ] `rtools-wasm` crate
- [ ] Browser-compatible builds
- [ ] Performance optimization
- [ ] Documentation
- [ ] Release preparation

---

## Testing Strategy

| Level | Tools | Coverage Target |
|-------|-------|-----------------|
| Unit | `cargo test` | 90%+ |
| Integration | `cargo test --test integration` | 80%+ |
| Property-based | `proptest` | Key algorithms |
| Benchmarks | `criterion` | All hot paths |
| WASM | `wasm-pack test` | Core operations |

---

## Performance Targets

| Operation | Target |
|-----------|--------|
| JPEG compress (10MP) | < 500ms |
| WebP convert (10MP) | < 300ms |
| PDF merge (100 pages) | < 2s |
| OCR (A4 page) | < 3s |
| AI organize (100 photos) | < 10s |
| Batch resize (1000 images) | < 30s |

---

## Security Considerations

- No network access by default (local-only)
- Sandboxed WASM execution
- Input validation on all boundaries
- Memory-safe Rust (no `unsafe` unless audited)
- Temporary file cleanup
- No telemetry without opt-in

---

## License

MIT OR Apache-2.0
