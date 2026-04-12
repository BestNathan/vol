//! Document structure for RAG retrieval.

use std::collections::HashMap;

/// Document retrieved from embedding store
#[derive(Debug, Clone)]
pub struct Document {
    /// Document content
    pub content: String,
    /// Metadata (source, created_at, etc.)
    pub metadata: HashMap<String, String>,
    /// Similarity score (higher = more relevant)
    pub score: Option<f32>,
}

impl Document {
    /// Create a new document with content
    pub fn new(content: String) -> Self {
        Self {
            content,
            metadata: HashMap::new(),
            score: None,
        }
    }

    /// Add metadata to document
    pub fn with_metadata(mut self, key: &str, value: &str) -> Self {
        self.metadata.insert(key.to_string(), value.to_string());
        self
    }

    /// Set similarity score
    pub fn with_score(mut self, score: f32) -> Self {
        self.score = Some(score);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_document_new() {
        let doc = Document::new("test content".to_string());
        assert_eq!(doc.content, "test content");
        assert!(doc.metadata.is_empty());
        assert!(doc.score.is_none());
    }

    #[test]
    fn test_document_with_metadata() {
        let doc = Document::new("test".to_string())
            .with_metadata("source", "knowledge_base")
            .with_metadata("category", "finance");

        assert_eq!(
            doc.metadata.get("source"),
            Some(&"knowledge_base".to_string())
        );
        assert_eq!(doc.metadata.get("category"), Some(&"finance".to_string()));
    }

    #[test]
    fn test_document_with_score() {
        let doc = Document::new("test".to_string()).with_score(0.85);
        assert_eq!(doc.score, Some(0.85));
    }
}
