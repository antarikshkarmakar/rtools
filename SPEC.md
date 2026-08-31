# rTools - Rust Image & PDF Processing Toolkit

## Project Overview

A high-performance, local-first image and PDF processing toolkit written in Rust, accessible via:
- **MCP (Model Context Protocol)** - For AI assistant integration
- **CLI** - For direct command-line usage
- **REST/gRPC API** - For service integration

**Core Philosophy**: Privacy-first, local processing, zero server uploads, WebAssembly-compatible.

---

## Feature Categories

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
│   ├── rtools-api/            # REST/gRPC API server (axum/tonic)
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

# Image commands
rtools image compress --input dir/ --output dir/ --quality 85
rtools image convert --to webp --input *.jpg --output out/
rtools image resize --width 1920 --height 1080 --maintain-aspect
rtools image crop --ratio 16:9 --gravity center
rtools image watermark --text "© 2024" --position bottom-right --opacity 0.5
rtools image filter --preset kodak-portra-400

# AI commands
rtools ai organize --input photos/ --output organized/
rtools ai rename --pattern "{date}_{subject}_{index}"
rtools ai alt-text --input image.jpg --lang en
rtools ai ocr --input scanned.pdf --output text.txt

# PDF commands
rtools pdf merge --input file1.pdf file2.pdf --output merged.pdf
rtools pdf compress --level heavy --input large.pdf --output small.pdf
rtools pdf split --pages 1-5,10-15 --input doc.pdf --output split/
rtools pdf ocr --input scanned.pdf --output searchable.pdf
rtools pdf redact --patterns "SSN:\d{3}-\d{2}-\d{4}" --input doc.pdf

# Batch operations
rtools batch --config batch.toml --parallel 4
```

### REST API (rtools-api)

```
POST   /api/v1/image/compress
POST   /api/v1/image/convert
POST   /api/v1/image/resize
POST   /api/v1/image/crop
POST   /api/v1/image/watermark
POST   /api/v1/image/filter

POST   /api/v1/ai/organize
POST   /api/v1/ai/rename
POST   /api/v1/ai/alt-text
POST   /api/v1/ai/ocr

POST   /api/v1/pdf/merge
POST   /api/v1/pdf/compress
POST   /api/v1/pdf/split
POST   /api/v1/pdf/ocr
POST   /api/v1/pdf/redact
POST   /api/v1/pdf/encrypt

POST   /api/v1/batch/process
GET    /api/v1/batch/status/:id
GET    /api/v1/health
```

### MCP Interface (rtools-mcp)

```json
{
  "name": "rtools",
  "version": "1.0.0",
  "tools": [
    {
      "name": "compress_image",
      "description": "Compress image with quality preservation",
      "inputSchema": {
        "type": "object",
        "properties": {
          "input_path": {"type": "string"},
          "output_path": {"type": "string"},
          "quality": {"type": "integer", "minimum": 1, "maximum": 100}
        },
        "required": ["input_path", "output_path"]
      }
    },
    {
      "name": "convert_to_webp",
      "description": "Convert JPG/PNG to WebP",
      "inputSchema": {...}
    },
    {
      "name": "ai_organize_photos",
      "description": "AI-organize photos into folders",
      "inputSchema": {...}
    },
    {
      "name": "merge_pdfs",
      "description": "Merge multiple PDFs",
      "inputSchema": {...}
    }
  ]
}
```

---

## Technology Stack

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

[pdf]
pdfium_path = "/usr/lib/libpdfium.so"
ocr_language = "eng"
ocr_dpi = 300

[ai]
model_dir = "~/.rtools/models"
device = "cpu"  # cpu, cuda, metal
batch_size = 8

[api]
host = "127.0.0.1"
port = 8080
max_upload_size = "100MB"
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