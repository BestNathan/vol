//! Task store trait and error types.

use async_trait::async_trait;
use thiserror::Error;

use crate::model::{Task, TaskId};

/// Store operation error
#[derive(Debug, Error)]
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

/// Task storage interface
#[async_trait]
pub trait TaskStore: Send + Sync {
    /// Save or update a task
    async fn save(&self, task: &Task) -> Result<()>;

    /// Load a task by ID
    async fn load(&self, id: &TaskId) -> Result<Task>;

    /// Delete a task by ID
    async fn delete(&self, id: &TaskId) -> Result<()>;

    /// List all tasks
    async fn list(&self) -> Result<Vec<Task>>;
}
