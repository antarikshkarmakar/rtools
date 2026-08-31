use crate::error::RToolsResult;
use crate::input::ProcessInput;
use crate::output::ProcessOutput;
use crate::types::ProcessStats;

/// Core processor trait for single-item operations
pub trait Processor: Send + Sync {
    /// Input type for this processor
    type Input: Send + Sync;
    /// Output type for this processor
    type Output: Send + Sync;
    /// Configuration type for this processor
    type Config: Send + Sync;
    /// Error type for this processor
    type Error: Into<crate::error::RToolsError> + std::fmt::Display;

    /// Process a single input
    fn process(&self, input: Self::Input, config: Self::Config) -> RToolsResult<Self::Output>;

    /// Validate configuration before processing
    fn validate_config(&self, config: &Self::Config) -> RToolsResult<()>;

    /// Estimate output size (optional optimization hint)
    fn estimate_output_size(
        &self,
        _input: &Self::Input,
        _config: &Self::Config,
    ) -> Option<u64> {
        None
    }

    /// Get processor name for logging
    fn name(&self) -> &str;
}

/// Batch processor trait for multi-item operations
pub trait BatchProcessor: Processor {
    /// Process multiple inputs
    fn process_batch(
        &self,
        inputs: Vec<Self::Input>,
        config: Self::Config,
    ) -> RToolsResult<Vec<Self::Output>> {
        inputs
            .into_iter()
            .map(|input| self.process(input, config.clone()))
            .collect()
    }

    /// Process inputs as a streaming iterator
    fn process_streaming(
        &self,
        inputs: Vec<Self::Input>,
        config: Self::Config,
    ) -> impl Iterator<Item = RToolsResult<Self::Output>> {
        let name = self.name().to_string();
        inputs.into_iter().map(move |input| {
            tracing::debug!("Processing with {}: {:?}", name, input);
            self.process(input, config.clone())
        })
    }
}

/// AI-enabled processor trait
pub trait AIProcessor: Processor {
    /// Model type
    type Model: Send + Sync;

    /// Load an AI model
    fn load_model(&mut self, model: Self::Model) -> RToolsResult<()>;

    /// Unload the current model
    fn unload_model(&mut self);

    /// Check if a model is loaded
    fn is_model_loaded(&self) -> bool;

    /// List available models
    fn available_models(&self) -> Vec<String>;
}

/// Metadata extractor trait
pub trait MetadataExtractor: Send + Sync {
    /// Extract metadata from a file
    fn extract(&self, path: &std::path::Path) -> RToolsResult<crate::types::ImageMetadata>;

    /// Batch extract metadata
    fn extract_batch(
        &self,
        paths: &[std::path::PathBuf],
    ) -> RToolsResult<Vec<crate::types::ImageMetadata>> {
        paths.iter().map(|p| self.extract(p)).collect()
    }
}