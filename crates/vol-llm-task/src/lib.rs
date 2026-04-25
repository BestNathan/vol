//! vol-llm-task: Task management for LLM Agent.
//!
//! Provides task data models, Store abstraction (memory + file backends),
//! and a TaskScheduler facade for dependency-aware scheduling.

mod model;
mod scheduler;
mod store;
mod stores;

pub use model::{Task, TaskId, TaskKind, TaskResult, TaskStatus};
pub use scheduler::TaskScheduler;
pub use store::{Result, StoreError, TaskStore};
pub use stores::{FileTaskStore, InMemoryTaskStore};
