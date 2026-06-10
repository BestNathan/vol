//! In-memory task store using DashMap.

use std::sync::atomic::{AtomicU64, Ordering};

use crate::model::{Task, TaskId, TaskStatus};
use crate::store::{Result, TaskStore};
use dashmap::DashMap;

/// In-memory task store — zero persistence, suitable for development and testing.
pub struct InMemoryTaskStore {
    tasks: DashMap<TaskId, Task>,
    next_id: AtomicU64,
}

impl Default for InMemoryTaskStore {
    fn default() -> Self {
        Self::new()
    }
}

impl InMemoryTaskStore {
    pub fn new() -> Self {
        Self {
            tasks: DashMap::new(),
            next_id: AtomicU64::new(1),
        }
    }

    fn assign_id(&self) -> TaskId {
        TaskId(self.next_id.fetch_add(1, Ordering::Relaxed))
    }
}

#[async_trait::async_trait]
impl TaskStore for InMemoryTaskStore {
    async fn create(&self, mut task: Task) -> Result<TaskId> {
        let id = self.assign_id();
        task.id = id;
        self.tasks.insert(id, task);
        Ok(id)
    }

    async fn get(&self, task_id: &TaskId) -> Result<Option<Task>> {
        Ok(self.tasks.get(task_id).map(|r| r.value().clone()))
    }

    async fn update(&self, task: Task) -> Result<()> {
        self.tasks.insert(task.id, task);
        Ok(())
    }

    async fn delete(&self, task_id: &TaskId) -> Result<()> {
        self.tasks.remove(task_id);
        Ok(())
    }

    async fn list(&self, status: Option<TaskStatus>) -> Result<Vec<Task>> {
        Ok(self
            .tasks
            .iter()
            .filter(|r| status.is_none_or(|s| r.value().status == s))
            .map(|r| r.value().clone())
            .collect())
    }

    async fn get_ready_tasks(&self) -> Result<Vec<TaskId>> {
        use std::collections::HashSet;

        let pending_tasks: Vec<_> = self
            .tasks
            .iter()
            .filter(|r| r.value().status == TaskStatus::Pending)
            .map(|r| (*r.key(), r.value().dependencies.clone()))
            .collect();

        let completed_ids: HashSet<TaskId> = self
            .tasks
            .iter()
            .filter(|r| r.value().status == TaskStatus::Completed)
            .map(|r| *r.key())
            .collect();

        let ready: Vec<TaskId> = pending_tasks
            .into_iter()
            .filter(|(_, deps)| deps.iter().all(|d| completed_ids.contains(d)))
            .map(|(id, _)| id)
            .collect();

        Ok(ready)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::TaskKind;

    #[tokio::test]
    async fn test_create_and_get() {
        let store = InMemoryTaskStore::new();
        let task = Task::new(TaskKind::Agent, "test task".to_string(), vec![]);
        let id = store.create(task).await.unwrap();
        let got = store.get(&id).await.unwrap().unwrap();
        assert_eq!(got.subject, "test task");
        assert_eq!(got.status, TaskStatus::Pending);
    }

    #[tokio::test]
    async fn test_ids_auto_increment() {
        let store = InMemoryTaskStore::new();
        let t1 = Task::new(TaskKind::Agent, "first".to_string(), vec![]);
        let id1 = store.create(t1).await.unwrap();

        let t2 = Task::new(TaskKind::Agent, "second".to_string(), vec![]);
        let id2 = store.create(t2).await.unwrap();

        assert_eq!(id1, TaskId(1));
        assert_eq!(id2, TaskId(2));
    }

    #[tokio::test]
    async fn test_list_by_status() {
        let store = InMemoryTaskStore::new();
        let t1 = Task::new(TaskKind::Agent, "task 1".to_string(), vec![]);
        let t2 = Task::new(TaskKind::Agent, "task 2".to_string(), vec![]);
        store.create(t1).await.unwrap();
        store.create(t2).await.unwrap();
        let pending = store.list(Some(TaskStatus::Pending)).await.unwrap();
        assert_eq!(pending.len(), 2);
        let completed = store.list(Some(TaskStatus::Completed)).await.unwrap();
        assert_eq!(completed.len(), 0);
    }

    #[tokio::test]
    async fn test_update_status() {
        let store = InMemoryTaskStore::new();
        let task = Task::new(TaskKind::Agent, "update me".to_string(), vec![]);
        let id = store.create(task).await.unwrap();
        let mut updated = store.get(&id).await.unwrap().unwrap();
        updated.status = TaskStatus::Completed;
        store.update(updated).await.unwrap();
        let got = store.get(&id).await.unwrap().unwrap();
        assert_eq!(got.status, TaskStatus::Completed);
    }

    #[tokio::test]
    async fn test_delete() {
        let store = InMemoryTaskStore::new();
        let task = Task::new(TaskKind::Agent, "delete me".to_string(), vec![]);
        let id = store.create(task).await.unwrap();
        store.delete(&id).await.unwrap();
        let got = store.get(&id).await.unwrap();
        assert!(got.is_none());
    }

    #[tokio::test]
    async fn test_get_ready_tasks_no_deps() {
        let store = InMemoryTaskStore::new();
        let t1 = Task::new(TaskKind::Agent, "task 1".to_string(), vec![]);
        let t2 = Task::new(TaskKind::Agent, "task 2".to_string(), vec![]);
        store.create(t1).await.unwrap();
        store.create(t2).await.unwrap();
        let ready = store.get_ready_tasks().await.unwrap();
        assert_eq!(ready.len(), 2);
    }

    #[tokio::test]
    async fn test_get_ready_tasks_with_deps() {
        let store = InMemoryTaskStore::new();
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
    async fn test_get_ready_tasks_blocked_dep() {
        let store = InMemoryTaskStore::new();
        let t1 = Task::new(TaskKind::Agent, "task 1".to_string(), vec![]);
        let id1 = store.create(t1).await.unwrap();

        let t2 = Task::new(TaskKind::Agent, "task 2".to_string(), vec![id1]);
        store.create(t2).await.unwrap();

        let ready = store.get_ready_tasks().await.unwrap();
        assert_eq!(ready.len(), 1);
        assert!(ready.iter().any(|id| *id == id1));
    }
}
