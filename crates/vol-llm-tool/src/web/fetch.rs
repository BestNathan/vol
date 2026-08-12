//! Web fetch provider trait.

use async_trait::async_trait;

/// Outcome status for a fetch request.
///
/// Every fetch result carries a status tag that the tool layer renders
/// as a human-readable marker (e.g. `<fetch success>`).
#[derive(Debug, Clone)]
pub enum FetchStatus {
    /// Successful fetch and extraction.
    Success,
    /// Successful fetch, but the raw body exceeded the size limit and was
    /// truncated before extraction.
    SuccessTruncated {
        /// Original body size in bytes (before truncation).
        original_bytes: usize,
        /// Truncated body size in bytes (after truncation).
        truncated_bytes: usize,
    },
    /// The URL redirected to a different host. The redirect was NOT followed.
    /// The caller should issue a new fetch to `target_url`.
    Redirect {
        /// The redirect target URL (cross-host).
        target_url: String,
    },
    /// The fetch failed with an error (network, HTTP, timeout, etc.).
    Error {
        /// Human-readable error message.
        message: String,
    },
}

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
    pub status: FetchStatus,
    pub content: String,
    pub title: Option<String>,
    /// Whether this result was served from cache.
    pub from_cache: bool,
}

/// Fetch error type
#[derive(Debug, Clone, thiserror::Error)]
pub enum FetchError {
    #[error("Request failed: {0}")]
    RequestFailed(String),
    #[error("URL not accessible: {0}")]
    NotAccessible(String),
    #[error("Invalid URL: {0}")]
    InvalidUrl(String),
    #[error("Response too large (max {max} bytes, got {actual} bytes)")]
    TooLarge { max: usize, actual: usize },
    #[error("Redirect loop detected (max {max} hops)")]
    RedirectLoop { max: u32 },
}

/// Trait for fetch providers (default, readability, etc.)
#[async_trait]
pub trait FetchFn: Send + Sync {
    /// Fetch and extract content from a URL
    async fn fetch(&self, url: &str, opts: FetchOptions) -> Result<FetchResult, FetchError>;
}
