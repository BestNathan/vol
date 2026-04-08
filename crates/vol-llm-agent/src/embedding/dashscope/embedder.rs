//! DashScope Embedding API implementation.
//!
//! Provides `DashScopeEmbedder` for generating embeddings using Alibaba Cloud DashScope.
//! Supports models like `text-embedding-v2`, `text-embedding-v3`.
//!
//! # Environment Variables
//!
//! - `DASHSCOPE_API_KEY`: API key for DashScope
//!
//! # Example
//!
//! ```rust,no_run
//! use vol_llm_agent::embedding::{DashScopeEmbedder, Embedder};
//!
//! #[tokio::main]
//! async fn main() {
//!     let embedder = DashScopeEmbedder::new("your-api-key");
//!     let embedding = embedder.embed("Hello, world!").await.unwrap();
//! }
//! ```

use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use vol_llm_core::Result;

use super::config::DashScopeConfig;
use crate::embedding::Embedder;

/// DashScope Embedder for generating embeddings
pub struct DashScopeEmbedder {
    client: Client,
    config: DashScopeConfig,
}

impl DashScopeEmbedder {
    /// Create a new DashScopeEmbedder with API key
    pub fn new(api_key: &str) -> Self {
        Self {
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .expect("Failed to create HTTP client"),
            config: DashScopeConfig::default().with_api_key(api_key),
        }
    }

    /// Create with custom configuration
    pub fn with_config(config: DashScopeConfig) -> Self {
        Self {
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(config.timeout_secs))
                .build()
                .expect("Failed to create HTTP client"),
            config,
        }
    }

    /// Create from environment variable DASHSCOPE_API_KEY
    pub fn from_env() -> Self {
        Self::with_config(DashScopeConfig::default())
    }
}

#[derive(Debug, Serialize)]
struct EmbeddingRequest {
    model: String,
    input: Vec<String>,
    parameters: Option<EmbeddingParameters>,
}

#[derive(Debug, Serialize)]
struct EmbeddingParameters {
    #[serde(skip_serializing_if = "Option::is_none")]
    text_type: Option<String>,
}

#[derive(Debug, Deserialize)]
struct EmbeddingResponse {
    output: EmbeddingOutput,
    #[allow(dead_code)]
    usage: EmbeddingUsage,
    #[allow(dead_code)]
    request_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct EmbeddingOutput {
    embeddings: Vec<Vec<f32>>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct EmbeddingUsage {
    total_tokens: u32,
}

#[async_trait]
impl Embedder for DashScopeEmbedder {
    async fn embed(&self, text: &str) -> Result<Vec<f32>> {
        let embeddings = self.embed_batch(&[text]).await?;
        Ok(embeddings.into_iter().next().unwrap_or_default())
    }

    async fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        let request = EmbeddingRequest {
            model: self.config.model.as_str().to_string(),
            input: texts.iter().map(|s| s.to_string()).collect(),
            parameters: None,
        };

        let response = self
            .client
            .post(&format!("{}/embeddings", self.config.base_url))
            .header("Authorization", &format!("Bearer {}", self.config.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| vol_llm_core::LLMError::Network(e))?;

        let status = response.status();
        if !status.is_success() {
            let body: String = response.text().await.unwrap_or_default();
            return Err(vol_llm_core::LLMError::Api {
                status: status.as_u16(),
                message: format!("DashScope API error: {} - {}", status, body),
            });
        }

        let result: EmbeddingResponse = response
            .json()
            .await
            .map_err(|e| {
                vol_llm_core::LLMError::Parse(format!("Failed to parse response: {}", e))
            })?;

        Ok(result.output.embeddings)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::embedding::dashscope::config::DashScopeModel;

    #[test]
    fn test_model_as_str() {
        assert_eq!(DashScopeModel::TextEmbeddingV2.as_str(), "text-embedding-v2");
        assert_eq!(DashScopeModel::TextEmbeddingV3.as_str(), "text-embedding-v3");
    }

    #[test]
    fn test_model_dimensions() {
        assert_eq!(DashScopeModel::TextEmbeddingV2.dimensions(), 1536);
        assert_eq!(DashScopeModel::TextEmbeddingV3.dimensions(), 1024);
    }

    #[test]
    fn test_config_default() {
        let config = DashScopeConfig::default();
        assert_eq!(config.model, DashScopeModel::TextEmbeddingV2);
        assert!(config.base_url.contains("dashscope"));
    }

    #[test]
    fn test_config_builder() {
        let config = DashScopeConfig::default()
            .with_api_key("test-key")
            .with_model(DashScopeModel::TextEmbeddingV3)
            .with_timeout(60);

        assert_eq!(config.api_key, "test-key");
        assert_eq!(config.model, DashScopeModel::TextEmbeddingV3);
        assert_eq!(config.timeout_secs, 60);
    }

    #[test]
    fn test_embedder_creation() {
        let _embedder = DashScopeEmbedder::new("test-key");
        let _embedder_from_env = DashScopeEmbedder::from_env();
    }
}
