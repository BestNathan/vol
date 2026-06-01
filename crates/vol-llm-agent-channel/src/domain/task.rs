use std::sync::Arc;

use async_trait::async_trait;
use vol_llm_task::{TaskId, TaskStatus, TaskStore};

use crate::agent_server_protocol::{
    AgentServerMessage, Operation, Payload, ProtocolError, TaskOperation, TaskPayload,
};
use crate::domain::handler::DomainHandler;

pub struct TaskHandler {
    store: Arc<dyn TaskStore>,
}

impl TaskHandler {
    pub fn new(store: Arc<dyn TaskStore>) -> Self {
        Self { store }
    }
}

#[async_trait]
impl DomainHandler for TaskHandler {
    fn name(&self) -> &str {
        "task"
    }

    fn operations(&self) -> Vec<Operation> {
        vec![
            Operation::Task(TaskOperation::List),
            Operation::Task(TaskOperation::Get),
        ]
    }

    async fn handle(
        &self,
        message: AgentServerMessage,
    ) -> Result<Vec<AgentServerMessage>, ProtocolError> {
        let op = match &message.operation {
            Operation::Task(op) => op.clone(),
            _ => return Err(ProtocolError::PayloadDecodeFailed("task")),
        };
        match (op, message.payload) {
            (TaskOperation::List, Payload::Task(TaskPayload::List { status, assignee })) => {
                let status_filter = status.and_then(|s| match s.as_str() {
                    "pending" => Some(TaskStatus::Pending),
                    "running" => Some(TaskStatus::Running),
                    "completed" => Some(TaskStatus::Completed),
                    "failed" => Some(TaskStatus::Failed),
                    "killed" => Some(TaskStatus::Killed),
                    _ => None,
                });
                let tasks = self.store.list(status_filter).await.unwrap_or_default();
                let filtered: Vec<serde_json::Value> = tasks
                    .into_iter()
                    .filter(|t| {
                        assignee.as_ref().map_or(true, |a| t.assignee.as_deref() == Some(a))
                    })
                    .map(|t| {
                        serde_json::json!({
                            "id": t.id.0,
                            "status": format!("{:?}", t.status).to_lowercase(),
                            "kind": format!("{:?}", t.kind).to_lowercase(),
                            "publisher": t.publisher,
                            "assignee": t.assignee,
                            "subject": t.subject,
                            "description": t.description,
                            "active_form": t.active_form,
                            "dependencies": t.dependencies.iter().map(|d| d.0).collect::<Vec<_>>(),
                            "blocks": t.blocks.iter().map(|d| d.0).collect::<Vec<_>>(),
                            "created_at": t.created_at.duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0),
                            "started_at": t.started_at.and_then(|s| s.duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).ok()),
                            "completed_at": t.completed_at.and_then(|s| s.duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).ok()),
                        })
                    })
                    .collect();
                Ok(vec![AgentServerMessage::new_result(
                    message.message_id,
                    Operation::Task(TaskOperation::List),
                    Payload::Task(TaskPayload::ListResult { tasks: filtered }),
                )])
            }
            (TaskOperation::Get, Payload::Task(TaskPayload::Get { task_id })) => {
                let task = self.store.get(&TaskId(task_id)).await.unwrap_or(None);
                let task_json = task.map(|t| {
                    serde_json::json!({
                        "id": t.id.0,
                        "status": format!("{:?}", t.status).to_lowercase(),
                        "kind": format!("{:?}", t.kind).to_lowercase(),
                        "publisher": t.publisher,
                        "assignee": t.assignee,
                        "subject": t.subject,
                        "description": t.description,
                        "active_form": t.active_form,
                        "dependencies": t.dependencies.iter().map(|d| d.0).collect::<Vec<_>>(),
                        "blocks": t.blocks.iter().map(|d| d.0).collect::<Vec<_>>(),
                        "created_at": t.created_at.duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0),
                        "started_at": t.started_at.and_then(|s| s.duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).ok()),
                        "completed_at": t.completed_at.and_then(|s| s.duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).ok()),
                    })
                });
                Ok(vec![AgentServerMessage::new_result(
                    message.message_id,
                    Operation::Task(TaskOperation::Get),
                    Payload::Task(TaskPayload::GetResult { task: task_json.unwrap_or(serde_json::Value::Null) }),
                )])
            }
            (TaskOperation::List, _) => Err(ProtocolError::PayloadDecodeFailed("task.list")),
            (TaskOperation::Get, _) => Err(ProtocolError::PayloadDecodeFailed("task.get")),
        }
    }
}
