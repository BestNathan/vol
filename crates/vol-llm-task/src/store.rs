//! TaskStore trait and error types.

use crate::model::{Task, TaskId, TaskStatus};

/// Store operation error
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

pub type Result<T> = std::result::Result<T, StoreError>;

/// Task storage interface — swappable backends (memory, file, database).
#[async_trait::async_trait]
pub trait TaskStore: Send + Sync {
    /// Create a task
    async fn create(&self, task: Task) -> Result<()>;

    /// Get a task by ID
    async fn get(&self, task_id: &TaskId) -> Result<Option<Task>>;

    /// Update a task
    async fn update(&self, task: Task) -> Result<()>;

    /// Delete a task
    async fn delete(&self, task_id: &TaskId) -> Result<()>;

    /// List tasks, optionally filtered by status
    async fn list(&self, status: Option<TaskStatus>) -> Result<Vec<Task>>;

    /// Get all task IDs that are Pending and have all dependencies Completed
    async fn get_ready_tasks(&self) -> Result<Vec<TaskId>>;
}
