//! System prompt context module.
//!
//! Provides modular prompt construction with caching support.
//!
//! # Overview
//!
//! This module enables building system prompts from reusable fragments
//! and templates, optimizing for LLM cache hit rates.
//!
//! # Components
//!
//! - [`PromptTemplate`] - Template definition with injection points
//! - [`PromptFragment`] - Reusable content fragments
//! - [`FragmentType`] - Fragment type enumeration
//! - [`PromptContext`] - Context manager for building prompts
//! - [`MessageAssembler`] - Message builder from prompt context

pub mod assembler;
pub mod context;
pub mod fragment;
pub mod template;

pub use assembler::MessageAssembler;
pub use context::PromptContext;
pub use fragment::{FragmentType, PromptFragment};
pub use template::PromptTemplate;
