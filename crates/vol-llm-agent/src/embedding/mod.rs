//! Embedding module for generating vector embeddings.
//!
//! This module provides:
//! - `Embedder` trait for embedding generation
//! - `DashScopeEmbedder` implementation for Alibaba Cloud DashScope
//!
//! # Example
//!
//! ```rust,no_run
//! use vol_llm_agent::embedding::{Embedder, DashScopeEmbedder};
//!
//! #[tokio::main]
//! async fn main() {
//!     let embedder = DashScopeEmbedder::new("your-api-key");
//!     let embedding = embedder.embed("Hello, world!").await.unwrap();
//! }
//! ```

pub mod dashscope;
pub mod embedder;

pub use dashscope::{DashScopeConfig, DashScopeEmbedder, DashScopeModel};
pub use embedder::Embedder;
