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
use super::{AgentStreamEvent, AgentResponse, AgentError};
use super::run_context::RunContext;
use std::sync::Arc;

enum ApprovalResult {
    Continue,
    Rejected { reason: String },
    Stop,
}

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
        self.config.triggers.iter().any(|t| {
            matches!(t, ApprovalTrigger::AfterIteration)
        })
    }

    fn needs_final_answer_approval(&self) -> bool {
        self.config.triggers.iter().any(|t| {
            matches!(t, ApprovalTrigger::BeforeFinalAnswer)
        })
    }

    async fn request_approval(&self, request: ApprovalRequest) -> Result<ApprovalResult, ApprovalError> {
        let timeout = if self.config.timeout_secs > 0 {
            Some(Duration::from_secs(self.config.timeout_secs))
        } else {
            None
        };

        match self.channel.request_approval(request, timeout).await {
            Ok(Some(response)) => {
                match response {
                    ApprovalResponse::Approved => Ok(ApprovalResult::Continue),
                    ApprovalResponse::Rejected { reason } => Ok(ApprovalResult::Rejected { reason }),
                }
            }
            Ok(None) => {
                Ok(match self.config.on_timeout {
                    TimeoutBehavior::Approve => ApprovalResult::Continue,
                    TimeoutBehavior::Reject { ref reason } => ApprovalResult::Rejected { reason: reason.clone() },
                    TimeoutBehavior::Stop => ApprovalResult::Stop,
                })
            }
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

    async fn intercept(&self, event: StreamEvent, ctx: &RunContext) -> PluginAction<Option<StreamEvent>> {
        match &event {
            Ok(AgentStreamEvent::ToolCallBegin { tool_name, arguments }) => {
                if self.needs_tool_approval(tool_name) {
                    let request = ApprovalRequest {
                        run_id: ctx.run_id.clone(),
                        request_type: ApprovalType::ToolExecution { tool_name: tool_name.clone() },
                        message: format!("Execute tool: {} with args: {}", tool_name, arguments),
                        metadata: serde_json::json!({ "tool_name": tool_name, "arguments": arguments }),
                    };

                    match self.request_approval(request).await {
                        Ok(ApprovalResult::Continue) => {}
                        Ok(ApprovalResult::Rejected { reason }) => {
                            return PluginAction::Continue(Some(Ok(AgentStreamEvent::ToolCallComplete {
                                tool_name: tool_name.clone(),
                                result: format!("Rejected: {}", reason),
                            })));
                        }
                        Ok(ApprovalResult::Stop) => {
                            return PluginAction::Abort(AgentError::Context("Stopped by user (HITL)".to_string()));
                        }
                        Err(e) => {
                            return PluginAction::Abort(AgentError::Context(format!("Approval error: {}", e)));
                        }
                    }
                }
            }

            Ok(AgentStreamEvent::IterationComplete { iteration, final_answer, .. }) => {
                if self.needs_iteration_pause() && final_answer.is_none() {
                    let request = ApprovalRequest {
                        run_id: ctx.run_id.clone(),
                        request_type: ApprovalType::ContinueIteration { iteration: *iteration },
                        message: format!("Iteration {} complete. Continue?", iteration),
                        metadata: serde_json::json!({ "iteration": iteration }),
                    };

                    match self.request_approval(request).await {
                        Ok(ApprovalResult::Continue) => {}
                        Ok(ApprovalResult::Rejected { reason }) => {
                            return PluginAction::ShortCircuit(AgentResponse {
                                content: String::new(),
                                reasoning: format!("Stopped after iteration {}: {}", iteration, reason),
                                iterations: *iteration,
                                tool_calls: Vec::new(),
                            });
                        }
                        Ok(ApprovalResult::Stop) => {
                            return PluginAction::Abort(AgentError::Context("Stopped by user (HITL)".to_string()));
                        }
                        Err(e) => {
                            return PluginAction::Abort(AgentError::Context(format!("Approval error: {}", e)));
                        }
                    }
                }
            }

            _ => {}
        }

        PluginAction::Continue(Some(event))
    }

    async fn on_complete(&self, _ctx: &RunContext, _response: &AgentResponse) -> PluginAction<()> {
        PluginAction::Continue(())
    }

    async fn on_error(&self, ctx: &RunContext, error: &AgentError) -> PluginAction<()> {
        tracing::error!(run_id = %ctx.run_id, error = %error, "Agent error");
        PluginAction::Continue(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockChannel;

    #[async_trait]
    impl ApprovalChannel for MockChannel {
        async fn request_approval(&self, _request: ApprovalRequest, _timeout: Option<Duration>) -> Result<Option<ApprovalResponse>, ApprovalError> {
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
