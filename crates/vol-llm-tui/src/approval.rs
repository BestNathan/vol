//! TUI approval channel — bridges ApprovalState to the ApprovalChannel trait.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tokio::sync::{Mutex, Notify};
use vol_llm_agent::react::{ApprovalChannel, ApprovalRequest, ApprovalResponse};
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
    /// Shared unsafe_mode flag — checked at runtime in the channel.
    pub unsafe_mode: Arc<AtomicBool>,
}

impl ApprovalState {
    pub fn new(unsafe_mode: bool) -> Self {
        Self {
            tool_name: Arc::new(Mutex::new(None)),
            reason: Arc::new(Mutex::new(None)),
            arguments: Arc::new(Mutex::new(None)),
            response: Arc::new(Mutex::new(None)),
            notify: Arc::new(Notify::new()),
            unsafe_mode: Arc::new(AtomicBool::new(unsafe_mode)),
        }
    }

    /// Sync check if there's a pending approval request.
    pub fn has_pending_approval(&self) -> bool {
        match self.tool_name.try_lock() {
            Ok(guard) => guard.is_some(),
            Err(_) => true,
        }
    }

    /// Clear the pending state after response is sent.
    pub async fn clear(&self) {
        *self.tool_name.lock().await = None;
        *self.reason.lock().await = None;
        *self.arguments.lock().await = None;
        *self.response.lock().await = None;
    }

    /// Create a HitlPlugin configured with a TuiApprovalChannel.
    pub fn into_hitl_plugin(self) -> vol_llm_agent::react::hitl::HitlPlugin<TuiApprovalChannel> {
        use vol_llm_agent::react::{ApprovalTrigger, HitlConfig};

        let channel = TuiApprovalChannel { state: self.clone() };
        let config = HitlConfig {
            triggers: vec![ApprovalTrigger::ToolExecution { tools: None }],
            timeout_secs: 0,
            on_timeout: vol_llm_agent::react::TimeoutBehavior::Approve,
            timeout_message: None,
        };
        vol_llm_agent::react::hitl::HitlPlugin::new(config, Arc::new(channel))
    }
}

/// ApprovalChannel implementation that bridges to the TUI's ApprovalState.
pub struct TuiApprovalChannel {
    state: ApprovalState,
}

#[async_trait::async_trait]
impl ApprovalChannel for TuiApprovalChannel {
    async fn request_approval(
        &self,
        request: ApprovalRequest,
        _timeout: Option<Duration>,
    ) -> Result<Option<ApprovalResponse>, ApprovalError> {
        // Check unsafe_mode at runtime — allows toggling mid-run
        if self.state.unsafe_mode.load(Ordering::Relaxed) {
            return Ok(Some(ApprovalResponse::approved()));
        }

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
