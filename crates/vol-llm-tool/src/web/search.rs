//! Web search provider trait.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Options for a web search request
#[derive(Debug, Clone, Default)]
pub struct SearchOptions {
    pub proxy_url: Option<String>,
    pub num_results: Option<usize>,
    pub allowed_domains: Option<Vec<String>>,
    pub blocked_domains: Option<Vec<String>>,
}

/// A single search result item
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchItem {
    pub title: String,
    pub url: String,
    pub snippet: Option<String>,
}

/// Search result containing query and items
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub query: String,
    pub results: Vec<SearchItem>,
}

/// Search error type
#[derive(Debug, thiserror::Error)]
pub enum SearchError {
    #[error("Search request failed: {0}")]
    RequestFailed(String),
    #[error("Failed to parse search response: {0}")]
    ParseError(String),
    #[error("Invalid API key")]
    InvalidApiKey,
}

/// Trait for search providers (Tavily, Google, etc.)
#[async_trait]
pub trait SearchFn: Send + Sync {
    /// Search the web for the given query
    async fn search(&self, query: &str, opts: SearchOptions) -> Result<SearchResult, SearchError>;
}
