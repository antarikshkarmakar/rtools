# Operation capabilities

This table is the checked-in, sorted projection of the runtime capability
registry returned by:

```bash
cargo run --locked --quiet -p rtools-cli -- --output-format json doctor
```

`available` operations are supported release paths. `experimental` operations
are executable but retain the stated release-safety limitation. `unavailable`
operations fail closed with `CAPABILITY_UNAVAILABLE`; installing or discovering
an external executable does not enable an operation until rTools has a verified
adapter for it.

The verification scripts compare every operation ID and state below with the
doctor JSON. They reject missing, extra, duplicate, unsorted, or misclassified
rows.

| Operation ID | State | Current contract |
|---|---|---|
| `ai.alt_text` | `unavailable` | No verified image-captioning provider adapter. |
| `ai.duplicates.delete` | `unavailable` | Destructive duplicate deletion is not implemented safely. |
| `ai.duplicates.move` | `unavailable` | Destructive duplicate moves are not implemented safely. |
| `ai.duplicates.report` | `experimental` | Report-only duplicate ranking has limited release-safety coverage. |
| `ai.duplicates.symlink` | `unavailable` | Destructive duplicate replacement with symlinks is not implemented safely. |
| `ai.ocr` | `unavailable` | No verified Tesseract OCR adapter. |
| `ai.organize.camera` | `unavailable` | Camera-based classification is not implemented. |
| `ai.organize.custom` | `unavailable` | Custom classification is not implemented. |
| `ai.organize.date` | `experimental` | Uses filesystem modification time when EXIF date is unavailable. |
| `ai.organize.location` | `unavailable` | Location classification is not implemented. |
| `ai.organize.subject` | `unavailable` | Subject classification is not implemented. |
| `ai.rename.ai` | `unavailable` | AI-generated filename descriptions are not implemented. |
| `ai.rename.deterministic` | `experimental` | Deterministic tokens are supported with limited release-safety coverage. |
| `batch.run` | `unavailable` | Typed batch recipe execution is not implemented. |
| `completions.generate` | `available` | Generate shell completions. |
| `config.init` | `available` | Create a configuration file without replacing an existing file. |
| `config.show` | `available` | Show the effective configuration with secrets redacted. |
| `config.validate` | `available` | Validate an explicit configuration file. |
| `doctor.report` | `available` | Report capabilities, provider diagnostics, limits, and writable paths. |
| `image.compress` | `available` | Compress a bounded single-frame image. |
| `image.convert` | `available` | Convert a bounded single-frame image. |
| `image.crop` | `available` | Crop a bounded single-frame image. |
| `image.exif.human` | `available` | Inspect EXIF metadata in human-readable form. |
| `image.exif.json` | `available` | Inspect EXIF metadata as one JSON report. |
| `image.filter` | `available` | Apply a supported film-filter preset to a bounded single-frame image. |
| `image.metadata.preserve` | `unavailable` | Metadata preservation is not implemented and fails before output creation. |
| `image.metadata.strip_gps` | `unavailable` | Selective GPS-only removal is not implemented; use drop-all. |
| `image.ocr` | `unavailable` | No verified Tesseract OCR adapter. |
| `image.resize` | `available` | Resize a bounded single-frame image. |
| `image.watermark.image` | `available` | Apply an image watermark with bounded decode and validated placement. |
| `image.watermark.text` | `unavailable` | Text rendering is not implemented for watermarks. |
| `pdf.compress` | `experimental` | Collision-safe publication requires an existing validated parent; PDF structure preservation is only partially verified. |
| `pdf.merge` | `experimental` | Collision-safe publication requires an existing validated parent; PDF structure preservation is only partially verified. |
| `pdf.ocr` | `unavailable` | No verified searchable-PDF OCR adapter. |
| `pdf.split` | `experimental` | The output directory must exist and validate before all page destinations are reserved; PDF structure preservation is only partially verified. |
| `pdf.text` | `unavailable` | PDF text extraction is not implemented in the CLI. |
| `pdf.to_image` | `unavailable` | No verified PDFium rendering adapter. |

Run `rtools --output-format json doctor` for the complete machine-readable
report, including provider configuration and executable diagnostics. Provider
diagnostics are informational: they never turn an unavailable operation into a
successful placeholder path.
