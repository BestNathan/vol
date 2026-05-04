use axum::{
    extract::{Path, Query, State, WebSocketUpgrade},
    response::IntoResponse,
    routing::get,
    Router,
};
use axum::extract::ws::{Message, WebSocket};
use futures::{SinkExt, StreamExt};
use std::sync::Arc;
use tracing::{info, warn};

use vol_llm_agent::AgentBuilder;
use vol_llm_provider::create_provider;

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

/// Run an agent instance from a definition, broadcasting events to connected WS clients.
async fn run_agent_instance(
    agent_def: vol_llm_agent::AgentDef,
    session: Arc<vol_session::Session>,
    llm_config: vol_llm_provider::LLMConfig,
    broadcast_tx: tokio::sync::broadcast::Sender<serde_json::Value>,
    agent_type: String,
    session_id: String,
    user_input: String,
) {
    // Create LLM client
    let llm = match create_provider(&llm_config) {
        Ok(client) => client,
        Err(e) => {
            let err = serde_json::json!({
                "message_type": "agent_error",
                "error": format!("Failed to create LLM client: {}", e),
            });
            let _ = broadcast_tx.send(err);
            return;
        }
    };

    // Build agent from definition
    let agent = AgentBuilder::new()
        .with_llm(Arc::from(llm))
        .with_system_prompt(agent_def.content)
        .with_session(session)
        .with_max_iterations(agent_def.max_iterations.unwrap_or(10))
        .build();

    let agent = match agent {
        Ok(a) => a,
        Err(e) => {
            let err = serde_json::json!({
                "message_type": "agent_error",
                "error": format!("Failed to build agent: {}", e),
            });
            let _ = broadcast_tx.send(err);
            return;
        }
    };

    info!(
        agent_type = %agent_type,
        session_id = %session_id,
        "Running agent with input: {}",
        user_input
    );

    // Run agent and broadcast result
    match agent.run(&user_input).await {
        Ok(response) => {
            let data = serde_json::json!({
                "message_type": "agent_complete",
                "content": response.content,
                "iterations": response.iterations,
            });
            let _ = broadcast_tx.send(data);
        }
        Err(e) => {
            let data = serde_json::json!({
                "message_type": "agent_error",
                "error": e.to_string(),
            });
            let _ = broadcast_tx.send(data);
        }
    }

    info!(agent_type = %agent_type, session_id = %session_id, "Agent run finished");
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
    let entry_store = Arc::new(vol_session::InMemoryEntryStore::new());
    let session = Arc::new(vol_session::Session::new(entry_store));

    let instance = state
        .instance_registry
        .get_or_create(&agent_type, &session_id, parent_session_id, session)
        .await;

    let broadcast_tx = instance.broadcast_tx.clone();
    let mut broadcast_rx = broadcast_tx.subscribe();

    // Add connection
    state.instance_registry.add_connection(&agent_type, &session_id, conn_id.clone()).await;

    // Get agent definition and LLM config for spawning
    let agent_def = (*state.agent_loader.get(&agent_type).await.unwrap()).clone();
    let llm_config = state.llm_config.clone();
    let session = instance.session.clone();
    let agent_spawned = Arc::new(tokio::sync::Mutex::new(false));

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

                        if !*agent_spawned.lock().await {
                            *agent_spawned.lock().await = true;
                            let content = content.to_string();
                            tokio::spawn({
                                let agent_def = agent_def.clone();
                                let llm_config = llm_config.clone();
                                let broadcast_tx = broadcast_tx.clone();
                                let session = session.clone();
                                let agent_type = agent_type.clone();
                                let session_id = session_id.clone();
                                async move {
                                    run_agent_instance(
                                        agent_def, session, llm_config, broadcast_tx,
                                        agent_type, session_id, content,
                                    ).await;
                                }
                            });
                        }
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
