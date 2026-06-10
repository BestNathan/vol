//! Axum routes and handlers for event ingestion.

use std::sync::atomic::Ordering;

use axum::{
    extract::State,
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

use crate::event::{ExtractedMetric, IngestBatch};
use crate::loki_writer::{LokiCommand, LokiWriterHealth};
use crate::tdengine_writer::{TdengineCommand, TdengineWriterHealth};

/// Shared application state.
#[derive(Clone)]
pub struct AppState {
    pub loki_tx: mpsc::Sender<LokiCommand>,
    pub tdengine_tx: mpsc::Sender<TdengineCommand>,
    pub loki_health: LokiWriterHealth,
    pub tdengine_health: TdengineWriterHealth,
}

/// Build the Axum router.
pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/api/v1/events", post(handle_events))
        .route("/health", get(handle_health))
        .with_state(state)
}

/// Health check response.
#[derive(Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: String,
    pub loki: bool,
    pub tdengine: bool,
}

/// Health check handler.
async fn handle_health(State(state): State<AppState>) -> Json<HealthResponse> {
    let loki_ok = state.loki_health.last_flush_ok.load(Ordering::SeqCst);
    let tdengine_ok = state.tdengine_health.last_flush_ok.load(Ordering::SeqCst);

    let status = if loki_ok || tdengine_ok {
        "ok".to_string()
    } else {
        "degraded".to_string()
    };

    Json(HealthResponse {
        status,
        loki: loki_ok,
        tdengine: tdengine_ok,
    })
}

/// Ingest events handler.
async fn handle_events(
    State(state): State<AppState>,
    Json(batch): Json<IngestBatch>,
) -> StatusCode {
    if batch.events.is_empty() {
        return StatusCode::BAD_REQUEST;
    }

    let count = batch.events.len();

    for event in batch.events {
        // Route to Loki
        let _ = state.loki_tx.send(LokiCommand::Event(event.clone())).await;

        // Extract metric and route to TDengine
        if let Some(metric) = ExtractedMetric::from_event(&event) {
            let _ = state
                .tdengine_tx
                .send(TdengineCommand::Metric(metric))
                .await;
        }
    }

    tracing::debug!(count, "Ingested events");
    StatusCode::ACCEPTED
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use http_body_util::BodyExt;
    use serde_json::json;
    use tower::ServiceExt;

    fn create_test_state() -> AppState {
        let (loki_tx, _) = mpsc::channel(100);
        let (tdengine_tx, _) = mpsc::channel(100);
        AppState {
            loki_tx,
            tdengine_tx,
            loki_health: LokiWriterHealth::default(),
            tdengine_health: TdengineWriterHealth::default(),
        }
    }

    #[tokio::test]
    async fn test_health_endpoint() {
        let state = create_test_state();
        let app = build_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let health: HealthResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(health.status, "degraded"); // initial state: both false
    }

    #[tokio::test]
    async fn test_ingest_events() {
        let state = create_test_state();
        let app = build_router(state);

        let batch = IngestBatch {
            events: vec![crate::event::IngestEvent {
                run_id: "run-1".to_string(),
                session_id: "session-1".to_string(),
                agent_id: "agent-1".to_string(),
                agent_type: "CodingAgent".to_string(),
                timestamp: chrono::Utc::now(),
                event: "ToolCallComplete".to_string(),
                data: json!({"tool_name": "bash", "result": "ok", "duration_ms": 100}),
            }],
        };

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/events")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_string(&batch).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::ACCEPTED);
    }

    #[tokio::test]
    async fn test_ingest_empty_batch_rejected() {
        let state = create_test_state();
        let app = build_router(state);

        let batch = IngestBatch { events: vec![] };

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/events")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_string(&batch).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
}
