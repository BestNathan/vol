//! TaskCreate tool — creates a new task with subject, description, activeForm.

use std::sync::Arc;

use async_trait::async_trait;
use serde::Deserialize;
use vol_llm_tool::{ExecutableTool, ToolContext, ToolResult, ToolResultType};

use crate::model::{Task, TaskKind};
use crate::store::TaskStore;

#[derive(Debug, Deserialize)]
struct TaskCreateParams {
    subject: String,
    #[serde(default)]
    description: String,
    #[serde(rename = "activeForm", default)]
    active_form: Option<String>,
    #[serde(default)]
    assignee: Option<String>,
}

pub struct TaskCreate {
    store: Arc<dyn TaskStore>,
}

impl TaskCreate {
    pub fn new(store: Arc<dyn TaskStore>) -> Self {
        Self { store }
    }
}

#[async_trait]
impl ExecutableTool for TaskCreate {
    fn name(&self) -> &'static str {
        "task_create"
    }

    fn description(&self) -> &'static str {
        "Use this tool to create a new task. Tasks are managed by the task scheduler and can have dependencies on other tasks."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "subject": {
                    "type": "string",
                    "description": "A brief title for the task"
                },
                "description": {
                    "type": "string",
                    "description": "What needs to be done"
                },
                "activeForm": {
                    "type": "string",
                    "description": "Present continuous form shown in spinner when in_progress (e.g., \"Running tests\")"
                },
                "assignee": {
                    "type": "string",
                    "description": "Agent type to assign this task to. Omit for open claim."
                }
            },
            "required": ["subject", "description"]
        })
    }

    async fn execute(
        &self,
        args: &serde_json::Value,
        context: &ToolContext,
    ) -> ToolResultType<ToolResult> {
        let params: TaskCreateParams = serde_json::from_value(args.clone()).map_err(|e| {
            vol_llm_tool::ToolError::InvalidArguments(format!("Failed to parse arguments: {e}"))
        })?;

        let mut task = Task::new(TaskKind::Agent, params.subject.clone(), vec![]);
        task.description = params.description;
        task.active_form = params.active_form;
        task.publisher = context.agent_def.as_ref().map(|a| a.r#type.clone());
        task.assignee = params.assignee;

        let id = self.store.create(task).await.map_err(|e| {
            vol_llm_tool::ToolError::ExecutionFailed(format!("Failed to create task: {e}"))
        })?;

        Ok(ToolResult::success(format!(
            "Task #{} created successfully: {}",
            id.0, params.subject
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stores::InMemoryTaskStore;

    fn tool() -> TaskCreate {
        TaskCreate::new(Arc::new(InMemoryTaskStore::new()))
    }

    #[tokio::test]
    async fn test_create_task_minimal() {
        let t = tool();
        let args = serde_json::json!({
            "subject": "write tests",
            "description": "add unit tests for the parser"
        });
        let result = t.execute(&args, &ToolContext::default()).await.unwrap();
        assert!(result.success);
    }

    #[tokio::test]
    async fn test_create_task_with_active_form() {
        let store = Arc::new(InMemoryTaskStore::new());
        let t = TaskCreate::new(store.clone());
        let args = serde_json::json!({
            "subject": "run tests",
            "description": "execute all tests",
            "activeForm": "Running tests"
        });
        let result = t.execute(&args, &ToolContext::default()).await.unwrap();
        assert!(result.success);

        // Verify the task was created with active_form
        let task = store.get(&crate::model::TaskId(1)).await.unwrap().unwrap();
        assert_eq!(task.active_form, Some("Running tests".to_string()));
    }
}
