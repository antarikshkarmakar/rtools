# Milestone 1 architecture

This document describes the current executable build. Roadmap components are
kept in [SPEC.md](../SPEC.md) and are not part of this architecture until a
verified adapter is registered in the runtime capability registry.

## Runtime boundaries

```mermaid
graph LR
    subgraph Adapters
        CLI[rtools-cli]
        API[rtools-api]
        MCP[rtools-mcp]
        WASM[rtools-wasm]
    end

    subgraph Processors
        CORE[rtools-core<br/>contracts, limits, output policy]
        IMG[rtools-image<br/>bounded image operations]
        PDF[rtools-pdf<br/>experimental structural operations]
        AI[rtools-ai<br/>deterministic/report-only operations]
    end

    FS[Local filesystem]
    ART[REST in-memory artifact store]

    CLI --> CORE
    API --> CORE
    MCP --> CORE
    WASM --> CORE
    CORE --> IMG
    CORE --> PDF
    CORE --> AI
    IMG --> FS
    PDF --> FS
    AI --> FS
    API --> ART
```

- `rtools-core` defines processor contracts, stable errors, resource limits,
  configuration, path validation, collision policy, temporary reservations,
  and atomic publication.
- CLI, REST, and MCP translate public parameters into those processor types.
  They do not enable unavailable behavior by silently changing an option.
- REST uploads live in request-owned temporary storage. Successful REST files
  are copied into an in-memory artifact store and returned by opaque artifact
  ID. There is no durable retention guarantee.
- MCP operates on server-local paths, but public results and errors do not
  expose host filesystem paths. It has no hosted-download contract.

## Image write path

```mermaid
flowchart TD
    Request --> Capability[Validate capability and options]
    Capability --> Input[Validate input and bounded decode]
    Input --> Plan[Resolve and validate output]
    Plan --> Reserve[Reserve destination and sibling temporary file]
    Reserve --> Encode[Encode with drop-all metadata policy]
    Encode --> Verify[Verify artifact]
    Verify --> Publish[Atomic publication]
    Publish --> Result[Return stats, MIME type, and warnings]
```

Executable image operations are compress, convert, resize, crop, supported
filters, image watermarking, and read-only EXIF inspection. JPEG quality is
1 through 100. Text watermarking, OCR, metadata preservation, and selective
GPS removal fail closed before output reservation.

## PDF write path

```mermaid
flowchart TD
    Request --> Options[Reject unsupported options]
    Options --> Inputs[Validate every ordered path input]
    Inputs --> Limits[Read metadata and enforce validation]
    Limits --> Load[Load PDFs with lopdf]
    Load --> Operation{Merge, medium compress, or PDF split}
    Operation --> Reserve[Reserve every final destination]
    Reserve --> Stage[Write and validate temporary artifacts]
    Stage --> Publish[Publish atomically]
```

PDF merge, medium compression, and PDF-page splitting are experimental because
structural fidelity has not been fully verified. Output parents must already
exist and cannot traverse symlink ancestors. Merge page numbering, light/heavy
compression, PDF-to-image, text extraction, and PDF OCR are unavailable.

## Deterministic organization and duplicate reporting

```mermaid
flowchart TD
    Request --> Gate[Validate strategy, pattern, or action]
    Gate --> Collect[Walk existing readable directory]
    Collect --> Sort[Sort paths deterministically]
    Sort --> Mode{Operation}
    Mode -->|Date organize| Dates[Plan year/month copies]
    Mode -->|Deterministic rename| Names[Preflight every final name]
    Mode -->|Duplicate report| Hash[Compute bounded perceptual hashes]
    Dates --> Commit[Transactional publication or dry-run plan]
    Names --> Commit
    Hash --> Report[Return report only]
```

Only date organization, deterministic rename tokens, and report-only duplicate
detection execute. Subject/location/camera/custom organization, AI-derived
rename tokens, alt text, OCR, sorting, and destructive duplicate actions are
unavailable. No model cache, model download, CLIP, BLIP, Tesseract, ONNX, or
geocoding pipeline exists in Milestone 1.

## Capability and adapter verification

`rtools --output-format json doctor` exports the operation registry.
`rtools-mcp --print-contracts` exports the MCP tool-to-operation contract that
also drives live `tools/list`. The Bash and PowerShell verification scripts
compare both runtime exports with their checked-in documentation, reject
duplicates, and run the structured adapter behavior matrix.

## Module dependencies

```mermaid
graph TD
    CLI[rtools-cli] --> Core[rtools-core]
    API[rtools-api] --> Core
    MCP[rtools-mcp] --> Core
    WASM[rtools-wasm] --> Core
    CLI --> Img[rtools-image]
    CLI --> PDF[rtools-pdf]
    CLI --> AI[rtools-ai]
    API --> Img
    API --> PDF
    API --> AI
    MCP --> Img
    MCP --> PDF
    MCP --> AI
    WASM --> Img
    Img --> Core
    PDF --> Core
    AI --> Core
    AI --> Img
```

Authentication, TLS termination, background jobs, batch execution, provider
model management, and durable artifact retention are outside the current
runtime boundary.
