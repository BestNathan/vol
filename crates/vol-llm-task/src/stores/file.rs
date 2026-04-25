//! File-based task store — stub for Task 4.

use crate::model::{Task, TaskId, TaskStatus};
use crate::store::{Result, TaskStore};
use std::path::PathBuf;

/// File-based task storage — stub.
pub struct FileTaskStore;

impl FileTaskStore {
    pub async fn new(_path: impl Into<PathBuf>) -> Result<Self> {
        Ok(FileTaskStore)
    }
}

#[async_trait::async_trait]
impl TaskStore for FileTaskStore {
    async fn create(&self, _task: Task) -> Result<()> { Ok(()) }
    async fn get(&self, _task_id: &TaskId) -> Result<Option<Task>> { Ok(None) }
    async fn update(&self, _task: Task) -> Result<()> { Ok(()) }
    async fn delete(&self, _task_id: &TaskId) -> Result<()> { Ok(()) }
    async fn list(&self, _status: Option<TaskStatus>) -> Result<Vec<Task>> { Ok(vec![]) }
    async fn get_ready_tasks(&self) -> Result<Vec<TaskId>> { Ok(vec![]) }
}
