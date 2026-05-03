//! WebSocket transport for agent channel communication.

use std::sync::Arc;

use async_trait::async_trait;
use axum::extract::ws::{Message as WsMessage, WebSocket, WebSocketUpgrade};
use axum::routing::get;
use axum::Router;
use futures::{SinkExt, StreamExt};
use serde_json::json;
use vol_llm_agent::react::AgentStreamEvent;

use crate::connection::{Connection, ConnectionHolder};
use crate::dispatcher::AgentDispatcher;
use crate::error::ConnectionError;
use crate::protocol::{InboundMessage, OutboundMessage};
use crate::request::RunResult;

/// Serialize an `AgentStreamEvent` to a `serde_json::Value`, stripping timestamps.
pub fn serialize_stream_event(event: &AgentStreamEvent) -> serde_json::Value {
    match event {
        AgentStreamEvent::AgentStart { input, .. } => {
            json!({ "type": "agent_start", "input": input })
        }
        AgentStreamEvent::AgentComplete { response, .. } => {
            json!({ "type": "agent_complete", "response": response })
        }
        AgentStreamEvent::AgentAborted { reason, .. } => {
            json!({ "type": "agent_aborted", "reason": reason })
        }
        AgentStreamEvent::MaxIterationsReached {
            current_iteration,
            max_iterations,
            ..
        } => {
            json!({
                "type": "max_iterations_reached",
                "current_iteration": current_iteration,
                "max_iterations": max_iterations,
            })
        }
        AgentStreamEvent::IterationContinued {
            from_iteration, ..
        } => {
            json!({
                "type": "iteration_continued",
                "from_iteration": from_iteration,
            })
        }
        AgentStreamEvent::LLMCallStart {
            iteration, messages, ..
        } => {
            json!({
                "type": "llm_call_start",
                "iteration": iteration,
                "messages": messages,
            })
        }
        AgentStreamEvent::LLMCallComplete {
            model, usage, ..
        } => {
            json!({
                "type": "llm_call_complete",
                "model": model,
                "usage": usage,
            })
        }
        AgentStreamEvent::LLMCallError { error, .. } => {
            json!({ "type": "llm_call_error", "error": error })
        }
        AgentStreamEvent::ThinkingStart { .. } => {
            json!({ "type": "thinking_start" })
        }
        AgentStreamEvent::ThinkingDelta { delta, .. } => {
            json!({ "type": "thinking_delta", "delta": delta })
        }
        AgentStreamEvent::ThinkingComplete { thinking, .. } => {
            json!({ "type": "thinking_complete", "thinking": thinking })
        }
        AgentStreamEvent::ContentStart { .. } => {
            json!({ "type": "content_start" })
        }
        AgentStreamEvent::ContentDelta { delta, .. } => {
            json!({ "type": "content_delta", "delta": delta })
        }
        AgentStreamEvent::ContentComplete { content, .. } => {
            json!({ "type": "content_complete", "content": content })
        }
        AgentStreamEvent::ToolCallBegin {
            tool_call_id,
            tool_name,
            arguments,
            ..
        } => {
            json!({
                "type": "tool_call_begin",
                "tool_call_id": tool_call_id,
                "tool_name": tool_name,
                "arguments": arguments,
            })
        }
        AgentStreamEvent::ToolCallComplete {
            tool_call_id,
            tool_name,
            result,
            duration_ms,
            ..
        } => {
            json!({
                "type": "tool_call_complete",
                "tool_call_id": tool_call_id,
                "tool_name": tool_name,
                "result": result,
                "duration_ms": duration_ms,
            })
        }
        AgentStreamEvent::ToolCallError {
            tool_call_id,
            tool_name,
            error,
            duration_ms,
            ..
        } => {
            json!({
                "type": "tool_call_error",
                "tool_call_id": tool_call_id,
                "tool_name": tool_name,
                "error": error,
                "duration_ms": duration_ms,
            })
        }
        AgentStreamEvent::ToolCallSkipped {
            tool_call_id,
            tool_name,
            reason,
            duration_ms,
            ..
        } => {
            json!({
                "type": "tool_call_skipped",
                "tool_call_id": tool_call_id,
                "tool_name": tool_name,
                "reason": reason,
                "duration_ms": duration_ms,
            })
        }
        AgentStreamEvent::ToolCallArgumentDelta {
            tool_call_id,
            tool_name,
            delta,
            ..
        } => {
            json!({
                "type": "tool_call_argument_delta",
                "tool_call_id": tool_call_id,
                "tool_name": tool_name,
                "delta": delta,
            })
        }
        AgentStreamEvent::IterationComplete {
            iteration,
            tool_calls,
            final_answer,
            ..
        } => {
            json!({
                "type": "iteration_complete",
                "iteration": iteration,
                "tool_calls": tool_calls,
                "final_answer": final_answer,
            })
        }
        AgentStreamEvent::PluginEvent { name, data, .. } => {
            json!({
                "type": "plugin_event",
                "name": name,
                "data": data,
            })
        }
    }
}

/// Serialize an outbound message to a JSON text string.
fn serialize_outbound(msg: &OutboundMessage) -> Result<String, ConnectionError> {
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

    /// Send an error message to the client.
    async fn send_error(
        &self,
        req_id: Option<String>,
        message: String,
    ) -> Result<(), ConnectionError> {
        let msg = OutboundMessage::Error { req_id, message };
        let text = serialize_outbound(&msg)?;
        self.send_text(&text).await
    }

    /// Send a raw text message over the WebSocket.
    async fn send_text(&self, text: &str) -> Result<(), ConnectionError> {
        let mut tx = self.tx.lock().await;
        tx.send(WsMessage::Text(text.to_string()))
            .await
            .map_err(|e| ConnectionError::WsSendError(e.to_string()))
    }

    /// Main connection loop: receive messages, dispatch to agents, send results.
    ///
    /// This method owns the connection and runs until the client disconnects
    /// or a fatal error occurs. It detaches from the holder before returning.
    pub async fn run(mut self) {
        // Send a connected message so the client knows the agent ID.
        if let Err(e) = self
            .send_connected(&self.agent_id)
            .await
        {
            tracing::warn!(%e, "failed to send connected message");
        }

        // Main message loop.
        loop {
            match self.recv().await {
                Some(Ok(msg)) => {
                    match self.handle_inbound(msg).await {
                        Ok(()) => {}
                        Err(e) => {
                            tracing::warn!(%e, "error handling inbound message");
                            // Try to send an error back, but don't panic if send fails.
                            let _ = self
                                .send_error(None, format!("handler error: {e}"))
                                .await;
                        }
                    }
                }
                Some(Err(e)) => {
                    tracing::warn!(%e, "receive error");
                    break;
                }
                None => {
                    // Connection closed.
                    tracing::info!("WebSocket connection closed");
                    break;
                }
            }
        }

        // Detach from holder on disconnect.
        self.holder.detach().await;
    }

    /// Send the `OutboundMessage::Connected` handshake.
    async fn send_connected(&self, agent_id: &str) -> Result<(), ConnectionError> {
        let msg = OutboundMessage::Connected {
            agent_id: agent_id.to_string(),
        };
        let text = serialize_outbound(&msg)?;
        self.send_text(&text).await
    }

    /// Handle a single inbound message.
    async fn handle_inbound(&self, msg: InboundMessage) -> Result<(), ConnectionError> {
        match msg {
            InboundMessage::Submit {
                req_id,
                target_id,
                input,
                metadata,
            } => {
                let mut request = crate::request::AgentRequest::with_id(&req_id, &target_id, &input);
                if let Some(meta) = metadata {
                    request.metadata = meta;
                }

                // Submit to dispatcher.
                let rx = match self.dispatcher.submit(request) {
                    Ok(rx) => rx,
                    Err(e) => {
                        return self.send_error(Some(req_id), e.to_string()).await;
                    }
                };

                // Wait for the result and send it back.
                match rx.await {
                    Ok(run_result) => {
                        self.send_result(&run_result).await
                    }
                    Err(_) => {
                        // The dispatcher's oneshot sender was dropped.
                        self.send_error(
                            Some(req_id),
                            "dispatcher dropped while processing request".to_string(),
                        )
                        .await
                    }
                }
            }
            InboundMessage::Cancel { req_id } => {
                let cancelled = self.dispatcher.cancel(&req_id).await;
                if cancelled {
                    // Send a cancellation confirmation.
                    let msg = OutboundMessage::Error {
                        req_id: Some(req_id),
                        message: "request cancelled".to_string(),
                    };
                    let text = serialize_outbound(&msg)?;
                    self.send_text(&text).await
                } else {
                    // Request was already executing or completed — can't cancel.
                    self.send_error(
                        Some(req_id),
                        "request not found in queue (already executing or completed)".to_string(),
                    )
                    .await
                }
            }
        }
    }
}

#[async_trait]
impl Connection for WsConnection {
    fn protocol(&self) -> &str {
        "ws"
    }

    async fn recv(&mut self) -> Option<Result<InboundMessage, ConnectionError>> {
        // Use the shared rx via the split stream. We need to re-borrow self.rx.
        // Since we can't split &mut self, we use a workaround: the rx field is
        // accessed through a mutable reference to self.
        let msg = self.rx.next().await?;

        match msg {
            Ok(WsMessage::Text(text)) => {
                match serde_json::from_str::<InboundMessage>(&text) {
                    Ok(msg) => Some(Ok(msg)),
                    Err(e) => Some(Err(ConnectionError::ParseError(e.to_string()))),
                }
            }
            Ok(WsMessage::Close(_)) => {
                None // Client initiated close.
            }
            Ok(WsMessage::Binary(_)) => {
                Some(Err(ConnectionError::ParseError(
                    "binary messages not supported".to_string(),
                )))
            }
            Ok(WsMessage::Ping(data)) => {
                // axum's WebSocket layer auto-responds to pings, but handle gracefully.
                tracing::debug!("WebSocket ping: {} bytes", data.len());
                // Recurse to get the next real message.
                self.recv().await
            }
            Ok(WsMessage::Pong(_)) => {
                // Ignore pongs.
                self.recv().await
            }
            Err(e) => Some(Err(ConnectionError::WsReceiveError(e.to_string()))),
        }
    }

    async fn send_event(&self, event: &AgentStreamEvent) -> Result<(), ConnectionError> {
        let event_value = serialize_stream_event(event);
        let msg = OutboundMessage::Event { event: event_value };
        let text = serialize_outbound(&msg)?;
        self.send_text(&text).await
    }

    async fn send_result(&self, result: &RunResult) -> Result<(), ConnectionError> {
        let response_value = match &result.response {
            Ok(resp) => serde_json::to_value(resp)
                .map_err(|e| ConnectionError::WsSendError(e.to_string()))?,
            Err(err) => json!({ "error": err.to_string() }),
        };

        let result_payload = json!({
            "req_id": result.req_id,
            "target_id": result.target_id,
            "run_id": result.run_id,
            "response": response_value,
        });

        let msg = OutboundMessage::Result {
            result: result_payload,
        };
        let text = serialize_outbound(&msg)?;
        self.send_text(&text).await
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

        Router::new()
            .route(
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
    let conn = WsConnection::new(socket, server.dispatcher.clone(), server.holder.clone(), agent_id);

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
            tracing::warn!("could not take ownership of WsConnection (refcount > 1), skipping run loop");
            // Still detach to clean up.
            server.holder.detach().await;
        }
    }
}
