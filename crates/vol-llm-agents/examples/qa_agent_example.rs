//! RAG Agent Example - Using QaAgent with Mock Embedder
//!
//! This example demonstrates how to use QaAgent for question answering:
//! - `MockEmbedder` for generating embeddings (no API required)
//! - `InMemoryStore` for vector storage
//! - `QaAgent` for Q&A business logic
//!
//! Run with: `cargo run --example qa_agent_example`

use async_trait::async_trait;
use std::sync::Arc;
use vol_llm_agent::{
    rag::{Document, Embedder, EmbeddingStore, InMemoryStore},
    RagConfig,
};
use vol_llm_agents::{QaAgent, QaAgentConfig};

// Mock Embedder for demonstration (no API call needed)
struct MockEmbedder;

#[async_trait]
impl Embedder for MockEmbedder {
    async fn embed(&self, _text: &str) -> vol_llm_core::Result<Vec<f32>> {
        Ok(vec![0.5f32; 128])
    }

    async fn embed_batch(&self, texts: &[&str]) -> vol_llm_core::Result<Vec<Vec<f32>>> {
        Ok(texts.iter().map(|_| vec![0.5f32; 128]).collect())
    }
}

// Mock LLM for demonstration
struct MockLlm;
#[async_trait::async_trait]
impl vol_llm_core::LLMClient for MockLlm {
    fn provider(&self) -> vol_llm_core::LLMProvider {
        vol_llm_core::LLMProvider::Anthropic
    }

    fn model(&self) -> &str {
        "mock-model"
    }

    fn supported_params(&self) -> &[vol_llm_core::SupportedParam] {
        &[]
    }

    async fn converse(
        &self,
        _request: vol_llm_core::ConversationRequest,
    ) -> vol_llm_core::Result<vol_llm_core::ConversationResponse> {
        Ok(vol_llm_core::ConversationResponse {
            message: vol_llm_core::Message::assistant(
                "这是一个模拟回答。在实际使用中，LLM 会基于检索到的文档生成真实回答。".to_string(),
            ),
            model: "mock".to_string(),
            usage: vol_llm_core::TokenUsage::default(),
            finish_reason: vol_llm_core::FinishReason::Stop,
            raw: None,
        })
    }

    async fn converse_stream(
        &self,
        _request: vol_llm_core::ConversationRequest,
    ) -> vol_llm_core::Result<vol_llm_core::StreamReceiver> {
        unimplemented!()
    }
}

#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    println!("=== QaAgent Example ===\n");

    // 1. Create embedder (mock, no API required)
    let embedder = Arc::new(MockEmbedder);
    println!("1. Created MockEmbedder");

    // 2. Create store and populate with knowledge
    let store = Arc::new(InMemoryStore::new());
    println!("2. Created InMemoryStore");

    println!("3. Adding sample knowledge...");

    // Sample knowledge base
    let knowledge = vec![
        ("Delta 对冲", "Delta 对冲是通过调整标的资产头寸使组合 Delta 接近零的策略。当 Delta 为正时卖出标的，为负时买入。"),
        ("Gamma 风险", "Gamma 衡量 Delta 对标的价格变动的敏感度。高 Gamma 意味着 Delta 变化快，需要频繁调整对冲。"),
        ("Vega 管理", "Vega 暴露组合对波动率变化的风险。可以通过买卖期权或使用 VIX 相关产品来管理 Vega。"),
        ("期限结构交易", "当近月 IV 低于远月时，可以做空近月做多远月，赚取期限结构差异。"),
    ];

    for (topic, content) in &knowledge {
        // Use embedder to generate real embedding
        let embedding = embedder.embed(&format!("{}: {}", topic, content)).await?;
        let doc = Document::new(format!("{}: {}", topic, content))
            .with_metadata("source", "trading_knowledge")
            .with_metadata("topic", *topic);
        store.insert(doc, embedding).await?;
    }
    println!("   Added {} knowledge items", knowledge.len());

    // 3. Create QaAgent
    let config = QaAgentConfig::default()
        .with_name("trading-assistant")
        .with_rag_config(RagConfig::default().with_top_k(2));

    let llm = Arc::new(MockLlm);
    let agent = QaAgent::new(llm, store, embedder, config);
    println!("4. Created QaAgent '{}'", agent.name());

    // 4. Ask questions
    println!("\n5. Asking questions:");

    let questions = vec!["如何进行 Delta 对冲？", "Gamma 风险如何管理？"];

    for question in questions {
        println!("\n   Q: {}", question);
        let response = agent.ask(question).await?;
        println!("   A: {}", response.answer);
        println!("   Sources: {} documents", response.sources.len());
    }

    println!("\n=== Example Complete ===");
    println!("\nTo use with real embeddings and LLM:");
    println!("1. Replace MockEmbedder with DashScopeEmbedder");
    println!("2. Set DASHSCOPE_API_KEY and LLM API keys");
    println!("3. Use real LLM client instead of MockLlm");

    Ok(())
}
