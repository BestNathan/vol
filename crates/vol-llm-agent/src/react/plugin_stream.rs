//! Plugin interceptor and listener utilities.

use super::plugin::{AgentPlugin, PluginDecision};
use super::run_context::{PluginRequest, RunContext};
use super::{AgentError, AgentResponse, AgentStreamEvent, AgentStreamReceiver};
use futures::FutureExt;
use std::sync::Arc;
use tokio::sync::mpsc;

/// Spawn one listener task per plugin, each subscribing to the event broadcast
/// channel and processing events sequentially.
///
/// Each task exits when the broadcast channel closes (all senders dropped),
/// guaranteeing all buffered events are processed before exit.
///
/// Returns a `JoinSet` that tracks all listener tasks for await.
#[allow(clippy::expect_used)]
pub fn spawn_listener_tasks(
    plugins: Vec<Arc<dyn AgentPlugin>>,
    ctx: RunContext,
) -> tokio::task::JoinSet<()> {
    let mut join_set = tokio::task::JoinSet::new();
    for plugin in plugins {
        let mut event_rx = ctx
            .event_tx
            .as_ref()
            .expect("event_tx should be set")
            .subscribe();
        let plugin = plugin.clone();
        let ctx = ctx.without_event_senders();
        join_set.spawn(async move {
            while let Ok(traced_event) = event_rx.recv().await {
                let event = traced_event.value();
                let _ = std::panic::AssertUnwindSafe(plugin.listen(event, &ctx))
                    .catch_unwind()
                    .await;
            }
        });
    }
    join_set
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
