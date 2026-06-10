//! TaskStop tool — stops a running task by setting its status to Killed.

use std::sync::Arc;

use async_trait::async_trait;
use serde::Deserialize;
use vol_llm_tool::{ExecutableTool, ToolContext, ToolResult, ToolResultType, ToolSensitivity};

use crate::model::{TaskId, TaskStatus};
use crate::store::TaskStore;

#[derive(Debug, Deserialize)]
struct TaskStopParams {
    task_id: String,
}

pub struct TaskStop {
    store: Arc<dyn TaskStore>,
}

impl TaskStop {
    pub fn new(store: Arc<dyn TaskStore>) -> Self {
        Self { store }
    }
}

#[async_trait]
impl ExecutableTool for TaskStop {
    fn name(&self) -> &'static str {
        "task_stop"
    }

    fn description(&self) -> &'static str {
        "Stops a running background task by its ID."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "task_id": {
                    "type": "string",
                    "description": "The ID of the background task to stop"
                }
            },
            "required": ["task_id"]
        })
    }

    fn sensitivity(&self, _args: &serde_json::Value) -> ToolSensitivity {
        ToolSensitivity::RequiresApproval {
            reason: "This will terminate a running task".to_string(),
        }
    }

    async fn execute(
        &self,
        args: &serde_json::Value,
        _context: &ToolContext,
    ) -> ToolResultType<ToolResult> {
        let params: TaskStopParams = serde_json::from_value(args.clone()).map_err(|e| {
            vol_llm_tool::ToolError::InvalidArguments(format!("Failed to parse arguments: {}", e))
        })?;

        let task_id: TaskId = params.task_id.parse::<u64>().map(TaskId).map_err(|e| {
            vol_llm_tool::ToolError::InvalidArguments(format!("Invalid task ID: {}", e))
        })?;

        let task = self.store.get(&task_id).await.map_err(|e| {
            vol_llm_tool::ToolError::ExecutionFailed(format!("Failed to get task: {}", e))
        })?;

        let mut task = match task {
            Some(t) => t,
            None => {
                return Ok(ToolResult::failure(format!(
                    "Task #{} not found",
                    task_id.0
                )));
            }
        };

        if matches!(task.status, TaskStatus::Completed | TaskStatus::Failed) {
            return Ok(ToolResult::failure(format!(
                "Task #{} is not running (status: {:?})",
                task_id.0, task.status
            )));
        }

        task.status = TaskStatus::Killed;
        task.completed_at = Some(std::time::SystemTime::now());
        let subject = task.subject.clone();

        self.store.update(task).await.map_err(|e| {
            vol_llm_tool::ToolError::ExecutionFailed(format!("Failed to stop task: {}", e))
        })?;

        Ok(ToolResult {
            success: true,
            content: format!("Successfully stopped task #{}: {}", task_id.0, subject),
            error: None,
            data: Some(serde_json::json!({
                "success": true,
                "task_id": task_id.0.to_string()
            })),
            call_id: String::new(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Task, TaskId, TaskKind};
    use crate::stores::InMemoryTaskStore;
    use vol_llm_tool::ExecutableTool;

    fn tool() -> TaskStop {
        TaskStop::new(Arc::new(InMemoryTaskStore::new()))
    }

    #[tokio::test]
    async fn test_stop_running_task() {
        let store = Arc::new(InMemoryTaskStore::new());
        let mut task = Task::new(TaskKind::Agent, "stop me".to_string(), vec![]);
        task.id = TaskId(1);
        task.status = TaskStatus::Running;
        store.create(task).await.unwrap();

        let t = TaskStop::new(store.clone());
        let args = serde_json::json!({ "task_id": "1" });
        let result = t.execute(&args, &ToolContext::default()).await.unwrap();
        assert!(result.success);

        let task = store.get(&TaskId(1)).await.unwrap().unwrap();
        assert_eq!(task.status, TaskStatus::Killed);
    }

    #[tokio::test]
    async fn test_stop_nonexistent_task() {
        let t = tool();
        let args = serde_json::json!({ "task_id": "999" });
        let result = t.execute(&args, &ToolContext::default()).await.unwrap();
        assert!(!result.success);
    }

    #[tokio::test]
    async fn test_stop_already_killed() {
        let store = Arc::new(InMemoryTaskStore::new());
        let mut task = Task::new(TaskKind::Agent, "already dead".to_string(), vec![]);
        task.id = TaskId(1);
        task.status = TaskStatus::Killed;
        store.create(task).await.unwrap();

        let t = TaskStop::new(store);
        let args = serde_json::json!({ "task_id": "1" });
        let result = t.execute(&args, &ToolContext::default()).await.unwrap();
        assert!(result.success);
    }
}
