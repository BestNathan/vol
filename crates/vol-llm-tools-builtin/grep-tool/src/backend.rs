//! GrepBackend trait — common interface for grep implementations.

use std::path::Path;

use crate::{GrepParams, SearchResult};

/// A grep backend provides a single search method.
/// Implementations may shell out to CLI tools or use Rust libraries.
#[async_trait::async_trait]
pub trait GrepBackend: Send + Sync {
    /// Check whether this backend is available in the current environment.
    fn is_available() -> bool
    where
        Self: Sized;

    /// Execute a grep search with the given parameters.
    /// Returns a Vec of SearchResult (one per matching file).
    async fn search(params: &GrepParams, root: &Path) -> Result<Vec<SearchResult>, String>;
}
