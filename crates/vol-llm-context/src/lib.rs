//! vol-llm-context: Structured context management for LLM agents.
//!
//! Attention-zone-aware sorting, budget-driven compression, and trait-based extensibility.
//!
//! # Architecture
//!
//! ```text
//! ContextContributor → ContextBlock{messages, anchor} → ContextBuilder → ContextOutput{messages}
//! ```
//!
//! # Quick Start
//!
//! ```rust
//! use vol_llm_context::{ContextBuilderBuilder, ContextContributor, AttentionAnchor};
//! use vol_llm_context::builtin::{FileContributor, FileSpec};
//!
//! #[tokio::main]
//! async fn main() {
//!     let builder = ContextBuilderBuilder::new(10000)
//!         .add_contributor(Box::new(FileContributor::new(vec![
//!             FileSpec::new("ROLE.md", AttentionAnchor::Head(0)),
//!             FileSpec::new("TASK.md", AttentionAnchor::Tail(0)),
//!         ])))
//!         .build();
//!
//!     let output = builder.build().await;
//!     // output.messages contains content from files that exist
//! }
//! ```

// Re-export core types from vol-llm-core to maintain stable public API.
pub use vol_llm_core::{AttentionAnchor, ContextBlock, ContextContributor, TokenBudget, estimate_tokens};

pub mod builder;
pub mod builtin;

pub use builder::{ContextBuilder, ContextBuilderBuilder, ContextOutput};
