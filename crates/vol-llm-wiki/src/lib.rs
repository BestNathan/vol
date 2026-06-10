//! vol-llm-wiki: LLM-powered wiki compression and management.
//!
//! Wiki pages live in `.agents/wikis/` with progressive loading
//! (index + directory injected, model reads pages on demand via `read` tool).
//! `WikiAgent` analyzes session conversations and creates/updates wiki pages.

mod agent;
mod config;
mod error;
mod injector;
mod loader;

pub use agent::{WikiAgent, WikiCompressResult};
pub use config::WikiAgentConfig;
pub use error::WikiAgentError;
pub use injector::WikiInjector;
pub use loader::{WikiLoader, WikiPage};
