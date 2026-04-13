//! Web tools: abstract provider traits for search and fetch.

pub mod fetch;
pub mod search;

pub use fetch::{FetchError, FetchFn, FetchOptions, FetchResult};
pub use search::{SearchError, SearchFn, SearchItem, SearchResult, SearchOptions};
