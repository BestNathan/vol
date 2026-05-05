use std::sync::Arc;

use axum::extract::ws::{Message as WsMessage, WebSocket};
use chrono::Utc;
use futures::{SinkExt, StreamExt};
use tracing::{error, info, warn};
use vol_llm_agent_channel::{Connection, Message};

use crate::events::{EventBus, ManagerEvent};
use crate::metrics::collector::MetricsCollector;
use crate::state::manager::AgentStateManager;
use crate::state::models::{AgentState, AgentStatus};
use crate::task::dispatcher::TaskDispatcher;

/// WebSocket connection adapter implementing the Connection trait.
struct ManagerConnection {
    tx: Arc<tokio::sync::Mutex<futures::stream::SplitSink<WebSocket, WsMessage>>>,
    rx: futures::stream::SplitStream<WebSocket>,
}

impl ManagerConnection {
    fn new(ws: WebSocket) -> Self {
        let (tx, rx) = ws.split();
        Self {
            tx: Arc::new(tokio::sync::Mutex::new(tx)),
            rx,
        }
    }
}

#[async_trait::async_trait]
impl Connection for ManagerConnection {
    fn protocol(&self) -> &str {
        "ws"
    }

    async fn recv(&mut self) -> Option<Result<Message, vol_llm_agent_channel::ConnectionError>> {
        let msg = self.rx.next().await?;
        match msg {
            Ok(WsMessage::Text(text)) => {
                match serde_json::from_str::<Message>(&text) {
                    Ok(msg) => Some(Ok(msg)),
                    Err(e) => Some(Err(vol_llm_agent_channel::ConnectionError::ParseError(e.to_string()))),
                }
            }
            Ok(WsMessage::Close(_)) => None,
            Ok(WsMessage::Binary(_) | WsMessage::Ping(_) | WsMessage::Pong(_)) => {
                self.recv().await
            }
            Err(e) => Some(Err(vol_llm_agent_channel::ConnectionError::WsReceiveError(e.to_string()))),
        }
    }

    async fn send(&self, msg: Message) -> Result<(), vol_llm_agent_channel::ConnectionError> {
        let text = serde_json::to_string(&msg)
            .map_err(|e| vol_llm_agent_channel::ConnectionError::WsSendError(e.to_string()))?;
        let mut tx = self.tx.lock().await;
        tx.send(WsMessage::Text(text))
            .await
            .map_err(|e| vol_llm_agent_channel::ConnectionError::WsSendError(e.to_string()))
    }
}

/// Handle a single WebSocket connection for an agent.
pub async fn handle_agent_connection(
    mut ws: WebSocket,
    token: Option<String>,
    state_manager: Arc<AgentStateManager>,
    metrics: Arc<MetricsCollector>,
    event_bus: Arc<EventBus>,
    task_dispatcher: Arc<TaskDispatcher>,
    expected_token: Option<String>,
) {
    // Auth check
    if let Some(expected) = &expected_token {
        if token.as_ref() != Some(expected) {
            let err = Message::Error {
                req_id: None,
                sender: "manager".to_string(),
                receiver: "client".to_string(),
                message: "invalid token".to_string(),
            };
            let text = serde_json::to_string(&err).unwrap();
            let _ = ws.send(WsMessage::Text(text)).await;
            return;
        }
    }

    let mut conn = ManagerConnection::new(ws);

    // Wait for register message (Submit with metadata type=register)
    let agent_id = match conn.recv().await {
        Some(Ok(Message::Submit { metadata, sender, input, .. })) => {
            let meta = metadata.as_ref();
            let is_register = meta.map_or(false, |m| {
                m.get("type").and_then(|v| v.as_str()) == Some("register")
            });
            if !is_register {
                warn!("First message was not register, closing connection");
                return;
            }

            let id = if sender != "client" { sender } else { input.clone() };

            // Parse register metadata from input field (JSON string)
            match serde_json::from_str::<serde_json::Value>(&input) {
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
            }
        }
        _ => {
            warn!("Connection closed before register");
            return;
        }
    };

    // Send Connected ack
    let _ = conn.send(Message::Connected {
        sender: "manager".to_string(),
        receiver: agent_id.clone(),
    }).await;

    metrics.agent_connections_current.inc();

    // Message loop
    loop {
        match conn.recv().await {
            Some(Ok(msg)) => {
                if let Err(e) = handle_message(
                    &msg, &agent_id, &state_manager, &metrics, &event_bus, &task_dispatcher,
                ).await {
                    let _ = conn.send(Message::Error {
                        req_id: None,
                        sender: "manager".to_string(),
                        receiver: agent_id.clone(),
                        message: e.to_string(),
                    }).await;
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

async fn handle_message(
    msg: &Message,
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

    match msg {
        Message::Submit { metadata, input, .. } => {
            let meta_type = metadata.as_ref().and_then(|m| {
                m.get("type").and_then(|v| v.as_str())
            }).unwrap_or("unknown");

            match meta_type {
                "heartbeat" => {
                    state_manager.update_heartbeat(agent_id).await;
                    let status = serde_json::from_str::<serde_json::Value>(input)
                        .ok()
                        .and_then(|v| v.get("status").and_then(|s| s.as_str().map(String::from)))
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
                    let data: serde_json::Value = serde_json::from_str::<serde_json::Value>(input)
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

                    // Extract task_id from metadata
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
        Message::Cancel { req_id, .. } => {
            info!(agent_id, req_id, "Received cancel request");
        }
        _ => {
            // Ignore Connected, Event, Result, Error from agent
        }
    }
    Ok(())
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
        let msg = Message::Submit {
            req_id: "req-1".to_string(),
            sender: "agent-a".to_string(),
            receiver: "manager".to_string(),
            input: "hello".to_string(),
            metadata: None,
        };
        let serialized = serde_json::to_string(&msg).unwrap();
        assert!(serialized.contains("submit"));
        assert!(serialized.contains("agent-a"));
    }
}
