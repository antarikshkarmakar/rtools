use crate::types::ImageFormat;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Input source for processing operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum InputSource {
    /// File on local filesystem
    File(PathBuf),
    /// Directory of files
    Directory(PathBuf),
    /// Raw bytes in memory
    Bytes(Vec<u8>),
    /// URL to fetch
    Url(String),
    /// Glob pattern
    Glob(String),
}

impl InputSource {
    /// Check if the source is a single file
    #[must_use]
    pub const fn is_file(&self) -> bool {
        matches!(self, InputSource::File(_))
    }

    /// Check if the source is a directory
    #[must_use]
    pub const fn is_directory(&self) -> bool {
        matches!(self, InputSource::Directory(_))
    }

    /// Get the path if this is a file source
    pub const fn as_path(&self) -> Option<&PathBuf> {
        match self {
            InputSource::File(path) | InputSource::Directory(path) => Some(path),
            _ => None,
        }
    }
}

/// Process input with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileInput {
    /// The input source
    pub source: InputSource,
    /// Detected format
    pub format: Option<ImageFormat>,
    /// File name
    pub name: Option<String>,
    /// Expected MIME type
    pub mime_type: Option<String>,
}

impl FileInput {
    /// Create a new file input from a path
    pub fn from_path(path: PathBuf) -> Self {
        let format = ImageFormat::from_path(&path);
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .map(std::string::ToString::to_string);

        Self {
            source: InputSource::File(path),
            format,
            name,
            mime_type: None,
        }
    }

    /// Create a new file input from bytes
    pub fn from_bytes(data: Vec<u8>, name: impl Into<String>) -> Self {
        let name = name.into();
        let format = ImageFormat::from_extension(name.rsplit('.').next().unwrap_or(""));

        Self {
            source: InputSource::Bytes(data),
            format,
            name: Some(name),
            mime_type: None,
        }
    }

    /// Create a new file input from a directory
    pub fn from_directory(path: PathBuf) -> Self {
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .map(std::string::ToString::to_string);

        Self {
            source: InputSource::Directory(path),
            format: None,
            name,
            mime_type: None,
        }
    }

    /// Create a new file input from a glob pattern
    pub fn from_glob(pattern: impl Into<String>) -> Self {
        Self {
            source: InputSource::Glob(pattern.into()),
            format: None,
            name: None,
            mime_type: None,
        }
    }
}

/// Conversion input for format conversion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversionInput {
    /// The file to convert
    pub file: FileInput,
    /// Target output format
    pub target_format: ImageFormat,
    /// Output path (optional)
    pub output_path: Option<PathBuf>,
}

/// Resize input
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResizeInput {
    /// The file to resize
    pub file: FileInput,
    /// Target width (optional)
    pub width: Option<u32>,
    /// Target height (optional)
    pub height: Option<u32>,
    /// Maintain aspect ratio
    pub maintain_aspect: bool,
    /// Resize algorithm
    pub algorithm: ResizeAlgorithm,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ResizeAlgorithm {
    Lanczos,
    Triangle,
    CatmullRom,
    NearestNeighbor,
    Lanczos3,
}

/// Crop input
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CropInput {
    /// The file to crop
    pub file: FileInput,
    /// Crop region
    pub region: CropRegion,
    /// Output path (optional)
    pub output_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CropRegion {
    /// Fixed pixel coordinates: x, y, width, height
    Pixels {
        x: u32,
        y: u32,
        width: u32,
        height: u32,
    },
    /// Standard aspect ratio with gravity
    AspectRatio {
        ratio: AspectRatio,
        gravity: Gravity,
    },
    /// Percentage-based crop
    Percentage {
        x: f64,
        y: f64,
        width: f64,
        height: f64,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AspectRatio {
    Original,
    Square,
    Portrait,
    Landscape,
    Wide,
    Ultrawide,
    Cinema,
    Custom(f64, f64),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Gravity {
    Center,
    North,
    South,
    East,
    West,
    NorthEast,
    NorthWest,
    SouthEast,
    SouthWest,
}
