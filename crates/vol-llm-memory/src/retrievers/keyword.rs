use async_trait::async_trait;

use crate::item::{MemoryFilter, MemoryItem};
use crate::retriever::MemoryRetriever;
use crate::store::MemoryStore;
use crate::Result;

/// Simple keyword-based retriever.
pub struct KeywordRetriever {
    store: Box<dyn MemoryStore>,
}

impl KeywordRetriever {
    pub fn new(store: Box<dyn MemoryStore>) -> Self {
        Self { store }
    }
}

#[async_trait]
impl MemoryRetriever for KeywordRetriever {
    async fn retrieve(&self, query: &str, k: usize) -> Result<Vec<MemoryItem>> {
        self.retrieve_with_filter(query, k, MemoryFilter::default())
            .await
    }

    async fn retrieve_with_filter(
        &self,
        query: &str,
        k: usize,
        filter: MemoryFilter,
    ) -> Result<Vec<MemoryItem>> {
        let all = self.store.list(filter).await?;
        let query_terms: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();

        let mut scored: Vec<(f32, MemoryItem)> = all
            .into_iter()
            .map(|item| {
                let score = score_item(&item.content, &query_terms);
                (score, item)
            })
            .collect();

        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

        let results: Vec<MemoryItem> = scored
            .into_iter()
            .filter(|(score, _)| *score > 0.0)
            .take(k)
            .map(|(_, item)| item)
            .collect();

        Ok(results)
    }
}

fn score_item(content: &str, query_terms: &[String]) -> f32 {
    let content_lower = content.to_lowercase();
    let mut score = 0.0;
    for term in query_terms {
        let matches = content_lower.matches(term.as_str()).count();
        if matches > 0 {
            score += (matches as f32) / (content.len().max(1) as f32 / 100.0);
        }
    }
    score
}
