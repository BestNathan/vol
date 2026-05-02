use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket};
use chrono::Utc;
use tracing::{error, info, warn};

use super::protocol::*;
use crate::events::{EventBus, ManagerEvent};
use crate::metrics::collector::MetricsCollector;
use crate::state::manager::AgentStateManager;
use crate::state::models::{AgentState, AgentStatus};
use crate::task::dispatcher::TaskDispatcher;

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
            let _ = ws
                .send(Message::Text(
                    serde_json::json!({"error": "invalid token"}).to_string(),
                ))
                .await;
            return;
        }
    }

    let agent_id;

    // Wait for register message
    match ws.recv().await {
        Some(Ok(Message::Text(text))) => {
            match serde_json::from_str::<WsMessage>(&text) {
                Ok(msg) if msg.message_type == "register" => {
                    match serde_json::from_value::<RegisterPayload>(msg.payload) {
                        Ok(payload) => {
                            let id = msg.agent_id.clone().unwrap_or_else(|| {
                                format!("{}:{}", payload.r#type, payload.name)
                            });
                            agent_id = Some(id.clone());

                            let state = AgentState {
                                agent_id: id.clone(),
                                name: payload.name,
                                r#type: payload.r#type,
                                version: payload.version,
                                capabilities: payload.capabilities,
                                host_info: crate::state::models::HostInfo {
                                    hostname: payload.host_info.hostname,
                                    os: payload.host_info.os,
                                    arch: payload.host_info.arch,
                                    ip: payload.host_info.ip,
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
                            let agent_type = state_manager
                                .get(&id).await.map(|s| s.r#type.clone()).unwrap_or_default();
                            metrics.increment_messages("register", &id, &agent_type);

                            // Send ack
                            let ack = WsMessage {
                                message_type: "register_ack".to_string(),
                                agent_id: Some(id.clone()),
                                task_id: None,
                                target_agent_id: None,
                                timestamp: Some(Utc::now().to_rfc3339()),
                                payload: serde_json::json!({
                                    "agent_id": id,
                                    "status": "ok"
                                }),
                            };
                            let _ = ws
                                .send(Message::Text(serde_json::to_string(&ack).unwrap()))
                                .await;
                        }
                        Err(e) => {
                            warn!("Invalid register payload: {}", e);
                            return;
                        }
                    }
                }
                _ => {
                    warn!("First message was not register, closing connection");
                    return;
                }
            }
        }
        Some(Ok(Message::Binary(_) | Message::Close(_) | Message::Ping(_) | Message::Pong(_))) => {
            warn!("Connection closed before register");
            return;
        }
        Some(Err(e)) => {
            warn!("WebSocket error before register: {}", e);
            return;
        }
        None => {
            warn!("Connection closed before register");
            return;
        }
    }

    let id = agent_id.clone().unwrap();
    metrics.agent_connections_current.inc();

    // Message loop
    loop {
        match ws.recv().await {
            Some(Ok(Message::Text(text))) => {
                if let Err(e) = handle_agent_message(
                    &text, &id, &state_manager, &metrics, &event_bus, &task_dispatcher,
                ).await {
                    let err_msg = WsMessage::error(&id, &e.to_string());
                    let _ = ws
                        .send(Message::Text(serde_json::to_string(&err_msg).unwrap()))
                        .await;
                }
            }
            Some(Ok(Message::Close(_))) => {
                info!(agent_id = %id, "Agent disconnected");
                state_manager
                    .update_status(&id, AgentStatus::Disconnected)
                    .await;
                metrics.agent_connections_current.dec();
                event_bus.emit(ManagerEvent::agent_disconnected(&id));
                break;
            }
            Some(Ok(Message::Binary(_) | Message::Ping(_) | Message::Pong(_))) => {
                // Ignore binary and ping/pong frames
            }
            Some(Err(e)) => {
                error!(agent_id = %id, "WebSocket error: {}", e);
                break;
            }
            None => {
                info!(agent_id = %id, "Agent connection closed");
                state_manager
                    .update_status(&id, AgentStatus::Disconnected)
                    .await;
                metrics.agent_connections_current.dec();
                event_bus.emit(ManagerEvent::agent_disconnected(&id));
                break;
            }
        }
    }
}

async fn handle_agent_message(
    text: &str,
    agent_id: &str,
    state_manager: &AgentStateManager,
    metrics: &MetricsCollector,
    event_bus: &EventBus,
    task_dispatcher: &TaskDispatcher,
) -> Result<(), anyhow::Error> {
    let msg: WsMessage = serde_json::from_str(text)?;
    let agent_type = state_manager
        .get(agent_id)
        .await
        .map(|s| s.r#type.clone())
        .unwrap_or_else(|| "unknown".to_string());

    match msg.message_type.as_str() {
        "heartbeat" => {
            let payload: HeartbeatPayload = serde_json::from_value(msg.payload)?;
            state_manager.update_heartbeat(agent_id).await;
            if payload.status == "Busy" {
                state_manager
                    .update_status(agent_id, AgentStatus::Busy)
                    .await;
            } else {
                state_manager
                    .update_status(agent_id, AgentStatus::Idle)
                    .await;
            }
            metrics.increment_messages("heartbeat", agent_id, &agent_type);
        }
        "metric" => {
            let payload: MetricPayload = serde_json::from_value(msg.payload)?;
            metrics.increment_metric_samples(agent_id);
            let sample_count = payload.samples.len();
            tracing::debug!(agent_id, sample_count, "Received metric samples");
            metrics.increment_messages("metric", agent_id, &agent_type);
        }
        "event" => {
            let payload: EventPayload = serde_json::from_value(msg.payload)?;
            event_bus.emit(ManagerEvent::agent_event(
                agent_id,
                &payload.event_name,
                payload.data,
            ));
            metrics.increment_messages("event", agent_id, &agent_type);
        }
        "task_result" => {
            let payload: TaskResultPayload = serde_json::from_value(msg.payload)?;
            if let Some(ref task_id) = msg.task_id {
                match payload.status.as_str() {
                    "Completed" => {
                        task_dispatcher.complete_task(task_id, payload.result, payload.duration_ms).await;
                        event_bus.emit(ManagerEvent::task_completed(task_id, agent_id));
                    }
                    "Failed" => {
                        let error = payload.error.as_deref().unwrap_or("unknown error");
                        task_dispatcher.fail_task(task_id, error).await;
                        event_bus.emit(ManagerEvent::task_failed(task_id, agent_id, error));
                    }
                    _ => {
                        warn!(task_id, agent_id, status = %payload.status, "Unknown task result status");
                    }
                }
            }
            metrics.increment_messages("task_result", agent_id, &agent_type);
        }
        unknown => {
            return Err(anyhow::anyhow!("Unknown message type: {}", unknown));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_register_message() {
        let json = serde_json::json!({
            "message_type": "register",
            "agent_id": "agent-1",
            "payload": {
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
            }
        });
        let msg: WsMessage = serde_json::from_value(json).unwrap();
        assert_eq!(msg.message_type, "register");
        assert_eq!(msg.agent_id.as_deref(), Some("agent-1"));

        let payload: RegisterPayload = serde_json::from_value(msg.payload).unwrap();
        assert_eq!(payload.name, "test-agent");
        assert_eq!(payload.capabilities.len(), 2);
    }

    #[test]
    fn test_parse_heartbeat_message() {
        let json = serde_json::json!({
            "message_type": "heartbeat",
            "agent_id": "agent-1",
            "payload": {
                "status": "Idle"
            }
        });
        let msg: WsMessage = serde_json::from_value(json).unwrap();
        let payload: HeartbeatPayload = serde_json::from_value(msg.payload).unwrap();
        assert_eq!(payload.status, "Idle");
    }

    #[test]
    fn test_parse_invalid_message() {
        let result = serde_json::from_str::<WsMessage>("not json");
        assert!(result.is_err());
    }

    #[test]
    fn test_ws_message_helpers() {
        let err = WsMessage::error("agent-1", "something broke");
        assert_eq!(err.message_type, "error");
        assert_eq!(err.agent_id.as_deref(), Some("agent-1"));
    }
}
