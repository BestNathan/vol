use std::sync::Arc;

use axum::extract::ws::{Message as WsMessage, WebSocket};
use chrono::Utc;
use futures::{SinkExt, StreamExt};
use tracing::{error, info, warn};
use vol_llm_agent_channel::{
    agent_server_protocol::{AgentOperation, AgentPayload, ErrorPayload},
    AgentServerMessage, Connection, ConnectionError, MessageKind, Operation, Payload,
};

use crate::events::{EventBus, ManagerEvent};
use crate::metrics::collector::MetricsCollector;
use crate::state::manager::AgentStateManager;
use crate::state::models::{AgentState, AgentStatus};
use crate::task::dispatcher::TaskDispatcher;

struct ManagerConnection {
    tx: Arc<tokio::sync::Mutex<futures::stream::SplitSink<WebSocket, WsMessage>>>,
    rx: tokio::sync::Mutex<futures::stream::SplitStream<WebSocket>>,
}

impl ManagerConnection {
    fn new(ws: WebSocket) -> Self {
        let (tx, rx) = ws.split();
        Self {
            tx: Arc::new(tokio::sync::Mutex::new(tx)),
            rx: tokio::sync::Mutex::new(rx),
        }
    }
}

#[async_trait::async_trait]
impl Connection for ManagerConnection {
    fn protocol(&self) -> &str {
        "ws"
    }

    async fn recv(&self) -> Option<Result<AgentServerMessage, ConnectionError>> {
        loop {
            let msg = {
                let mut rx = self.rx.lock().await;
                rx.next().await?
            };

            match msg {
                Ok(WsMessage::Text(text)) => {
                    return Some(
                        serde_json::from_str::<AgentServerMessage>(&text)
                            .map_err(|e| ConnectionError::ParseError(e.to_string())),
                    );
                }
                Ok(WsMessage::Close(_)) => return None,
                Ok(WsMessage::Binary(_) | WsMessage::Ping(_) | WsMessage::Pong(_)) => continue,
                Err(e) => return Some(Err(ConnectionError::WsReceiveError(e.to_string()))),
            }
        }
    }

    async fn send(&self, msg: AgentServerMessage) -> Result<(), ConnectionError> {
        let text = serde_json::to_string(&msg)
            .map_err(|e| ConnectionError::WsSendError(e.to_string()))?;
        let mut tx = self.tx.lock().await;
        tx.send(WsMessage::Text(text))
            .await
            .map_err(|e| ConnectionError::WsSendError(e.to_string()))
    }
}

pub async fn handle_agent_connection(
    ws: WebSocket,
    token: Option<String>,
    state_manager: Arc<AgentStateManager>,
    metrics: Arc<MetricsCollector>,
    event_bus: Arc<EventBus>,
    task_dispatcher: Arc<TaskDispatcher>,
    expected_token: Option<String>,
) {
    if let Some(expected) = &expected_token {
        if token.as_ref() != Some(expected) {
            let err = error_message(
                None,
                "manager",
                "client",
                "unauthorized",
                "invalid token".to_string(),
            );
            let text = serde_json::to_string(&err).unwrap();
            let mut ws = ws;
            let _ = ws.send(WsMessage::Text(text)).await;
            return;
        }
    }

    let conn = ManagerConnection::new(ws);

    let agent_id = match conn.recv().await {
        Some(Ok(msg)) => match parse_register_message(msg) {
            Some((id, input)) => match serde_json::from_str::<serde_json::Value>(&input) {
                Ok(reg_data) => {
                    let name = reg_data.get("name").and_then(|v| v.as_str()).unwrap_or("unknown").to_string();
                    let agent_type = reg_data.get("type").and_then(|v| v.as_str()).unwrap_or("unknown").to_string();
                    let version = reg_data.get("version").and_then(|v| v.as_str()).unwrap_or("0.0.0").to_string();
                    let capabilities = reg_data.get("capabilities")
                        .and_then(|v| v.as_array())
                        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                        .unwrap_or_default();

                    let host_info = reg_data.get("host_info");
                    let hostname = host_info.and_then(|h| h.get("hostname").and_then(|v| v.as_str())).unwrap_or("unknown");
                    let os = host_info.and_then(|h| h.get("os").and_then(|v| v.as_str())).unwrap_or("unknown");
                    let arch = host_info.and_then(|h| h.get("arch").and_then(|v| v.as_str())).unwrap_or("unknown");
                    let ip = host_info.and_then(|h| h.get("ip").and_then(|v| v.as_str())).unwrap_or("0.0.0.0");

                    let state = AgentState {
                        agent_id: id.clone(),
                        name,
                        r#type: agent_type,
                        version,
                        capabilities,
                        host_info: crate::state::models::HostInfo {
                            hostname: hostname.to_string(),
                            os: os.to_string(),
                            arch: arch.to_string(),
                            ip: ip.to_string(),
                        },
                        status: AgentStatus::Idle,
                        connected_at: Utc::now(),
                        last_heartbeat: Utc::now(),
                    };
                    state_manager.register(state).await;
                    metrics.agent_registered_total.set(
                        state_manager.list_all().await.len() as f64,
                    );
                    event_bus.emit(ManagerEvent::agent_registered(&id));
                    id
                }
                Err(e) => {
                    warn!("Invalid register payload: {}", e);
                    return;
                }
            },
            None => {
                warn!("First message was not register, closing connection");
                return;
            }
        },
        _ => {
            warn!("Connection closed before register");
            return;
        }
    };

    let _ = conn.send(ack_message("manager", &agent_id)).await;

    metrics.agent_connections_current.inc();

    loop {
        match conn.recv().await {
            Some(Ok(msg)) => {
                if let Err(e) = handle_message(
                    &msg, &agent_id, &state_manager, &metrics, &event_bus, &task_dispatcher,
                ).await {
                    let _ = conn.send(error_message(
                        Some(msg.message_id.clone()),
                        "manager",
                        &agent_id,
                        "manager_error",
                        e.to_string(),
                    )).await;
                }
            }
            Some(Err(e)) => {
                error!(agent_id = %agent_id, "WebSocket error: {}", e);
                break;
            }
            None => {
                info!(agent_id = %agent_id, "Agent disconnected");
                state_manager
                    .update_status(&agent_id, AgentStatus::Disconnected)
                    .await;
                metrics.agent_connections_current.dec();
                event_bus.emit(ManagerEvent::agent_disconnected(&agent_id));
                break;
            }
        }
    }
}

fn parse_register_message(msg: AgentServerMessage) -> Option<(String, String)> {
    let AgentServerMessage {
        sender,
        kind: MessageKind::Command,
        operation: Operation::Agent(AgentOperation::Submit),
        payload: Payload::Agent(AgentPayload::Submit { input, metadata, .. }),
        ..
    } = msg else {
        return None;
    };

    let is_register = metadata
        .as_ref()
        .is_some_and(|m| m.get("type").and_then(|v| v.as_str()) == Some("register"));
    if !is_register {
        return None;
    }

    let id = if sender != "client" { sender } else { input.clone() };
    Some((id, input))
}

async fn handle_message(
    msg: &AgentServerMessage,
    agent_id: &str,
    state_manager: &AgentStateManager,
    metrics: &MetricsCollector,
    event_bus: &EventBus,
    task_dispatcher: &TaskDispatcher,
) -> Result<(), anyhow::Error> {
    let agent_type = state_manager
        .get(agent_id)
        .await
        .map(|s| s.r#type.clone())
        .unwrap_or_else(|| "unknown".to_string());

    match (&msg.kind, &msg.operation, &msg.payload) {
        (
            MessageKind::Command,
            Operation::Agent(AgentOperation::Submit),
            Payload::Agent(AgentPayload::Submit { metadata, input, .. }),
        ) => {
            let meta_type = metadata.as_ref().and_then(|m| {
                m.get("type").and_then(|v| v.as_str())
            }).unwrap_or("unknown");

            match meta_type {
                "heartbeat" => {
                    state_manager.update_heartbeat(agent_id).await;
                    let status = serde_json::from_str::<serde_json::Value>(input)
                        .ok()
                        .and_then(|v| v.get("status").and_then(|s| s.as_str()).map(|s| s.to_string()))
                        .unwrap_or_else(|| "Idle".to_string());
                    if status == "Busy" {
                        state_manager.update_status(agent_id, AgentStatus::Busy).await;
                    } else {
                        state_manager.update_status(agent_id, AgentStatus::Idle).await;
                    }
                    metrics.increment_messages("heartbeat", agent_id, &agent_type);
                }
                "metric" => {
                    metrics.increment_metric_samples(agent_id);
                    metrics.increment_messages("metric", agent_id, &agent_type);
                }
                "event" => {
                    let data = serde_json::from_str::<serde_json::Value>(input)
                        .unwrap_or(serde_json::Value::Null);
                    let event_name = data.get("event_name").and_then(|v| v.as_str()).unwrap_or("unknown").to_string();
                    event_bus.emit(ManagerEvent::agent_event(agent_id, &event_name, data));
                    metrics.increment_messages("event", agent_id, &agent_type);
                }
                "task_result" => {
                    let data = serde_json::from_str::<serde_json::Value>(input)
                        .unwrap_or(serde_json::Value::Null);
                    let status = data.get("status").and_then(|v| v.as_str()).unwrap_or("unknown");
                    let result = data.get("result").cloned();
                    let error = data.get("error").and_then(|v| v.as_str());

                    let task_id = metadata.as_ref()
                        .and_then(|m| m.get("task_id"))
                        .and_then(|v| v.as_str());

                    if let Some(task_id) = task_id {
                        match status {
                            "Completed" => {
                                task_dispatcher.complete_task(task_id, result, None).await;
                                event_bus.emit(ManagerEvent::task_completed(task_id, agent_id));
                            }
                            "Failed" => {
                                let error_msg = error.unwrap_or("unknown error");
                                task_dispatcher.fail_task(task_id, error_msg).await;
                                event_bus.emit(ManagerEvent::task_failed(task_id, agent_id, error_msg));
                            }
                            _ => {
                                warn!(task_id, agent_id, status = %status, "Unknown task result status");
                            }
                        }
                    }
                    metrics.increment_messages("task_result", agent_id, &agent_type);
                }
                unknown => {
                    return Err(anyhow::anyhow!("Unknown submit message type: {}", unknown));
                }
            }
        }
        (
            MessageKind::Command,
            Operation::Agent(AgentOperation::Cancel),
            Payload::Agent(AgentPayload::Cancel { req_id }),
        ) => {
            info!(agent_id, req_id, "Received cancel request");
        }
        _ => {}
    }
    Ok(())
}

fn ack_message(sender: &str, receiver: &str) -> AgentServerMessage {
    AgentServerMessage {
        protocol: "agent-server/1".to_string(),
        message_id: uuid::Uuid::new_v4().to_string(),
        sender: sender.to_string(),
        receiver: receiver.to_string(),
        kind: MessageKind::Ack,
        operation: Operation::Agent(AgentOperation::Submit),
        payload: Payload::Agent(AgentPayload::SubmitAck {
            run_id: String::new(),
            accepted: true,
        }),
        meta: Default::default(),
    }
}

fn error_message(
    message_id: Option<String>,
    sender: &str,
    receiver: &str,
    code: &str,
    message: String,
) -> AgentServerMessage {
    AgentServerMessage {
        protocol: "agent-server/1".to_string(),
        message_id: message_id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
        sender: sender.to_string(),
        receiver: receiver.to_string(),
        kind: MessageKind::Error,
        operation: Operation::Agent(AgentOperation::Submit),
        payload: Payload::Error(ErrorPayload {
            code: code.to_string(),
            message,
            detail: None,
            terminal: false,
        }),
        meta: Default::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_register_metadata() {
        let json = serde_json::json!({
            "name": "test-agent",
            "type": "react-agent",
            "version": "0.1.0",
            "capabilities": ["Read", "Bash"],
            "host_info": {
                "hostname": "host1",
                "os": "linux",
                "arch": "x86_64",
                "ip": "10.0.0.1"
            }
        });
        assert_eq!(json.get("name").and_then(|v| v.as_str()), Some("test-agent"));
    }

    #[test]
    fn test_parse_heartbeat_metadata() {
        let meta = serde_json::json!({
            "type": "heartbeat",
            "status": "Idle"
        });
        assert_eq!(meta.get("type").and_then(|v| v.as_str()), Some("heartbeat"));
    }

    #[test]
    fn test_message_serialization() {
        let msg = AgentServerMessage::new_command(
            "req-1",
            Operation::Agent(AgentOperation::Submit),
            Payload::Agent(AgentPayload::Submit {
                input: "hello".to_string(),
                target: None,
                metadata: None,
            }),
        );
        let serialized = serde_json::to_string(&msg).unwrap();
        assert!(serialized.contains("agent-server/1"));
        assert!(serialized.contains("hello"));
    }
}
