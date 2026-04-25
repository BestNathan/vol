//! File-based task store — one JSON file per task, cross-process safe.

use std::io::{Read, Seek, Write};
use std::path::{Path, PathBuf};

use fd_lock::RwLock;

use crate::model::{Task, TaskId, TaskStatus};
use crate::store::{Result, StoreError, TaskStore};

/// File-based task store — persists each task to its own JSON file.
///
/// Tasks are stored in `{basedir}/tasks/{id}.json`.
/// IDs are auto-incrementing u64, assigned on create.
/// The `.lock` file serves as both flock handle and ID counter.
pub struct FileTaskStore {
    tasks_dir: PathBuf,
}

impl FileTaskStore {
    /// Create a new FileTaskStore, using `{basedir}/tasks/` for storage.
    pub async fn new(basedir: impl AsRef<Path>) -> Result<Self> {
        let tasks_dir = basedir.as_ref().join("tasks");
        tokio::fs::create_dir_all(&tasks_dir)
            .await
            .map_err(StoreError::Io)?;
        Ok(Self { tasks_dir })
    }

    /// Allocate the next task ID under exclusive flock.
    fn allocate_id(&self) -> Result<TaskId> {
        let lock_path = self.tasks_dir.join(".lock");
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(&lock_path)
            .map_err(StoreError::Io)?;

        let mut flock = RwLock::new(file);
        let mut guard = flock.write().map_err(StoreError::Io)?;

        let mut buf = String::new();
        guard.read_to_string(&mut buf).map_err(StoreError::Io)?;
        let current: u64 = buf.trim().parse().unwrap_or(0);

        let next = current + 1;
        guard.rewind().map_err(StoreError::Io)?;
        guard.set_len(0).map_err(StoreError::Io)?;
        write!(guard, "{}", next).map_err(StoreError::Io)?;
        guard.flush().map_err(StoreError::Io)?;

        Ok(TaskId(next))
    }

    fn write_task_file(&self, task: &Task) -> Result<()> {
        let path = self.task_path(&task.id);
        let content = serde_json::to_string_pretty(task)
            .map_err(|e| StoreError::Serialization(e.to_string()))?;
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, &content).map_err(StoreError::Io)?;
        std::fs::rename(&tmp, &path).map_err(StoreError::Io)?;
        Ok(())
    }

    fn read_task_file(&self, path: &Path) -> Result<Option<Task>> {
        match std::fs::read_to_string(path) {
            Ok(content) => {
                let task: Task = serde_json::from_str(&content)
                    .map_err(|e| StoreError::Serialization(e.to_string()))?;
                Ok(Some(task))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(StoreError::Io(e)),
        }
    }

    fn task_path(&self, task_id: &TaskId) -> PathBuf {
        self.tasks_dir.join(format!("{}.json", task_id.0))
    }

    fn scan_task_ids(&self) -> Result<Vec<u64>> {
        let mut ids = Vec::new();
        for entry in std::fs::read_dir(&self.tasks_dir).map_err(StoreError::Io)? {
            let entry = entry.map_err(StoreError::Io)?;
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

    fn load_all_tasks(&self) -> Result<Vec<Task>> {
        let ids = self.scan_task_ids()?;
        let mut tasks = Vec::with_capacity(ids.len());
        for id in &ids {
            let path = self.task_path(&TaskId(*id));
            if let Some(task) = self.read_task_file(&path)? {
                tasks.push(task);
            }
        }
        Ok(tasks)
    }
}

#[async_trait::async_trait]
impl TaskStore for FileTaskStore {
    async fn create(&self, mut task: Task) -> Result<TaskId> {
        let id = self.allocate_id()?;
        task.id = id;
        self.write_task_file(&task)?;
        Ok(id)
    }

    async fn get(&self, task_id: &TaskId) -> Result<Option<Task>> {
        let path = self.task_path(task_id);
        self.read_task_file(&path)
    }

    async fn update(&self, task: Task) -> Result<()> {
        let path = self.task_path(&task.id);
        if !path.exists() {
            return Err(StoreError::NotFound(format!("Task {}", task.id)));
        }
        let content = serde_json::to_string_pretty(&task)
            .map_err(|e| StoreError::Serialization(e.to_string()))?;
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, &content).map_err(StoreError::Io)?;
        std::fs::rename(&tmp, &path).map_err(StoreError::Io)
    }

    async fn delete(&self, task_id: &TaskId) -> Result<()> {
        let path = self.task_path(task_id);
        match tokio::fs::remove_file(&path).await {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(StoreError::Io(e)),
        }
    }

    async fn list(&self, status: Option<TaskStatus>) -> Result<Vec<Task>> {
        let tasks = self.load_all_tasks()?;
        Ok(tasks
            .into_iter()
            .filter(|t| status.is_none_or(|s| t.status == s))
            .collect())
    }

    async fn get_ready_tasks(&self) -> Result<Vec<TaskId>> {
        let tasks = self.load_all_tasks()?;
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

        let mut t1_done = store.get(&id1).await.unwrap().unwrap();
        t1_done.status = TaskStatus::Completed;
        store.update(t1_done).await.unwrap();

        let t2 = Task::new(TaskKind::Agent, "task 2".to_string(), vec![id1]);
        store.create(t2).await.unwrap();

        let t3 = Task::new(TaskKind::Agent, "task 3".to_string(), vec![]);
        store.create(t3).await.unwrap();

        let ready = store.get_ready_tasks().await.unwrap();
        assert_eq!(ready.len(), 2);
    }

    #[tokio::test]
    async fn test_each_task_is_separate_file() {
        let (store, dir) = temp_store().await;
        let tasks_dir = dir.path().join("tasks");

        let t1 = Task::new(TaskKind::Agent, "task one".to_string(), vec![]);
        store.create(t1).await.unwrap();

        let t2 = Task::new(TaskKind::Agent, "task two".to_string(), vec![]);
        store.create(t2).await.unwrap();

        let entries: Vec<_> = std::fs::read_dir(&tasks_dir)
            .unwrap()
            .filter_map(|e| {
                let e = e.unwrap();
                let name = e.file_name();
                let name = name.to_str().unwrap().to_string();
                name.ends_with(".json").then_some(e)
            })
            .collect();
        assert_eq!(entries.len(), 2);

        for entry in entries {
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

        let mtime_before =
            std::fs::metadata(tasks_dir.join("1.json")).unwrap().modified().unwrap();

        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        let mut updated = store.get(&id2).await.unwrap().unwrap();
        updated.description = "modified 2".to_string();
        store.update(updated).await.unwrap();

        let mtime_after =
            std::fs::metadata(tasks_dir.join("1.json")).unwrap().modified().unwrap();
        assert_eq!(mtime_before, mtime_after);
    }

    #[tokio::test]
    async fn test_concurrent_creates_unique_ids() {
        let (store, _dir) = temp_store().await;
        let store = std::sync::Arc::new(store);

        let handles: Vec<_> = (0..10)
            .map(|i| {
                let store = std::sync::Arc::clone(&store);
                tokio::spawn(async move {
                    let task = Task::new(
                        TaskKind::Agent,
                        format!("concurrent task {}", i),
                        vec![],
                    );
                    store.create(task).await.unwrap()
                })
            })
            .collect();

        let mut ids = Vec::new();
        for h in handles {
            ids.push(h.await.unwrap());
        }

        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), 10, "all 10 concurrent creates must have unique IDs");
    }

    #[tokio::test]
    async fn test_counter_persists_across_restarts() {
        let (store, dir) = temp_store().await;
        let t1 = Task::new(TaskKind::Agent, "first".to_string(), vec![]);
        let id1 = store.create(t1).await.unwrap();

        let store2 = FileTaskStore::new(dir.path()).await.unwrap();
        let t2 = Task::new(TaskKind::Agent, "second".to_string(), vec![]);
        let id2 = store2.create(t2).await.unwrap();

        assert_ne!(id1, id2, "second create must get a different ID");
    }
}
