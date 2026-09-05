# MCP server reference

The rTools MCP server processes local image and PDF paths. The runtime
capability registry and [capability table](operations/capabilities.md) are the
source of truth. A listed tool can still represent an unavailable capability;
such calls return a tool error with `structuredContent.code` and do not create
an artifact.

## Setup

Build `rtools-mcp` and configure an MCP client to launch the resulting binary:

```bash
cargo build --release --bin rtools-mcp
```

```json
{
  "mcpServers": {
    "rtools": { "command": "/absolute/path/to/rtools-mcp" }
  }
}
```

All input directories must exist and be readable. Directory traversal errors
are returned rather than skipped. Explicit output parents must exist and must
not contain symlink ancestors. Outputs never replace an existing path.

## Verified adapter contract

The verification scripts parse this table and compare each operation state
with the runtime registry. `structured_errors=true` means processor failures
include at least `code`, `message`, and optional `operation_id` in
`structuredContent`.

| Tool | Operation ID | State | Adapter contract |
|---|---|---|---|
| `compress_image` | `image.compress` | `available` | `quality=1..100`; `structured_errors=true` |
| `convert_image` | `image.convert` | `available` | explicit target format; `quality=1..100`; `structured_errors=true` |
| `resize_image` | `image.resize` | `available` | width and/or height; fixed output quality 85; `structured_errors=true` |
| `organize_photos` | `ai.organize.date` | `experimental` | date only; derived output directories must exist; `structured_errors=true` |
| `rename_photos` | `ai.rename.deterministic` | `experimental` | deterministic tokens only; `structured_errors=true` |
| `generate_alt_text` | `ai.alt_text` | `unavailable` | no provider; `structured_errors=true` |
| `find_duplicates` | `ai.duplicates.report` | `experimental` | report only; finite threshold 0..1; `structured_errors=true` |
| `compress_pdf` | `pdf.compress` | `experimental` | `level=medium`; light/heavy unavailable; `structured_errors=true` |
| `merge_pdfs` | `pdf.merge` | `experimental` | two or more PDFs; `structured_errors=true` |
| `extract_text` | `ai.ocr` | `unavailable` | no OCR provider; `structured_errors=true` |
| `get_metadata` | `image.exif.json` | `available` | read-only EXIF and file information; `structured_errors=true` |

Unknown enum values, including unknown PDF compression levels, are invalid MCP
parameters. Recognized but unavailable values return
`CAPABILITY_UNAVAILABLE`; they are never substituted with a supported value.
Only PDF `medium` compression is implemented. The MCP server does not expose a
PDF split tool in Milestone 1.

Date organization uses filesystem modification time and never performs AI
classification. Deterministic rename supports `{date}`, `{time}`,
`{datetime}`, `{index}`, `{name}`, and `{ext}`. `{subject}` and AI-generated
names are unavailable. Alt text and OCR are unavailable because no verified
provider adapter is registered; there is no model download or cache workflow.

## Error shape

Processor failures are MCP tool errors (`isError: true`) with a human-readable
text block and a machine-readable object:

```json
{
  "isError": true,
  "structuredContent": {
    "code": "CAPABILITY_UNAVAILABLE",
    "message": "Capability unavailable for ai.alt_text: ...",
    "operation_id": "ai.alt_text"
  }
}
```

Malformed tool arguments and unknown enum values use MCP invalid-parameter
errors. No MCP tool implements background jobs, retention, hosted downloads,
authentication, TLS termination, AI model acquisition, or OCR setup.
