//! Embedding store trait for vector search.

use super::document::Document;
use async_trait::async_trait;
use vol_llm_core::Result;

/// Vector storage and search trait
///
/// Implementors can use ChromaDB, Qdrant, Milvus, or any other vector store.
#[async_trait]
pub trait EmbeddingStore: Send + Sync {
    /// Search for similar documents by embedding
    async fn search(&self, query: &[f32], k: usize) -> Result<Vec<Document>>;

    /// Insert a document with its embedding
    async fn insert(&self, document: Document, embedding: Vec<f32>) -> Result<()>;

    /// Insert multiple documents with embeddings
    ///
    /// Default implementation: serial calls to `insert`
    async fn insert_batch(&self, documents: &[(Document, Vec<f32>)]) -> Result<()> {
        for (doc, emb) in documents {
            self.insert(doc.clone(), emb.clone()).await?;
        }
        Ok(())
    }
}
