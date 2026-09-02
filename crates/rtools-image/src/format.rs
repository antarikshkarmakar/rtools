use rtools_core::types::ImageFormat;

/// Image format utilities
pub const fn is_lossy(format: &ImageFormat) -> bool {
    matches!(
        format,
        ImageFormat::Jpeg | ImageFormat::Webp | ImageFormat::Avif
    )
}

pub const fn supports_transparency(format: &ImageFormat) -> bool {
    matches!(
        format,
        ImageFormat::Png
            | ImageFormat::Webp
            | ImageFormat::Avif
            | ImageFormat::Gif
            | ImageFormat::Ico
    )
}
