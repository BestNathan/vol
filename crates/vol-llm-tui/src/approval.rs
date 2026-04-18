//! TUI approval handler — shows approval requests in the ratatui UI.

use std::sync::Arc;
use tokio::sync::{Mutex, Notify};
use vol_llm_agent::react::{ApprovalHandler, ApprovalRequest, ApprovalResponse, BoxedApprovalHandler};
use vol_llm_agent::react::hitl::ApprovalError;

/// Shared state for pending approval requests in the TUI.
#[derive(Clone)]
pub struct ApprovalState {
    /// Current pending tool name (for display).
    pub tool_name: Arc<Mutex<Option<String>>>,
    /// Current pending reason (for display).
    pub reason: Arc<Mutex<Option<String>>>,
    /// Current pending arguments preview (for display).
    pub arguments: Arc<Mutex<Option<String>>>,
    /// Response to be set by keyboard handler: (approved, reason).
    pub response: Arc<Mutex<Option<(bool, Option<String>)>>>,
    /// Notifier signaled when response is set by keyboard handler.
    pub notify: Arc<Notify>,
}

impl ApprovalState {
    pub fn new() -> Self {
        Self {
            tool_name: Arc::new(Mutex::new(None)),
            reason: Arc::new(Mutex::new(None)),
            arguments: Arc::new(Mutex::new(None)),
            response: Arc::new(Mutex::new(None)),
            notify: Arc::new(Notify::new()),
        }
    }

    pub fn into_handler(self) -> BoxedApprovalHandler {
        BoxedApprovalHandler::new(TuiApprovalHandler { state: self.clone() })
    }

    /// Check if there's a pending approval request.
    pub async fn is_pending(&self) -> bool {
        self.tool_name.lock().await.is_some()
    }

    /// Sync check if there's a pending approval request.
    /// If the lock is currently held, that means the agent is actively
    /// waiting for approval — treat it as pending.
    pub fn has_pending_approval(&self) -> bool {
        match self.tool_name.try_lock() {
            Ok(guard) => guard.is_some(),
            Err(_) => true, // lock held → agent is setting up approval → treat as pending
        }
    }

    /// Clear the pending state after response is sent.
    pub async fn clear(&self) {
        *self.tool_name.lock().await = None;
        *self.reason.lock().await = None;
        *self.arguments.lock().await = None;
        *self.response.lock().await = None;
    }
}

/// Approval handler that stores the request and waits for UI response.
pub struct TuiApprovalHandler {
    state: ApprovalState,
}

#[async_trait::async_trait]
impl ApprovalHandler for TuiApprovalHandler {
    async fn request_approval(
        &self,
        request: ApprovalRequest,
    ) -> Result<Option<ApprovalResponse>, ApprovalError> {
        tracing::info!(tool = %request.tool_name, reason = %request.reason, "TUI approval request received");

        // Store the pending request for UI display
        *self.state.tool_name.lock().await = Some(request.tool_name.clone());
        *self.state.reason.lock().await = Some(request.reason.clone());
        *self.state.arguments.lock().await = Some(
            serde_json::to_string_pretty(&request.metadata)
                .unwrap_or_default(),
        );

        // Wait for keyboard handler to set the response
        self.state.notify.notified().await;

        // Read and return the response
        let resp = self.state.response.lock().await.take();
        match resp {
            Some((true, _)) => Ok(Some(ApprovalResponse::approved())),
            Some((false, reason)) => Ok(Some(ApprovalResponse::rejected(
                reason.unwrap_or_else(|| "User rejected".to_string()),
            ))),
            None => Ok(None),
        }
    }
}

