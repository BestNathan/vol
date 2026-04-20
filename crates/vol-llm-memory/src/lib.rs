//! vol-llm-memory: Layered memory abstractions for cross-session agent memory.

mod item;
mod manager;
mod memory_store;
mod retriever;
mod retrievers;
mod store;

pub use item::{MemoryFilter, MemoryItem, MemoryKind};
pub use manager::MemoryManager;
pub use memory_store::InMemoryStore;
pub use retriever::MemoryRetriever;
pub use retrievers::keyword::KeywordRetriever;
pub use store::MemoryStore;

/// Result type for memory operations
pub type Result<T> = std::result::Result<T, MemoryError>;

/// Error type for memory operations
#[derive(Debug, thiserror::Error)]
pub enum MemoryError {
    #[error("Memory not found: {0}")]
    NotFound(String),
    #[error("Storage error: {0}")]
    Storage(String),
    #[error("Retrieval error: {0}")]
    Retrieval(String),
}
