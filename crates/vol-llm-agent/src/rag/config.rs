//! RAG configuration.

/// RAG agent configuration
#[derive(Debug, Clone)]
pub struct RagConfig {
    /// Number of documents to retrieve
    pub top_k: usize,
    /// Similarity threshold (0-1, documents below this are filtered)
    pub similarity_threshold: f32,
    /// Whether to return raw scores
    pub return_scores: bool,
    /// Maximum tokens for LLM generation
    pub max_tokens: u32,
    /// Temperature for generation (low for accuracy)
    pub temperature: f32,
}

impl Default for RagConfig {
    fn default() -> Self {
        Self {
            top_k: 5,
            similarity_threshold: 0.3,
            return_scores: true,
            max_tokens: 1024,
            temperature: 0.1, // Low temperature for factual accuracy
        }
    }
}

impl RagConfig {
    /// Create a new config with custom top_k
    pub fn with_top_k(mut self, top_k: usize) -> Self {
        self.top_k = top_k;
        self
    }

    /// Set similarity threshold
    pub fn with_threshold(mut self, threshold: f32) -> Self {
        self.similarity_threshold = threshold;
        self
    }

    /// Set temperature
    pub fn with_temperature(mut self, temp: f32) -> Self {
        self.temperature = temp;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = RagConfig::default();
        assert_eq!(config.top_k, 5);
        assert_eq!(config.similarity_threshold, 0.3);
        assert_eq!(config.temperature, 0.1);
    }

    #[test]
    fn test_builder_pattern() {
        let config = RagConfig::default()
            .with_top_k(10)
            .with_threshold(0.5)
            .with_temperature(0.2);

        assert_eq!(config.top_k, 10);
        assert_eq!(config.similarity_threshold, 0.5);
        assert_eq!(config.temperature, 0.2);
    }
}
