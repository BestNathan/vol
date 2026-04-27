//! vol-llm-wiki: LLM-powered wiki compression and management.
//!
//! Wiki pages live in `.agent/wikis/` with progressive loading
//! (index + directory injected, model reads pages on demand via `read` tool).
//! `WikiAgent` analyzes session conversations and creates/updates wiki pages.

mod loader;
mod injector;
mod agent;
mod config;
mod error;

pub use agent::{WikiAgent, WikiCompressResult};
pub use config::WikiAgentConfig;
pub use error::WikiAgentError;
pub use loader::{WikiLoader, WikiPage};
pub use injector::WikiInjector;
