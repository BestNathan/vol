//! Embedding generation trait.

use async_trait::async_trait;
use vol_llm_core::Result;

/// Embedding generator trait
///
/// Implementors can use API calls (OpenAI, DashScope, etc.) or local models.
#[async_trait]
pub trait Embedder: Send + Sync {
    /// Generate embedding for a single text
    async fn embed(&self, text: &str) -> Result<Vec<f32>>;

    /// Generate embeddings for multiple texts
    ///
    /// Default implementation: serial calls to `embed`
    async fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        let mut embeddings = Vec::with_capacity(texts.len());
        for text in texts {
            embeddings.push(self.embed(text).await?);
        }
        Ok(embeddings)
    }
}
