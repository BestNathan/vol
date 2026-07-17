//! TaskUpdate tool — update task status, description, and dependencies.

use std::sync::Arc;

use async_trait::async_trait;
use serde::Deserialize;
use vol_llm_tool::{ExecutableTool, ToolContext, ToolResult, ToolResultType, ToolSensitivity};

use crate::model::{TaskId, TaskStatus};
use crate::store::TaskStore;

#[derive(Debug, Deserialize)]
struct TaskUpdateParams {
    #[serde(rename = "taskId")]
    task_id: String,
    subject: Option<String>,
    description: Option<String>,
    #[serde(rename = "activeForm")]
    active_form: Option<String>,
    status: Option<String>,
    #[serde(rename = "addDependencies", default)]
    add_dependencies: Vec<String>,
    #[serde(rename = "addBlocks", default)]
    add_blocks: Vec<String>,
}

pub struct TaskUpdate {
    store: Arc<dyn TaskStore>,
}

impl TaskUpdate {
    pub fn new(store: Arc<dyn TaskStore>) -> Self {
        Self { store }
    }
}

#[async_trait]
impl ExecutableTool for TaskUpdate {
    fn name(&self) -> &'static str {
        "task_update"
    }

    fn description(&self) -> &'static str {
        "Use this tool to update a task's status, description, or dependency relationships."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "taskId": {
                    "type": "string",
                    "description": "The ID of the task to update"
                },
                "subject": {
                    "type": "string",
                    "description": "New subject for the task"
                },
                "description": {
                    "type": "string",
                    "description": "New description for the task"
                },
                "activeForm": {
                    "type": "string",
                    "description": "Present continuous form shown in spinner when in_progress"
                },
                "status": {
                    "type": "string",
                    "enum": ["pending", "running", "completed", "failed", "killed"],
                    "description": "New status for the task"
                },
                "addDependencies": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Task IDs that this task depends on"
                },
                "addBlocks": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Task IDs that depend on this task"
                }
            },
            "required": ["taskId"]
        })
    }

    fn sensitivity(&self, _args: &serde_json::Value) -> ToolSensitivity {
        ToolSensitivity::RequiresApproval {
            reason: "This operation modifies task state and dependencies".to_string(),
        }
    }

    async fn execute(
        &self,
        args: &serde_json::Value,
        _context: &ToolContext,
    ) -> ToolResultType<ToolResult> {
        let params: TaskUpdateParams = serde_json::from_value(args.clone()).map_err(|e| {
            vol_llm_tool::ToolError::InvalidArguments(format!("Failed to parse arguments: {e}"))
        })?;

        let task_id: TaskId = params.task_id.parse::<u64>().map(TaskId).map_err(|e| {
            vol_llm_tool::ToolError::InvalidArguments(format!("Invalid task ID: {e}"))
        })?;

        let task = self.store.get(&task_id).await.map_err(|e| {
            vol_llm_tool::ToolError::ExecutionFailed(format!("Failed to get task: {e}"))
        })?;

        let mut task = match task {
            Some(t) => t,
            None => {
                return Ok(ToolResult {
                    success: false,
                    content: format!("Task #{} not found", task_id.0),
                    error: Some("Task not found".to_string()),
                    data: Some(serde_json::json!({
                        "success": false,
                        "taskId": task_id.0.to_string(),
                        "updatedFields": []
                    })),
                    call_id: String::new(),
                });
            }
        };

        let mut updated_fields = Vec::new();

        if let Some(ref subject) = params.subject {
            task.subject = subject.clone();
            updated_fields.push("subject");
        }
        if let Some(ref description) = params.description {
            task.description = description.clone();
            updated_fields.push("description");
        }
        if let Some(ref active_form) = params.active_form {
            task.active_form = Some(active_form.clone());
            updated_fields.push("activeForm");
        }
        if let Some(ref status) = params.status {
            let new_status = match status.as_str() {
                "pending" => TaskStatus::Pending,
                "running" => TaskStatus::Running,
                "completed" => TaskStatus::Completed,
                "failed" => TaskStatus::Failed,
                "killed" => TaskStatus::Killed,
                other => {
                    return Ok(ToolResult {
                        success: false,
                        content: format!("Invalid status: {other}"),
                        error: Some(format!("Invalid status: {other}")),
                        data: Some(serde_json::json!({
                            "success": false,
                            "taskId": task_id.0.to_string(),
                            "updatedFields": []
                        })),
                        call_id: String::new(),
                    });
                }
            };
            task.status = new_status;
            updated_fields.push("status");
        }

        for dep_id_str in &params.add_dependencies {
            if let Ok(dep_id) = dep_id_str.parse::<u64>().map(TaskId) {
                if !task.dependencies.contains(&dep_id) {
                    task.dependencies.push(dep_id);
                }
            }
        }
        if !params.add_dependencies.is_empty() {
            updated_fields.push("dependencies");
        }

        for block_id_str in &params.add_blocks {
            if let Ok(block_id) = block_id_str.parse::<u64>().map(TaskId) {
                if !task.blocks.contains(&block_id) {
                    task.blocks.push(block_id);
                }
            }
        }
        if !params.add_blocks.is_empty() {
            updated_fields.push("blocks");
        }

        self.store.update(task).await.map_err(|e| {
            vol_llm_tool::ToolError::ExecutionFailed(format!("Failed to update task: {e}"))
        })?;

        Ok(ToolResult {
            success: true,
            content: format!("Updated task #{} {}", task_id.0, updated_fields.join(", ")),
            error: None,
            data: Some(serde_json::json!({
                "success": true,
                "taskId": task_id.0.to_string(),
                "updatedFields": updated_fields
            })),
            call_id: String::new(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::TaskKind;
    use crate::stores::InMemoryTaskStore;
    use crate::Task;
    use vol_llm_tool::ExecutableTool;

    fn tool() -> TaskUpdate {
        TaskUpdate::new(Arc::new(InMemoryTaskStore::new()))
    }

    #[tokio::test]
    async fn test_update_status() {
        let store = Arc::new(InMemoryTaskStore::new());
        let id = store
            .create(Task::new(TaskKind::Agent, "update me".to_string(), vec![]))
            .await
            .unwrap();

        let t = TaskUpdate::new(store.clone());
        let args = serde_json::json!({
            "taskId": id.0.to_string(),
            "status": "completed"
        });
        let result = t.execute(&args, &ToolContext::default()).await.unwrap();
        assert!(result.success);

        let task = store.get(&id).await.unwrap().unwrap();
        assert_eq!(task.status, TaskStatus::Completed);
    }

    #[tokio::test]
    async fn test_update_nonexistent_returns_failure() {
        let t = tool();
        let args = serde_json::json!({
            "taskId": "999",
            "status": "completed"
        });
        let result = t.execute(&args, &ToolContext::default()).await.unwrap();
        assert!(!result.success);
    }

    #[tokio::test]
    async fn test_add_dependencies() {
        let store = Arc::new(InMemoryTaskStore::new());
        let id1 = store
            .create(Task::new(TaskKind::Agent, "dep".to_string(), vec![]))
            .await
            .unwrap();
        let id2 = store
            .create(Task::new(TaskKind::Agent, "main".to_string(), vec![]))
            .await
            .unwrap();

        let t = TaskUpdate::new(store.clone());
        let args = serde_json::json!({
            "taskId": id2.0.to_string(),
            "addDependencies": [id1.0.to_string()]
        });
        t.execute(&args, &ToolContext::default()).await.unwrap();

        let task = store.get(&id2).await.unwrap().unwrap();
        assert_eq!(task.dependencies, vec![id1]);
    }

    #[tokio::test]
    async fn test_add_blocks() {
        let store = Arc::new(InMemoryTaskStore::new());
        let id1 = store
            .create(Task::new(TaskKind::Agent, "main".to_string(), vec![]))
            .await
            .unwrap();
        let id2 = store
            .create(Task::new(TaskKind::Agent, "after".to_string(), vec![]))
            .await
            .unwrap();

        let t = TaskUpdate::new(store.clone());
        let args = serde_json::json!({
            "taskId": id1.0.to_string(),
            "addBlocks": [id2.0.to_string()]
        });
        t.execute(&args, &ToolContext::default()).await.unwrap();

        let task = store.get(&id1).await.unwrap().unwrap();
        assert_eq!(task.blocks, vec![id2]);
    }
}
