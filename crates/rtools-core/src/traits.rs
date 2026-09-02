use crate::error::RToolsResult;

/// Core processor trait for single-item operations
pub trait Processor: Send + Sync {
    /// Input type for this processor
    type Input: Send + Sync;
    /// Output type for this processor
    type Output: Send + Sync;
    /// Configuration type for this processor
    type Config: Send + Sync + Clone;
    /// Error type for this processor
    type Error: Into<crate::error::RToolsError> + std::fmt::Display;

    /// Validate and process a single input.
    ///
    /// # Errors
    ///
    /// Returns an error when processing fails.
    fn process(&self, input: Self::Input, config: Self::Config) -> RToolsResult<Self::Output> {
        self.validate_config(&config)?;
        self.process_validated(input, config)
    }

    /// Process a single input after its configuration has been validated.
    ///
    /// # Errors
    ///
    /// Returns an error when processing fails.
    fn process_validated(
        &self,
        input: Self::Input,
        config: Self::Config,
    ) -> RToolsResult<Self::Output>;

    /// Validate configuration before processing.
    ///
    /// # Errors
    ///
    /// Returns an error when the configuration is invalid.
    fn validate_config(&self, config: &Self::Config) -> RToolsResult<()>;

    /// Estimate output size (optional optimization hint)
    fn estimate_output_size(&self, _input: &Self::Input, _config: &Self::Config) -> Option<u64> {
        None
    }

    /// Get processor name for logging
    fn name(&self) -> &str;
}

/// Batch processor trait for multi-item operations
pub trait BatchProcessor: Processor {
    /// Process multiple inputs.
    ///
    /// # Errors
    ///
    /// Returns the first error produced while processing an input.
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
    ) -> impl Iterator<Item = RToolsResult<Self::Output>>
    where
        Self::Input: std::fmt::Debug,
    {
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

    /// Load an AI model.
    ///
    /// # Errors
    ///
    /// Returns an error when the model cannot be loaded.
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
    /// Extract metadata from a file.
    ///
    /// # Errors
    ///
    /// Returns an error when metadata cannot be extracted.
    fn extract(&self, path: &std::path::Path) -> RToolsResult<crate::types::ImageMetadata>;

    /// Batch extract metadata.
    ///
    /// # Errors
    ///
    /// Returns the first error produced while extracting metadata.
    fn extract_batch(
        &self,
        paths: &[std::path::PathBuf],
    ) -> RToolsResult<Vec<crate::types::ImageMetadata>> {
        paths.iter().map(|p| self.extract(p)).collect()
    }
}
