use derive_more::Display;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Supported image formats
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Display)]
#[serde(rename_all = "lowercase")]
pub enum ImageFormat {
    #[display("jpeg")]
    Jpeg,
    #[display("png")]
    Png,
    #[display("webp")]
    Webp,
    #[display("avif")]
    Avif,
    #[display("heic")]
    Heic,
    #[display("heif")]
    Heif,
    #[display("tiff")]
    Tiff,
    #[display("bmp")]
    Bmp,
    #[display("gif")]
    Gif,
    #[display("ico")]
    Ico,
    #[display("jxl")]
    Jxl,
    #[display("hdr")]
    Hdr,
    #[display("exr")]
    Exr,
    #[display("pdf")]
    Pdf,
}

impl ImageFormat {
    /// Get file extensions for this format
    pub const fn extensions(&self) -> &'static [&'static str] {
        match self {
            ImageFormat::Jpeg => &["jpg", "jpeg"],
            ImageFormat::Png => &["png"],
            ImageFormat::Webp => &["webp"],
            ImageFormat::Avif => &["avif"],
            ImageFormat::Heic => &["heic"],
            ImageFormat::Heif => &["heif"],
            ImageFormat::Tiff => &["tiff", "tif"],
            ImageFormat::Bmp => &["bmp"],
            ImageFormat::Gif => &["gif"],
            ImageFormat::Ico => &["ico"],
            ImageFormat::Jxl => &["jxl"],
            ImageFormat::Hdr => &["hdr"],
            ImageFormat::Exr => &["exr"],
            ImageFormat::Pdf => &["pdf"],
        }
    }

    /// Detect format from file extension
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_lowercase().as_str() {
            "jpg" | "jpeg" => Some(ImageFormat::Jpeg),
            "png" => Some(ImageFormat::Png),
            "webp" => Some(ImageFormat::Webp),
            "avif" => Some(ImageFormat::Avif),
            "heic" => Some(ImageFormat::Heic),
            "heif" => Some(ImageFormat::Heif),
            "tiff" | "tif" => Some(ImageFormat::Tiff),
            "bmp" => Some(ImageFormat::Bmp),
            "gif" => Some(ImageFormat::Gif),
            "ico" => Some(ImageFormat::Ico),
            "jxl" => Some(ImageFormat::Jxl),
            "hdr" => Some(ImageFormat::Hdr),
            "exr" => Some(ImageFormat::Exr),
            "pdf" => Some(ImageFormat::Pdf),
            _ => None,
        }
    }

    /// Detect format from file path
    pub fn from_path(path: &Path) -> Option<Self> {
        path.extension()
            .and_then(|ext| ext.to_str())
            .and_then(Self::from_extension)
    }

    /// Get MIME type string
    pub const fn mime_type(&self) -> &'static str {
        match self {
            ImageFormat::Jpeg => "image/jpeg",
            ImageFormat::Png => "image/png",
            ImageFormat::Webp => "image/webp",
            ImageFormat::Avif => "image/avif",
            ImageFormat::Heic => "image/heic",
            ImageFormat::Heif => "image/heif",
            ImageFormat::Tiff => "image/tiff",
            ImageFormat::Bmp => "image/bmp",
            ImageFormat::Gif => "image/gif",
            ImageFormat::Ico => "image/ico",
            ImageFormat::Jxl => "image/jxl",
            ImageFormat::Hdr => "image/hdr",
            ImageFormat::Exr => "image/exr",
            ImageFormat::Pdf => "application/pdf",
        }
    }
}

/// Supported PDF operations output formats
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PdfOutputFormat {
    Pdf,
    Png,
    Jpeg,
    Webp,
}

/// Content type for MIME detection
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Display)]
#[serde(rename_all = "lowercase")]
pub enum ContentType {
    #[display("image/jpeg")]
    ImageJpeg,
    #[display("image/png")]
    ImagePng,
    #[display("image/webp")]
    ImageWebp,
    #[display("image/avif")]
    ImageAvif,
    #[display("image/heic")]
    ImageHeic,
    #[display("image/heif")]
    ImageHeif,
    #[display("image/tiff")]
    ImageTiff,
    #[display("image/bmp")]
    ImageBmp,
    #[display("image/gif")]
    ImageGif,
    #[display("image/ico")]
    ImageIco,
    #[display("image/jxl")]
    ImageJxl,
    #[display("application/pdf")]
    ApplicationPdf,
    #[display("application/octet-stream")]
    OctetStream,
}

/// Image metadata extracted from EXIF and file info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageMetadata {
    pub width: u32,
    pub height: u32,
    pub format: ImageFormat,
    pub file_size: u64,
    pub color_space: Option<String>,
    pub bit_depth: Option<u16>,
    pub exif: Option<ExifData>,
}

/// PDF metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PdfMetadata {
    pub page_count: usize,
    pub page_sizes: Vec<PageSize>,
    pub title: Option<String>,
    pub author: Option<String>,
    pub subject: Option<String>,
    pub creator: Option<String>,
    pub producer: Option<String>,
    pub creation_date: Option<String>,
    pub modification_date: Option<String>,
    pub file_size: u64,
    pub is_encrypted: bool,
    pub has_images: bool,
    pub has_text_layer: bool,
}

/// Page size for PDF
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageSize {
    pub width: f64,
    pub height: f64,
    pub unit: PageSizeUnit,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PageSizeUnit {
    Points,
    Millimeters,
    Inches,
}

/// EXIF data container
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExifData {
    pub camera_make: Option<String>,
    pub camera_model: Option<String>,
    pub lens_model: Option<String>,
    pub datetime_original: Option<String>,
    pub datetime_digitized: Option<String>,
    pub gps_latitude: Option<f64>,
    pub gps_longitude: Option<f64>,
    pub gps_altitude: Option<f64>,
    pub exposure_time: Option<String>,
    pub f_number: Option<f64>,
    pub iso: Option<u32>,
    pub focal_length: Option<f64>,
    pub flash: Option<u16>,
    pub orientation: Option<u32>,
}

/// Processing statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessStats {
    pub input_size: u64,
    pub output_size: u64,
    pub compression_ratio: f64,
    pub processing_time_ms: u64,
    pub memory_used_mb: f64,
}
