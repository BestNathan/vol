//! WebSocket transport for Agent Server Protocol messages.

use std::sync::Arc;

use async_trait::async_trait;
use axum::extract::ws::{Message as WsMessage, WebSocket, WebSocketUpgrade};
use axum::routing::get;
use axum::Router;
use futures::{SinkExt, StreamExt};

use crate::agent_server_protocol::AgentServerMessage;
use crate::connection::Connection;
use crate::error::ConnectionError;
use crate::service::JsonRpcMessageService;

fn serialize_message(msg: &AgentServerMessage) -> Result<String, ConnectionError> {
    serde_json::to_string(msg).map_err(|e| ConnectionError::WsSendError(e.to_string()))
}

pub struct WsConnection {
    tx: Arc<tokio::sync::Mutex<futures::stream::SplitSink<WebSocket, WsMessage>>>,
    rx: tokio::sync::Mutex<futures::stream::SplitStream<WebSocket>>,
}

impl WsConnection {
    pub fn new(ws: WebSocket) -> Self {
        let (tx, rx) = ws.split();
        Self {
            tx: Arc::new(tokio::sync::Mutex::new(tx)),
            rx: tokio::sync::Mutex::new(rx),
        }
    }
}

#[async_trait]
impl Connection for WsConnection {
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
                Ok(WsMessage::Binary(_)) => {
                    return Some(Err(ConnectionError::ParseError(
                        "binary messages not supported".to_string(),
                    )));
                }
                Ok(WsMessage::Ping(_)) | Ok(WsMessage::Pong(_)) => continue,
                Err(e) => return Some(Err(ConnectionError::WsReceiveError(e.to_string()))),
            }
        }
    }

    async fn send(&self, msg: AgentServerMessage) -> Result<(), ConnectionError> {
        let text = serialize_message(&msg)?;
        let mut tx = self.tx.lock().await;
        tx.send(WsMessage::Text(text))
            .await
            .map_err(|e| ConnectionError::WsSendError(e.to_string()))
    }
}

pub struct WsServer<S> {
    service: Arc<S>,
}

impl<S> WsServer<S>
where
    S: JsonRpcMessageService,
{
    pub fn new(service: Arc<S>) -> Self {
        Self { service }
    }

    pub fn into_axum_router(self) -> Router {
        let server = Arc::new(self);

        Router::new().route(
            "/ws",
            get(move |ws: WebSocketUpgrade| {
                let server = server.clone();
                async move { ws.on_upgrade(move |socket| handle_ws(socket, server)) }
            }),
        )
    }
}

async fn handle_ws<S>(socket: WebSocket, server: Arc<WsServer<S>>)
where
    S: JsonRpcMessageService,
{
    let conn = WsConnection::new(socket);
    server.service.serve_connection(Arc::new(conn)).await;
}
