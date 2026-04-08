//! RAG Agent implementation.

use std::sync::Arc;
use vol_llm_core::{LLMClient, Message, ConversationRequest};
use vol_llm_core::Result;
use super::{Document, Embedder, EmbeddingStore, RagConfig};

/// RAG response containing answer and source documents
pub struct RagResponse {
    /// Generated answer
    pub answer: String,
    /// Source documents used
    pub sources: Vec<Document>,
}

/// RAG Agent for retrieval-augmented generation
///
/// Provides separate `retrieve()` and `generate()` methods for flexibility,
/// plus a convenience `query()` method that combines both.
pub struct RagAgent {
    llm: Arc<dyn LLMClient>,
    store: Arc<dyn EmbeddingStore>,
    embedder: Arc<dyn Embedder>,
    config: RagConfig,
}

impl RagAgent {
    /// Create a new RagAgent
    pub fn new(
        llm: Arc<dyn LLMClient>,
        store: Arc<dyn EmbeddingStore>,
        embedder: Arc<dyn Embedder>,
        config: RagConfig,
    ) -> Self {
        Self { llm, store, embedder, config }
    }

    /// Retrieve relevant documents for a query
    ///
    /// This method only performs retrieval, without generating an answer.
    /// Useful when you want to inspect or filter retrieved documents before generation.
    pub async fn retrieve(&self, query: &str) -> Result<Vec<Document>> {
        // 1. Generate query embedding
        let embedding = self.embedder.embed(query).await?;

        // 2. Search vector store
        let docs = self.store.search(&embedding, self.config.top_k).await?;

        // 3. Filter by similarity threshold
        let filtered: Vec<Document> = docs
            .into_iter()
            .filter(|d| {
                d.score
                    .map(|s| s >= self.config.similarity_threshold)
                    .unwrap_or(true) // Keep documents without scores
            })
            .collect();

        Ok(filtered)
    }

    /// Generate an answer given query and retrieved documents
    ///
    /// This method only performs generation, assuming you already have retrieved documents.
    /// Useful when you want to control retrieval separately or re-use documents.
    pub async fn generate(&self, query: &str, docs: &[Document]) -> Result<String> {
        // Build RAG prompt with context
        let prompt = self.build_rag_prompt(query, docs);

        // Call LLM
        let request = ConversationRequest::with_history(None, vec![prompt]);
        let response = self.llm.converse(request).await?;

        Ok(response.message.content
            .map(|c| c.as_str().to_string())
            .unwrap_or_default())
    }

    /// Full RAG query: retrieve documents and generate answer
    ///
    /// This is a convenience method that combines `retrieve()` and `generate()`.
    pub async fn query(&self, query: &str) -> Result<RagResponse> {
        // 1. Retrieve documents
        let docs = self.retrieve(query).await?;

        // 2. Generate answer
        let answer = self.generate(query, &docs).await?;

        Ok(RagResponse {
            answer,
            sources: docs,
        })
    }

    /// Build RAG prompt with retrieved context
    fn build_rag_prompt(&self, query: &str, docs: &[Document]) -> Message {
        let context: String = docs
            .iter()
            .map(|d| d.content.as_str())
            .collect::<Vec<_>>()
            .join("\n\n---\n\n");

        let system_prompt = r#"你是一名知识助手。请根据提供的参考资料回答问题。

要求：
1. 只基于参考资料回答，不要编造信息
2. 如果参考资料不足以回答问题，明确告知用户
3. 回答时注明信息来源

参考资料：
{context}

用户问题：{query}"#;

        let formatted = system_prompt
            .replace("{context}", &context)
            .replace("{query}", query);

        Message::user(formatted)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;

    // Mock Embedder for testing
    struct MockEmbedder;

    #[async_trait]
    impl Embedder for MockEmbedder {
        async fn embed(&self, _text: &str) -> Result<Vec<f32>> {
            Ok(vec![0.1; 128]) // Dummy embedding
        }
    }

    // Mock EmbeddingStore for testing
    struct MockStore;

    #[async_trait]
    impl EmbeddingStore for MockStore {
        async fn search(&self, _query: &[f32], k: usize) -> Result<Vec<Document>> {
            Ok(vec![
                Document::new("Reference document 1".to_string()).with_score(0.9),
                Document::new("Reference document 2".to_string()).with_score(0.8),
            ].into_iter().take(k).collect())
        }

        async fn insert(&self, _document: Document, _embedding: Vec<f32>) -> Result<()> {
            Ok(())
        }
    }

    // Mock LLMClient for testing
    struct MockLlm;

    #[async_trait]
    impl LLMClient for MockLlm {
        fn provider(&self) -> vol_llm_core::LLMProvider {
            vol_llm_core::LLMProvider::Anthropic
        }

        fn model(&self) -> &str {
            "test-model"
        }

        fn supported_params(&self) -> &[vol_llm_core::SupportedParam] {
            &[]
        }

        async fn converse(&self, _request: ConversationRequest) -> Result<vol_llm_core::ConversationResponse> {
            Ok(vol_llm_core::ConversationResponse {
                message: Message::assistant("Based on the references, the answer is...".to_string()),
                model: "test".to_string(),
                usage: vol_llm_core::TokenUsage::default(),
                finish_reason: vol_llm_core::FinishReason::Stop,
                raw: None,
            })
        }

        async fn converse_stream(&self, _request: ConversationRequest) -> Result<vol_llm_core::StreamReceiver> {
            unimplemented!()
        }
    }

    #[test]
    fn test_rag_agent_creation() {
        let llm = Arc::new(MockLlm);
        let store = Arc::new(MockStore);
        let embedder = Arc::new(MockEmbedder);
        let config = RagConfig::default();

        let _agent = RagAgent::new(llm, store, embedder, config);
        // Test passes if code compiles
    }

    #[test]
    fn test_rag_config_default() {
        let config = RagConfig::default();
        assert_eq!(config.top_k, 5);
        assert_eq!(config.similarity_threshold, 0.3);
    }
}
