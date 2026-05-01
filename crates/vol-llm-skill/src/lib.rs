//! vol-llm-skill: Skill definition, discovery, loading, and invocation for LLM agents.
//!
//! # Architecture
//!
//! File-based SKILL.md discovery from `.agents/skills/` directories with progressive disclosure.
//! Skills are read-only prompt content loaded via a `skill` tool.
//!
//! # Quick Start
//!
//! ```rust
//! use vol_llm_skill::{SkillLoader, SkillTool, SkillInjector};
//!
//! #[tokio::main]
//! async fn main() {
//!     let mut loader = SkillLoader::new(None);
//!     loader.discover_all().await.unwrap();
//!
//!     let tool = SkillTool::new(std::sync::Arc::new(loader));
//!     // Register tool in ToolRegistry
//! }
//! ```

mod def;
mod injector;
mod loader;
mod tool;

pub use def::{SkillDef, SkillMetadata, SkillScope};
pub use injector::SkillInjector;
pub use loader::SkillLoader;
pub use tool::SkillTool;

/// Result type for skill operations
pub type Result<T> = std::result::Result<T, SkillError>;

/// Error type for skill operations
#[derive(Debug, thiserror::Error)]
pub enum SkillError {
    #[error("Skill not found: {0}")]
    NotFound(String),
    #[error("Discovery error: {0}")]
    Discovery(String),
    #[error("Parse error: {0}")]
    Parse(String),
}
