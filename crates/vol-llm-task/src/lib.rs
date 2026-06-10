//! vol-llm-task: Task management for LLM Agent.
//!
//! Provides task data models, Store abstraction (memory + file backends),
//! TaskScheduler facade, and LLM tools for task management.

pub(crate) mod cli;
mod model;
mod scheduler;
mod store;
mod stores;
pub mod tools;

pub use model::{Task, TaskId, TaskKind, TaskResult, TaskStatus};
pub use scheduler::TaskScheduler;
pub use store::{Result, StoreError, TaskStore};
pub use stores::{DatabaseTaskStore, FileTaskStore, InMemoryTaskStore};
