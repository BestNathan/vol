//! WebSocket transport for agent channel communication.

use std::sync::Arc;

use async_trait::async_trait;
use axum::extract::ws::{Message as WsMessage, WebSocket, WebSocketUpgrade};
use axum::routing::get;
use axum::Router;
use futures::{SinkExt, StreamExt};

use crate::connection::{Connection, ConnectionHolder};
use crate::dispatcher::AgentDispatcher;
use crate::error::ConnectionError;
use crate::protocol::Message;

/// Serialize a `Message` to a JSON text string.
fn serialize_message(msg: &Message) -> Result<String, ConnectionError> {
    serde_json::to_string(msg).map_err(|e| ConnectionError::WsSendError(e.to_string()))
}

/// Active WebSocket connection implementing the `Connection` trait.
pub struct WsConnection {
    /// WebSocket text sender (mutex-wrapped for concurrent sends).
    tx: Arc<tokio::sync::Mutex<futures::stream::SplitSink<WebSocket, WsMessage>>>,
    /// WebSocket text receiver.
    rx: futures::stream::SplitStream<WebSocket>,
    /// Dispatcher for executing agent requests.
    dispatcher: Arc<AgentDispatcher>,
    /// Holder this connection is attached to (for detach on close).
    holder: Arc<ConnectionHolder>,
    /// Agent ID to advertise in the handshake.
    agent_id: String,
}

impl WsConnection {
    /// Create a new `WsConnection` from a split WebSocket.
    pub fn new(
        ws: WebSocket,
        dispatcher: Arc<AgentDispatcher>,
        holder: Arc<ConnectionHolder>,
        agent_id: String,
    ) -> Self {
        let (tx, rx) = ws.split();
        Self {
            tx: Arc::new(tokio::sync::Mutex::new(tx)),
            rx,
            dispatcher,
            holder,
            agent_id,
        }
    }

    /// Main connection loop: receive messages, dispatch to agents, send results.
    ///
    /// This method owns the connection and runs until the client disconnects
    /// or a fatal error occurs. It detaches from the holder before returning.
    pub async fn run(mut self) {
        let connected = Message::Connected {
            sender: self.agent_id.clone(),
            receiver: "client".to_string(),
        };
        let _ = self.send(connected).await;

        loop {
            match self.recv().await {
                Some(Ok(msg)) => match self.handle_message(msg).await {
                    Ok(()) => {}
                    Err(e) => {
                        tracing::warn!(%e, "error handling inbound message");
                        let _ = self
                            .send(Message::Error {
                                req_id: None,
                                sender: self.agent_id.clone(),
                                receiver: "client".to_string(),
                                message: format!("handler error: {e}"),
                            })
                            .await;
                    }
                },
                Some(Err(e)) => {
                    tracing::warn!(%e, "receive error");
                    break;
                }
                None => {
                    tracing::info!("WebSocket connection closed");
                    break;
                }
            }
        }

        self.holder.detach().await;
    }

    async fn handle_message(&self, msg: Message) -> Result<(), ConnectionError> {
        match msg {
            Message::Submit {
                req_id,
                input,
                metadata,
                ..
            } => {
                let mut request =
                    crate::request::AgentRequest::with_id_and_input(&req_id, &self.agent_id, input);
                if let Some(meta) = metadata {
                    request.metadata = meta;
                }

                let rx = match self.dispatcher.submit(request) {
                    Ok(rx) => rx,
                    Err(e) => {
                        return self
                            .send(Message::Error {
                                req_id: Some(req_id),
                                sender: self.agent_id.clone(),
                                receiver: "client".to_string(),
                                message: e.to_string(),
                            })
                            .await;
                    }
                };

                match rx.await {
                    Ok(run_result) => {
                        let response_value = match &run_result.response {
                            Ok(resp) => serde_json::to_value(resp)
                                .map_err(|e| ConnectionError::WsSendError(e.to_string()))?,
                            Err(err) => serde_json::json!({ "error": err.to_string() }),
                        };

                        let result = Message::Result {
                            req_id: run_result.req_id,
                            sender: self.agent_id.clone(),
                            receiver: "client".to_string(),
                            result: response_value,
                        };
                        self.send(result).await
                    }
                    Err(_) => {
                        self.send(Message::Error {
                            req_id: Some(req_id),
                            sender: self.agent_id.clone(),
                            receiver: "client".to_string(),
                            message: "dispatcher dropped while processing request".to_string(),
                        })
                        .await
                    }
                }
            }
            Message::Cancel { req_id, .. } => {
                let cancelled = self.dispatcher.cancel(&req_id).await;
                if cancelled {
                    self.send(Message::Error {
                        req_id: Some(req_id),
                        sender: self.agent_id.clone(),
                        receiver: "client".to_string(),
                        message: "request cancelled".to_string(),
                    })
                    .await
                } else {
                    self.send(Message::Error {
                        req_id: Some(req_id),
                        sender: self.agent_id.clone(),
                        receiver: "client".to_string(),
                        message: "request not found in queue (already executing or completed)"
                            .to_string(),
                    })
                    .await
                }
            }
            _ => Ok(()),
        }
    }
}

#[async_trait]
impl Connection for WsConnection {
    fn protocol(&self) -> &str {
        "ws"
    }

    async fn recv(&mut self) -> Option<Result<Message, ConnectionError>> {
        let msg = self.rx.next().await?;
        match msg {
            Ok(WsMessage::Text(text)) => match serde_json::from_str::<Message>(&text) {
                Ok(msg) => Some(Ok(msg)),
                Err(e) => Some(Err(ConnectionError::ParseError(e.to_string()))),
            },
            Ok(WsMessage::Close(_)) => None,
            Ok(WsMessage::Binary(_)) => Some(Err(ConnectionError::ParseError(
                "binary messages not supported".to_string(),
            ))),
            Ok(WsMessage::Ping(data)) => {
                tracing::debug!("WebSocket ping: {} bytes", data.len());
                self.recv().await
            }
            Ok(WsMessage::Pong(_)) => self.recv().await,
            Err(e) => Some(Err(ConnectionError::WsReceiveError(e.to_string()))),
        }
    }

    async fn send(&self, msg: Message) -> Result<(), ConnectionError> {
        let text = serialize_message(&msg)?;
        let mut tx = self.tx.lock().await;
        tx.send(WsMessage::Text(text))
            .await
            .map_err(|e| ConnectionError::WsSendError(e.to_string()))
    }
}

/// WebSocket server that manages agent connections.
///
/// Holds a dispatcher and a connection holder, and provides an axum router
/// with a `/ws` endpoint for clients to connect.
pub struct WsServer {
    dispatcher: Arc<AgentDispatcher>,
    holder: Arc<ConnectionHolder>,
    /// Human-readable agent identifier for the handshake message.
    agent_id: String,
}

impl WsServer {
    /// Create a new WebSocket server.
    pub fn new(
        dispatcher: Arc<AgentDispatcher>,
        holder: Arc<ConnectionHolder>,
        agent_id: impl Into<String>,
    ) -> Self {
        Self {
            dispatcher,
            holder,
            agent_id: agent_id.into(),
        }
    }

    /// Build an axum `Router` with a `/ws` WebSocket endpoint.
    pub fn into_axum_router(self) -> Router {
        let agent_id = self.agent_id.clone();
        let server = Arc::new(self);

        Router::new().route(
            "/ws",
            get({
                let server = server.clone();
                let agent_id = agent_id.clone();
                move |ws: WebSocketUpgrade| {
                    let server = server.clone();
                    let agent_id = agent_id.clone();
                    async move { ws.on_upgrade(move |socket| handle_ws(socket, server, agent_id)) }
                }
            }),
        )
    }
}

/// Handler for an upgraded WebSocket connection.
async fn handle_ws(socket: WebSocket, server: Arc<WsServer>, agent_id: String) {
    let conn = WsConnection::new(
        socket,
        server.dispatcher.clone(),
        server.holder.clone(),
        agent_id,
    );

    // Attach the connection to the holder so the plugin can forward events.
    let conn_arc = Arc::new(conn);
    server.holder.attach(conn_arc.clone()).await;

    // Run the connection loop. This consumes the Arc-wrapped connection,
    // but since WsConnection::run takes `mut self`, we need to unwrap.
    // We use Arc::try_unwrap, and if it fails (someone else holds a ref),
    // we just log and return.
    match Arc::try_unwrap(conn_arc) {
        Ok(conn) => conn.run().await,
        Err(_) => {
            tracing::warn!(
                "could not take ownership of WsConnection (refcount > 1), skipping run loop"
            );
            // Still detach to clean up.
            server.holder.detach().await;
        }
    }
}
