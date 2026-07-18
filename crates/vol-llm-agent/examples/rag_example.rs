//! RAG Example - Using DashScopeEmbedder with InMemoryStore
//!
//! This example demonstrates how to use the RAG module with:
//! - `DashScopeEmbedder` for generating embeddings via DashScope API
//! - `InMemoryStore` for storing and searching vectors
//!
//! Run with: `cargo run --example rag_example`

use std::sync::Arc;
use vol_llm_agent::rag::{DashScopeEmbedder, Document, EmbeddingStore, InMemoryStore};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== RAG Example ===\n");

    // 1. Create embedder (uses DASHSCOPE_API_KEY from environment)
    let api_key = std::env::var("DASHSCOPE_API_KEY").unwrap_or_else(|_| "your-api-key".to_string());
    let _embedder = Arc::new(DashScopeEmbedder::new(&api_key));
    println!("1. Created DashScopeEmbedder");

    // 2. Create in-memory store
    let store = Arc::new(InMemoryStore::new());
    println!("2. Created InMemoryStore");

    // 3. Populate store with sample knowledge
    println!("3. Adding sample documents...");

    let documents = vec![
        ("什么是 Delta？", "Delta 是期权希腊值之一，衡量期权价格对标的资产价格变动的敏感度。Call 期权 Delta 为正，Put 期权 Delta 为负。"),
        ("什么是 Vega？", "Vega 衡量期权价格对隐含波动率 (IV) 变动的敏感度。Vega 越高，IV 变化对期权价格的影响越大。"),
        ("什么是对冲？", "对冲是通过建立相反头寸来降低风险的策略。例如，持有期权的同时交易标的资产来中和 Delta。"),
        ("什么是 IV 期限结构？", "IV 期限结构是不同到期日期权隐含波动率的曲线。正常结构是远月 IV 高于近月，倒挂则相反。"),
    ];

    for (doc_text, _explanation) in &documents {
        // In real usage, you would call the DashScope API to get embeddings
        // For this example, we'll use a dummy embedding
        let dummy_embedding = vec![0.1f32; 1536]; // text-embedding-v2 dimension

        let doc = Document::new(doc_text.to_string())
            .with_metadata("source", "knowledge_base")
            .with_metadata("category", "options_trading");

        store.insert(doc, dummy_embedding).await?;
    }
    println!("   Added {} documents", documents.len());

    // 4. Create RagAgent (requires a mock LLM for this example)
    // Note: In real usage, you would provide a real LLM client
    println!("\n4. RagAgent creation requires an LLM client");
    println!("   See rag_agent_example.rs for full RagAgent usage");

    // 5. Demonstrate search only
    println!("\n5. Testing vector search:");
    let query = "Delta 是什么？";
    println!("   Query: {query}");

    // Use dummy embedding for search (in real usage, embedder.embed(query).await)
    let dummy_query_embedding = vec![0.1f32; 1536];
    let results = store.search(&dummy_query_embedding, 2).await?;

    println!("   Found {} documents", results.len());
    for (i, doc) in results.iter().enumerate() {
        println!(
            "   [{}] Score: {:.4}, Content: {}...",
            i + 1,
            doc.score.unwrap_or(0.0),
            doc.content.chars().take(30).collect::<String>()
        );
    }

    println!("\n=== Example Complete ===");
    println!("\nTo use with real embeddings:");
    println!("1. Set DASHSCOPE_API_KEY environment variable");
    println!("2. Replace dummy embeddings with: embedder.embed(text).await");
    println!("3. Provide a real LLM client for RagAgent");

    Ok(())
}
