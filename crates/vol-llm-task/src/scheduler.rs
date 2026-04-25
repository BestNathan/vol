//! TaskScheduler — dependency-aware scheduling facade over TaskStore.

use std::path::PathBuf;
use std::sync::Arc;

use crate::model::{Task, TaskId, TaskKind, TaskResult, TaskStatus};
use crate::store::{Result, TaskStore};

/// Scheduler that delegates to a TaskStore backend.
pub struct TaskScheduler {
    store: Arc<dyn TaskStore>,
    work_dir: PathBuf,
}

impl TaskScheduler {
    /// Create a new TaskScheduler with the given store backend.
    pub fn new(store: Arc<dyn TaskStore>, work_dir: PathBuf) -> Self {
        Self { store, work_dir }
    }

    /// Create a new task and persist it to the store.
    pub async fn create_task(
        &self,
        kind: TaskKind,
        description: String,
        dependencies: Vec<TaskId>,
    ) -> Result<TaskId> {
        let task = Task::new(kind, description, dependencies);
        let id = task.id;
        self.store.create(task).await?;
        Ok(id)
    }

    /// Mark a task as completed with its result.
    pub async fn mark_completed(
        &self,
        task_id: &TaskId,
        result: TaskResult,
        summary: String,
    ) -> Result<()> {
        let mut task = self
            .store
            .get(task_id)
            .await?
            .ok_or_else(|| {
                crate::store::StoreError::NotFound(format!("Task {}", task_id))
            })?;

        task.status = TaskStatus::Completed;
        task.result = Some(result);
        task.summary = Some(summary);
        task.completed_at = Some(std::time::SystemTime::now());

        self.store.update(task).await
    }

    /// Mark a task as failed.
    pub async fn mark_failed(&self, task_id: &TaskId, error: String) -> Result<()> {
        let mut task = self
            .store
            .get(task_id)
            .await?
            .ok_or_else(|| {
                crate::store::StoreError::NotFound(format!("Task {}", task_id))
            })?;

        task.status = TaskStatus::Failed;
        task.summary = Some(format!("Failed: {}", error));
        task.completed_at = Some(std::time::SystemTime::now());

        self.store.update(task).await
    }

    /// Terminate a running task.
    pub async fn kill(&self, task_id: &TaskId) -> Result<()> {
        let mut task = self
            .store
            .get(task_id)
            .await?
            .ok_or_else(|| {
                crate::store::StoreError::NotFound(format!("Task {}", task_id))
            })?;

        task.status = TaskStatus::Killed;
        task.completed_at = Some(std::time::SystemTime::now());

        self.store.update(task).await
    }

    /// Check if all tasks are in a terminal state (Completed, Failed, or Killed).
    pub async fn all_complete(&self) -> Result<bool> {
        let tasks = self.store.list(None).await?;
        Ok(!tasks.is_empty()
            && tasks
                .iter()
                .all(|t| matches!(
                    t.status,
                    TaskStatus::Completed | TaskStatus::Failed | TaskStatus::Killed
                )))
    }

    /// Get the underlying store reference for direct queries.
    pub fn store(&self) -> &Arc<dyn TaskStore> {
        &self.store
    }

    /// Get the working directory.
    pub fn work_dir(&self) -> &PathBuf {
        &self.work_dir
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stores::InMemoryTaskStore;

    fn scheduler() -> TaskScheduler {
        TaskScheduler::new(
            Arc::new(InMemoryTaskStore::new()),
            PathBuf::from("/tmp"),
        )
    }

    #[tokio::test]
    async fn test_create_task() {
        let sched = scheduler();
        let id = sched
            .create_task(TaskKind::Agent, "test".to_string(), vec![])
            .await
            .unwrap();
        assert!(id.0 > 0);
    }

    #[tokio::test]
    async fn test_mark_completed() {
        let sched = scheduler();
        let id = sched
            .create_task(TaskKind::Agent, "done".to_string(), vec![])
            .await
            .unwrap();

        let result = TaskResult {
            success: true,
            output_truncated: "output".to_string(),
            output_file: PathBuf::from("/tmp/out.txt"),
        };
        sched
            .mark_completed(&id, result, "all good".to_string())
            .await
            .unwrap();

        let task = sched.store().get(&id).await.unwrap().unwrap();
        assert_eq!(task.status, TaskStatus::Completed);
        assert_eq!(task.summary, Some("all good".to_string()));
    }

    #[tokio::test]
    async fn test_mark_failed() {
        let sched = scheduler();
        let id = sched
            .create_task(TaskKind::Agent, "fail".to_string(), vec![])
            .await
            .unwrap();

        sched
            .mark_failed(&id, "something broke".to_string())
            .await
            .unwrap();

        let task = sched.store().get(&id).await.unwrap().unwrap();
        assert_eq!(task.status, TaskStatus::Failed);
    }

    #[tokio::test]
    async fn test_kill() {
        let sched = scheduler();
        let id = sched
            .create_task(TaskKind::Agent, "kill me".to_string(), vec![])
            .await
            .unwrap();

        sched.kill(&id).await.unwrap();

        let task = sched.store().get(&id).await.unwrap().unwrap();
        assert_eq!(task.status, TaskStatus::Killed);
    }

    #[tokio::test]
    async fn test_all_complete() {
        let sched = scheduler();
        let t1 = sched
            .create_task(TaskKind::Agent, "task 1".to_string(), vec![])
            .await
            .unwrap();
        let t2 = sched
            .create_task(TaskKind::Agent, "task 2".to_string(), vec![])
            .await
            .unwrap();

        // Not complete yet — both still Pending
        assert!(!sched.all_complete().await.unwrap());

        // Complete t1
        let result = TaskResult {
            success: true,
            output_truncated: String::new(),
            output_file: PathBuf::from("/tmp"),
        };
        sched
            .mark_completed(&t1, result, "done".to_string())
            .await
            .unwrap();

        // Still not complete — t2 is pending
        assert!(!sched.all_complete().await.unwrap());

        // Kill t2
        sched.kill(&t2).await.unwrap();

        // Now all complete
        assert!(sched.all_complete().await.unwrap());
    }

    #[tokio::test]
    async fn test_all_complete_empty() {
        let sched = scheduler();
        // No tasks = not "all complete" (empty set)
        assert!(!sched.all_complete().await.unwrap());
    }

    #[tokio::test]
    async fn test_ready_tasks_flow() {
        let sched = scheduler();

        // Create t1 (no deps) and t2 (depends on t1)
        let t1 = sched
            .create_task(TaskKind::Agent, "task 1".to_string(), vec![])
            .await
            .unwrap();
        let _t2 = sched
            .create_task(TaskKind::Agent, "task 2".to_string(), vec![t1])
            .await
            .unwrap();

        // Only t1 is ready
        let ready = sched.store().get_ready_tasks().await.unwrap();
        assert_eq!(ready.len(), 1);
        assert_eq!(&ready[0], &t1);

        // Complete t1
        let result = TaskResult {
            success: true,
            output_truncated: String::new(),
            output_file: PathBuf::from("/tmp"),
        };
        sched
            .mark_completed(&t1, result, "done".to_string())
            .await
            .unwrap();

        // Now t2 is also ready
        let ready = sched.store().get_ready_tasks().await.unwrap();
        assert_eq!(ready.len(), 1);
    }
}
