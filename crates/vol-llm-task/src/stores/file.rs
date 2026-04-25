//! File-based task store using JSON files.

use std::path::PathBuf;

use crate::model::{Task, TaskId, TaskStatus};
use crate::store::{Result, StoreError, TaskStore};
use tokio::fs;

/// File-based task storage — persists tasks to a JSON file on disk.
pub struct FileTaskStore {
    path: PathBuf,
}

impl FileTaskStore {
    /// Create a new FileTaskStore, loading existing tasks from disk.
    pub async fn new(path: impl Into<PathBuf>) -> Result<Self> {
        let path = path.into();
        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await.map_err(StoreError::Io)?;
        }
        // File is created on first write; read returns empty if not exists
        Ok(Self { path })
    }

    /// Read all tasks from disk
    async fn load_tasks(&self) -> Result<Vec<Task>> {
        match fs::read_to_string(&self.path).await {
            Ok(content) => {
                let tasks: Vec<Task> = serde_json::from_str(&content)
                    .map_err(|e| StoreError::Serialization(e.to_string()))?;
                Ok(tasks)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
            Err(e) => Err(StoreError::Io(e)),
        }
    }

    /// Write all tasks to disk atomically (write to temp, then rename).
    async fn save_tasks(&self, tasks: &[Task]) -> Result<()> {
        let tmp = self.path.with_extension("json.tmp");
        let content = serde_json::to_string_pretty(tasks)
            .map_err(|e| StoreError::Serialization(e.to_string()))?;
        fs::write(&tmp, content).await.map_err(StoreError::Io)?;
        fs::rename(&tmp, &self.path).await.map_err(StoreError::Io)?;
        Ok(())
    }
}

#[async_trait::async_trait]
impl TaskStore for FileTaskStore {
    async fn create(&self, task: Task) -> Result<TaskId> {
        let id = task.id;
        let mut tasks = self.load_tasks().await?;
        if tasks.iter().any(|t| t.id == id) {
            return Err(StoreError::Internal(format!("Duplicate task ID: {id}")));
        }
        tasks.push(task);
        self.save_tasks(&tasks).await?;
        Ok(id)
    }

    async fn get(&self, task_id: &TaskId) -> Result<Option<Task>> {
        let tasks = self.load_tasks().await?;
        Ok(tasks.into_iter().find(|t| t.id == *task_id))
    }

    async fn update(&self, task: Task) -> Result<()> {
        let mut tasks = self.load_tasks().await?;
        if let Some(pos) = tasks.iter().position(|t| t.id == task.id) {
            tasks[pos] = task;
        } else {
            return Err(StoreError::NotFound(format!("Task {}", task.id)));
        }
        self.save_tasks(&tasks).await
    }

    async fn delete(&self, task_id: &TaskId) -> Result<()> {
        let mut tasks = self.load_tasks().await?;
        tasks.retain(|t| t.id != *task_id);
        self.save_tasks(&tasks).await
    }

    async fn list(&self, status: Option<TaskStatus>) -> Result<Vec<Task>> {
        let tasks = self.load_tasks().await?;
        Ok(tasks
            .into_iter()
            .filter(|t| status.is_none_or(|s| t.status == s))
            .collect())
    }

    async fn get_ready_tasks(&self) -> Result<Vec<TaskId>> {
        let tasks = self.load_tasks().await?;
        let completed_ids: std::collections::HashSet<TaskId> = tasks
            .iter()
            .filter(|t| t.status == TaskStatus::Completed)
            .map(|t| t.id.clone())
            .collect();

        let ready = tasks
            .iter()
            .filter(|t| {
                t.status == TaskStatus::Pending
                    && t.dependencies.iter().all(|d| completed_ids.contains(d))
            })
            .map(|t| t.id.clone())
            .collect();

        Ok(ready)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::TaskKind;

    async fn temp_store() -> (FileTaskStore, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("tasks.json");
        let store = FileTaskStore::new(path).await.unwrap();
        (store, dir)
    }

    #[tokio::test]
    async fn test_create_and_get() {
        let (store, _dir) = temp_store().await;
        let task = Task::new(TaskKind::Agent, "file task".to_string(), vec![]);
        let id = task.id.clone();
        store.create(task).await.unwrap();
        let got = store.get(&id).await.unwrap().unwrap();
        assert_eq!(got.description, "file task");
    }

    #[tokio::test]
    async fn test_persistence() {
        let (store, _dir) = temp_store().await;
        let task = Task::new(TaskKind::Agent, "persist".to_string(), vec![]);
        let id = task.id.clone();
        store.create(task).await.unwrap();

        // Create a new store instance pointing to the same file
        let path = store.path.clone();
        let store2 = FileTaskStore::new(path).await.unwrap();
        let got = store2.get(&id).await.unwrap().unwrap();
        assert_eq!(got.description, "persist");
    }

    #[tokio::test]
    async fn test_update() {
        let (store, _dir) = temp_store().await;
        let task = Task::new(TaskKind::Agent, "original".to_string(), vec![]);
        let id = task.id.clone();
        store.create(task).await.unwrap();

        let mut updated = store.get(&id).await.unwrap().unwrap();
        updated.description = "modified".to_string();
        updated.status = TaskStatus::Completed;
        store.update(updated).await.unwrap();

        // Verify via fresh store
        let path = store.path.clone();
        let store2 = FileTaskStore::new(path).await.unwrap();
        let got = store2.get(&id).await.unwrap().unwrap();
        assert_eq!(got.description, "modified");
        assert_eq!(got.status, TaskStatus::Completed);
    }

    #[tokio::test]
    async fn test_delete() {
        let (store, _dir) = temp_store().await;
        let task = Task::new(TaskKind::Agent, "delete me".to_string(), vec![]);
        let id = task.id.clone();
        store.create(task).await.unwrap();
        store.delete(&id).await.unwrap();
        let got = store.get(&id).await.unwrap();
        assert!(got.is_none());
    }

    #[tokio::test]
    async fn test_get_ready_tasks() {
        let (store, _dir) = temp_store().await;
        let t1 = Task::new(TaskKind::Agent, "task 1".to_string(), vec![]);
        let id1 = t1.id.clone();
        store.create(t1).await.unwrap();

        // Complete t1
        let mut t1_done = store.get(&id1).await.unwrap().unwrap();
        t1_done.status = TaskStatus::Completed;
        store.update(t1_done).await.unwrap();

        // t2 depends on t1
        let t2 = Task::new(TaskKind::Agent, "task 2".to_string(), vec![id1]);
        store.create(t2).await.unwrap();

        // t3 no deps
        let t3 = Task::new(TaskKind::Agent, "task 3".to_string(), vec![]);
        store.create(t3).await.unwrap();

        let ready = store.get_ready_tasks().await.unwrap();
        assert_eq!(ready.len(), 2); // t2 (dep done) + t3 (no deps)
    }
}
