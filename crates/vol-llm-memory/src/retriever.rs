use async_trait::async_trait;

use crate::item::{MemoryFilter, MemoryItem};
use crate::{MemoryError, Result};

/// Relevance search trait for retrieving memories.
#[async_trait]
pub trait MemoryRetriever: Send + Sync {
    async fn retrieve(&self, query: &str, k: usize) -> Result<Vec<MemoryItem>>;

    async fn retrieve_with_filter(
        &self,
        query: &str,
        k: usize,
        filter: MemoryFilter,
    ) -> Result<Vec<MemoryItem>>;
}
