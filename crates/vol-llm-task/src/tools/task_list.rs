//! TaskList tool — lists tasks with optional status filter.

use std::sync::Arc;

use async_trait::async_trait;
use serde::Deserialize;
use vol_llm_tool::{ExecutableTool, ToolContext, ToolResult, ToolResultType};

use crate::model::TaskStatus;
use crate::store::TaskStore;

#[derive(Debug, Deserialize)]
struct TaskListParams {
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    assignee: Option<String>,
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
                },
                "assignee": {
                    "type": "string",
                    "description": "Filter by assignee: 'me' (current agent), specific agent_type, or 'unassigned'"
                }
            }
        })
    }

    async fn execute(
        &self,
        args: &serde_json::Value,
        context: &ToolContext,
    ) -> ToolResultType<ToolResult> {
        let params: TaskListParams = serde_json::from_value(args.clone()).map_err(|e| {
            vol_llm_tool::ToolError::InvalidArguments(format!("Failed to parse arguments: {}", e))
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

        let mut tasks = self.store.list(status).await.map_err(|e| {
            vol_llm_tool::ToolError::ExecutionFailed(format!("Failed to list tasks: {}", e))
        })?;

        // Apply assignee filter
        if let Some(ref assignee_filter) = params.assignee {
            let effective_filter = match assignee_filter.as_str() {
                "me" => context.agent_def.as_ref().map(|a| a.r#type.clone()),
                "unassigned" => Some(String::new()),
                other => Some(other.to_string()),
            };

            if let Some(filter) = effective_filter {
                if filter.is_empty() {
                    tasks.retain(|t| t.assignee.is_none());
                } else {
                    tasks.retain(|t| t.assignee.as_deref() == Some(&filter));
                }
            }
        }

        let tasks_json: Vec<serde_json::Value> = tasks
            .iter()
            .map(|t| {
                serde_json::json!({
                    "id": t.id.0.to_string(),
                    "status": format!("{:?}", t.status).to_lowercase(),
                    "subject": t.subject,
                    "summary": t.summary,
                    "publisher": t.publisher,
                    "assignee": t.assignee,
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
    use vol_llm_core::AgentDef;

    fn tool() -> TaskList {
        TaskList::new(Arc::new(InMemoryTaskStore::new()))
    }

    fn make_context(agent_type: &str) -> ToolContext {
        ToolContext::default().with_agent_def(AgentDef::new(agent_type, String::new()))
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

    #[tokio::test]
    async fn test_list_by_assignee_me() {
        let store: Arc<dyn TaskStore> = Arc::new(InMemoryTaskStore::new());
        let mut t1 = Task::new(TaskKind::Agent, "Task 1".into(), vec![]);
        t1.assignee = Some("coding".into());
        store.create(t1).await.unwrap();

        let mut t2 = Task::new(TaskKind::Agent, "Task 2".into(), vec![]);
        t2.assignee = Some("qa".into());
        store.create(t2).await.unwrap();

        let t3 = Task::new(TaskKind::Agent, "Task 3".into(), vec![]);
        store.create(t3).await.unwrap();

        let tool = TaskList::new(store.clone());
        let ctx = make_context("coding");
        let args = serde_json::json!({"assignee": "me"});
        let result = tool.execute(&args, &ctx).await.unwrap();

        let data = result.data.unwrap();
        let tasks = data.get("tasks").unwrap().as_array().unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0]["subject"], "Task 1");
    }

    #[tokio::test]
    async fn test_list_unassigned() {
        let store: Arc<dyn TaskStore> = Arc::new(InMemoryTaskStore::new());
        let mut t1 = Task::new(TaskKind::Agent, "Assigned".into(), vec![]);
        t1.assignee = Some("coding".into());
        store.create(t1).await.unwrap();

        let t2 = Task::new(TaskKind::Agent, "Unassigned".into(), vec![]);
        store.create(t2).await.unwrap();

        let tool = TaskList::new(store.clone());
        let ctx = make_context("qa");
        let args = serde_json::json!({"assignee": "unassigned"});
        let result = tool.execute(&args, &ctx).await.unwrap();

        let data = result.data.unwrap();
        let tasks = data.get("tasks").unwrap().as_array().unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0]["subject"], "Unassigned");
    }

    #[tokio::test]
    async fn test_list_by_specific_assignee() {
        let store: Arc<dyn TaskStore> = Arc::new(InMemoryTaskStore::new());
        let mut t1 = Task::new(TaskKind::Agent, "Coding task".into(), vec![]);
        t1.assignee = Some("coding".into());
        store.create(t1).await.unwrap();

        let mut t2 = Task::new(TaskKind::Agent, "Another coding task".into(), vec![]);
        t2.assignee = Some("coding".into());
        store.create(t2).await.unwrap();

        let mut t3 = Task::new(TaskKind::Agent, "QA task".into(), vec![]);
        t3.assignee = Some("qa".into());
        store.create(t3).await.unwrap();

        let tool = TaskList::new(store.clone());
        let ctx = make_context("manager");
        let args = serde_json::json!({"assignee": "coding"});
        let result = tool.execute(&args, &ctx).await.unwrap();

        let data = result.data.unwrap();
        let tasks = data.get("tasks").unwrap().as_array().unwrap();
        assert_eq!(tasks.len(), 2);
    }
}
