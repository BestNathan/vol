//! WebSocket transport for agent channel communication.

use std::sync::Arc;

use async_trait::async_trait;
use axum::extract::ws::{Message as WsMessage, WebSocket, WebSocketUpgrade};
use axum::routing::get;
use axum::Router;
use futures::{SinkExt, StreamExt};

use crate::agent_server_protocol::{
    AgentOperation, AgentPayload, AgentServerMessage, ErrorPayload, MessageKind, Operation, Payload,
};
use crate::connection::{Connection, ConnectionHolder};
use crate::dispatcher::AgentDispatcher;
use crate::error::ConnectionError;
use crate::protocol::Message;

/// Serialize an `AgentServerMessage` to a JSON text string.
fn serialize_message(msg: &AgentServerMessage) -> Result<String, ConnectionError> {
    serde_json::to_string(msg).map_err(|e| ConnectionError::WsSendError(e.to_string()))
}

fn protocol_to_old_message(msg: AgentServerMessage) -> Result<Message, ConnectionError> {
    match (msg.kind, msg.operation, msg.payload) {
        (
            MessageKind::Command,
            Operation::Agent(AgentOperation::Submit),
            Payload::Agent(AgentPayload::Submit {
                input, metadata, ..
            }),
        ) => Ok(Message::Submit {
            req_id: msg.message_id,
            sender: msg.sender,
            receiver: msg.receiver,
            input,
            metadata: metadata.map(|m| m.into_iter().collect()),
        }),
        (MessageKind::Command, Operation::Agent(AgentOperation::Cancel), _) => {
            Ok(Message::Cancel {
                req_id: msg.message_id,
                sender: msg.sender,
                receiver: msg.receiver,
            })
        }
        (
            MessageKind::Event,
            Operation::Agent(AgentOperation::Event),
            Payload::Agent(AgentPayload::Event { event, .. }),
        ) => Ok(Message::Event {
            sender: msg.sender,
            receiver: msg.receiver,
            event,
        }),
        (MessageKind::Result, _, payload) => Ok(Message::Result {
            req_id: msg.message_id,
            sender: msg.sender,
            receiver: msg.receiver,
            result: serde_json::to_value(payload)
                .map_err(|e| ConnectionError::ParseError(e.to_string()))?,
        }),
        (MessageKind::Error, _, Payload::Error(ErrorPayload { message, .. })) => {
            Ok(Message::Error {
                req_id: Some(msg.message_id),
                sender: msg.sender,
                receiver: msg.receiver,
                message,
            })
        }
        (MessageKind::Ack, _, _) => Ok(Message::Connected {
            sender: msg.sender,
            receiver: msg.receiver,
        }),
        _ => Err(ConnectionError::ParseError(
            "unsupported protocol message for ws shim".to_string(),
        )),
    }
}

fn old_message_to_protocol(msg: Message) -> Result<AgentServerMessage, ConnectionError> {
    match msg {
        Message::Submit {
            req_id,
            sender,
            receiver,
            input,
            metadata,
        } => Ok(AgentServerMessage {
            protocol: "agent-server/1".to_string(),
            message_id: req_id,
            sender,
            receiver,
            kind: MessageKind::Command,
            operation: Operation::Agent(AgentOperation::Submit),
            payload: Payload::Agent(AgentPayload::Submit {
                input,
                target: None,
                metadata: metadata.map(|m| m.into_iter().collect()),
                run_id: None,
            }),
            meta: Default::default(),
        }),
        Message::Cancel {
            req_id,
            sender,
            receiver,
        } => Ok(AgentServerMessage {
            protocol: "agent-server/1".to_string(),
            message_id: req_id,
            sender,
            receiver,
            kind: MessageKind::Command,
            operation: Operation::Agent(AgentOperation::Cancel),
            payload: Payload::Agent(AgentPayload::SubmitAck {
                run_id: String::new(),
                accepted: false,
            }),
            meta: Default::default(),
        }),
        Message::Connected { sender, receiver } => Ok(AgentServerMessage {
            protocol: "agent-server/1".to_string(),
            message_id: uuid::Uuid::new_v4().to_string(),
            sender,
            receiver,
            kind: MessageKind::Ack,
            operation: Operation::Agent(AgentOperation::Submit),
            payload: Payload::Agent(AgentPayload::SubmitAck {
                run_id: String::new(),
                accepted: true,
            }),
            meta: Default::default(),
        }),
        Message::Event {
            sender,
            receiver,
            event,
        } => Ok(AgentServerMessage {
            protocol: "agent-server/1".to_string(),
            message_id: uuid::Uuid::new_v4().to_string(),
            sender,
            receiver,
            kind: MessageKind::Event,
            operation: Operation::Agent(AgentOperation::Event),
            payload: Payload::Agent(AgentPayload::Event {
                run_id: String::new(),
                event,
            }),
            meta: Default::default(),
        }),
        Message::Result {
            req_id,
            sender,
            receiver,
            result,
        } => Ok(AgentServerMessage {
            protocol: "agent-server/1".to_string(),
            message_id: req_id,
            sender,
            receiver,
            kind: MessageKind::Result,
            operation: Operation::Agent(AgentOperation::Submit),
            payload: Payload::Agent(AgentPayload::SubmitResult {
                run_id: String::new(),
                response: result,
            }),
            meta: Default::default(),
        }),
        Message::Error {
            req_id,
            sender,
            receiver,
            message,
        } => Ok(AgentServerMessage {
            protocol: "agent-server/1".to_string(),
            message_id: req_id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
            sender,
            receiver,
            kind: MessageKind::Error,
            operation: Operation::Agent(AgentOperation::Submit),
            payload: Payload::Error(ErrorPayload {
                code: "legacy_error".to_string(),
                message,
                detail: None,
                terminal: false,
            }),
            meta: Default::default(),
        }),
    }
}

/// Active WebSocket connection implementing the `Connection` trait.
pub struct WsConnection {
    /// WebSocket text sender (mutex-wrapped for concurrent sends).
    tx: Arc<tokio::sync::Mutex<futures::stream::SplitSink<WebSocket, WsMessage>>>,
    /// WebSocket text receiver (mutex-wrapped for &self recv).
    rx: tokio::sync::Mutex<futures::stream::SplitStream<WebSocket>>,
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
            rx: tokio::sync::Mutex::new(rx),
            dispatcher,
            holder,
            agent_id,
        }
    }

    /// Main connection loop: receive messages, dispatch to agents, send results.
    ///
    /// This method owns the connection and runs until the client disconnects
    /// or a fatal error occurs. It detaches from the holder before returning.
    pub async fn run(self) {
        let connected = AgentServerMessage {
            protocol: "agent-server/1".to_string(),
            message_id: uuid::Uuid::new_v4().to_string(),
            sender: self.agent_id.clone(),
            receiver: "client".to_string(),
            kind: MessageKind::Ack,
            operation: Operation::Agent(AgentOperation::Submit),
            payload: Payload::Agent(AgentPayload::SubmitAck {
                run_id: String::new(),
                accepted: true,
            }),
            meta: Default::default(),
        };
        let _ = self.send(connected).await;

        loop {
            match self.recv().await {
                Some(Ok(msg)) => {
                    let msg = match protocol_to_old_message(msg) {
                        Ok(msg) => msg,
                        Err(e) => {
                            tracing::warn!(%e, "error decoding protocol message for ws transport");
                            continue;
                        }
                    };
                    match self.handle_message(msg).await {
                        Ok(()) => {}
                        Err(e) => {
                            tracing::warn!(%e, "error handling inbound message");
                            let _ = self
                                .send(AgentServerMessage {
                                    protocol: "agent-server/1".to_string(),
                                    message_id: uuid::Uuid::new_v4().to_string(),
                                    sender: self.agent_id.clone(),
                                    receiver: "client".to_string(),
                                    kind: MessageKind::Error,
                                    operation: Operation::Agent(AgentOperation::Submit),
                                    payload: Payload::Error(ErrorPayload {
                                        code: "handler_error".to_string(),
                                        message: format!("handler error: {e}"),
                                        detail: None,
                                        terminal: false,
                                    }),
                                    meta: Default::default(),
                                })
                                .await;
                        }
                    }
                }
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
                    crate::request::AgentRequest::with_run_id(&req_id, &self.agent_id, &input);
                if let Some(meta) = metadata {
                    request.metadata = meta;
                }

                let rx = match self.dispatcher.submit(request) {
                    Ok(rx) => rx,
                    Err(e) => {
                        let err = protocol_to_old_message(AgentServerMessage {
                            protocol: "agent-server/1".to_string(),
                            message_id: req_id.clone(),
                            sender: self.agent_id.clone(),
                            receiver: "client".to_string(),
                            kind: MessageKind::Error,
                            operation: Operation::Agent(AgentOperation::Submit),
                            payload: Payload::Error(ErrorPayload {
                                code: "submit_failed".to_string(),
                                message: e.to_string(),
                                detail: None,
                                terminal: false,
                            }),
                            meta: Default::default(),
                        })?;
                        return self.send(old_message_to_protocol(err)?).await;
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
                            req_id: run_result.run_id,
                            sender: self.agent_id.clone(),
                            receiver: "client".to_string(),
                            result: response_value,
                        };
                        self.send(old_message_to_protocol(result)?).await
                    }
                    Err(_) => {
                        let err = Message::Error {
                            req_id: Some(req_id),
                            sender: self.agent_id.clone(),
                            receiver: "client".to_string(),
                            message: "dispatcher dropped while processing request".to_string(),
                        };
                        self.send(old_message_to_protocol(err)?).await
                    }
                }
            }
            Message::Cancel { req_id, .. } => {
                let cancelled = self.dispatcher.cancel(&req_id).await;
                if cancelled {
                    let err = Message::Error {
                        req_id: Some(req_id),
                        sender: self.agent_id.clone(),
                        receiver: "client".to_string(),
                        message: "request cancelled".to_string(),
                    };
                    self.send(old_message_to_protocol(err)?).await
                } else {
                    let err = Message::Error {
                        req_id: Some(req_id),
                        sender: self.agent_id.clone(),
                        receiver: "client".to_string(),
                        message: "request not found in queue (already executing or completed)"
                            .to_string(),
                    };
                    self.send(old_message_to_protocol(err)?).await
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

    async fn recv(&self) -> Option<Result<AgentServerMessage, ConnectionError>> {
        let msg = {
            let mut rx = self.rx.lock().await;
            rx.next().await?
        };
        match msg {
            Ok(WsMessage::Text(text)) => match serde_json::from_str::<AgentServerMessage>(&text) {
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

    async fn send(&self, msg: AgentServerMessage) -> Result<(), ConnectionError> {
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
