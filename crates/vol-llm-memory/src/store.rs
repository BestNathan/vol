use async_trait::async_trait;

use crate::item::{MemoryFilter, MemoryItem};
use crate::Result;

/// Persistence trait for memory CRUD operations.
#[async_trait]
pub trait MemoryStore: Send + Sync {
    async fn add(&self, item: MemoryItem) -> Result<String>;
    async fn get(&self, id: &str) -> Result<Option<MemoryItem>>;
    async fn remove(&self, id: &str) -> Result<bool>;
    async fn update(&self, item: MemoryItem) -> Result<()>;
    async fn list(&self, filter: MemoryFilter) -> Result<Vec<MemoryItem>>;
    async fn remove_many(&self, filter: MemoryFilter) -> Result<usize>;
}
