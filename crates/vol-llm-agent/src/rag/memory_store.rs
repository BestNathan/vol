//! In-memory vector store implementation for testing and demonstration.
//!
//! This is a simple, non-persistent store useful for:
//! - Testing RAG functionality without external dependencies
//! - Demonstrating how to implement `EmbeddingStore`
//! - Small-scale applications that don't need persistence

use async_trait::async_trait;
use std::sync::RwLock;
use vol_llm_core::Result;

use super::{Document, EmbeddingStore};

/// A document with its embedding stored in memory
struct StoredDocument {
    #[allow(dead_code)]
    id: String,
    document: Document,
    embedding: Vec<f32>,
}

/// In-memory vector store
///
/// Stores documents and their embeddings in memory. Uses cosine similarity
/// for search. Thread-safe via `RwLock`.
///
/// # Example
///
/// ```rust
/// use vol_llm_agent::rag::{InMemoryStore, EmbeddingStore, Document};
///
/// let store = InMemoryStore::new();
/// let doc = Document::new("test content".to_string());
/// let embedding = vec![0.1f32; 128];
///
/// // store.insert(doc, embedding).await.unwrap();
/// ```
pub struct InMemoryStore {
    documents: RwLock<Vec<StoredDocument>>,
}

impl Default for InMemoryStore {
    fn default() -> Self {
        Self::new()
    }
}

impl InMemoryStore {
    /// Create a new empty in-memory store
    pub fn new() -> Self {
        Self {
            documents: RwLock::new(Vec::new()),
        }
    }

    /// Create a new store with initial capacity hint
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            documents: RwLock::new(Vec::with_capacity(capacity)),
        }
    }

    /// Get the number of documents stored
    #[allow(clippy::unwrap_used)]
    pub fn len(&self) -> usize {
        self.documents.read().unwrap().len()
    }

    /// Check if the store is empty
    #[allow(clippy::unwrap_used)]
    pub fn is_empty(&self) -> bool {
        self.documents.read().unwrap().is_empty()
    }

    /// Clear all documents from the store
    #[allow(clippy::unwrap_used)]
    pub fn clear(&self) {
        let mut docs = self.documents.write().unwrap();
        docs.clear();
    }

    /// Calculate cosine similarity between two vectors
    fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
        let dot_product: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

        if norm_a == 0.0 || norm_b == 0.0 {
            0.0
        } else {
            dot_product / (norm_a * norm_b)
        }
    }
}

#[async_trait]
impl EmbeddingStore for InMemoryStore {
    #[allow(clippy::unwrap_used, clippy::expect_used)]
    async fn search(&self, query: &[f32], k: usize) -> Result<Vec<Document>> {
        let docs = self.documents.read().unwrap();

        // Calculate similarity for all documents
        let mut scores: Vec<(usize, f32)> = docs
            .iter()
            .enumerate()
            .map(|(i, doc)| (i, Self::cosine_similarity(query, &doc.embedding)))
            .collect();

        // Sort by score descending
        scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Take top k results
        let results: Vec<Document> = scores
            .into_iter()
            .take(k)
            .map(|(i, score)| {
                let mut doc = docs.get(i).expect("valid index").document.clone();
                doc = doc.with_score(score);
                doc
            })
            .collect();

        Ok(results)
    }

    #[allow(clippy::unwrap_used)]
    async fn insert(&self, document: Document, embedding: Vec<f32>) -> Result<()> {
        let id = uuid::Uuid::new_v4().to_string();

        let mut docs = self.documents.write().unwrap();
        docs.push(StoredDocument {
            id,
            document,
            embedding,
        });

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rag::Document;

    #[test]
    fn test_store_new() {
        let store = InMemoryStore::new();
        assert!(store.is_empty());
        assert_eq!(store.len(), 0);
    }

    #[test]
    fn test_store_with_capacity() {
        let store = InMemoryStore::with_capacity(100);
        assert!(store.is_empty());
    }

    #[test]
    fn test_store_clear() {
        let store = InMemoryStore::new();
        // Note: actual insert test would be async, covered below

        // Simulate having documents (would need async for real insert)
        // For now just verify clear doesn't panic
        store.clear();
        assert!(store.is_empty());
    }

    #[test]
    fn test_cosine_similarity() {
        // Identical vectors = 1.0
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        let sim = InMemoryStore::cosine_similarity(&a, &b);
        assert!((sim - 1.0).abs() < 0.001);

        // Orthogonal vectors = 0.0
        let c = vec![0.0, 1.0, 0.0];
        let sim2 = InMemoryStore::cosine_similarity(&a, &c);
        assert!((sim2 - 0.0).abs() < 0.001);

        // Opposite vectors = -1.0
        let d = vec![-1.0, 0.0, 0.0];
        let sim3 = InMemoryStore::cosine_similarity(&a, &d);
        assert!((sim3 - (-1.0)).abs() < 0.001);
    }

    #[tokio::test]
    async fn test_insert_and_search() {
        let store = InMemoryStore::new();

        // Insert documents with different embeddings
        let doc1 = Document::new("Document about cats".to_string());
        let emb1 = vec![1.0, 0.0, 0.0]; // "cats" direction

        let doc2 = Document::new("Document about dogs".to_string());
        let emb2 = vec![0.0, 1.0, 0.0]; // "dogs" direction

        let doc3 = Document::new("Document about cats and kittens".to_string());
        let emb3 = vec![0.9, 0.1, 0.0]; // Similar to "cats"

        store.insert(doc1, emb1).await.unwrap();
        store.insert(doc2, emb2).await.unwrap();
        store.insert(doc3, emb3).await.unwrap();

        assert_eq!(store.len(), 3);

        // Search for "cats" - should return cat-related docs first
        let query = vec![1.0, 0.0, 0.0];
        let results = store.search(&query, 2).await.unwrap();

        assert_eq!(results.len(), 2);
        assert!(results[0].score.unwrap() > results[1].score.unwrap());
        assert!(results[0].content.contains("cats"));
    }

    #[tokio::test]
    async fn test_search_with_limit() {
        let store = InMemoryStore::with_capacity(10);

        for i in 0..10 {
            let doc = Document::new(format!("Document {i}"));
            let emb = vec![1.0; 128]; // All same embedding
            store.insert(doc, emb).await.unwrap();
        }

        let query = vec![1.0; 128];
        let results = store.search(&query, 3).await.unwrap();

        assert_eq!(results.len(), 3);
    }
}
