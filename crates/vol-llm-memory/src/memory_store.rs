use std::collections::HashMap;
use std::sync::Arc;
use async_trait::async_trait;
use tokio::sync::RwLock;

use crate::item::{MemoryFilter, MemoryItem};
use crate::store::MemoryStore;
use crate::{MemoryError, Result};

/// Thread-safe, non-persistent in-memory store.
#[derive(Clone)]
pub struct InMemoryStore {
    items: Arc<RwLock<HashMap<String, MemoryItem>>>,
}

impl InMemoryStore {
    pub fn new() -> Self {
        Self {
            items: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl Default for InMemoryStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl MemoryStore for InMemoryStore {
    async fn add(&self, item: MemoryItem) -> Result<String> {
        let id = item.id.clone();
        self.items.write().await.insert(id.clone(), item);
        Ok(id)
    }

    async fn get(&self, id: &str) -> Result<Option<MemoryItem>> {
        Ok(self.items.read().await.get(id).cloned())
    }

    async fn remove(&self, id: &str) -> Result<bool> {
        Ok(self.items.write().await.remove(id).is_some())
    }

    async fn update(&self, item: MemoryItem) -> Result<()> {
        let id = item.id.clone();
        let mut items = self.items.write().await;
        if items.contains_key(&id) {
            items.insert(id, item);
            Ok(())
        } else {
            Err(MemoryError::NotFound(format!(
                "Memory item with id '{}' not found",
                id
            )))
        }
    }

    async fn list(&self, filter: MemoryFilter) -> Result<Vec<MemoryItem>> {
        let items = self.items.read().await;
        Ok(items.values().filter(|item| filter.matches(item)).cloned().collect())
    }

    async fn remove_many(&self, filter: MemoryFilter) -> Result<usize> {
        let mut items = self.items.write().await;
        let ids_to_remove: Vec<String> = items
            .iter()
            .filter(|(_, item)| filter.matches(item))
            .map(|(id, _)| id.clone())
            .collect();
        let count = ids_to_remove.len();
        for id in ids_to_remove {
            items.remove(&id);
        }
        Ok(count)
    }
}
