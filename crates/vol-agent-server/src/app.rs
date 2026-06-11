use std::sync::Arc;

use axum::Router;
use vol_llm_agent_protocol::JsonRpcServer;

use crate::config::ServerConfig;
use crate::control_plane::core::ControlPlaneServerCore;
use crate::control_plane::endpoint::{ControlConnectionRole, ControlPlaneEndpoint};
use crate::control_plane::state::ControlPlaneState;
use crate::data_plane::DataPlaneServerCore;
use crate::routes::{self, WsOwner};

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

    app = mount_ws_routes(
        app,
        ws_owner,
        control_core,
        data_core,
        &config.control_plane.client_ws_path,
        &config.control_plane.node_ws_path,
    )?;

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
