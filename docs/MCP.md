# MCP Server Reference

## Overview

The rtools MCP server provides AI-powered image and PDF processing capabilities that can be used with Claude Desktop and other MCP-compatible clients.

## Installation

### Build from Source

```bash
git clone https://github.com/yourusername/rtools.git
cd rtools
cargo build --release --bin rtools-mcp
```

The binary will be at `target/release/rtools-mcp`.

## Configuration

### Claude Desktop

Add to `claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "rtools": {
      "command": "/path/to/rtools-mcp",
      "env": {}
    }
  }
}
```

### Claude Code (CLI)

```bash
claude mcp add rtools /path/to/rtools-mcp
```

## Available Tools

### Image Processing Tools

#### `compress_image`

Compress an image with quality preservation.

**Input Schema:**
```json
{
  "input_path": "string (required)",
  "output_path": "string (optional)",
  "quality": "integer (optional, 1-100, default: 85)"
}
```

**Example:**
```
User: Compress the photo at /path/to/photo.jpg with quality 80
Assistant: I'll compress that image for you.
[Uses compress_image tool with input_path="/path/to/photo.jpg", quality=80]
```

#### `convert_image`

Convert image to different format (WebP, PNG, JPG, AVIF).

**Input Schema:**
```json
{
  "input_path": "string (required)",
  "target_format": "string (required)",
  "output_path": "string (optional)",
  "quality": "integer (optional, 1-100, default: 85)"
}
```

**Supported Formats:**
- `webp` - WebP (recommended for web)
- `png` - PNG (lossless)
- `jpg` / `jpeg` - JPEG
- `avif` - AVIF (modern, best compression)

**Example:**
```
User: Convert my PNG image to WebP format
Assistant: I'll convert that to WebP for you.
[Uses convert_image tool with target_format="webp"]
```

#### `resize_image`

Resize image by dimensions.

**Input Schema:**
```json
{
  "input_path": "string (required)",
  "width": "integer (optional)",
  "height": "integer (optional)",
  "maintain_aspect": "boolean (optional, default: true)"
}
```

**Example:**
```
User: Resize the banner image to 1920 pixels wide
Assistant: I'll resize that for you while maintaining the aspect ratio.
[Uses resize_image tool with width=1920]
```

### Organization Tools

#### `organize_photos`

AI-organize photos into folders.

**Input Schema:**
```json
{
  "input_dir": "string (required)",
  "output_dir": "string (required)",
  "strategy": "string (optional, default: 'date')"
}
```

**Strategies:**
- `date` - Organize by year/month
- `subject` - Organize by AI-detected subject
- `location` - Organize by GPS coordinates

**Example:**
```
User: Organize all my photos from ~/Pictures into ~/Organized
Assistant: I'll organize your photos by date into year/month folders.
[Uses organize_photos tool]
```

#### `rename_photos`

AI-rename photos with descriptive names.

**Input Schema:**
```json
{
  "input_dir": "string (required)",
  "pattern": "string (optional, default: '{date}_{subject}_{index}')",
  "dry_run": "boolean (optional, default: false)"
}
```

**Pattern Variables:**
- `{date}` - Date (YYYYMMDD)
- `{time}` - Time (HHMMSS)
- `{datetime}` - Date and time
- `{subject}` - AI-detected subject
- `{index}` - Sequential number
- `{name}` - Original filename

**Example:**
```
User: Rename all photos in ~/Downloads with descriptive names
Assistant: I'll rename your photos with AI-generated descriptions.
[Uses rename_photos tool]
```

#### `generate_alt_text`

Generate accessibility alt text for an image.

**Input Schema:**
```json
{
  "input_path": "string (required)",
  "language": "string (optional, default: 'en')"
}
```

**Example:**
```
User: Generate alt text for my product image
Assistant: I'll generate descriptive alt text for accessibility.
[Uses generate_alt_text tool]
Assistant: The alt text is: "A sleek black wireless headphone set on a white background with subtle shadow"
```

#### `find_duplicates`

Find duplicate images by visual similarity.

**Input Schema:**
```json
{
  "input_dir": "string (required)",
  "threshold": "number (optional, 0.0-1.0, default: 0.9)"
}
```

**Example:**
```
User: Check if I have any duplicate photos in ~/Photos
Assistant: I'll scan for duplicate images using perceptual hashing.
[Uses find_duplicates tool]
Assistant: Found 3 duplicate groups with 7 duplicate files total.
```

### PDF Tools

#### `compress_pdf`

Compress PDF file size.

**Input Schema:**
```json
{
  "input_path": "string (required)",
  "output_path": "string (optional)",
  "level": "string (optional, default: 'medium')"
}
```

**Levels:**
- `light` - Minimal compression, best quality
- `medium` - Balanced compression
- `heavy` - Maximum compression

**Example:**
```
User: Compress this PDF, it's too large to email
Assistant: I'll compress the PDF to reduce its size.
[Uses compress_pdf tool with level="heavy"]
```

#### `merge_pdfs`

Merge multiple PDF files into one.

**Input Schema:**
```json
{
  "input_paths": ["string array (required, minimum 2)"],
  "output_path": "string (required)"
}
```

**Example:**
```
User: Merge chapter1.pdf, chapter2.pdf, and chapter3.pdf into book.pdf
Assistant: I'll merge those three PDFs into a single document.
[Uses merge_pdfs tool]
```

### Text Extraction Tools

#### `extract_text`

Extract text from image or PDF using OCR.

**Input Schema:**
```json
{
  "input_path": "string (required)",
  "language": "string (optional, default: 'eng')"
}
```

**Supported Languages:**
- `eng` - English
- `fra` - French
- `deu` - German
- `spa` - Spanish
- `ita` - Italian
- `por` - Portuguese
- `rus` - Russian
- `jpn` - Japanese
- `chi_sim` - Chinese (Simplified)
- `chi_tra` - Chinese (Traditional)
- `kor` - Korean
- `ara` - Arabic

**Example:**
```
User: Extract all text from this scanned document
Assistant: I'll use OCR to extract the text from the scan.
[Uses extract_text tool]
Assistant: I found the following text in the document: "..."
```

#### `get_metadata`

Get image metadata including EXIF data.

**Input Schema:**
```json
{
  "input_path": "string (required)"
}
```

**Returns:**
- Image dimensions
- File size
- Color space
- EXIF data (camera, lens, date, GPS, exposure, etc.)

**Example:**
```
User: What camera settings were used for this photo?
Assistant: I'll extract the EXIF metadata for you.
[Uses get_metadata tool]
Assistant: The photo was taken with an iPhone 15 Pro, f/1.8, 1/125s, ISO 100, at 6.86mm focal length.
```

## Tool Usage Examples

### Complete Workflow

```
User: I have 500 photos from my vacation. Can you organize them, rename with descriptive names, and find any duplicates?

Assistant: I'll help you with that! Let me:
1. First, organize your photos by date
2. Then rename them with descriptive names
3. Finally, check for any duplicates

[Uses organize_photos tool]
Assistant: Organized 500 photos into 12 date-based folders.

[Uses rename_photos tool]
Assistant: Renamed all photos with descriptive names like "2024-01-15_sunset_beach_001.jpg"

[Uses find_duplicates tool]
Assistant: Found 15 duplicate photos (exact matches) that can be removed.
```

### Batch Processing

```
User: Compress all my vacation photos to WebP for my website

Assistant: I'll compress and convert all your photos to WebP format.

[For each image, uses compress_image tool with target_format="webp"]

Assistant: Successfully processed 500 photos:
- Average compression: 65% smaller
- Format: WebP
- Ready for web deployment
```

## Error Handling

The MCP server returns errors in a consistent format:

```json
{
  "error": {
    "code": "PROCESSING_ERROR",
    "message": "Failed to compress image: Invalid quality value"
  }
}
```

**Common Errors:**
- `FILE_NOT_FOUND` - Input file doesn't exist
- `INVALID_INPUT` - Invalid parameters
- `UNSUPPORTED_FORMAT` - File format not supported
- `PROCESSING_ERROR` - Error during processing

## Performance Notes

- Images are processed locally (no cloud upload)
- Large batches may take some time
- AI operations (organize, rename) require model download on first use
- Models are cached in `~/.rtools/models/`