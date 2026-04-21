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
//! use vol_llm_context::{ContextBuilderBuilder, ContextContributor};
//! use vol_llm_context::builtin::{RoleContributor, TaskContributor, RulesContributor};
//!
//! #[tokio::main]
//! async fn main() {
//!     let builder = ContextBuilderBuilder::new(10000)
//!         .add_contributor(Box::new(RoleContributor::new("You are a coding assistant")))
//!         .add_contributor(Box::new(TaskContributor::new("Fix the bug")))
//!         .build();
//!
//!     let output = builder.build().await;
//!     assert!(!output.messages.is_empty());
//! }
//! ```

pub mod block;
pub mod builder;
pub mod builtin;
pub mod contributor;

pub use block::{estimate_tokens, AttentionAnchor, ContextBlock, TokenBudget};
pub use builder::{ContextBuilder, ContextBuilderBuilder, ContextOutput};
pub use contributor::ContextContributor;
