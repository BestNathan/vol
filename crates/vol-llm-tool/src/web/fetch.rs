//! Web fetch provider trait.

use async_trait::async_trait;

/// Options for a web fetch request
#[derive(Debug, Clone, Default)]
pub struct FetchOptions {
    pub proxy_url: Option<String>,
    pub prompt: Option<String>,
    pub max_length: Option<usize>,
}

/// Fetch result containing extracted content
#[derive(Debug, Clone)]
pub struct FetchResult {
    pub url: String,
    pub content: String,
    pub title: Option<String>,
}

/// Fetch error type
#[derive(Debug, thiserror::Error)]
pub enum FetchError {
    #[error("Request failed: {0}")]
    RequestFailed(String),
    #[error("URL not accessible: {0}")]
    NotAccessible(String),
    #[error("Invalid URL: {0}")]
    InvalidUrl(String),
    #[error("Response too large (max {max} bytes, got {actual} bytes)")]
    TooLarge { max: usize, actual: usize },
}

/// Trait for fetch providers (default, readability, etc.)
#[async_trait]
pub trait FetchFn: Send + Sync {
    /// Fetch and extract content from a URL
    async fn fetch(&self, url: &str, opts: FetchOptions)
        -> Result<FetchResult, FetchError>;
}
