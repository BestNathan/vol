//! TaskList tool — lists tasks with optional status filter.

use std::sync::Arc;

use async_trait::async_trait;
use serde::Deserialize;
use vol_llm_tool::{ExecutableTool, ToolContext, ToolResult, ToolResultType};

use crate::model::TaskStatus;
use crate::store::TaskStore;

#[derive(Debug, Deserialize)]
struct TaskListParams {
    status: Option<String>,
}

pub struct TaskList {
    store: Arc<dyn TaskStore>,
}

impl TaskList {
    pub fn new(store: Arc<dyn TaskStore>) -> Self {
        Self { store }
    }
}

#[async_trait]
impl ExecutableTool for TaskList {
    fn name(&self) -> &'static str {
        "task_list"
    }

    fn description(&self) -> &'static str {
        "Use this tool to list all tasks in the task list."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "status": {
                    "type": "string",
                    "enum": ["pending", "running", "completed", "failed", "killed"],
                    "description": "Filter by task status"
                }
            }
        })
    }

    async fn execute(
        &self,
        args: &serde_json::Value,
        _context: &ToolContext,
    ) -> ToolResultType<ToolResult> {
        let params: TaskListParams = serde_json::from_value(args.clone())
            .map_err(|e| {
                vol_llm_tool::ToolError::InvalidArguments(format!(
                    "Failed to parse arguments: {}",
                    e
                ))
            })?;

        let status = match params.status.as_deref() {
            Some("pending") => Some(TaskStatus::Pending),
            Some("running") => Some(TaskStatus::Running),
            Some("completed") => Some(TaskStatus::Completed),
            Some("failed") => Some(TaskStatus::Failed),
            Some("killed") => Some(TaskStatus::Killed),
            Some(other) => {
                return Ok(ToolResult::failure(format!(
                    "Invalid status: {}. Valid values: pending, running, completed, failed, killed",
                    other
                )));
            }
            None => None,
        };

        let tasks = self.store.list(status).await.map_err(|e| {
            vol_llm_tool::ToolError::ExecutionFailed(format!(
                "Failed to list tasks: {}",
                e
            ))
        })?;

        let tasks_json: Vec<serde_json::Value> = tasks
            .iter()
            .map(|t| {
                serde_json::json!({
                    "id": t.id.0.to_string(),
                    "status": format!("{:?}", t.status).to_lowercase(),
                    "subject": t.subject,
                    "summary": t.summary,
                })
            })
            .collect();

        let result_data = serde_json::json!({
            "tasks": tasks_json
        });

        Ok(ToolResult {
            success: true,
            content: if tasks.is_empty() {
                "No tasks found".to_string()
            } else {
                format!("Found {} tasks", tasks.len())
            },
            error: None,
            data: Some(result_data),
            call_id: String::new(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Task, TaskKind};
    use crate::stores::InMemoryTaskStore;

    fn tool() -> TaskList {
        TaskList::new(Arc::new(InMemoryTaskStore::new()))
    }

    #[tokio::test]
    async fn test_list_empty() {
        let t = tool();
        let args = serde_json::json!({});
        let result = t.execute(&args, &ToolContext::default()).await.unwrap();
        assert!(result.success);
        let data = result.data.unwrap();
        let tasks = data.get("tasks").unwrap().as_array().unwrap();
        assert_eq!(tasks.len(), 0);
    }

    #[tokio::test]
    async fn test_list_all() {
        let store = Arc::new(InMemoryTaskStore::new());
        store
            .create(Task::new(TaskKind::Agent, "task 1".to_string(), vec![]))
            .await
            .unwrap();
        store
            .create(Task::new(TaskKind::Agent, "task 2".to_string(), vec![]))
            .await
            .unwrap();

        let t = TaskList::new(store);
        let args = serde_json::json!({});
        let result = t.execute(&args, &ToolContext::default()).await.unwrap();
        let data = result.data.unwrap();
        let tasks = data.get("tasks").unwrap().as_array().unwrap();
        assert_eq!(tasks.len(), 2);
    }

    #[tokio::test]
    async fn test_list_by_status() {
        let store = Arc::new(InMemoryTaskStore::new());
        let _id = store
            .create(Task::new(TaskKind::Agent, "done".to_string(), vec![]))
            .await
            .unwrap();

        // Mark as completed
        let mut task = store.get(&_id).await.unwrap().unwrap();
        task.status = TaskStatus::Completed;
        store.update(task).await.unwrap();

        // Also create a pending task
        store
            .create(Task::new(TaskKind::Agent, "pending".to_string(), vec![]))
            .await
            .unwrap();

        let t = TaskList::new(store);
        let args = serde_json::json!({ "status": "completed" });
        let result = t.execute(&args, &ToolContext::default()).await.unwrap();
        let data = result.data.unwrap();
        let tasks = data.get("tasks").unwrap().as_array().unwrap();
        assert_eq!(tasks.len(), 1);
    }
}
