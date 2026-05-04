use axum::{
    extract::{Query, State, WebSocketUpgrade},
    response::IntoResponse,
    routing::get,
    Router,
};

use crate::AppRouterState;

#[derive(Debug, serde::Deserialize)]
pub struct WsQuery {
    pub token: Option<String>,
}

/// Create the WebSocket router.
pub fn create_ws_router() -> Router<AppRouterState> {
    Router::new()
        .route("/ws", get(upgrade_ws))
        .merge(crate::ws::router::create_agent_ws_router())
}

async fn upgrade_ws(
    Query(query): Query<WsQuery>,
    ws: WebSocketUpgrade,
    State(state): State<AppRouterState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| {
        crate::ws::handler::handle_agent_connection(
            socket,
            query.token,
            state.state_manager.clone(),
            state.metrics.clone(),
            state.event_bus.clone(),
            state.task_dispatcher.clone(),
            state.config.security.token.clone(),
        )
    })
}
