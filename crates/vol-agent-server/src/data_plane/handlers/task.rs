use std::sync::Arc;

use async_trait::async_trait;
use vol_llm_task::{TaskStatus, TaskStore};

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
                            "id": t.id,
                            "status": format!("{:?}", t.status).to_lowercase(),
                            "kind": format!("{:?}", t.kind).to_lowercase(),
                            "publisher": t.publisher,
                            "assignee": t.assignee,
                            "subject": t.subject,
                            "description": t.description,
                            "active_form": t.active_form,
                            "dependencies": t.dependencies,
                            "blocks": t.blocks,
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
                let task = self.store.get(&task_id).await.unwrap_or(None);
                let task_json = task.map(|t| {
                    serde_json::json!({
                        "id": t.id,
                        "status": format!("{:?}", t.status).to_lowercase(),
                        "kind": format!("{:?}", t.kind).to_lowercase(),
                        "publisher": t.publisher,
                        "assignee": t.assignee,
                        "subject": t.subject,
                        "description": t.description,
                        "active_form": t.active_form,
                        "dependencies": t.dependencies,
                        "blocks": t.blocks,
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
    use vol_llm_task::{InMemoryTaskStore, Task, TaskId, TaskKind, TaskStore};

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

    /// Store holding a single dependency-free task, which gets id 1.
    async fn handler_with_one_task() -> TaskHandler {
        let store = Arc::new(InMemoryTaskStore::new());
        store
            .create(Task::new(TaskKind::Manual, "first".to_string(), vec![]))
            .await
            .expect("create first");
        TaskHandler::new(store)
    }

    /// Store holding task 1 (blocks task 2) and task 2 (depends on task 1).
    async fn handler_with_dependent_task() -> TaskHandler {
        let store = Arc::new(InMemoryTaskStore::new());
        let first = store
            .create(Task::new(TaskKind::Manual, "first".to_string(), vec![]))
            .await
            .expect("create first");
        let second = store
            .create(Task::new(
                TaskKind::Manual,
                "second".to_string(),
                vec![first],
            ))
            .await
            .expect("create second");
        let mut first_task = store
            .get(&first)
            .await
            .expect("get first")
            .expect("present");
        first_task.blocks = vec![second];
        store.update(first_task).await.expect("update first");
        TaskHandler::new(store)
    }

    async fn list(handler: &TaskHandler) -> serde_json::Value {
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
            .expect("list");
        replies[0].payload.data_json()
    }

    async fn get(handler: &TaskHandler, task_id: TaskId) -> serde_json::Value {
        let replies = handler
            .handle(msg(
                "1",
                Operation::Task(TaskOperation::Get),
                Payload::Task(TaskPayload::Get { task_id }),
            ))
            .await
            .expect("get");
        replies[0].payload.data_json()
    }

    #[tokio::test]
    async fn test_task_list_emits_string_ids() {
        let handler = handler_with_one_task().await;
        let result = list(&handler).await;
        let tasks = result["tasks"].as_array().expect("array");
        assert_eq!(tasks[0]["id"], serde_json::json!("1"));
        assert_eq!(tasks[0]["dependencies"], serde_json::json!([]));
        assert_eq!(tasks[0]["blocks"], serde_json::json!([]));
    }

    #[tokio::test]
    async fn test_task_list_emits_string_ids_in_dependency_arrays() {
        let handler = handler_with_dependent_task().await;
        let result = list(&handler).await;
        let tasks = result["tasks"].as_array().expect("array");
        let first = tasks
            .iter()
            .find(|t| t["id"] == serde_json::json!("1"))
            .expect("task 1");
        let second = tasks
            .iter()
            .find(|t| t["id"] == serde_json::json!("2"))
            .expect("task 2");
        assert_eq!(first["blocks"], serde_json::json!(["2"]));
        assert_eq!(second["dependencies"], serde_json::json!(["1"]));
    }

    #[tokio::test]
    async fn test_task_get_emits_string_ids_including_dependencies() {
        let handler = handler_with_dependent_task().await;
        let result = get(&handler, TaskId(2)).await;
        assert_eq!(result["task"]["id"], serde_json::json!("2"));
        assert_eq!(result["task"]["dependencies"], serde_json::json!(["1"]));
        assert_eq!(result["task"]["blocks"], serde_json::json!([]));
    }

    #[tokio::test]
    async fn test_task_get_emits_string_ids_in_blocks() {
        let handler = handler_with_dependent_task().await;
        let result = get(&handler, TaskId(1)).await;
        assert_eq!(result["task"]["id"], serde_json::json!("1"));
        assert_eq!(result["task"]["blocks"], serde_json::json!(["2"]));
        assert_eq!(result["task"]["dependencies"], serde_json::json!([]));
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
                Payload::Task(TaskPayload::Get {
                    task_id: TaskId(99999),
                }),
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
                Payload::Task(TaskPayload::Get { task_id: TaskId(0) }),
            ))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("task.list"));
    }
}
