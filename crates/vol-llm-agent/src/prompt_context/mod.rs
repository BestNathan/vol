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

pub mod fragment;
pub mod template;

pub use fragment::{FragmentType, PromptFragment};
pub use template::PromptTemplate;
