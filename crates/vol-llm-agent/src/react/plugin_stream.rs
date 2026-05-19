//! Plugin interceptor and listener utilities.

use super::plugin::{AgentPlugin, PluginDecision};
use super::run_context::{PluginRequest, RunContext};
use super::{AgentError, AgentResponse, AgentStreamEvent, AgentStreamReceiver};
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc};
use vol_tracing::TracedEvent;

/// Spawn a listener task that subscribes to the event bus and calls
/// `plugin.listen()` on all events.
///
/// # Shutdown Behavior
///
/// The listener task exits when the broadcast channel is closed, then waits for
/// every in-flight `plugin.listen()` task to finish before returning.
pub fn spawn_listener_task(
    plugins: Vec<Arc<dyn AgentPlugin>>,
    ctx: RunContext,
    mut event_rx: broadcast::Receiver<TracedEvent<AgentStreamEvent>>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut listen_tasks = tokio::task::JoinSet::new();
        while let Ok(traced_event) = event_rx.recv().await {
            let event = traced_event.value();
            for plugin in &plugins {
                let plugin = plugin.clone();
                let event = event.clone();
                let ctx = ctx.clone();

                listen_tasks.spawn(async move {
                    plugin.listen(&event, &ctx).await;
                });
            }
        }

        while let Some(result) = listen_tasks.join_next().await {
            if let Err(join_err) = result {
                tracing::warn!(%join_err, "Plugin listener task panicked");
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
                // Run plugins sequentially - first non-Continue decision wins
                let mut decision = PluginDecision::Continue;
                for plugin in &plugins {
                    match plugin.intercept(event.value(), &ctx).await {
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
                ctx.emit_traced(event).await;
            }
        }
    }
}

/// Configuration snapshot for audit/logging
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AgentConfigSnapshot {
    pub max_iterations: u32,
    pub max_history_messages: usize,
    pub context_builder_summary: String,
}

impl From<&super::AgentConfig> for AgentConfigSnapshot {
    fn from(config: &super::AgentConfig) -> Self {
        let contributor_names: Vec<&str> = config.context_builder.contributor_names();
        Self {
            max_iterations: config.def.as_ref()
                .and_then(|d| d.max_iterations)
                .unwrap_or(5),
            max_history_messages: config.def.as_ref()
                .and_then(|d| d.max_history_messages)
                .unwrap_or(20),
            context_builder_summary: contributor_names.join(", "),
        }
    }
}

impl Default for AgentConfigSnapshot {
    fn default() -> Self {
        Self {
            max_iterations: 5,
            max_history_messages: 20,
            context_builder_summary: String::new(),
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
            .send(Ok(AgentStreamEvent::agent_start(ctx.user_input)))
            .await;

        let _ = tx.send(Ok(AgentStreamEvent::agent_complete())).await;
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
            .send(Ok(AgentStreamEvent::agent_start(ctx.user_input.clone())))
            .await;

        let _ = tx.send(Ok(AgentStreamEvent::agent_complete())).await;
    });

    Ok(AgentStreamReceiver::new(rx))
}
