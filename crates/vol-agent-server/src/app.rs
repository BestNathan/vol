use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use futures_util::{SinkExt, StreamExt};
use tokio::time;
use tokio_tungstenite::connect_async;
use tower_http::trace::TraceLayer;
use vol_llm_agent_protocol::agent_server_protocol::{
    AgentServerMessage, ControlOperation, ControlPayload, MessageKind, MessageMeta, NodeHeartbeat,
    NodeLoad, NodeRegistration, Operation, Payload,
};
use vol_llm_agent_protocol::transport::jsonrpc::codec::encode_jsonrpc_message;
use vol_llm_agent_protocol::JsonRpcServer;

use crate::config::ServerConfig;
use crate::control_plane::core::ControlPlaneServerCore;
use crate::control_plane::endpoint::{ControlConnectionRole, ControlPlaneEndpoint};
use crate::control_plane::state::ControlPlaneState;
use crate::data_plane::DataPlaneServerCore;
use crate::routes::{self, WsOwner};

fn spawn_data_plane_connector(
    control_url: String,
    node_id: String,
    name: String,
    version: String,
    heartbeat_secs: u64,
    data_core: Arc<DataPlaneServerCore>,
) {
    tokio::spawn(async move {
        let mut backoff = 1u64;
        let max_backoff = 60u64;
        let mut revision: u64 = 0;

        loop {
            revision += 1;
            tracing::info!(
                dir = "dp > cp",
                control_url = %control_url,
                node_id = %node_id,
                "data-plane connecting to control-plane"
            );

            let ws_stream = match connect_async(&control_url).await {
                Ok((stream, _)) => {
                    tracing::info!(
                        dir = "dp > cp",
                        remote = %control_url,
                        node_id = %node_id,
                        "data-plane connected to control-plane"
                    );
                    stream
                }
                Err(e) => {
                    tracing::warn!(
                        dir = "dp > cp",
                        control_url = %control_url,
                        error = %e,
                        backoff_secs = backoff,
                        "data-plane failed to connect to control-plane, retrying"
                    );
                    time::sleep(Duration::from_secs(backoff)).await;
                    backoff = (backoff * 2).min(max_backoff);
                    continue;
                }
            };
            backoff = 1;

            let (mut write, mut read) = ws_stream.split();

            // ── Send register ─────────────────────────────────────

            let register_msg = match encode_jsonrpc_message(AgentServerMessage {
                protocol: "agent-server-protocol".to_string(),
                message_id: uuid::Uuid::new_v4().to_string(),
                sender: node_id.clone(),
                receiver: "control-plane".to_string(),
                kind: MessageKind::Command,
                operation: Operation::Control(ControlOperation::Register),
                payload: Payload::Control(ControlPayload::Register(NodeRegistration {
                    node_id: node_id.clone(),
                    name: name.clone(),
                    version: version.clone(),
                })),
                meta: MessageMeta::default(),
            }) {
                Ok(s) => s,
                Err(e) => {
                    tracing::error!(error = %e, "failed to encode register message");
                    return;
                }
            };

            if let Err(e) = write
                .send(tokio_tungstenite::tungstenite::Message::Text(register_msg))
                .await
            {
                tracing::warn!(error = %e, "failed to send register message");
                continue;
            }

            // ── Read RegisterAck ────────────────────────────────────

            match time::timeout(Duration::from_secs(10), read.next()).await {
                Ok(Some(Ok(tokio_tungstenite::tungstenite::Message::Text(text)))) => {
                    if let Ok(ack) = serde_json::from_str::<serde_json::Value>(&text) {
                        let accepted = ack
                            .get("result")
                            .and_then(|r| r.get("accepted"))
                            .and_then(serde_json::Value::as_bool)
                            .unwrap_or(false);
                        let generation = ack
                            .get("result")
                            .and_then(|r| r.get("generation"))
                            .and_then(serde_json::Value::as_u64)
                            .unwrap_or(0);
                        if accepted {
                            tracing::info!(node_id = %node_id, generation = generation, "registered with control-plane");
                        } else {
                            tracing::warn!(node_id = %node_id, generation = generation, "control-plane rejected registration");
                            continue;
                        }
                    } else {
                        tracing::warn!(node_id = %node_id, text = %text, "unexpected register response");
                        continue;
                    }
                }
                Ok(Some(Err(e))) => {
                    tracing::warn!(error = %e, "websocket error waiting for register ack");
                    continue;
                }
                _ => {
                    tracing::warn!("no register ack received within timeout");
                    continue;
                }
            }

            // ── Send capability snapshot ───────────────────────────

            let agent_ids = data_core.list_agent_ids().await;
            let agents: Vec<_> = agent_ids
                .into_iter()
                .map(
                    |id| vol_llm_agent_protocol::agent_server_protocol::AgentCapability {
                        agent_id: id.clone(),
                        name: id,
                        description: None,
                        status: Some("idle".to_string()),
                    },
                )
                .collect();

            let snapshot_msg = match encode_jsonrpc_message(AgentServerMessage {
                protocol: "agent-server-protocol".to_string(),
                message_id: uuid::Uuid::new_v4().to_string(),
                sender: node_id.clone(),
                receiver: "control-plane".to_string(),
                kind: MessageKind::Command,
                operation: Operation::Control(ControlOperation::CapabilitySnapshot),
                payload: Payload::Control(ControlPayload::CapabilitySnapshot(
                    vol_llm_agent_protocol::agent_server_protocol::CapabilitySnapshot {
                        node_id: node_id.clone(),
                        revision,
                        generated_at_ms: None,
                        agents,
                        tools: vec![],
                        mcp_servers: vec![],
                        skills: vec![],
                    },
                )),
                meta: MessageMeta::default(),
            }) {
                Ok(s) => s,
                Err(e) => {
                    tracing::error!(error = %e, "failed to encode snapshot message");
                    return;
                }
            };

            if let Err(e) = write
                .send(tokio_tungstenite::tungstenite::Message::Text(snapshot_msg))
                .await
            {
                tracing::warn!(error = %e, "failed to send capability snapshot");
                continue;
            }

            // ── Read SnapshotAck ─────────────────────────────────

            match time::timeout(Duration::from_secs(10), read.next()).await {
                Ok(Some(Ok(tokio_tungstenite::tungstenite::Message::Text(text)))) => {
                    if let Ok(ack) = serde_json::from_str::<serde_json::Value>(&text) {
                        let revision = ack
                            .get("result")
                            .and_then(|r| r.get("revision"))
                            .and_then(serde_json::Value::as_u64)
                            .unwrap_or(0);
                        tracing::info!(node_id = %node_id, revision = revision, "snapshot acknowledged by control-plane");
                    } else {
                        tracing::warn!(node_id = %node_id, text = %text, "unexpected snapshot response");
                    }
                }
                Ok(Some(Err(e))) => {
                    tracing::warn!(error = %e, "websocket error waiting for snapshot ack");
                }
                _ => {
                    tracing::warn!("no snapshot ack received within timeout");
                }
            }

            // ── Heartbeat + read loop ──────────────────────────────

            let heartbeat_interval = Duration::from_secs(heartbeat_secs);
            let mut heartbeat_tick = time::interval(heartbeat_interval);
            heartbeat_tick.tick().await; // skip immediate tick

            let mut connected = true;
            while connected {
                tokio::select! {
                    _ = heartbeat_tick.tick() => {
                        let hb_msg = match encode_jsonrpc_message(AgentServerMessage {
                            protocol: "agent-server-protocol".to_string(),
                            message_id: uuid::Uuid::new_v4().to_string(),
                            sender: node_id.clone(),
                            receiver: "control-plane".to_string(),
                            kind: MessageKind::Command,
                            operation: Operation::Control(ControlOperation::Heartbeat),
                            payload: Payload::Control(ControlPayload::Heartbeat(
                                NodeHeartbeat {
                                    node_id: node_id.clone(),
                                    status: "online".to_string(),
                                    load: NodeLoad { running: 0, queued: 0 },
                                },
                            )),
                            meta: MessageMeta::default(),
                        }) {
                            Ok(s) => s,
                            Err(e) => {
                                tracing::error!(error = %e, "failed to encode heartbeat message");
                                continue;
                            }
                        };

                        if write
                            .send(tokio_tungstenite::tungstenite::Message::Text(hb_msg))
                            .await
                            .is_err()
                        {
                            tracing::warn!("heartbeat send failed, reconnecting");
                            connected = false;
                            continue;
                        }

                        // Read HeartbeatAck (non-blocking with short timeout)
                        match time::timeout(Duration::from_secs(3), read.next()).await {
                            Ok(Some(Ok(tokio_tungstenite::tungstenite::Message::Text(text)))) => {
                                if let Ok(ack) = serde_json::from_str::<serde_json::Value>(&text) {
                                    let ack_node = ack.get("result")
                                        .and_then(|r| r.get("node_id"))
                                        .and_then(|n| n.as_str())
                                        .unwrap_or("");
                                    tracing::debug!(node_id = %node_id, ack_node = %ack_node, "heartbeat acknowledged");
                                } else {
                                    tracing::debug!(node_id = %node_id, text = %text, "heartbeat response");
                                }
                            }
                            Ok(Some(Err(e))) => {
                                tracing::warn!(error = %e, "websocket error reading heartbeat ack, reconnecting");
                                connected = false;
                            }
                            _ => {
                                tracing::warn!("heartbeat ack timeout, reconnecting");
                                connected = false;
                            }
                        }
                    }
                    msg = read.next() => {
                        match msg {
                            Some(Ok(_)) => {}
                            Some(Err(e)) => {
                                tracing::warn!(error = %e, "websocket read error, reconnecting");
                                connected = false;
                            }
                            None => {
                                tracing::warn!("websocket closed by control-plane, reconnecting");
                                connected = false;
                            }
                        }
                    }
                }
            }
            // Send close frame before reconnecting
            let _ = write.close().await;
            tracing::info!(
                dir = "dp > cp",
                node_id = %node_id,
                "data-plane disconnected from control-plane, will reconnect"
            );
        }
    });
}

pub async fn run(mut config: ServerConfig) -> Result<(), String> {
    config.expand_tilde();

    let control_plane_enabled = config.server.roles.control_plane;
    let data_plane_enabled = config.server.roles.data_plane;
    let ws_owner = routes::ws_owner(control_plane_enabled, data_plane_enabled)?;

    let mut app = routes::base_router();

    let control_core = if control_plane_enabled {
        tracing::info!("Building ControlPlaneServerCore");
        Some(Arc::new(
            ControlPlaneServerCore::new(Arc::new(ControlPlaneState::new())).await?,
        ))
    } else {
        None
    };

    let data_core = if data_plane_enabled {
        tracing::info!(
            working_dir = %config.runtime.working_dir,
            store_dir = %config.runtime.store_dir,
            "Building DataPlaneServerCore"
        );
        let core =
            DataPlaneServerCore::builder(&config.runtime.working_dir, &config.runtime.store_dir)
                .with_task_store_config(config.runtime.task_store.clone())
                .with_session_store_config(config.runtime.session_store.clone())
                .build()
                .await?;
        core.discover_agents().await?;
        Some(Arc::new(core))
    } else {
        None
    };

    // In combined mode, register the local data-plane node with the local control plane.
    if control_plane_enabled && data_plane_enabled {
        if let Some(control) = control_core.as_ref() {
            let node_id = config
                .data_plane
                .node_id
                .clone()
                .unwrap_or_else(|| "local-data-plane".to_string());
            let name = config
                .data_plane
                .name
                .clone()
                .unwrap_or_else(|| node_id.clone());
            crate::data_plane::reporter::register_local_data_plane(
                control.state.clone(),
                node_id,
                name,
                env!("CARGO_PKG_VERSION").to_string(),
            )?;
        }
    }

    // ── Remote control-plane registration (standalone data-plane) ──────

    if !control_plane_enabled && data_plane_enabled {
        if let Some(ref control_url) = config.data_plane.control_url {
            let node_id = config
                .data_plane
                .node_id
                .clone()
                .unwrap_or_else(|| "dp-unknown".to_string());
            let name = config
                .data_plane
                .name
                .clone()
                .unwrap_or_else(|| "data-plane".to_string());

            if let Some(ref data) = data_core {
                spawn_data_plane_connector(
                    control_url.clone(),
                    node_id,
                    name,
                    env!("CARGO_PKG_VERSION").to_string(),
                    config.data_plane.heartbeat_secs,
                    data.clone(),
                );
            }
        }
    }

    app = mount_ws_routes(
        app,
        ws_owner,
        control_core,
        data_core,
        &config.control_plane.client_ws_path,
        &config.control_plane.node_ws_path,
    )?;

    app = app.layer(TraceLayer::new_for_http());

    let addr = format!("{}:{}", config.server.host, config.server.port);
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .map_err(|e| format!("Failed to bind {addr}: {e}"))?;

    tracing::info!("agent server started on {}", addr);
    axum::serve(listener, app)
        .await
        .map_err(|e| format!("Server error: {e}"))
}

fn mount_ws_routes(
    mut app: Router,
    ws_owner: WsOwner,
    control_core: Option<Arc<ControlPlaneServerCore>>,
    data_core: Option<Arc<DataPlaneServerCore>>,
    client_ws_path: &str,
    node_ws_path: &str,
) -> Result<Router, String> {
    match ws_owner {
        WsOwner::ControlPlane => {
            let control = control_core.ok_or_else(|| {
                "control plane selected for /ws but core was not built".to_string()
            })?;

            let client_endpoint = Arc::new(ControlPlaneEndpoint::new(
                control.clone(),
                ControlConnectionRole::Client,
            ));
            tracing::info!(client_ws_path, "mounting control-plane client websocket");
            app = app.merge(JsonRpcServer::new(client_endpoint, client_ws_path).into_axum_router());

            if node_ws_path != client_ws_path {
                let node_endpoint = Arc::new(ControlPlaneEndpoint::new(
                    control,
                    ControlConnectionRole::DataPlaneNode,
                ));
                tracing::info!(node_ws_path, "mounting control-plane node websocket");
                app = app.merge(JsonRpcServer::new(node_endpoint, node_ws_path).into_axum_router());
            }
        }
        WsOwner::DataPlane => {
            let data = data_core
                .ok_or_else(|| "data plane selected for /ws but core was not built".to_string())?;
            tracing::info!(client_ws_path, "mounting data-plane websocket");
            app = app.merge(JsonRpcServer::new(data, client_ws_path).into_axum_router());
        }
    }

    Ok(app)
}
