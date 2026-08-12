//! Web tools: abstract provider traits for search and fetch.

pub mod fetch;
pub mod proxy;
pub mod retry;
pub mod search;

pub use fetch::{FetchError, FetchFn, FetchOptions, FetchResult, FetchStatus};
pub use proxy::ProxyConfig;
pub use retry::RetryConfig;
pub use search::{SearchError, SearchFn, SearchItem, SearchOptions, SearchResult};
