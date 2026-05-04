use axum::{
    extract::{Path, Query, State, WebSocketUpgrade},
    response::IntoResponse,
    routing::get,
    Router,
};
use axum::extract::ws::{Message, WebSocket};
use futures::{SinkExt, StreamExt};
use tracing::{info, warn};

use crate::AppRouterState;

/// Create routes for agent-specific WebSocket connections.
pub fn create_agent_ws_router() -> Router<AppRouterState> {
    Router::new()
        .route("/ws/agents/:agent_type/session/:session_id", get(upgrade_agent_ws))
}

#[derive(Debug, serde::Deserialize)]
pub struct AgentWsQuery {
    pub parent_session_id: Option<String>,
}

async fn upgrade_agent_ws(
    Path((agent_type, session_id)): Path<(String, String)>,
    query: Option<Query<AgentWsQuery>>,
    ws: WebSocketUpgrade,
    State(state): State<AppRouterState>,
) -> impl IntoResponse {
    // Validate agent type exists in AgentLoader
    let agent_def = state.agent_loader.get(&agent_type).await;
    if agent_def.is_none() {
        warn!(agent_type = %agent_type, "Agent type not found in definitions");
        return axum::http::StatusCode::NOT_FOUND.into_response();
    }

    let parent_session_id = query.map(|q| q.0.parent_session_id).flatten();

    ws.on_upgrade(move |socket| {
        handle_agent_ws(socket, agent_type, session_id, parent_session_id, state)
    })
}

async fn handle_agent_ws(
    ws: WebSocket,
    agent_type: String,
    session_id: String,
    parent_session_id: Option<String>,
    state: AppRouterState,
) {
    let conn_id = uuid::Uuid::new_v4().to_string();

    // Get or create instance with in-memory session for now
    let entry_store = std::sync::Arc::new(vol_session::InMemoryEntryStore::new());
    let session = std::sync::Arc::new(vol_session::Session::new(entry_store));

    let instance = state
        .instance_registry
        .get_or_create(&agent_type, &session_id, parent_session_id, session)
        .await;

    let broadcast_tx = instance.broadcast_tx.clone();
    let mut broadcast_rx = broadcast_tx.subscribe();

    // Add connection
    state.instance_registry.add_connection(&agent_type, &session_id, conn_id.clone()).await;

    // Split WebSocket into send/receive halves
    let (mut ws_tx, mut ws_rx) = ws.split();

    // Send welcome message
    let welcome = serde_json::json!({
        "message_type": "connected",
        "agent_type": agent_type,
        "session_id": session_id,
    });
    let _ = ws_tx.send(Message::Text(welcome.to_string())).await;

    // Spawn broadcast receiver task
    let ws_sender = tokio::spawn(async move {
        loop {
            match broadcast_rx.recv().await {
                Ok(data) => {
                    let msg = Message::Text(data.to_string());
                    if ws_tx.send(msg).await.is_err() {
                        break;
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
            }
        }
    });

    // Main receive loop
    loop {
        match ws_rx.next().await {
            Some(Ok(Message::Text(text))) => {
                if let Ok(input) = serde_json::from_str::<serde_json::Value>(&text) {
                    if let Some(content) = input.get("content").and_then(|v| v.as_str()) {
                        info!(agent_type = %agent_type, session_id = %session_id, "Received user input: {}", content);
                    }
                }
            }
            Some(Ok(Message::Close(_))) | None => {
                break;
            }
            Some(Ok(Message::Binary(_) | Message::Ping(_) | Message::Pong(_))) => {}
            Some(Err(e)) => {
                warn!(error = %e, "WebSocket error");
                break;
            }
        }
    }

    // Cleanup
    state.instance_registry.remove_connection(&agent_type, &session_id, &conn_id).await;
    let _ = ws_sender.abort();
}
