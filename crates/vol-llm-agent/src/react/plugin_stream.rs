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

            // Pass through plugin interceptors
            let mut current = Some(raw_event);

            for plugin in &self.plugins {
                match current {
                    Some(event) => {
                        match plugin.intercept(event, &self.ctx).await {
                            PluginAction::Continue(Some(e)) => current = Some(e),
                            PluginAction::Continue(None) => {
                                // Event dropped, continue outer loop to get next event
                                current = None;
                                break;
                            }
                            PluginAction::ShortCircuit(response) => {
                                // Short-circuit: send final response immediately
                                return Some(Ok(AgentStreamEvent::AgentComplete { response }));
                            }
                            PluginAction::Skip => {
                                // Skip this event, continue outer loop
                                current = None;
                                break;
                            }
                            PluginAction::Abort(e) => {
                                return Some(Err(e));
                            }
                        }
                    }
                    None => {
                        // Event was dropped or skipped, continue outer loop
                        current = None;
                        break;
                    }
                }
            }

            // If we have an event after all plugins, return it
            if current.is_some() {
                return current;
            }
            // Otherwise, continue loop to get next event
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
    pub system_prompt_hash: String,
    pub verbose: bool,
}

impl From<&super::AgentConfig> for AgentConfigSnapshot {
    fn from(config: &super::AgentConfig) -> Self {
        use std::hash::{Hash, Hasher};
        use std::collections::hash_map::DefaultHasher;

        let mut hasher = DefaultHasher::new();
        config.system_prompt.hash(&mut hasher);

        Self {
            max_iterations: config.max_iterations,
            max_history_messages: config.max_history_messages,
            system_prompt_hash: format!("{:x}", hasher.finish()),
            verbose: config.verbose,
        }
    }
}

impl Default for AgentConfigSnapshot {
    fn default() -> Self {
        Self {
            max_iterations: 5,
            max_history_messages: 20,
            system_prompt_hash: String::new(),
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
