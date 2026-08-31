# API Reference

## Base URL

```
http://localhost:8080
```

## Authentication

API key authentication (optional):

```bash
curl -H "X-API-Key: your-api-key" http://localhost:8080/api/v1/image/compress
```

## Image Endpoints

### POST /api/v1/image/compress

Compress an image with quality preservation.

**Request:**
```bash
curl -X POST http://localhost:8080/api/v1/image/compress \
  -F "file=@photo.jpg" \
  -F "quality=85" \
  -F "format=webp"
```

**Parameters:**
| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| file | File | Yes | - | Image file to compress |
| quality | Integer | No | 85 | Quality (1-100) |
| format | String | No | - | Output format (jpg, png, webp, avif) |
| preserve_metadata | Boolean | No | true | Keep EXIF data |
| strip_gps | Boolean | No | false | Remove GPS coordinates |

**Response:**
```json
{
  "success": true,
  "message": "Compressed photo.jpg",
  "output_path": "/tmp/rtools/photo_compressed.jpg",
  "stats": {
    "input_size": 5242880,
    "output_size": 1048576,
    "compression_ratio": 0.2,
    "processing_time_ms": 150
  }
}
```

### POST /api/v1/image/convert

Convert image to different format.

**Request:**
```bash
curl -X POST http://localhost:8080/api/v1/image/convert \
  -F "file=@photo.jpg" \
  -F "format=webp" \
  -F "quality=90"
```

**Parameters:**
| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| file | File | Yes | - | Image file to convert |
| format | String | Yes | - | Target format (webp, png, jpg, avif, tiff) |
| quality | Integer | No | 85 | Quality for lossy formats |

**Response:**
```json
{
  "success": true,
  "message": "Converted photo.jpg to webp",
  "output_path": "/tmp/rtools/photo.webp"
}
```

### POST /api/v1/image/resize

Resize image by dimensions.

**Request:**
```bash
curl -X POST http://localhost:8080/api/v1/image/resize \
  -F "file=@photo.jpg" \
  -F "width=1920" \
  -F "height=1080" \
  -F "maintain_aspect=true"
```

**Parameters:**
| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| file | File | Yes | - | Image file to resize |
| width | Integer | No | - | Target width in pixels |
| height | Integer | No | - | Target height in pixels |
| maintain_aspect | Boolean | No | true | Maintain aspect ratio |

**Response:**
```json
{
  "success": true,
  "message": "Resized photo.jpg",
  "output_path": "/tmp/rtools/photo_1920x1080.jpg"
}
```

### POST /api/v1/image/crop

Crop image to specific region.

**Request:**
```bash
curl -X POST http://localhost:8080/api/v1/image/crop \
  -F "file=@photo.jpg" \
  -F "region=100,100,800,600" \
  -F "ratio=16:9" \
  -F "gravity=center"
```

**Parameters:**
| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| file | File | Yes | - | Image file to crop |
| region | String | No | - | Crop region (x,y,width,height) |
| ratio | String | No | - | Aspect ratio (16:9, 4:3, 1:1) |
| gravity | String | No | center | Gravity point (center, north, south, etc.) |

### POST /api/v1/image/watermark

Add watermark to image.

**Request:**
```bash
curl -X POST http://localhost:8080/api/v1/image/watermark \
  -F "file=@photo.jpg" \
  -F "text=© 2024 My Company" \
  -F "position=bottom-right" \
  -F "opacity=0.5"
```

**Parameters:**
| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| file | File | Yes | - | Image file |
| text | String | No | - | Watermark text |
| image | File | No | - | Watermark image |
| position | String | No | bottom-right | Position (topleft, topright, bottomleft, bottomright, center) |
| opacity | Float | No | 0.5 | Opacity (0.0-1.0) |

### POST /api/v1/image/filter

Apply film filter to image.

**Request:**
```bash
curl -X POST http://localhost:8080/api/v1/image/filter \
  -F "file=@photo.jpg" \
  -F "preset=kodak-portra-400" \
  -F "strength=1.0"
```

**Parameters:**
| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| file | File | Yes | - | Image file |
| preset | String | Yes | - | Filter preset (kodak-portra-400, fuji-velvia-50, etc.) |
| strength | Float | No | 1.0 | Filter strength (0.0-1.0) |

**Available Presets:**
- `kodak-portra-400` / `portra`
- `kodak-gold-200` / `gold`
- `kodak-ektar-100` / `ektar`
- `fuji-pro-400h` / `fuji`
- `fuji-velvia-50` / `velvia`
- `fuji-superia-400` / `superia`
- `polaroid-sx70` / `polaroid`
- `polaroid-600`
- `ilford-hp5` / `hp5`
- `ilford-fp4` / `fp4`
- `trix-400` / `trix`
- `cinestill-800t` / `cinestill`
- `lomography-400` / `lomo`
- `agfa-vista-200` / `agfa`

### POST /api/v1/image/metadata

Get image metadata and EXIF data.

**Request:**
```bash
curl -X POST http://localhost:8080/api/v1/image/metadata \
  -F "file=@photo.jpg"
```

**Response:**
```json
{
  "success": true,
  "metadata": {
    "width": 4032,
    "height": 3024,
    "format": "Jpeg",
    "file_size": 5242880,
    "color_space": "Rgb8",
    "exif": {
      "camera_make": "Apple",
      "camera_model": "iPhone 15 Pro",
      "datetime_original": "2024-01-15 14:30:22",
      "gps_latitude": 37.7749,
      "gps_longitude": -122.4194,
      "exposure_time": "1/125",
      "f_number": 1.8,
      "iso": 100,
      "focal_length": 6.86
    }
  }
}
```

## PDF Endpoints

### POST /api/v1/pdf/merge

Merge multiple PDF files.

**Request:**
```bash
curl -X POST http://localhost:8080/api/v1/pdf/merge \
  -F "files=@file1.pdf" \
  -F "files=@file2.pdf" \
  -F "files=@file3.pdf"
```

**Parameters:**
| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| files | File[] | Yes | - | PDF files to merge (minimum 2) |

**Response:**
```json
{
  "success": true,
  "message": "Merged 3 PDFs",
  "output_path": "/tmp/rtools/merged.pdf"
}
```

### POST /api/v1/pdf/compress

Compress PDF file size.

**Request:**
```bash
curl -X POST http://localhost:8080/api/v1/pdf/compress \
  -F "file=@document.pdf" \
  -F "level=medium"
```

**Parameters:**
| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| file | File | Yes | - | PDF file to compress |
| level | String | No | medium | Compression level (light, medium, heavy) |

**Response:**
```json
{
  "success": true,
  "message": "Compressed document.pdf",
  "output_path": "/tmp/rtools/document_compressed.pdf",
  "stats": {
    "input_size": 10485760,
    "output_size": 3145728,
    "compression_ratio": 0.3
  }
}
```

### POST /api/v1/pdf/split

Split PDF into individual pages.

**Request:**
```bash
curl -X POST http://localhost:8080/api/v1/pdf/split \
  -F "file=@document.pdf" \
  -F "pages=1-5,10,15-20"
```

**Parameters:**
| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| file | File | Yes | - | PDF file to split |
| pages | String | No | all | Page ranges (e.g., "1-5,10,15-20") |

### POST /api/v1/pdf/ocr

Extract text from scanned PDF.

**Request:**
```bash
curl -X POST http://localhost:8080/api/v1/pdf/ocr \
  -F "file=@scanned.pdf" \
  -F "language=eng" \
  -F "dpi=300"
```

**Parameters:**
| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| file | File | Yes | - | PDF file to OCR |
| language | String | No | eng | Tesseract language |
| dpi | Integer | No | 300 | Resolution for OCR |

## AI Endpoints

### POST /api/v1/ai/organize

AI-organize photos into folders.

**Request:**
```bash
curl -X POST http://localhost:8080/api/v1/ai/organize \
  -F "files=@photo1.jpg" \
  -F "files=@photo2.jpg" \
  -F "files=@photo3.jpg" \
  -F "strategy=date"
```

**Parameters:**
| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| files | File[] | Yes | - | Photos to organize |
| strategy | String | No | date | Strategy (date, subject, location) |

**Response:**
```json
{
  "success": true,
  "message": "Organized 3 photos",
  "results": {
    "count": 3,
    "folders_created": ["2024/01", "2024/02"]
  }
}
```

### POST /api/v1/ai/rename

AI-rename photos with descriptive names.

**Request:**
```bash
curl -X POST http://localhost:8080/api/v1/ai/rename \
  -F "files=@IMG_001.jpg" \
  -F "files=@IMG_002.jpg" \
  -F "pattern={date}_{subject}_{index}"
```

**Parameters:**
| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| files | File[] | Yes | - | Photos to rename |
| pattern | String | No | {date}_{subject}_{index} | Filename pattern |

**Pattern Variables:**
- `{date}` - Date (YYYYMMDD)
- `{time}` - Time (HHMMSS)
- `{datetime}` - Date and time
- `{subject}` - AI-detected subject
- `{index}` - Sequential number
- `{name}` - Original filename

### POST /api/v1/ai/alt-text

Generate accessibility alt text.

**Request:**
```bash
curl -X POST http://localhost:8080/api/v1/ai/alt-text \
  -F "file=@photo.jpg" \
  -F "language=en"
```

**Parameters:**
| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| file | File | Yes | - | Image file |
| language | String | No | en | Language code |

**Response:**
```json
{
  "success": true,
  "results": [
    {
      "path": "photo.jpg",
      "alt_text": "A golden retriever playing fetch in a sunny park",
      "confidence": 0.92
    }
  ]
}
```

### POST /api/v1/ai/duplicates

Find duplicate images by visual similarity.

**Request:**
```bash
curl -X POST http://localhost:8080/api/v1/ai/duplicates \
  -F "files=@photo1.jpg" \
  -F "files=@photo2.jpg" \
  -F "files=@photo3.jpg" \
  -F "threshold=0.9"
```

**Parameters:**
| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| files | File[] | Yes | - | Photos to check |
| threshold | Float | No | 0.9 | Similarity threshold (0.0-1.0) |

**Response:**
```json
{
  "success": true,
  "results": {
    "groups": 2,
    "originals": 1,
    "duplicates": 2
  }
}
```

## Error Responses

All endpoints return errors in this format:

```json
{
  "error": {
    "code": "INVALID_INPUT",
    "message": "Quality must be between 1 and 100"
  }
}
```

**Error Codes:**
| Code | HTTP Status | Description |
|------|-------------|-------------|
| INVALID_INPUT | 400 | Invalid request parameters |
| FILE_NOT_FOUND | 404 | Input file not found |
| UNSUPPORTED_FORMAT | 400 | File format not supported |
| PROCESSING_ERROR | 500 | Error during processing |
| TIMEOUT | 504 | Processing timeout |
| RATE_LIMITED | 429 | Too many requests |

## Rate Limiting

Default limits:
- 100 requests per minute
- 10 concurrent uploads
- 100MB max file size

## Health Check

```bash
curl http://localhost:8080/health
```

**Response:**
```json
{
  "status": "ok",
  "version": "0.1.0"
}
```