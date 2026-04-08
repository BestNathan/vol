//! RAG (Retrieval-Augmented Generation) module.
//!
//! Provides `RagAgent` for retrieval-augmented generation, separate from `ReActAgent`.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                    RagAgent                                 │
//! │                                                             │
//! │  - llm: Arc<dyn LLMClient>                                 │
//! │  - store: Arc<dyn EmbeddingStore>                          │
//! │  - embedder: Arc<dyn Embedder>                             │
//! │                                                             │
//! │  + retrieve(query) -> Vec<Document>                        │
//! │  + generate(query, docs) -> String                         │
//! │  + query(query) -> RagResponse                             │
//! └─────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Usage
//!
//! ```rust,no_run
//! use vol_llm_agent::{RagAgent, RagConfig, Document};
//! use vol_llm_agent::rag::{Embedder, EmbeddingStore, InMemoryStore};
//!
//! // Implement Embedder for your use case
//! // Then create RagAgent with InMemoryStore for testing:
//! // let store = InMemoryStore::new();
//! // let rag = RagAgent::new(llm, Arc::new(store), embedder, config);
//! ```

mod agent;
mod config;
mod document;
mod embedding;
mod store;
mod memory_store;

pub use agent::{RagAgent, RagResponse};
pub use config::RagConfig;
pub use document::Document;
pub use embedding::Embedder;
pub use store::EmbeddingStore;
pub use memory_store::InMemoryStore;
