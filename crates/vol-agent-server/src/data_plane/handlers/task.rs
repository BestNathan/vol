use std::sync::Arc;

use async_trait::async_trait;
use vol_llm_task::{TaskId, TaskStatus, TaskStore};

use vol_llm_agent_protocol::agent_server_protocol::{
    AgentServerMessage, Operation, Payload, ProtocolError, TaskOperation, TaskPayload,
};
use vol_llm_agent_protocol::DomainHandler;

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
                        assignee
                            .as_ref()
                            .is_none_or(|a| t.assignee.as_deref() == Some(a))
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
                    Payload::Task(TaskPayload::GetResult {
                        task: task_json.unwrap_or(serde_json::Value::Null),
                    }),
                )])
            }
            (TaskOperation::List, _) => Err(ProtocolError::PayloadDecodeFailed("task.list")),
            (TaskOperation::Get, _) => Err(ProtocolError::PayloadDecodeFailed("task.get")),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use vol_llm_agent_protocol::agent_server_protocol::{
        AgentServerMessage, MessageKind, Operation, Payload, TaskOperation, TaskPayload,
    };
    use vol_llm_agent_protocol::DomainHandler;
    use vol_llm_task::InMemoryTaskStore;

    use super::TaskHandler;

    fn msg(id: &str, op: Operation, payload: Payload) -> AgentServerMessage {
        AgentServerMessage {
            protocol: "agent-server/1".to_string(),
            message_id: id.to_string(),
            sender: "client".to_string(),
            receiver: "data-plane".to_string(),
            kind: MessageKind::Command,
            operation: op,
            payload,
            meta: Default::default(),
        }
    }

    #[tokio::test]
    async fn task_list_returns_empty_with_empty_store() {
        let store: Arc<dyn vol_llm_task::TaskStore> = Arc::new(InMemoryTaskStore::new());
        let handler = TaskHandler::new(store);
        let replies = handler
            .handle(msg(
                "1",
                Operation::Task(TaskOperation::List),
                Payload::Task(TaskPayload::List {
                    status: None,
                    assignee: None,
                }),
            ))
            .await
            .unwrap();
        let json = replies[0].payload.data_json();
        let tasks = json["tasks"].as_array().unwrap();
        assert!(tasks.is_empty());
    }

    #[tokio::test]
    async fn task_get_returns_null_for_nonexistent_task() {
        let store: Arc<dyn vol_llm_task::TaskStore> = Arc::new(InMemoryTaskStore::new());
        let handler = TaskHandler::new(store);
        let replies = handler
            .handle(msg(
                "1",
                Operation::Task(TaskOperation::Get),
                Payload::Task(TaskPayload::Get { task_id: 99999 }),
            ))
            .await
            .unwrap();
        let json = replies[0].payload.data_json();
        assert!(json["task"].is_null());
    }

    #[tokio::test]
    async fn task_handler_rejects_non_task_operation() {
        let store: Arc<dyn vol_llm_task::TaskStore> = Arc::new(InMemoryTaskStore::new());
        let handler = TaskHandler::new(store);
        let err = handler
            .handle(msg(
                "1",
                Operation::Log(vol_llm_agent_protocol::agent_server_protocol::LogOperation::List),
                Payload::Log(vol_llm_agent_protocol::agent_server_protocol::LogPayload::List),
            ))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("task"));
    }

    #[tokio::test]
    async fn task_list_with_wrong_payload_returns_error() {
        let store: Arc<dyn vol_llm_task::TaskStore> = Arc::new(InMemoryTaskStore::new());
        let handler = TaskHandler::new(store);
        let err = handler
            .handle(msg(
                "1",
                Operation::Task(TaskOperation::List),
                Payload::Task(TaskPayload::Get { task_id: 0 }),
            ))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("task.list"));
    }
}
