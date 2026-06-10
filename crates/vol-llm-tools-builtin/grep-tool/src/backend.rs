//! GrepBackend trait — common interface for grep implementations.

use std::path::Path;

use vol_llm_sandbox::Sandbox;

use crate::{GrepParams, SearchResult};

/// A grep backend provides a single search method.
/// Implementations may shell out to CLI tools or use Rust libraries.
#[async_trait::async_trait]
pub trait GrepBackend: Send + Sync {
    /// Execute a grep search with the given parameters and sandbox.
    /// Returns a Vec of SearchResult (one per matching file).
    async fn search(
        params: &GrepParams,
        root: &Path,
        sandbox: &dyn Sandbox,
    ) -> Result<Vec<SearchResult>, String>;
}
