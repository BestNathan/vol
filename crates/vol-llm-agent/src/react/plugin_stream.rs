//! Plugin interceptor and listener utilities.

use super::plugin::{AgentPlugin, PluginDecision};
use super::run_context::{PluginContext, PluginRequest, RunContext};
use super::{AgentError, AgentResponse, AgentStreamEvent, AgentStreamReceiver};
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc};
use vol_tracing::TracedEvent;

/// Spawn a listener task that subscribes to the event bus and calls
/// `plugin.listen()` on all events (fire-and-forget, parallel execution).
///
/// # Shutdown Behavior
///
/// The listener task exits when the broadcast channel is closed, which happens
/// when all `RunContext` instances holding senders are dropped. The shutdown
/// sequence is:
/// 1. Agent completes → drops its `RunContext` (1 sender dropped)
/// 2. Interceptor exits (plugin_rx closed) → drops its `RunContext` (1 sender dropped)
/// 3. Listener sees `RecvError` (all senders dropped) → exits
///
/// This ensures all pending `plugin.listen()` calls have time to complete.
///
/// # Sender Reference Counting
///
/// This function accepts a `PluginContext` which does NOT contain sender references.
/// The broadcast subscription is created separately and passed in.
/// This ensures the broadcast channel sender count remains at 2 (agent + interceptor)
/// and drops to 0 when both drop their RunContext instances.
pub fn spawn_listener_task(
    plugins: Vec<Arc<dyn AgentPlugin>>,
    plugin_ctx: PluginContext,
    mut event_rx: broadcast::Receiver<TracedEvent<AgentStreamEvent>>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        while let Ok(traced_event) = event_rx.recv().await {
            let event = traced_event.value();
            // Fire all listeners in parallel
            for plugin in &plugins {
                let plugin = plugin.clone();
                let event = event.clone();
                let plugin_ctx = plugin_ctx.clone();

                tokio::spawn(async move {
                    plugin.listen(&event, &plugin_ctx).await;
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
///
/// # Sender Reference Counting
///
/// This function accepts only the broadcast sender and PluginContext, not a full RunContext.
/// PluginContext does NOT contain sender references, so the broadcast channel sender
/// count remains at 1 (just the agent).
pub async fn run_interceptor_loop(
    mut plugin_rx: mpsc::Receiver<PluginRequest>,
    plugins: Vec<Arc<dyn AgentPlugin>>,
    event_tx: broadcast::Sender<TracedEvent<AgentStreamEvent>>,
    plugin_ctx: PluginContext,
) {
    while let Some(msg) = plugin_rx.recv().await {
        match msg {
            PluginRequest::Intercept { event, tx } => {
                // Run plugins sequentially - first non-Continue decision wins
                let mut decision = PluginDecision::Continue;
                for plugin in &plugins {
                    match plugin.intercept(event.value(), &plugin_ctx).await {
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
                // Only Emit uses the event_tx sender
                let _ = event_tx.send(event);
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
    _response: AgentResponse,
    ctx: RunContext,
    _run_id: String,
) -> Result<AgentStreamReceiver, AgentError> {
    let (tx, rx) = mpsc::channel(10);

    tokio::spawn(async move {
        let _ = tx
            .send(Ok(AgentStreamEvent::AgentStart {
                input: ctx.user_input,
            }))
            .await;

        let _ = tx.send(Ok(AgentStreamEvent::AgentComplete)).await;
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
        let _ = tx
            .send(Ok(AgentStreamEvent::AgentStart {
                input: ctx.user_input.clone(),
            }))
            .await;

        let _ = tx.send(Ok(AgentStreamEvent::AgentComplete)).await;
    });

    Ok(AgentStreamReceiver::new(rx))
}
