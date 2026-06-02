//! HTTP transport for Agent Server Protocol messages.

use std::sync::Arc;

use axum::extract::Query;
use axum::http::StatusCode;
use axum::response::sse::{Event, Sse};
use axum::response::IntoResponse;
use axum::routing::post;
use axum::{Json, Router};
use serde::Deserialize;

use crate::agent_server_protocol::{AgentServerMessage, ErrorPayload, Operation, SystemOperation};
use crate::server_core::AgentServerCore;

#[derive(Deserialize)]
struct StreamQuery {
    stream: Option<bool>,
}

pub struct HttpTransport {
    core: Arc<AgentServerCore>,
}

impl HttpTransport {
    pub fn new(core: Arc<AgentServerCore>) -> Self {
        Self { core }
    }

    pub fn into_axum_router(self) -> Router {
        let transport = Arc::new(self);

        Router::new().route(
            "/",
            post({
                let transport = transport.clone();
                move |query, body| handle_post(transport, query, body)
            }),
        )
    }
}

async fn handle_post(
    transport: Arc<HttpTransport>,
    Query(query): Query<StreamQuery>,
    Json(message): Json<AgentServerMessage>,
) -> axum::response::Response {
    if query.stream.unwrap_or(false) {
        handle_sse(transport, message).await
    } else {
        handle_blocking(transport, message).await
    }
}

async fn handle_blocking(
    transport: Arc<HttpTransport>,
    message: AgentServerMessage,
) -> axum::response::Response {
    match transport.core.handle(message).await {
        Ok(messages) => (StatusCode::OK, Json(messages)).into_response(),
        Err(error) => {
            let message = protocol_error_message(error.to_string());
            (StatusCode::OK, Json(vec![message])).into_response()
        }
    }
}

async fn handle_sse(
    transport: Arc<HttpTransport>,
    message: AgentServerMessage,
) -> axum::response::Response {
    let messages = match transport.core.handle(message).await {
        Ok(messages) => messages,
        Err(error) => vec![protocol_error_message(error.to_string())],
    };

    let stream = async_stream::stream! {
        for message in messages {
            match serde_json::to_string(&message) {
                Ok(json) => yield Ok::<_, std::convert::Infallible>(Event::default().data(json)),
                Err(error) => {
                    let fallback = protocol_error_message(error.to_string());
                    let json = serde_json::to_string(&fallback).unwrap_or_else(|_| "{}".to_string());
                    yield Ok(Event::default().data(json));
                }
            }
        }
    };

    Sse::new(stream).into_response()
}

fn protocol_error_message(message: String) -> AgentServerMessage {
    AgentServerMessage::new_error(
        uuid::Uuid::new_v4().to_string(),
        Operation::System(SystemOperation::Connected),
        ErrorPayload {
            code: "dispatch_error".to_string(),
            message,
            detail: None,
            terminal: false,
        },
    )
}
