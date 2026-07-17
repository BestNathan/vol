//! QA Agent: RAG-powered question answering agent.
//!
//! Provides a simple Q&A interface using retrieval-augmented generation.
//! Business logic layer that wraps `RagAgent` for domain-specific use cases.

use std::sync::Arc;
use vol_llm_agent::{Document, Embedder, EmbeddingStore, RagAgent, RagConfig};
use vol_llm_core::Result;

/// QA Agent configuration
#[derive(Debug, Clone)]
pub struct QaAgentConfig {
    /// Agent name/identifier
    pub name: String,
    /// RAG configuration
    pub rag_config: RagConfig,
    /// System prompt for the agent
    pub system_prompt: String,
}

impl Default for QaAgentConfig {
    fn default() -> Self {
        Self {
            name: "qa-agent".to_string(),
            rag_config: RagConfig::default(),
            system_prompt: r#"你是一名专业的知识助手。请根据检索到的资料回答问题。

要求：
1. 只基于检索到的资料回答，不要编造
2. 如果资料不足，明确告知用户
3. 回答清晰、准确、简洁
4. 必要时注明信息来源

请基于以下检索结果回答问题。"#
                .to_string(),
        }
    }
}

impl QaAgentConfig {
    /// Create config with custom name
    pub fn with_name(mut self, name: &str) -> Self {
        self.name = name.to_string();
        self
    }

    /// Set RAG configuration
    pub fn with_rag_config(mut self, config: RagConfig) -> Self {
        self.rag_config = config;
        self
    }

    /// Set system prompt
    pub fn with_system_prompt(mut self, prompt: &str) -> Self {
        self.system_prompt = prompt.to_string();
        self
    }
}

/// QA Agent
///
/// Wraps `RagAgent` with domain-specific business logic.
/// Can be extended for specific use cases (e.g., customer support, internal knowledge base).
pub struct QaAgent<E, S>
where
    E: Embedder,
    S: EmbeddingStore,
{
    rag: RagAgent,
    config: QaAgentConfig,
    _embedder: std::marker::PhantomData<E>,
    _store: std::marker::PhantomData<S>,
}

impl<E, S> QaAgent<E, S>
where
    E: Embedder + 'static,
    S: EmbeddingStore + 'static,
{
    /// Create a new QA Agent
    pub fn new(
        llm: Arc<dyn vol_llm_core::LLMClient>,
        store: Arc<S>,
        embedder: Arc<E>,
        config: QaAgentConfig,
    ) -> Self {
        let rag = RagAgent::new(llm, store, embedder, config.rag_config.clone());

        Self {
            rag,
            config,
            _embedder: std::marker::PhantomData,
            _store: std::marker::PhantomData,
        }
    }

    /// Ask a question and get an answer
    pub async fn ask(&self, question: &str) -> Result<QaResponse> {
        // Use RagAgent to retrieve and generate
        let rag_response = self.rag.query(question).await?;

        Ok(QaResponse {
            question: question.to_string(),
            answer: rag_response.answer,
            sources: rag_response.sources,
            agent_name: self.config.name.clone(),
        })
    }

    /// Get the agent name
    pub fn name(&self) -> &str {
        &self.config.name
    }
}

/// QA Response
#[derive(Debug)]
pub struct QaResponse {
    /// Original question
    pub question: String,
    /// Generated answer
    pub answer: String,
    /// Source documents used
    pub sources: Vec<Document>,
    /// Agent name that generated this response
    pub agent_name: String,
}

impl QaResponse {
    /// Get answer with sources formatted as text
    pub fn with_sources(&self) -> String {
        let mut result = self.answer.clone();

        if !self.sources.is_empty() {
            result.push_str("\n\n--- 参考资料 ---\n");
            for (i, doc) in self.sources.iter().enumerate() {
                let source = doc
                    .metadata
                    .get("source")
                    .map(std::string::String::as_str)
                    .unwrap_or("unknown");
                result.push_str(&format!(
                    "{}. [{}] {}\n",
                    i + 1,
                    source,
                    doc.content.chars().take(100).collect::<String>()
                ));
            }
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use vol_llm_agent::rag::{InMemoryStore, RagConfig};
    use vol_llm_core::{
        ConversationRequest, ConversationResponse, FinishReason, LLMClient, LLMProvider, Message,
        SupportedParam, TokenUsage,
    };

    // Mock Embedder for testing
    struct MockEmbedder;

    #[async_trait]
    impl Embedder for MockEmbedder {
        async fn embed(&self, _text: &str) -> Result<Vec<f32>> {
            // Return a simple embedding for testing
            Ok(vec![0.5f32; 128])
        }
    }

    // Mock LLMClient for testing
    struct MockLlm;

    #[async_trait]
    impl LLMClient for MockLlm {
        fn provider(&self) -> LLMProvider {
            LLMProvider::Anthropic
        }

        fn model(&self) -> &str {
            "test-model"
        }

        fn supported_params(&self) -> &[SupportedParam] {
            &[]
        }

        async fn converse(&self, _request: ConversationRequest) -> Result<ConversationResponse> {
            Ok(ConversationResponse {
                message: Message::assistant(
                    "Based on the knowledge base, the answer is...".to_string(),
                ),
                model: "test".to_string(),
                usage: TokenUsage::default(),
                finish_reason: FinishReason::Stop,
                raw: None,
            })
        }

        async fn converse_stream(
            &self,
            _request: ConversationRequest,
        ) -> Result<vol_llm_core::StreamReceiver> {
            unimplemented!()
        }
    }

    #[tokio::test]
    async fn test_qa_agent_creation() {
        let store = Arc::new(InMemoryStore::new());
        let embedder = Arc::new(MockEmbedder);
        let llm = Arc::new(MockLlm);
        let config = QaAgentConfig::default();

        let _agent = QaAgent::new(llm, store, embedder, config);
        // Test passes if code compiles
    }

    #[tokio::test]
    async fn test_qa_agent_ask() {
        let store = Arc::new(InMemoryStore::new());
        let embedder = Arc::new(MockEmbedder);
        let llm = Arc::new(MockLlm);
        let config = QaAgentConfig::default().with_name("test-agent");

        let agent = QaAgent::new(llm, store.clone(), embedder, config);

        // Insert a test document
        let doc = Document::new("Test knowledge about Rust programming".to_string())
            .with_metadata("source", "knowledge_base");
        store.insert(doc, vec![0.5f32; 128]).await.unwrap();

        let response = agent.ask("What is Rust?").await.unwrap();

        assert_eq!(response.question, "What is Rust?");
        assert_eq!(response.agent_name, "test-agent");
        assert!(!response.answer.is_empty());
    }

    #[test]
    fn test_qa_config_builder() {
        let config = QaAgentConfig::default()
            .with_name("customer-support")
            .with_rag_config(RagConfig::default().with_top_k(3));

        assert_eq!(config.name, "customer-support");
        assert_eq!(config.rag_config.top_k, 3);
    }
}
