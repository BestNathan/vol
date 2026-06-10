//! Web tools: abstract provider traits for search and fetch.

pub mod fetch;
pub mod proxy;
pub mod search;

pub use fetch::{FetchError, FetchFn, FetchOptions, FetchResult};
pub use proxy::ProxyConfig;
pub use search::{SearchError, SearchFn, SearchItem, SearchOptions, SearchResult};
