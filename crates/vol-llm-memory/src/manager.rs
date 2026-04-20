use crate::item::{MemoryFilter, MemoryItem};
use crate::retriever::MemoryRetriever;
use crate::store::MemoryStore;
use crate::Result;

/// Orchestrator combining Store + Retriever into a single agent-facing API.
pub struct MemoryManager {
    store: Box<dyn MemoryStore>,
    retriever: Box<dyn MemoryRetriever>,
}

impl MemoryManager {
    pub fn new(store: Box<dyn MemoryStore>, retriever: Box<dyn MemoryRetriever>) -> Self {
        Self { store, retriever }
    }

    pub async fn add(&self, item: MemoryItem) -> Result<String> {
        self.store.add(item).await
    }

    pub async fn get(&self, id: &str) -> Result<Option<MemoryItem>> {
        self.store.get(id).await
    }

    pub async fn remove(&self, id: &str) -> Result<bool> {
        self.store.remove(id).await
    }

    pub async fn search(&self, query: &str, k: usize) -> Result<Vec<MemoryItem>> {
        self.retriever.retrieve(query, k).await
    }

    pub async fn search_with_filter(
        &self,
        query: &str,
        k: usize,
        filter: MemoryFilter,
    ) -> Result<Vec<MemoryItem>> {
        self.retriever.retrieve_with_filter(query, k, filter).await
    }

    /// Format retrieved memories as prompt-injectable text.
    pub async fn inject_context(&self, query: &str, max_items: usize) -> Result<String> {
        let memories = self.search(query, max_items).await?;
        if memories.is_empty() {
            return Ok(String::new());
        }

        let mut output = String::from("Relevant memories:\n");
        for (i, mem) in memories.iter().enumerate() {
            output.push_str(&format!("{}. [{}] {}\n", i + 1, mem.kind, mem.content));
            if !mem.tags.is_empty() {
                output.push_str(&format!("   Tags: {}\n", mem.tags.join(", ")));
            }
        }
        Ok(output)
    }

    pub async fn list(&self, filter: MemoryFilter) -> Result<Vec<MemoryItem>> {
        self.store.list(filter).await
    }

    pub async fn remove_many(&self, filter: MemoryFilter) -> Result<usize> {
        self.store.remove_many(filter).await
    }

    /// Stub: Extract and store memories from a completed session.
    /// TODO: Implement LLM-based extraction when memory extraction is in scope.
    #[allow(dead_code)]
    pub async fn summarize_session(
        &self,
        _messages: &[vol_llm_core::Message],
    ) -> Result<Vec<MemoryItem>> {
        Ok(Vec::new())
    }
}
