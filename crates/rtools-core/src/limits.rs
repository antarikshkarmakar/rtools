use crate::error::{RToolsError, RToolsResult};
use serde::{Deserialize, Serialize};

/// Upper bounds applied before resource-intensive processing begins.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ResourceLimits {
    /// Maximum number of bytes accepted from one input file.
    pub max_input_bytes: u64,
    /// Maximum number of pixels accepted after decoding an image header.
    pub max_decoded_pixels: u64,
    /// Maximum number of pages accepted from one PDF.
    pub max_pdf_pages: u64,
    /// Maximum number of items accepted in one batch.
    pub max_batch_items: u64,
    /// Maximum operation duration in milliseconds.
    pub max_duration_ms: u64,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            max_input_bytes: 100 * 1024 * 1024,
            max_decoded_pixels: 100_000_000,
            max_pdf_pages: 2_000,
            max_batch_items: 10_000,
            max_duration_ms: 300_000,
        }
    }
}

impl ResourceLimits {
    /// Check a file's byte size before reading or decoding it.
    ///
    /// # Errors
    ///
    /// Returns `ResourceLimitExceeded` when the size exceeds `max_input_bytes`.
    pub const fn check_input_bytes(&self, actual: u64) -> RToolsResult<()> {
        Self::check("input_bytes", actual, self.max_input_bytes)
    }

    /// Check image dimensions before allocating decoded pixels.
    ///
    /// # Errors
    ///
    /// Returns `ResourceLimitExceeded` when the dimensions overflow or exceed
    /// `max_decoded_pixels`.
    pub fn check_decoded_pixels(&self, width: u32, height: u32) -> RToolsResult<()> {
        let actual = u64::from(width).checked_mul(u64::from(height)).ok_or(
            RToolsError::ResourceLimitExceeded {
                resource: "decoded_pixels",
                actual: u64::MAX,
                limit: self.max_decoded_pixels,
            },
        )?;
        Self::check("decoded_pixels", actual, self.max_decoded_pixels)
    }

    /// Check a PDF page count before processing pages.
    ///
    /// # Errors
    ///
    /// Returns `ResourceLimitExceeded` when the count exceeds `max_pdf_pages`.
    pub const fn check_pdf_pages(&self, actual: u64) -> RToolsResult<()> {
        Self::check("pdf_pages", actual, self.max_pdf_pages)
    }

    /// Check a batch item count before processing its inputs.
    ///
    /// # Errors
    ///
    /// Returns `ResourceLimitExceeded` when the count exceeds `max_batch_items`.
    pub const fn check_batch_items(&self, actual: u64) -> RToolsResult<()> {
        Self::check("batch_items", actual, self.max_batch_items)
    }

    /// Check an operation duration before permitting additional work.
    ///
    /// # Errors
    ///
    /// Returns `ResourceLimitExceeded` when the duration exceeds
    /// `max_duration_ms`.
    pub const fn check_duration_ms(&self, actual: u64) -> RToolsResult<()> {
        Self::check("duration_ms", actual, self.max_duration_ms)
    }

    const fn check(resource: &'static str, actual: u64, limit: u64) -> RToolsResult<()> {
        if actual > limit {
            return Err(RToolsError::ResourceLimitExceeded {
                resource,
                actual,
                limit,
            });
        }
        Ok(())
    }
}
