//! HTTP-based approval channel with axum router.

use crate::react::hitl::*;
use tokio::sync::{mpsc, oneshot};
use std::sync::Arc;

struct ApprovalRequestWithCallback {
    request: ApprovalRequest,
    callback: oneshot::Sender<ApprovalResponse>,
}

/// HTTP-based approval channel
///
/// This channel sends approval requests to an HTTP endpoint.
/// The caller is responsible for running the HTTP server that handles
/// the approval requests.
pub struct HttpApprovalChannel {
    rx: Arc<tokio::sync::Mutex<mpsc::Receiver<ApprovalRequestWithCallback>>>,
}

impl HttpApprovalChannel {
    /// Create a new HTTP approval channel
    ///
    /// Returns the channel and a sender for submitting approval requests.
    /// The sender should be passed to the HTTP handler.
    pub fn new() -> (Self, mpsc::Sender<ApprovalRequestWithCallback>) {
        let (tx, rx) = mpsc::channel(100);
        (
            Self { rx: Arc::new(tokio::sync::Mutex::new(rx)) },
            tx,
        )
    }

    /// Create an HTTP router for handling approval requests
    #[cfg(feature = "http")]
    pub fn create_router(
        _sender: mpsc::Sender<ApprovalRequestWithCallback>,
    ) -> axum::Router {
        use axum::{extract::State, http::StatusCode, routing::post, Json};
        use serde::{Deserialize, Serialize};

        #[derive(Debug, Deserialize)]
        struct ApprovalPayload {
            approve: bool,
            reason: Option<String>,
        }

        #[derive(Debug, Serialize)]
        struct SuccessResponse {
            success: bool,
        }

        async fn handle_approval(
            State(_sender): State<mpsc::Sender<ApprovalRequestWithCallback>>,
            Json(_payload): Json<ApprovalPayload>,
        ) -> Result<Json<SuccessResponse>, StatusCode> {
            // Note: In a real implementation, you'd need to track pending requests
            // and route the response to the correct callback. This is a simplified
            // example.
            Ok(Json(SuccessResponse { success: true }))
        }

        axum::Router::new()
            .route("/api/approval", post(handle_approval))
    }
}

impl Default for HttpApprovalChannel {
    fn default() -> Self {
        Self::new().0
    }
}

#[async_trait::async_trait]
impl ApprovalChannel for HttpApprovalChannel {
    async fn request_approval(
        &self,
        _request: ApprovalRequest,
        _timeout: Option<std::time::Duration>,
    ) -> Result<Option<ApprovalResponse>, ApprovalError> {
        // Note: This is a simplified implementation. In a real HTTP channel,
        // you would send the request to an external HTTP endpoint and wait
        // for the response. This implementation uses an internal channel
        // that would be connected to an HTTP handler.
        //
        // Since this is a framework-level implementation, we'll return
        // a placeholder that indicates the channel is not yet configured.
        Err(ApprovalError::Transport(
            "HTTP channel requires HTTP handler to be configured".to_string()
        ))
    }
}

// Alternative simpler implementation that's more practical:
// Use a shared state to store pending requests and their responses

use std::collections::HashMap;

/// Simpler HTTP approval channel using shared state
pub struct SimpleHttpApprovalChannel {
    pending_requests: Arc<tokio::sync::Mutex<HashMap<String, oneshot::Sender<ApprovalResponse>>>>,
}

impl SimpleHttpApprovalChannel {
    pub fn new() -> Self {
        Self {
            pending_requests: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
        }
    }

    /// Get the sender handle for submitting approval responses
    pub fn pending_requests(&self) -> Arc<tokio::sync::Mutex<HashMap<String, oneshot::Sender<ApprovalResponse>>>> {
        self.pending_requests.clone()
    }

    /// Create an HTTP router for handling approval responses
    #[cfg(feature = "http")]
    pub fn create_router(&self) -> axum::Router {
        use axum::{extract::{State, Path}, http::StatusCode, routing::post, Json};
        use serde::{Deserialize, Serialize};

        #[derive(Debug, Deserialize)]
        struct ApprovalPayload {
            approve: bool,
            reason: Option<String>,
        }

        #[derive(Debug, Serialize)]
        struct SuccessResponse {
            success: bool,
        }

        async fn handle_approval(
            State(pending): State<Arc<tokio::sync::Mutex<HashMap<String, oneshot::Sender<ApprovalResponse>>>>>,
            Path(run_id): Path<String>,
            Json(payload): Json<ApprovalPayload>,
        ) -> Result<Json<SuccessResponse>, StatusCode> {
            let response = if payload.approve {
                ApprovalResponse::Approved
            } else {
                ApprovalResponse::Rejected {
                    reason: payload.reason.unwrap_or_else(|| "User rejected".to_string()),
                }
            };

            let mut pending = pending.lock().await;
            if let Some(callback) = pending.remove(&run_id) {
                let _ = callback.send(response);
                Ok(Json(SuccessResponse { success: true }))
            } else {
                Err(StatusCode::NOT_FOUND)
            }
        }

        axum::Router::new()
            .route("/api/approval/:run_id", post(handle_approval))
            .with_state(self.pending_requests.clone())
    }
}

impl Default for SimpleHttpApprovalChannel {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl ApprovalChannel for SimpleHttpApprovalChannel {
    async fn request_approval(
        &self,
        request: ApprovalRequest,
        timeout: Option<std::time::Duration>,
    ) -> Result<Option<ApprovalResponse>, ApprovalError> {
        let (callback_tx, callback_rx): (oneshot::Sender<ApprovalResponse>, oneshot::Receiver<ApprovalResponse>) = oneshot::channel();

        // Register pending request
        {
            let mut pending = self.pending_requests.lock().await;
            pending.insert(request.run_id.clone(), callback_tx);
        }

        // In a real implementation, here you would:
        // 1. Send the approval request to an external service
        // 2. Or wait for an HTTP callback to arrive

        // For now, we just wait for the callback
        let result = if let Some(timeout_dur) = timeout {
            tokio::time::timeout(timeout_dur, callback_rx)
                .await
                .map_err(|_| ApprovalError::Timeout)?
        } else {
            callback_rx.await
        };

        // Clean up pending request
        {
            let mut pending = self.pending_requests.lock().await;
            pending.remove(&request.run_id);
        }

        result
            .map(Some)
            .map_err(|_| ApprovalError::ChannelClosed)
    }
}
