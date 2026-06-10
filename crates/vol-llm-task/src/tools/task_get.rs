//! TaskGet tool — retrieves a task by ID.

use std::sync::Arc;

use async_trait::async_trait;
use serde::Deserialize;
use vol_llm_tool::{ExecutableTool, ToolContext, ToolResult, ToolResultType};

use crate::model::{Task, TaskId};
use crate::store::TaskStore;

#[derive(Debug, Deserialize)]
struct TaskGetParams {
    #[serde(rename = "taskId")]
    task_id: String,
}

pub struct TaskGet {
    store: Arc<dyn TaskStore>,
}

impl TaskGet {
    pub fn new(store: Arc<dyn TaskStore>) -> Self {
        Self { store }
    }
}

#[async_trait]
impl ExecutableTool for TaskGet {
    fn name(&self) -> &'static str {
        "task_get"
    }

    fn description(&self) -> &'static str {
        "Use this tool to retrieve a specific task by its ID."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "taskId": {
                    "type": "string",
                    "description": "The ID of the task to retrieve"
                }
            },
            "required": ["taskId"]
        })
    }

    async fn execute(
        &self,
        args: &serde_json::Value,
        _context: &ToolContext,
    ) -> ToolResultType<ToolResult> {
        let params: TaskGetParams = serde_json::from_value(args.clone()).map_err(|e| {
            vol_llm_tool::ToolError::InvalidArguments(format!("Failed to parse arguments: {}", e))
        })?;

        let task_id: TaskId = params.task_id.parse::<u64>().map(TaskId).map_err(|e| {
            vol_llm_tool::ToolError::InvalidArguments(format!("Invalid task ID: {}", e))
        })?;

        let task = self.store.get(&task_id).await.map_err(|e| {
            vol_llm_tool::ToolError::ExecutionFailed(format!("Failed to get task: {}", e))
        })?;

        match task {
            Some(t) => {
                let task_json = serialize_task(&t);
                Ok(ToolResult {
                    success: true,
                    content: format_task_display(&t),
                    error: None,
                    data: Some(serde_json::json!({ "task": task_json })),
                    call_id: String::new(),
                })
            }
            None => Ok(ToolResult {
                success: true,
                content: "Task not found".to_string(),
                error: None,
                data: Some(serde_json::json!({ "task": null })),
                call_id: String::new(),
            }),
        }
    }
}

fn serialize_task(task: &Task) -> serde_json::Value {
    serde_json::json!({
        "id": task.id.0.to_string(),
        "subject": task.subject,
        "description": task.description,
        "status": format!("{:?}", task.status).to_lowercase(),
        "dependencies": task.dependencies.iter().map(|id| id.0.to_string()).collect::<Vec<_>>(),
        "blocks": task.blocks.iter().map(|id| id.0.to_string()).collect::<Vec<_>>(),
        "summary": task.summary,
    })
}

fn format_task_display(task: &Task) -> String {
    let mut lines = vec![
        format!("Task #{}: {}", task.id.0, task.subject),
        format!("Status: {}", format!("{:?}", task.status).to_lowercase()),
        format!("Description: {}", task.description),
    ];

    if !task.dependencies.is_empty() {
        lines.push(format!(
            "Dependencies: {}",
            task.dependencies
                .iter()
                .map(|id| format!("#{}", id.0))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    if !task.blocks.is_empty() {
        lines.push(format!(
            "Blocks: {}",
            task.blocks
                .iter()
                .map(|id| format!("#{}", id.0))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }

    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::TaskKind;
    use crate::stores::InMemoryTaskStore;
    use vol_llm_tool::ExecutableTool;

    fn tool() -> TaskGet {
        TaskGet::new(Arc::new(InMemoryTaskStore::new()))
    }

    #[tokio::test]
    async fn test_get_existing_task() {
        let store = Arc::new(InMemoryTaskStore::new());
        let id = store
            .create(Task::new(TaskKind::Agent, "get me".to_string(), vec![]))
            .await
            .unwrap();

        let t = TaskGet::new(store);
        let args = serde_json::json!({ "taskId": id.0.to_string() });
        let result = t.execute(&args, &ToolContext::default()).await.unwrap();
        assert!(result.success);
        let data = result.data.unwrap();
        let task = data.get("task").unwrap();
        assert_eq!(task.get("subject").unwrap(), "get me");
    }

    #[tokio::test]
    async fn test_get_nonexistent_task() {
        let t = tool();
        let args = serde_json::json!({ "taskId": "999" });
        let result = t.execute(&args, &ToolContext::default()).await.unwrap();
        assert!(result.success);
        let data = result.data.unwrap();
        assert!(data.get("task").unwrap().is_null());
    }
}
