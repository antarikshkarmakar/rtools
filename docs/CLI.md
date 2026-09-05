# CLI Reference

## Usage

```bash
rtools [GLOBAL_OPTIONS] <COMMAND> [COMMAND_OPTIONS]
```

## Global Options

| Option | Short | Description |
|--------|-------|-------------|
| `--config` | `-c` | Configuration file path |
| `--verbose` | `-v` | Enable verbose output |
| `--dry-run` | `-d` | Exact no-write plans for date organize/deterministic rename; other commands fail unavailable |
| `--help` | `-h` | Show help |
| `--version` | `-V` | Show version |

## Commands

### Image Processing

All explicit filesystem output paths require an existing parent directory.
rTools validates that parent before reserving or encoding an artifact and does
not create missing output directories. For example, run `mkdir -p processed`
before passing `--output processed/photo.png`.

#### `rtools image compress`

Compress images with quality preservation.

```bash
rtools image compress [OPTIONS]
```

**Options:**
| Option | Short | Default | Description |
|--------|-------|---------|-------------|
| `--input` | `-i` | - | Input file(s) |
| `--output` | `-o` | - | Output path |
| `--quality` | `-q` | 85 | Quality (1-100) |
| `--format` | `-f` | - | Output format |
| `--preserve-metadata` | - | false | Unavailable; `true` fails before input/output access |
| `--strip-gps` | - | false | Unavailable; `true` fails before input/output access |

The safe executable metadata policy is the default: writing operations
re-encode and drop metadata. Metadata preservation and selective GPS removal
are recognized compatibility flags, but both return
`CAPABILITY_UNAVAILABLE` when selected.

**Examples:**
```bash
# Compress single image
rtools image compress -i photo.jpg -q 80

# Compress all JPEGs in directory
rtools image compress -i *.jpg -o compressed/

# Compress and convert to WebP
rtools image compress -i photo.jpg -f webp -q 85
```

#### `rtools image convert`

Convert image format.

```bash
rtools image convert [OPTIONS]
```

**Options:**
| Option | Short | Default | Description |
|--------|-------|---------|-------------|
| `--input` | `-i` | - | Input file(s) |
| `--format` | `-f` | - | Target format |
| `--output` | `-o` | - | Output path |
| `--quality` | `-q` | 85 | Quality for lossy formats (1-100) |

**Examples:**
```bash
# Convert to WebP
rtools image convert -i photo.jpg -f webp

# Convert all PNGs to JPEG
rtools image convert -i *.png -f jpg -q 90

# Convert to AVIF
rtools image convert -i photo.png -f avif
```

#### `rtools image resize`

Resize images.

```bash
rtools image resize [OPTIONS]
```

**Options:**
| Option | Short | Default | Description |
|--------|-------|---------|-------------|
| `--input` | `-i` | - | Input file(s) |
| `--width` | `-w` | - | Target width |
| `--height` | - | - | Target height |
| `--maintain-aspect` | - | true | Maintain aspect ratio |
| `--output` | `-o` | - | Output path |

**Examples:**
```bash
# Resize to 1920px width
rtools image resize -i photo.jpg -w 1920

# Resize to specific dimensions
rtools image resize -i photo.jpg -w 800 -h 600

# Resize without maintaining aspect ratio
rtools image resize -i photo.jpg -w 1000 -h 1000 --maintain-aspect=false
```

#### `rtools image crop`

Crop images.

```bash
rtools image crop [OPTIONS]
```

**Options:**
| Option | Short | Default | Description |
|--------|-------|---------|-------------|
| `--input` | `-i` | - | Input file(s) |
| `--region` | `-r` | - | Crop region (x,y,w,h) |
| `--ratio` | `-a` | - | Aspect ratio |
| `--gravity` | `-g` | center | Gravity point |
| `--output` | `-o` | - | Output path |

**Examples:**
```bash
# Crop to 16:9
rtools image crop -i photo.jpg -a 16:9

# Crop specific region
rtools image crop -i photo.jpg -r 100,100,800,600

# Crop with gravity
rtools image crop -i photo.jpg -a 1:1 -g north
```

#### `rtools image watermark`

Add watermark to images.

```bash
rtools image watermark [OPTIONS]
```

**Options:**
| Option | Short | Default | Description |
|--------|-------|---------|-------------|
| `--input` | `-i` | - | Input file(s) |
| `--text` | `-t` | - | Unavailable text watermark; fails before file access |
| `--image` | - | - | Watermark image (executable mode) |
| `--position` | `-p` | bottom-right | Position |
| `--opacity` | - | 0.5 | Opacity (0.0-1.0) |
| `--output` | `-o` | - | Output path |

**Examples:**
```bash
# Add image watermark
rtools image watermark -i photo.jpg --image logo.png

# Custom image-watermark position and opacity
rtools image watermark -i photo.jpg --image logo.png -p center --opacity 0.3
```

`--text` remains visible for compatibility but returns
`CAPABILITY_UNAVAILABLE`; this build has no text-rendering provider.

#### `rtools image filter`

Apply film filter to images.

```bash
rtools image filter [OPTIONS]
```

**Options:**
| Option | Short | Default | Description |
|--------|-------|---------|-------------|
| `--input` | `-i` | - | Input file(s) |
| `--preset` | `-p` | - | Filter preset |
| `--strength` | - | 1.0 | Filter strength |
| `--output` | `-o` | - | Output path |

**CLI presets:**
- `kodak-portra-400` / `portra`
- `kodak-gold-200` / `gold`
- `fuji-pro-400h` / `fuji`
- `fuji-velvia-50` / `velvia`
- `polaroid-sx70` / `polaroid`
- `trix-400` / `trix`

**Examples:**
```bash
# Apply Kodak Portra filter
rtools image filter -i photo.jpg -p portra

# Apply with reduced strength
rtools image filter -i photo.jpg -p fuji --strength 0.5
```

#### `rtools image exif`

View EXIF metadata.

```bash
rtools image exif [OPTIONS]
```

**Options:**
| Option | Short | Default | Description |
|--------|-------|---------|-------------|
| `--input` | `-i` | - | Input file(s) |
| `--format` | `-f` | human | Output format (`human`, `json`) |

**Examples:**
```bash
# View EXIF data
rtools image exif -i photo.jpg

# Export as JSON
rtools image exif -i photo.jpg -f json
```

#### `rtools image ocr`

Unavailable in Milestone 1. The command is registered for compatibility but
returns `CAPABILITY_UNAVAILABLE` before reading the image or writing output;
no verified Tesseract adapter is configured.

```bash
rtools image ocr [OPTIONS]
```

**Options:**
| Option | Short | Default | Description |
|--------|-------|---------|-------------|
| `--input` | `-i` | - | Input file(s) |
| `--language` | `-l` | eng | Tesseract language |
| `--output` | `-o` | - | Output file |

There is intentionally no executable OCR example for this build.

### PDF Processing

#### `rtools pdf merge`

Merge multiple PDFs.

```bash
rtools pdf merge [OPTIONS]
```

**Options:**
| Option | Short | Description |
|--------|-------|-------------|
| `--input` | `-i` | Input PDF files (minimum 2) |
| `--output` | `-o` | Output file |

**Examples:**
```bash
# Merge two PDFs
rtools pdf merge -i file1.pdf file2.pdf -o merged.pdf

# Merge multiple PDFs
rtools pdf merge -i *.pdf -o combined.pdf
```

#### `rtools pdf compress`

Compress PDF file size.

```bash
rtools pdf compress [OPTIONS]
```

**Options:**
| Option | Short | Default | Description |
|--------|-------|---------|-------------|
| `--input` | `-i` | - | Input PDF file |
| `--output` | `-o` | - | Output file |
| `--level` | `-l` | medium | Only implemented level; light/heavy fail unavailable |

**Examples:**
```bash
# Medium compression (the only implemented level)
rtools pdf compress -i document.pdf -l medium
```

#### `rtools pdf split`

Split PDF into pages.

```bash
rtools pdf split [OPTIONS]
```

**Options:**
| Option | Short | Description |
|--------|-------|-------------|
| `--input` | `-i` | Input PDF file |
| `--pages` | `-p` | Page ranges |
| `--output` | `-o` | Existing output directory |

**Examples:**
```bash
# Split all pages
rtools pdf split -i document.pdf -o pages/

# Extract specific pages
rtools pdf split -i document.pdf -p 1-5,10,15-20 -o extracted/
```

### AI Operations

#### `rtools ai organize`

Organize photos deterministically by modification date. AI-derived strategies
are unavailable. A live run requires each planned year/month directory to
already exist; use global `--dry-run` first to inspect the exact paths.

```bash
rtools ai organize [OPTIONS]
```

**Options:**
| Option | Short | Default | Description |
|--------|-------|---------|-------------|
| `--input` | `-i` | - | Input directory |
| `--output` | `-o` | - | Prepared output directory tree |
| `--strategy` | `-s` | date | `date` only; other known modes are unavailable |

**Examples:**
```bash
# Preview exact year/month destinations without writing
rtools --dry-run ai organize -i ~/Photos -o ~/Organized -s date
```

#### `rtools ai rename`

Rename photos with deterministic filename tokens. AI-generated descriptions
are unavailable.

```bash
rtools ai rename [OPTIONS]
```

**Options:**
| Option | Short | Default | Description |
|--------|-------|---------|-------------|
| `--input` | `-i` | - | Input directory |
| `--pattern` | `-p` | {date}_{name}_{index} | Deterministic filename pattern |
| `--dry-run` | - | false | Preview changes |

**Examples:**
```bash
# Rename with default pattern
rtools ai rename -i ~/Photos

# Preview rename
rtools ai rename -i ~/Photos --dry-run

# Custom pattern
rtools ai rename -i ~/Photos -p "{date}_{index}"
```

#### `rtools ai alt-text`

Unavailable in Milestone 1 because no verified captioning provider is
registered. The command returns `CAPABILITY_UNAVAILABLE` and writes nothing.

```bash
rtools ai alt-text [OPTIONS]
```

**Options:**
| Option | Short | Default | Description |
|--------|-------|---------|-------------|
| `--input` | `-i` | - | Input file(s) |
| `--language` | `-l` | en | Language |
| `--output` | `-o` | - | Output file |

There is intentionally no executable alt-text example for this build.

#### `rtools ai duplicates`

Find duplicate images.

```bash
rtools ai duplicates [OPTIONS]
```

**Options:**
| Option | Short | Default | Description |
|--------|-------|---------|-------------|
| `--input` | `-i` | - | Input directory |
| `--threshold` | `-t` | 0.9 | Finite similarity threshold (0.0-1.0) |
| `--action` | `-a` | report | Only report is executable; mutations are unavailable |

**Examples:**
```bash
# Find duplicates
rtools ai duplicates -i ~/Photos

```

`move`, `delete`, and `symlink` are recognized actions but return
`CAPABILITY_UNAVAILABLE` before mutation. Only `--action report` executes.

### Batch Processing

#### `rtools batch`

Unavailable in Milestone 1. The command returns
`CAPABILITY_UNAVAILABLE` without executing the recipe.

```bash
rtools batch [OPTIONS]
```

**Options:**
| Option | Short | Description |
|--------|-------|-------------|
| `--config` | `-c` | Batch config file |
| `--jobs` | `-j` | Parallel jobs |

There is intentionally no executable batch recipe or command example for this
build. Run supported operations individually.

### Configuration

#### `rtools config show`

Show current configuration.

```bash
rtools config show
```

#### `rtools config init`

Generate default configuration file.

```bash
rtools config init [-o rtools.toml]
```

The selected configuration file's parent directory must already exist. The
command does not create a missing parent directory.

#### `rtools config validate`

Validate configuration file.

```bash
rtools config validate -c rtools.toml
```

### Shell Completions

```bash
# Generate completions for bash
rtools completions bash > ~/.bash_completion.d/rtools

# Generate completions for zsh
rtools completions zsh > ~/.zsh/completions/_rtools

# Generate completions for fish
rtools completions fish > ~/.config/fish/completions/rtools.fish
```
