//! File-based task store — one JSON file per task.

use std::path::{Path, PathBuf};

use crate::model::{Task, TaskId, TaskStatus};
use crate::store::{Result, StoreError, TaskStore};
use tokio::fs;

/// File-based task store — persists each task to its own JSON file.
///
/// Tasks are stored in `{basedir}/tasks/{id}.json`.
/// IDs are auto-incrementing u64, assigned on create.
pub struct FileTaskStore {
    tasks_dir: PathBuf,
}

impl FileTaskStore {
    /// Create a new FileTaskStore, using `{basedir}/tasks/` for storage.
    /// Creates the tasks directory if it doesn't exist.
    pub async fn new(basedir: impl AsRef<Path>) -> Result<Self> {
        let tasks_dir = basedir.as_ref().join("tasks");
        fs::create_dir_all(&tasks_dir)
            .await
            .map_err(StoreError::Io)?;
        Ok(Self { tasks_dir })
    }

    /// Read a single task from disk by its file path.
    async fn read_task_file(&self, path: &Path) -> Result<Option<Task>> {
        match fs::read_to_string(path).await {
            Ok(content) => {
                let task: Task = serde_json::from_str(&content)
                    .map_err(|e| StoreError::Serialization(e.to_string()))?;
                Ok(Some(task))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(StoreError::Io(e)),
        }
    }

    /// Write a single task to disk atomically (write to temp, then rename).
    async fn write_task_file(&self, task: &Task) -> Result<()> {
        let path = self.task_path(&task.id);
        let tmp = path.with_extension("json.tmp");
        let content = serde_json::to_string_pretty(task)
            .map_err(|e| StoreError::Serialization(e.to_string()))?;
        fs::write(&tmp, content).await.map_err(StoreError::Io)?;
        fs::rename(&tmp, &path).await.map_err(StoreError::Io)?;
        Ok(())
    }

    /// Get the file path for a task ID.
    fn task_path(&self, task_id: &TaskId) -> PathBuf {
        self.tasks_dir.join(format!("{}.json", task_id.0))
    }

    /// Scan the tasks directory for existing task IDs.
    async fn scan_task_ids(&self) -> Result<Vec<u64>> {
        let mut ids = Vec::new();
        let mut entries = fs::read_dir(&self.tasks_dir)
            .await
            .map_err(StoreError::Io)?;
        while let Some(entry) = entries.next_entry().await.map_err(StoreError::Io)? {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if let Some(id_str) = name.strip_suffix(".json") {
                if let Ok(id) = id_str.parse::<u64>() {
                    ids.push(id);
                }
            }
        }
        Ok(ids)
    }

    /// Load all tasks from disk.
    async fn load_all_tasks(&self) -> Result<Vec<Task>> {
        let ids = self.scan_task_ids().await?;
        let mut tasks = Vec::with_capacity(ids.len());
        for id in &ids {
            let path = self.task_path(&TaskId(*id));
            if let Some(task) = self.read_task_file(&path).await? {
                tasks.push(task);
            }
        }
        Ok(tasks)
    }

    /// Compute the next available task ID.
    async fn next_id(&self) -> Result<TaskId> {
        let ids = self.scan_task_ids().await?;
        let next = ids.iter().max().map_or(1, |m| m + 1);
        Ok(TaskId(next))
    }
}

#[async_trait::async_trait]
impl TaskStore for FileTaskStore {
    async fn create(&self, mut task: Task) -> Result<TaskId> {
        let id = self.next_id().await?;
        task.id = id;
        self.write_task_file(&task).await?;
        Ok(id)
    }

    async fn get(&self, task_id: &TaskId) -> Result<Option<Task>> {
        let path = self.task_path(task_id);
        self.read_task_file(&path).await
    }

    async fn update(&self, task: Task) -> Result<()> {
        let path = self.task_path(&task.id);
        if !path.exists() {
            return Err(StoreError::NotFound(format!("Task {}", task.id)));
        }
        self.write_task_file(&task).await
    }

    async fn delete(&self, task_id: &TaskId) -> Result<()> {
        let path = self.task_path(task_id);
        match fs::remove_file(&path).await {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(StoreError::Io(e)),
        }
    }

    async fn list(&self, status: Option<TaskStatus>) -> Result<Vec<Task>> {
        let tasks = self.load_all_tasks().await?;
        Ok(tasks
            .into_iter()
            .filter(|t| status.is_none_or(|s| t.status == s))
            .collect())
    }

    async fn get_ready_tasks(&self) -> Result<Vec<TaskId>> {
        let tasks = self.load_all_tasks().await?;
        let completed_ids: std::collections::HashSet<TaskId> = tasks
            .iter()
            .filter(|t| t.status == TaskStatus::Completed)
            .map(|t| t.id)
            .collect();

        let ready = tasks
            .iter()
            .filter(|t| {
                t.status == TaskStatus::Pending
                    && t.dependencies.iter().all(|d| completed_ids.contains(d))
            })
            .map(|t| t.id)
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
        let store = FileTaskStore::new(dir.path()).await.unwrap();
        (store, dir)
    }

    #[tokio::test]
    async fn test_create_and_get() {
        let (store, _dir) = temp_store().await;
        let task = Task::new(TaskKind::Agent, "file task".to_string(), vec![]);
        let id = store.create(task).await.unwrap();
        let got = store.get(&id).await.unwrap().unwrap();
        assert_eq!(got.description, "file task");
    }

    #[tokio::test]
    async fn test_persistence() {
        let (store, dir) = temp_store().await;
        let task = Task::new(TaskKind::Agent, "persist".to_string(), vec![]);
        let id = store.create(task).await.unwrap();

        // Create a new store instance pointing to the same basedir
        let store2 = FileTaskStore::new(dir.path()).await.unwrap();
        let got = store2.get(&id).await.unwrap().unwrap();
        assert_eq!(got.description, "persist");
    }

    #[tokio::test]
    async fn test_update() {
        let (store, dir) = temp_store().await;
        let task = Task::new(TaskKind::Agent, "original".to_string(), vec![]);
        let id = store.create(task).await.unwrap();

        let mut updated = store.get(&id).await.unwrap().unwrap();
        updated.description = "modified".to_string();
        updated.status = TaskStatus::Completed;
        store.update(updated).await.unwrap();

        // Verify via fresh store
        let store2 = FileTaskStore::new(dir.path()).await.unwrap();
        let got = store2.get(&id).await.unwrap().unwrap();
        assert_eq!(got.description, "modified");
        assert_eq!(got.status, TaskStatus::Completed);
    }

    #[tokio::test]
    async fn test_delete() {
        let (store, _dir) = temp_store().await;
        let task = Task::new(TaskKind::Agent, "delete me".to_string(), vec![]);
        let id = store.create(task).await.unwrap();
        store.delete(&id).await.unwrap();
        let got = store.get(&id).await.unwrap();
        assert!(got.is_none());
    }

    #[tokio::test]
    async fn test_update_nonexistent_returns_not_found() {
        let (store, _dir) = temp_store().await;
        let fake = Task {
            id: TaskId(999),
            status: TaskStatus::Pending,
            kind: TaskKind::Agent,
            description: "ghost".to_string(),
            dependencies: vec![],
            result: None,
            summary: None,
            output_file: None,
            created_at: std::time::SystemTime::now(),
            started_at: None,
            completed_at: None,
        };
        let err = store.update(fake).await.unwrap_err();
        assert!(matches!(err, StoreError::NotFound(_)));
    }

    #[tokio::test]
    async fn test_get_ready_tasks() {
        let (store, _dir) = temp_store().await;
        let t1 = Task::new(TaskKind::Agent, "task 1".to_string(), vec![]);
        let id1 = store.create(t1).await.unwrap();

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

    #[tokio::test]
    async fn test_each_task_is_separate_file() {
        let (store, dir) = temp_store().await;
        let tasks_dir = dir.path().join("tasks");

        let t1 = Task::new(TaskKind::Agent, "task one".to_string(), vec![]);
        store.create(t1).await.unwrap();

        let t2 = Task::new(TaskKind::Agent, "task two".to_string(), vec![]);
        store.create(t2).await.unwrap();

        // Should have exactly 2 files in tasks/
        let entries: Vec<_> = std::fs::read_dir(&tasks_dir).unwrap().collect();
        assert_eq!(entries.len(), 2);

        // Files should be named {id}.json
        for entry in entries {
            let entry = entry.unwrap();
            let name = entry.file_name();
            let name = name.to_str().unwrap();
            assert!(name.ends_with(".json"), "unexpected file: {}", name);
        }
    }

    #[tokio::test]
    async fn test_update_one_task_does_not_touch_others() {
        let (store, dir) = temp_store().await;
        let tasks_dir = dir.path().join("tasks");

        let t1 = Task::new(TaskKind::Agent, "original 1".to_string(), vec![]);
        store.create(t1).await.unwrap();

        let t2 = Task::new(TaskKind::Agent, "original 2".to_string(), vec![]);
        let id2 = store.create(t2).await.unwrap();

        // Record modification time of task 1
        let mtime_before =
            std::fs::metadata(tasks_dir.join("1.json")).unwrap().modified().unwrap();

        // Sleep briefly to ensure different mtime
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        // Update only task 2
        let mut updated = store.get(&id2).await.unwrap().unwrap();
        updated.description = "modified 2".to_string();
        store.update(updated).await.unwrap();

        // Task 1 file should be unchanged
        let mtime_after =
            std::fs::metadata(tasks_dir.join("1.json")).unwrap().modified().unwrap();
        assert_eq!(mtime_before, mtime_after);
    }
}
