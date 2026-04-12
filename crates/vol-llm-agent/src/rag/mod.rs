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
//! use vol_llm_agent::{RagAgent, RagConfig};
//! use vol_llm_agent::rag::{Embedder, EmbeddingStore, InMemoryStore, DashScopeEmbedder};
//!
//! // Create embedder and store
//! let embedder = DashScopeEmbedder::from_env();
//! let store = InMemoryStore::new();
//!
//! // Then create RagAgent:
//! // let rag = RagAgent::new(llm, Arc::new(store), Arc::new(embedder), config);
//! ```

mod agent;
mod config;
mod document;
mod memory_store;
mod store;

pub use agent::{RagAgent, RagResponse};
pub use config::RagConfig;
pub use document::Document;
pub use memory_store::InMemoryStore;
pub use store::EmbeddingStore;

// Re-export embedding types from embedding module for backward compatibility
pub use crate::embedding::{DashScopeConfig, DashScopeEmbedder, DashScopeModel, Embedder};
