# REST API Reference

## Milestone 1 transport and lifecycle

The REST server listens on `http://127.0.0.1:8080` by default. It currently
supports plaintext HTTP, no authentication, and wildcard CORS only. This makes
the default suitable for loopback development, not direct exposure to an
untrusted network. Configuration fails closed if authentication, an API key,
TLS material, TLS, or a custom CORS allowlist is requested; none of those
settings is silently ignored.

There is no application-level rate limiter, job queue, durable artifact store,
or retention guarantee in Milestone 1. `api.max_upload_size` is enforced as the
maximum encoded HTTP request-body size (100 MiB by default), including
multipart framing and scalar fields.

Successful writing operations return an opaque artifact object:

```json
{
  "artifact": {
    "id": "artifact-random-token.png",
    "download_url": "/api/v1/artifacts/artifact-random-token.png",
    "name": "photo_compressed.png",
    "media_type": "image/png"
  }
}
```

Use `GET /api/v1/artifacts/:id` while the same server process is running. The
response uses `Cache-Control: private, no-store`. Artifacts live only in a
server-owned temporary directory and are deleted when that server instance
shuts down. Public responses never contain host filesystem paths. Clients that
need durable storage must download and store the bytes themselves.

## Multipart rules

Endpoint schemas are strict:

- File and scalar field names must match the tables below.
- A singular file or scalar field may appear only once. `files` may repeat.
- Scalar fields must be valid UTF-8 and use the documented integer, number,
  boolean (`true` or `false`), or enum syntax. Non-finite numbers are rejected.
- Uploaded files require a filename. Client filenames are reduced to a display
  basename and are never used as a storage path. Absolute paths, traversal
  components, duplicate filenames, reserved names, and unusual characters
  cannot select or overwrite server files.
- Unknown, duplicate, missing, or invalid fields return structured HTTP 400.

## Available image endpoints

### `POST /api/v1/image/compress`

| Field | Type | Required | Default | Values |
|---|---|---:|---|---|
| `file` | file | yes | - | Supported image filename extension required |
| `quality` | integer | no | `image.default_quality` | 1-100 |
| `format` | string | no | input format | `jpg`, `jpeg`, `png`, `webp`, `avif`, `tiff`, `bmp`, `gif`, `ico` |
| `preserve_metadata` | boolean | no | `false` | `true` returns `CAPABILITY_UNAVAILABLE` |
| `strip_gps` | boolean | no | `false` | `true` returns `CAPABILITY_UNAVAILABLE` |

```bash
curl -X POST http://127.0.0.1:8080/api/v1/image/compress \
  -F "file=@photo.jpg" -F "quality=85" -F "format=webp"
```

The JSON response includes `success`, `message`, `artifact`, optional `stats`,
and metadata/orientation `warnings` when present.

### `POST /api/v1/image/convert`

| Field | Type | Required | Default | Values |
|---|---|---:|---|---|
| `file` | file | yes | - | Supported image filename extension required |
| `format` | string | yes | - | Same formats as image compress |
| `quality` | integer | no | `image.default_quality` | 1-100 |
| `preserve_metadata` | boolean | no | `false` | `true` is unavailable |
| `strip_gps` | boolean | no | `false` | `true` is unavailable |

The JSON response includes `success`, `message`, `artifact`, and optional
`warnings`.

### `POST /api/v1/image/resize`

| Field | Type | Required | Default | Values |
|---|---|---:|---|---|
| `file` | file | yes | - | Supported image filename extension required |
| `width` | integer | conditionally | - | Positive pixel width |
| `height` | integer | conditionally | - | Positive pixel height |
| `maintain_aspect` | boolean | no | `true` | `true` or `false` |

At least one of `width` or `height` is required. The JSON response includes an
opaque `artifact`; resize output and decoded input limits come from the shared
resource configuration.

### `POST /api/v1/image/metadata`

| Field | Type | Required | Default |
|---|---|---:|---|
| `file` | file | yes | - |
| `include_exif` | boolean | no | `true` |
| `include_dimensions` | boolean | no | `true` |
| `include_file_info` | boolean | no | `true` |

This read-only endpoint returns `success` and `metadata`; it creates no
download artifact.

## Available PDF endpoints

### `POST /api/v1/pdf/merge`

Repeat the `files` field at least twice. Order in the multipart body is merge
order. No other fields are accepted.

```bash
curl -X POST http://127.0.0.1:8080/api/v1/pdf/merge \
  -F "files=@first.pdf" -F "files=@second.pdf"
```

The response artifact is a downloadable PDF named `merged.pdf`.

### `POST /api/v1/pdf/compress`

| Field | Type | Required | Default | Values |
|---|---|---:|---|---|
| `file` | file | yes | - | PDF |
| `level` | string | no | `pdf.compression_level` (`medium` by default) | Only `medium` is effective in Milestone 1 |
| `remove_metadata` | boolean | no | `false` | `true` or `false` |

`light` and `heavy` return `CAPABILITY_UNAVAILABLE` rather than pretending to
change behavior. The response artifact is a downloadable PDF.

## Available AI endpoints

### `POST /api/v1/ai/organize`

Repeat `files` one or more times. Optional `strategy` defaults to `date`.
Recognized strategies `subject`, `type`, and `gps` return
`CAPABILITY_UNAVAILABLE` before request files are written; any other value is
invalid and returns HTTP 400. Successful responses contain an `artifacts`
array. Date organization is experimental.

### `POST /api/v1/ai/rename`

| Field | Type | Required | Default |
|---|---|---:|---|
| `files` | file(s) | yes | - |
| `pattern` | string | no | `{date}_{name}_{index}` |
| `start_number` | integer | no | `1` |

Supported deterministic tokens are `{date}`, `{time}`, `{datetime}`, `{index}`,
`{name}`, and `{ext}`. The pattern must produce one portable filename; paths,
reserved device names, malformed tokens, and the AI `{subject}` token are
rejected. Successful responses contain `names` and downloadable `artifacts`.

### `POST /api/v1/ai/duplicates`

| Field | Type | Required | Default | Values |
|---|---|---:|---|---|
| `files` | file(s) | yes | - | One or more images |
| `threshold` | number | no | `0.9` | Finite value from 0.0 through 1.0 |
| `algorithm` | string | no | `perceptual` | `average`, `perceptual`, `difference` |

This experimental endpoint is report-only. It returns counts and never moves,
deletes, or links uploaded files.

## Unavailable REST adapters

These routes return HTTP 501 with `CAPABILITY_UNAVAILABLE` before parsing or
writing an uploaded body:

- `POST /api/v1/image/crop`
- `POST /api/v1/image/filter`
- `POST /api/v1/image/watermark`
- `POST /api/v1/pdf/split`
- `POST /api/v1/pdf/ocr`
- `POST /api/v1/ai/alt-text`

Their corresponding CLI/core operations may have a different capability state;
the statement here is specifically about the Milestone-1 REST adapter.

## Errors

Handler errors are JSON:

```json
{
  "success": false,
  "code": "INVALID_INPUT",
  "message": "Invalid input: Multipart field 'width' must be an integer"
}
```

Common mappings are HTTP 400 for invalid input/unsupported formats, 409 for an
existing output, 413 for configured resource limits, 501 for unavailable
capabilities, and 500 for processing/configuration failures. Oversized
multipart requests use the same structured resource-limit error shape.

## Health check

`GET /health` returns:

```json
{
  "status": "ok",
  "version": "0.1.0"
}
```
