//! Plugin stream wrapper and short-circuit utilities.

use super::plugin::*;
use super::{AgentStreamEvent, AgentResponse, AgentStreamReceiver, AgentError};
use super::run_context::RunContext;
use tokio::sync::mpsc;
use std::sync::Arc;
use serde::{Deserialize, Serialize};

/// Wraps internal stream and applies plugin interceptors
pub struct PluginStream {
    inner: AgentStreamReceiver,
    plugins: Vec<Arc<dyn AgentPlugin>>,
    ctx: RunContext,
}

impl PluginStream {
    pub fn new(
        inner: AgentStreamReceiver,
        plugins: Vec<Arc<dyn AgentPlugin>>,
        ctx: RunContext,
    ) -> Self {
        Self { inner, plugins, ctx }
    }

    pub async fn recv(&mut self) -> Option<Result<AgentStreamEvent, AgentError>> {
        loop {
            // Get next event from inner stream
            let raw_event = self.inner.recv().await?;

            // Apply plugin interceptors sequentially
            match raw_event {
                Ok(event) => {
                    // Apply interceptors
                    for plugin in &self.plugins {
                        match plugin.intercept(&event, &self.ctx).await {
                            PluginDecision::Continue => {
                                // Continue to next plugin
                            }
                            PluginDecision::Skip => {
                                // Skip this event, continue outer loop to get next event
                                break;
                            }
                            PluginDecision::Abort(reason) => {
                                return Some(Err(AgentError::Context(reason)));
                            }
                        }
                    }

                    // If we get here, event was not skipped or aborted
                    // Call listen() on all plugins for observability and audit logging
                    let plugins_clone = self.plugins.clone();
                    let event_clone = event.clone();
                    let ctx_clone = self.ctx.clone();
                    tokio::spawn(async move {
                        for plugin in &plugins_clone {
                            plugin.listen(&event_clone, &ctx_clone).await;
                        }
                    });

                    return Some(Ok(event));
                }
                Err(e) => {
                    // Error events pass through unchanged
                    return Some(Err(e));
                }
            }
        }
    }

    /// Convert into a channel-based receiver
    pub fn into_receiver(self) -> AgentStreamReceiver {
        let (tx, rx) = mpsc::channel(100);

        tokio::spawn(async move {
            let mut stream = self;

            while let Some(event) = stream.recv().await {
                if tx.send(event).await.is_err() {
                    break;  // Receiver dropped
                }
            }
        });

        AgentStreamReceiver::new(rx)
    }
}

/// Configuration snapshot for audit/logging
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfigSnapshot {
    pub max_iterations: u32,
    pub max_history_messages: usize,
    pub prompt_context_cache_key: String,
    pub verbose: bool,
}

impl From<&super::AgentConfig> for AgentConfigSnapshot {
    fn from(config: &super::AgentConfig) -> Self {
        Self {
            max_iterations: config.max_iterations,
            max_history_messages: config.max_history_messages,
            prompt_context_cache_key: config.prompt_context.cache_key().to_string(),
            verbose: config.verbose,
        }
    }
}

impl Default for AgentConfigSnapshot {
    fn default() -> Self {
        Self {
            max_iterations: 5,
            max_history_messages: 20,
            prompt_context_cache_key: String::new(),
            verbose: false,
        }
    }
}

/// Create a stream that immediately returns a response (short-circuit)
pub async fn create_shortcircuit_stream(
    response: AgentResponse,
    ctx: RunContext,
    _run_id: String,
) -> Result<AgentStreamReceiver, AgentError> {
    let (tx, rx) = mpsc::channel(10);

    tokio::spawn(async move {
        let _ = tx.send(Ok(AgentStreamEvent::AgentStart {
            input: ctx.user_input,
        })).await;

        let _ = tx.send(Ok(AgentStreamEvent::AgentComplete { response })).await;
    });

    Ok(AgentStreamReceiver::new(rx))
}

/// Create a stream that returns empty response (skip)
pub async fn create_skip_stream(
    ctx: RunContext,
    _run_id: String,
) -> Result<AgentStreamReceiver, AgentError> {
    let (tx, rx) = mpsc::channel(10);

    tokio::spawn(async move {
        let _ = tx.send(Ok(AgentStreamEvent::AgentStart {
            input: ctx.user_input.clone(),
        })).await;

        let _ = tx.send(Ok(AgentStreamEvent::AgentComplete {
            response: AgentResponse {
                content: String::new(),
                reasoning: String::new(),
                iterations: 0,
                tool_calls: Vec::new(),
            },
        })).await;
    });

    Ok(AgentStreamReceiver::new(rx))
}
