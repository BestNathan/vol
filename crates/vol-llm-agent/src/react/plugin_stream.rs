//! Plugin interceptor and listener utilities.

use super::plugin::{AgentPlugin, PluginDecision};
use super::run_context::{PluginRequest, RunContext};
use super::{AgentStreamEvent, AgentResponse, AgentStreamReceiver, AgentError};
use tokio::sync::mpsc;
use std::sync::Arc;

/// Spawn a listener task that subscribes to the event bus and calls
/// `plugin.listen()` on all events (fire-and-forget, parallel execution).
///
/// This is called once at agent startup to wire up the plugin listener.
pub fn spawn_listener_task(
    plugins: Vec<Arc<dyn AgentPlugin>>,
    ctx: RunContext,
) -> tokio::task::JoinHandle<()> {
    let mut event_rx = ctx.event_tx.subscribe();

    tokio::spawn(async move {
        while let Ok(event) = event_rx.recv().await {
            // Fire all listeners in parallel, don't wait
            for plugin in &plugins {
                let plugin = plugin.clone();
                let event = event.clone();
                let ctx = ctx.clone();
                tokio::spawn(async move {
                    plugin.listen(&event, &ctx).await;
                });
            }
        }
    })
}

/// Run the interceptor loop, processing plugin requests from the channel.
///
/// This function:
/// - Listens on the `plugin_rx` channel for `PluginRequest` messages
/// - For `Intercept` requests: runs all plugins' `intercept()` methods sequentially
///   and returns the aggregated `PluginDecision`
/// - For `Emit` requests: broadcasts the event to the event bus
///
/// This is called once at agent startup to handle all plugin interception.
pub async fn run_interceptor_loop(
    mut plugin_rx: mpsc::Receiver<PluginRequest>,
    plugins: Vec<Arc<dyn AgentPlugin>>,
    ctx: RunContext,
) {
    while let Some(msg) = plugin_rx.recv().await {
        match msg {
            PluginRequest::Intercept { event, tx } => {
                let mut decision = PluginDecision::Continue;
                for plugin in &plugins {
                    match plugin.intercept(&event, &ctx).await {
                        PluginDecision::Continue => continue,
                        PluginDecision::Skip => {
                            decision = PluginDecision::Skip;
                            break;
                        }
                        PluginDecision::Abort(reason) => {
                            decision = PluginDecision::Abort(reason);
                            break;
                        }
                    }
                }
                let _ = tx.send(decision);
            }
            PluginRequest::Emit { event } => {
                let _ = ctx.event_tx.send(event);
            }
        }
    }
}

/// Configuration snapshot for audit/logging
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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
