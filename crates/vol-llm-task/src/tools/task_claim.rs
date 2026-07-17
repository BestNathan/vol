//! task_claim — atomically claim a pending task.

use std::sync::Arc;

use async_trait::async_trait;
use serde::Deserialize;
use vol_llm_tool::{
    ExecutableTool, ToolContext, ToolError, ToolResult, ToolResultType, ToolSensitivity,
};

use crate::model::{TaskId, TaskStatus};
use crate::store::TaskStore;

#[derive(Debug, Deserialize)]
struct TaskClaimParams {
    #[serde(rename = "taskId")]
    task_id: String,
}

pub struct TaskClaim {
    store: Arc<dyn TaskStore>,
}

impl TaskClaim {
    pub fn new(store: Arc<dyn TaskStore>) -> Self {
        Self { store }
    }
}

#[async_trait]
impl ExecutableTool for TaskClaim {
    fn name(&self) -> &'static str {
        "task_claim"
    }

    fn description(&self) -> &'static str {
        "Claim a pending task and execute it. \
         Sets task status to Running and assigns it to you. \
         Returns the task content (subject + description) so you can start working on it."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "taskId": {
                    "type": "string",
                    "description": "ID of the task to claim (e.g. 't1', 't42')"
                }
            },
            "required": ["taskId"]
        })
    }

    fn sensitivity(&self, _args: &serde_json::Value) -> ToolSensitivity {
        ToolSensitivity::Safe
    }

    async fn execute(
        &self,
        args: &serde_json::Value,
        context: &ToolContext,
    ) -> ToolResultType<ToolResult> {
        let params: TaskClaimParams = serde_json::from_value(args.clone())
            .map_err(|e| ToolError::InvalidArguments(format!("Failed to parse task ID: {e}")))?;

        let raw = params.task_id.trim_start_matches('t');
        let id_num: u64 = raw.parse().map_err(|_| {
            ToolError::InvalidArguments(format!("Invalid task ID: {}", params.task_id))
        })?;
        let task_id = TaskId(id_num);

        let caller_type = context
            .agent_def
            .as_ref()
            .map(|a| a.r#type.clone())
            .ok_or_else(|| {
                ToolError::ExecutionFailed("agent identity required for task_claim".into())
            })?;

        let mut task = self
            .store
            .get(&task_id)
            .await
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?
            .ok_or_else(|| {
                ToolError::ExecutionFailed(format!("Task {} not found", params.task_id))
            })?;

        if task.status != TaskStatus::Pending {
            return Err(ToolError::ExecutionFailed(format!(
                "Task {} is not in Pending status (current: {:?})",
                params.task_id, task.status
            )));
        }

        let ready_ids = self
            .store
            .get_ready_tasks()
            .await
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;
        if !ready_ids.contains(&task_id) {
            let uncompleted: Vec<String> = task
                .dependencies
                .iter()
                .filter(|d| !ready_ids.contains(d))
                .map(std::string::ToString::to_string)
                .collect();
            return Err(ToolError::ExecutionFailed(format!(
                "Task {} has uncompleted dependencies: [{}]",
                params.task_id,
                uncompleted.join(", ")
            )));
        }

        task.status = TaskStatus::Running;
        task.assignee = Some(caller_type);
        task.started_at = Some(std::time::SystemTime::now());
        self.store
            .update(task.clone())
            .await
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;

        let output = format!(
            "Task {} claimed and now Running.\n\n---\n# {}\n\n{}",
            params.task_id, task.subject, task.description
        );

        Ok(ToolResult {
            success: true,
            content: output,
            error: None,
            data: Some(serde_json::json!({
                "task": {
                    "id": task.id.to_string(),
                    "subject": task.subject,
                    "description": task.description,
                    "status": "Running",
                    "publisher": task.publisher,
                    "assignee": task.assignee,
                }
            })),
            call_id: String::new(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Task, TaskKind};
    use crate::stores::InMemoryTaskStore;
    use vol_llm_core::AgentDef;

    fn make_context(agent_type: &str) -> ToolContext {
        ToolContext::default().with_agent_def(AgentDef::new(agent_type, String::new()))
    }

    #[tokio::test]
    async fn test_claim_pending_task() {
        let store: Arc<dyn TaskStore> = Arc::new(InMemoryTaskStore::new());
        let mut task = Task::new(TaskKind::Agent, "Test task".into(), vec![]);
        task.description = "Do something useful".into();
        let task_id = store.create(task).await.unwrap();

        let tool = TaskClaim::new(store.clone());
        let ctx = make_context("coding");
        let args = serde_json::json!({"taskId": task_id.to_string()});
        let result = tool.execute(&args, &ctx).await.unwrap();

        assert!(result.content.contains("Test task"));
        assert!(result.content.contains("Do something useful"));

        let stored = store.get(&task_id).await.unwrap().unwrap();
        assert_eq!(stored.status, TaskStatus::Running);
        assert_eq!(stored.assignee, Some("coding".into()));
    }

    #[tokio::test]
    async fn test_claim_non_pending_fails() {
        let store: Arc<dyn TaskStore> = Arc::new(InMemoryTaskStore::new());
        let mut task = Task::new(TaskKind::Agent, "Done task".into(), vec![]);
        task.status = TaskStatus::Completed;
        let task_id = store.create(task).await.unwrap();

        let tool = TaskClaim::new(store.clone());
        let ctx = make_context("qa");
        let args = serde_json::json!({"taskId": task_id.to_string()});
        let result = tool.execute(&args, &ctx).await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("not in Pending status"));
    }

    #[tokio::test]
    async fn test_claim_without_identity_fails() {
        let store: Arc<dyn TaskStore> = Arc::new(InMemoryTaskStore::new());
        let task = Task::new(TaskKind::Agent, "Test".into(), vec![]);
        let task_id = store.create(task).await.unwrap();

        let tool = TaskClaim::new(store.clone());
        let ctx = ToolContext::default();
        let args = serde_json::json!({"taskId": task_id.to_string()});
        let result = tool.execute(&args, &ctx).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("agent identity required"));
    }

    #[tokio::test]
    async fn test_claim_with_uncompleted_dependency_fails() {
        let store: Arc<dyn TaskStore> = Arc::new(InMemoryTaskStore::new());
        let dep = Task::new(TaskKind::Agent, "Dependency".into(), vec![]);
        let dep_id = store.create(dep).await.unwrap();

        let task = Task::new(TaskKind::Agent, "Depends on other".into(), vec![dep_id]);
        let task_id = store.create(task).await.unwrap();

        let tool = TaskClaim::new(store.clone());
        let ctx = make_context("coding");
        let args = serde_json::json!({"taskId": task_id.to_string()});
        let result = tool.execute(&args, &ctx).await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("uncompleted dependencies"));
    }

    #[tokio::test]
    async fn test_claim_not_found() {
        let store: Arc<dyn TaskStore> = Arc::new(InMemoryTaskStore::new());
        let tool = TaskClaim::new(store.clone());
        let ctx = make_context("coding");
        let args = serde_json::json!({"taskId": "t999"});
        let result = tool.execute(&args, &ctx).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }
}
