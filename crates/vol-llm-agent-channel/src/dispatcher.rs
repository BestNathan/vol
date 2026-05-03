//! Agent dispatcher for single-agent request queueing.

use std::collections::VecDeque;
use std::sync::Arc;

use tokio::sync::{Mutex, Notify, oneshot};
use vol_llm_agent::ReActAgent;

use crate::error::ChannelError;
use crate::request::{AgentRequest, PendingRequest, RunResult};

/// Internal state shared between the dispatcher and its background loop.
struct DispatcherState {
    queue: Mutex<VecDeque<PendingRequest>>,
    notify: Notify,
    busy: tokio::sync::Mutex<()>,
}

impl DispatcherState {
    fn new() -> Self {
        Self {
            queue: Mutex::new(VecDeque::new()),
            notify: Notify::new(),
            busy: tokio::sync::Mutex::new(()),
        }
    }
}

/// Wraps a `ReActAgent` with a FIFO request queue.
///
/// Clone to share across tasks (internally Arc-backed).
/// Each dispatcher spawns one background task that processes requests one at a time.
#[derive(Clone)]
pub struct AgentDispatcher {
    agent: Arc<ReActAgent>,
    state: Arc<DispatcherState>,
}

impl AgentDispatcher {
    /// Create a new dispatcher for the given agent.
    ///
    /// The dispatcher starts a background task that processes queued requests FIFO.
    pub fn new(agent: ReActAgent) -> Self {
        let state = Arc::new(DispatcherState::new());
        let agent = Arc::new(agent);

        // Spawn the background execution loop
        tokio::spawn(Self::run_loop(agent.clone(), state.clone()));

        Self {
            agent,
            state,
        }
    }

    /// Submit a request. Returns immediately with a receiver for the result.
    pub fn submit(&self, request: AgentRequest) -> Result<oneshot::Receiver<RunResult>, ChannelError> {
        let (tx, rx) = oneshot::channel();
        let pending = PendingRequest { request, tx };

        // Push to queue and notify the background loop.
        let state = self.state.clone();
        tokio::task::spawn(async move {
            state.queue.lock().await.push_back(pending);
            state.notify.notify_one();
        });

        Ok(rx)
    }

    /// Cancel a queued request. Returns false if already executing or completed.
    pub async fn cancel(&self, req_id: &str) -> bool {
        let mut queue = self.state.queue.lock().await;

        if let Some(pos) = queue.iter().position(|p| p.request.req_id == req_id) {
            let pending = queue.remove(pos).unwrap();
            // Drop the sender without sending — the receiver will get RecvError.
            drop(pending.tx);
            true
        } else {
            false
        }
    }

    /// Number of requests waiting in the queue.
    pub async fn queue_len(&self) -> usize {
        self.state.queue.lock().await.len()
    }

    /// Whether the dispatcher is currently executing a request.
    pub fn is_busy(&self) -> bool {
        // The busy lock is held by the run_loop while executing.
        // try_lock succeeds means NOT busy (nobody holds it).
        self.state.busy.try_lock().is_err()
    }

    /// Background loop that processes requests FIFO.
    async fn run_loop(agent: Arc<ReActAgent>, state: Arc<DispatcherState>) {
        loop {
            // Wait for a notification.
            state.notify.notified().await;

            // Acquire the busy lock — ensures only one request runs at a time.
            let _busy_permit = state.busy.lock().await;

            // Pop the next request from the front of the queue.
            let pending = {
                let mut queue = state.queue.lock().await;
                queue.pop_front()
            };

            let Some(pending) = pending else {
                // Queue was empty (race between notify and pop_front).
                // The notify was consumed, wait for the next one.
                continue;
            };

            // Execute the agent run.
            let result = agent.run(&pending.request.input).await;

            let run_result = RunResult {
                req_id: pending.request.req_id.clone(),
                target_id: pending.request.target_id.clone(),
                run_id: result.as_ref().ok().map(|r| r.run_id.clone()),
                response: result.map_err(|e| ChannelError::AgentError(e.to_string())),
            };

            // Send result back. If the caller cancelled, receiver is gone — fine.
            let _ = pending.tx.send(run_result);
        }
    }
}
