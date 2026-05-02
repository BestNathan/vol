use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::Serialize;
use tokio::sync::RwLock;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Serialize, PartialEq)]
pub enum TaskStatus {
    Pending,
    Dispatched,
    Running,
    Completed,
    Failed,
    Timeout,
}

#[derive(Debug, Clone, Serialize)]
pub struct Task {
    pub id: String,
    pub agent_id: String,
    pub task_type: String,
    pub parameters: serde_json::Value,
    pub timeout: Duration,
    pub status: TaskStatus,
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub created_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dispatched_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<DateTime<Utc>>,
}

pub struct TaskDispatcher {
    tasks: Arc<RwLock<HashMap<String, Task>>>,
}

impl TaskDispatcher {
    pub fn new() -> Self {
        Self {
            tasks: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Create a new task and return it.
    pub async fn create_task(
        &self,
        agent_id: &str,
        task_type: &str,
        parameters: serde_json::Value,
        timeout: Option<Duration>,
    ) -> Task {
        let id = Uuid::new_v4().to_string();
        let task = Task {
            id: id.clone(),
            agent_id: agent_id.to_string(),
            task_type: task_type.to_string(),
            parameters,
            timeout: timeout.unwrap_or(Duration::from_secs(300)),
            status: TaskStatus::Pending,
            result: None,
            error: None,
            created_at: Utc::now(),
            dispatched_at: None,
            completed_at: None,
        };
        let mut guard = self.tasks.write().await;
        guard.insert(id, task.clone());
        task
    }

    /// Get a task by ID.
    pub async fn get_task(&self, task_id: &str) -> Option<Task> {
        let guard = self.tasks.read().await;
        guard.get(task_id).cloned()
    }

    /// Update task status.
    pub async fn update_status(&self, task_id: &str, status: TaskStatus) {
        let mut guard = self.tasks.write().await;
        if let Some(task) = guard.get_mut(task_id) {
            task.status = status;
            if status == TaskStatus::Dispatched {
                task.dispatched_at = Some(Utc::now());
            }
        }
    }

    /// Mark task as completed with optional result.
    pub async fn complete_task(
        &self,
        task_id: &str,
        result: Option<serde_json::Value>,
        _duration_ms: Option<u64>,
    ) {
        let mut guard = self.tasks.write().await;
        if let Some(task) = guard.get_mut(task_id) {
            task.status = TaskStatus::Completed;
            task.result = result;
            task.completed_at = Some(Utc::now());
        }
    }

    /// Mark task as failed with error message.
    pub async fn fail_task(&self, task_id: &str, error: &str) {
        let mut guard = self.tasks.write().await;
        if let Some(task) = guard.get_mut(task_id) {
            task.status = TaskStatus::Failed;
            task.error = Some(error.to_string());
            task.completed_at = Some(Utc::now());
        }
    }

    /// Mark task as timed out.
    pub async fn timeout_task(&self, task_id: &str) {
        let mut guard = self.tasks.write().await;
        if let Some(task) = guard.get_mut(task_id) {
            task.status = TaskStatus::Timeout;
            task.completed_at = Some(Utc::now());
        }
    }

    /// List all tasks.
    pub async fn list_tasks(&self) -> Vec<Task> {
        let guard = self.tasks.read().await;
        guard.values().cloned().collect()
    }

    /// List tasks for a specific agent.
    pub async fn list_tasks_by_agent(&self, agent_id: &str) -> Vec<Task> {
        let guard = self.tasks.read().await;
        guard
            .values()
            .filter(|t| t.agent_id == agent_id)
            .cloned()
            .collect()
    }
}

impl Default for TaskDispatcher {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_create_task() {
        let td = TaskDispatcher::new();
        let task = td
            .create_task(
                "agent-1",
                "run-query",
                json!({"query": "select *"}),
                Some(Duration::from_secs(60)),
            )
            .await;
        assert_eq!(task.status, TaskStatus::Pending);
        assert_eq!(task.agent_id, "agent-1");
        assert_eq!(task.task_type, "run-query");

        let got = td.get_task(&task.id).await;
        assert!(got.is_some());
    }

    #[tokio::test]
    async fn test_update_task_status() {
        let td = TaskDispatcher::new();
        let task = td.create_task("agent-1", "test", json!({}), None).await;
        let id = task.id.clone();
        td.update_status(&id, TaskStatus::Dispatched).await;
        let got = td.get_task(&id).await.unwrap();
        assert_eq!(got.status, TaskStatus::Dispatched);
    }

    #[tokio::test]
    async fn test_complete_task() {
        let td = TaskDispatcher::new();
        let task = td.create_task("agent-1", "test", json!({}), None).await;
        let id = task.id.clone();
        td.complete_task(&id, Some(json!({"result": "ok"})), None).await;
        let got = td.get_task(&id).await.unwrap();
        assert_eq!(got.status, TaskStatus::Completed);
        assert!(got.result.is_some());
    }

    #[tokio::test]
    async fn test_fail_task() {
        let td = TaskDispatcher::new();
        let task = td.create_task("agent-1", "test", json!({}), None).await;
        let id = task.id.clone();
        td.fail_task(&id, "something went wrong").await;
        let got = td.get_task(&id).await.unwrap();
        assert_eq!(got.status, TaskStatus::Failed);
    }

    #[tokio::test]
    async fn test_list_tasks() {
        let td = TaskDispatcher::new();
        td.create_task("a", "t1", json!({}), None).await;
        td.create_task("b", "t2", json!({}), None).await;
        let all = td.list_tasks().await;
        assert_eq!(all.len(), 2);

        let filtered = td.list_tasks_by_agent("a").await;
        assert_eq!(filtered.len(), 1);
    }
}
