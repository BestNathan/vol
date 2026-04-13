//! Human-in-the-Loop support for ReAct Agent.
//!
//! Features:
//! - Synchronous approval waiting
//! - Configurable timeout behavior
//! - Pluggable approval channel (HTTP, WebSocket, CLI, etc.)

use async_trait::async_trait;
use std::time::Duration;
use thiserror::Error;

/// Approval request context
#[derive(Debug, Clone)]
pub struct ApprovalRequest {
    pub run_id: String,
    pub request_type: ApprovalType,
    pub message: String,
    pub metadata: serde_json::Value,
}

/// Type of approval needed
#[derive(Debug, Clone, PartialEq)]
pub enum ApprovalType {
    ToolExecution { tool_name: String },
    ContinueIteration { iteration: u32 },
    FinalAnswer,
    Custom { name: String },
}

/// Approval response
#[derive(Debug, Clone)]
pub enum ApprovalResponse {
    Approved,
    Rejected { reason: String },
}

/// Approval channel trait - pluggable transport for approval requests
#[async_trait]
pub trait ApprovalChannel: Send + Sync {
    /// Send approval request and wait for response (synchronous)
    ///
    /// Returns Ok(None) on timeout, caller should handle based on config
    async fn request_approval(
        &self,
        request: ApprovalRequest,
        timeout: Option<Duration>,
    ) -> Result<Option<ApprovalResponse>, ApprovalError>;
}

#[derive(Debug, Error)]
pub enum ApprovalError {
    #[error("Channel closed")]
    ChannelClosed,

    #[error("Timeout waiting for approval")]
    Timeout,

    #[error("Transport error: {0}")]
    Transport(String),
}

/// HITL configuration
#[derive(Debug, Clone)]
pub struct HitlConfig {
    /// Triggers that require approval
    pub triggers: Vec<ApprovalTrigger>,

    /// Timeout for each approval request (0 = no timeout)
    pub timeout_secs: u64,

    /// Behavior on timeout
    pub on_timeout: TimeoutBehavior,

    /// Timeout message (if applicable)
    pub timeout_message: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ApprovalTrigger {
    /// Require approval before executing specific tools
    /// None = all tools, Some([...]) = specific tools
    ToolExecution { tools: Option<Vec<String>> },

    /// Require approval after each iteration (before next iteration)
    AfterIteration,

    /// Require approval before sending final answer
    BeforeFinalAnswer,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TimeoutBehavior {
    /// Auto-approve on timeout
    Approve,

    /// Auto-reject on timeout
    Reject { reason: String },

    /// Stop execution on timeout
    Stop,
}

impl Default for HitlConfig {
    fn default() -> Self {
        Self {
            triggers: vec![],
            timeout_secs: 0,
            on_timeout: TimeoutBehavior::Approve,
            timeout_message: None,
        }
    }
}

use super::plugin::*;
use super::run_context::PluginContext;
use super::AgentStreamEvent;
use std::sync::Arc;

/// Human-in-the-Loop plugin
pub struct HitlPlugin<C: ApprovalChannel> {
    config: HitlConfig,
    channel: Arc<C>,
}

impl<C: ApprovalChannel> HitlPlugin<C> {
    pub fn new(config: HitlConfig, channel: Arc<C>) -> Self {
        Self { config, channel }
    }

    fn needs_tool_approval(&self, tool_name: &str) -> bool {
        self.config.triggers.iter().any(|t| {
            if let ApprovalTrigger::ToolExecution { tools } = t {
                match tools {
                    None => true,
                    Some(list) => list.contains(&tool_name.to_string()),
                }
            } else {
                false
            }
        })
    }

    fn needs_iteration_pause(&self) -> bool {
        self.config
            .triggers
            .iter()
            .any(|t| matches!(t, ApprovalTrigger::AfterIteration))
    }

    async fn request_approval(
        &self,
        request: ApprovalRequest,
    ) -> Result<ApprovalResponse, ApprovalError> {
        let timeout = if self.config.timeout_secs > 0 {
            Some(Duration::from_secs(self.config.timeout_secs))
        } else {
            None
        };

        match self.channel.request_approval(request, timeout).await {
            Ok(Some(response)) => Ok(response),
            Ok(None) => match &self.config.on_timeout {
                TimeoutBehavior::Approve => Ok(ApprovalResponse::Approved),
                TimeoutBehavior::Reject { reason } => Ok(ApprovalResponse::Rejected {
                    reason: reason.clone(),
                }),
                TimeoutBehavior::Stop => Err(ApprovalError::Timeout),
            },
            Err(e) => Err(e),
        }
    }
}

#[async_trait]
impl<C: ApprovalChannel + 'static> AgentPlugin for HitlPlugin<C> {
    fn id(&self) -> PluginId {
        "human_in_loop".to_string()
    }

    fn priority(&self) -> u32 {
        25
    }

    /// Interceptor hook - checks for approval requirements
    async fn intercept(&self, event: &AgentStreamEvent, ctx: &PluginContext) -> PluginDecision {
        match event {
            AgentStreamEvent::ToolCallBegin {
                tool_call_id,
                tool_name,
                arguments,
            } => {
                if self.needs_tool_approval(tool_name) {
                    let request = ApprovalRequest {
                        run_id: ctx.run_id.clone(),
                        request_type: ApprovalType::ToolExecution {
                            tool_name: tool_name.clone(),
                        },
                        message: format!("Execute tool: {} with args: {}", tool_name, arguments),
                        metadata: serde_json::json!({ "tool_call_id": tool_call_id, "tool_name": tool_name, "arguments": arguments }),
                    };

                    match self.request_approval(request).await {
                        Ok(ApprovalResponse::Approved) => PluginDecision::Continue,
                        Ok(ApprovalResponse::Rejected { reason }) => {
                            // Return skip with rejection reason - caller should handle
                            PluginDecision::Abort(format!("Rejected: {}", reason))
                        }
                        Err(ApprovalError::Timeout) => {
                            PluginDecision::Abort("Approval timeout".to_string())
                        }
                        Err(e) => PluginDecision::Abort(format!("Approval error: {}", e)),
                    }
                } else {
                    PluginDecision::Continue
                }
            }

            AgentStreamEvent::IterationComplete {
                iteration,
                final_answer,
                ..
            } => {
                if self.needs_iteration_pause() && final_answer.is_none() {
                    let request = ApprovalRequest {
                        run_id: ctx.run_id.clone(),
                        request_type: ApprovalType::ContinueIteration {
                            iteration: *iteration,
                        },
                        message: format!("Iteration {} complete. Continue?", iteration),
                        metadata: serde_json::json!({ "iteration": iteration }),
                    };

                    match self.request_approval(request).await {
                        Ok(ApprovalResponse::Approved) => PluginDecision::Continue,
                        Ok(ApprovalResponse::Rejected { reason }) => PluginDecision::Abort(
                            format!("Stopped after iteration {}: {}", iteration, reason),
                        ),
                        Err(ApprovalError::Timeout) => {
                            PluginDecision::Abort("Approval timeout".to_string())
                        }
                        Err(e) => PluginDecision::Abort(format!("Approval error: {}", e)),
                    }
                } else {
                    PluginDecision::Continue
                }
            }

            _ => PluginDecision::Continue,
        }
    }

    /// Listener hook - logs HITL events for audit
    async fn listen(&self, event: &AgentStreamEvent, ctx: &PluginContext) {
        match event {
            AgentStreamEvent::ToolCallBegin { tool_call_id, tool_name, .. } => {
                if self.needs_tool_approval(tool_name) {
                    tracing::info!(
                        run_id = %ctx.run_id,
                        tool_call_id = %tool_call_id,
                        tool_name = %tool_name,
                        "HITL: Tool execution requires approval"
                    );
                }
            }
            AgentStreamEvent::IterationComplete { iteration, .. } => {
                if self.needs_iteration_pause() {
                    tracing::info!(
                        run_id = %ctx.run_id,
                        iteration = %iteration,
                        "HITL: Iteration pause requires approval"
                    );
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockChannel;

    #[async_trait]
    impl ApprovalChannel for MockChannel {
        async fn request_approval(
            &self,
            _request: ApprovalRequest,
            _timeout: Option<Duration>,
        ) -> Result<Option<ApprovalResponse>, ApprovalError> {
            Ok(Some(ApprovalResponse::Approved))
        }
    }

    #[test]
    fn test_hitl_config_default() {
        let config = HitlConfig::default();
        assert_eq!(config.triggers.len(), 0);
        assert_eq!(config.timeout_secs, 0);
    }

    #[test]
    fn test_approval_trigger_variants() {
        let _tool_trigger = ApprovalTrigger::ToolExecution { tools: None };
        let _iteration_trigger = ApprovalTrigger::AfterIteration;
        let _final_trigger = ApprovalTrigger::BeforeFinalAnswer;
    }
}

/// CLI approval handler — runs as a background task.
///
/// Receives approval requests from RunContext's approval channel,
/// prompts the user on stdin, and sends back responses.
///
/// # Usage
///
/// Spawn this on a background thread after creating RunContext:
///
/// ```ignore
/// let (ctx, _plugin_rx, approval_rx) = RunContext::new(...);
/// run_cli_approval_loop(approval_rx);
/// ```
pub fn run_cli_approval_loop(
    rx: tokio::sync::mpsc::Receiver<(
        super::run_context::ApprovalRequest,
        tokio::sync::oneshot::Sender<super::run_context::ApprovalResponse>,
    )>,
) {
    use std::io::{self, BufRead, Write};

    std::thread::spawn(move || {
        let stdin = io::stdin();
        let mut rx = rx; // Make mutable

        while let Some((request, tx)) = rx.blocking_recv() {
            // Display request
            println!();
            println!("⚠ Approval required:");
            println!("  Tool: {}", request.tool_name);
            println!("  Reason: {}", request.reason);
            print!("  Approve? [y/n] > ");
            let _ = io::stdout().flush();

            // Read response
            let mut line = String::new();
            let approved = match stdin.lock().read_line(&mut line) {
                Ok(_) => {
                    let trimmed = line.trim().to_lowercase();
                    trimmed == "y" || trimmed == "yes" || trimmed.is_empty()
                }
                Err(_) => false,
            };

            let response = if approved {
                super::run_context::ApprovalResponse::approved()
            } else {
                super::run_context::ApprovalResponse::rejected("User rejected".into())
            };

            let _ = tx.send(response);
        }
    });
}
