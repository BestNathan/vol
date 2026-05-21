---
type: concept
category: framework
tags: [dispatcher, queue, fifo, request]
created: 2026-05-05
updated: 2026-05-21
source_count: 2
---

# Agent Dispatcher

**Category:** Request queueing
**Related:** [[vol-llm-agent-channel-crate]], [[http-transport]], [[react-pattern]], [[agent-router]], [[jsonrpc-server-handler]]

## Definition

`AgentDispatcher` wraps a `ReActAgent` with a FIFO request queue. It provides immediate return on `submit()` with a oneshot receiver for the result, while a background task processes requests one at a time.

## Key Points
- Created with `AgentDispatcher::new(agent)` which spawns a background `run_loop` task [[http-transport-impl]]
- `submit(request) -> Result<oneshot::Receiver<RunResult>>` pushes to queue and returns immediately [[http-transport-impl]]
- `cancel(run_id) -> bool` removes a request from the queue if still pending [[run-id-unification]]
- `queue_len() -> usize` returns pending request count [[http-transport-impl]]
- `is_busy() -> bool` checks if currently executing a request [[http-transport-impl]]

## How It Works

1. Caller invokes `submit()`, which enqueues the request and notifies the background loop
2. Background loop (spawned once at creation) waits for notifications
3. On notification, acquires a "busy" mutex (ensures single execution)
4. Pops front of FIFO queue and executes `agent.run_with_id(&input, request.run_id)`
5. Wraps result in `RunResult { run_id, target_id, response }` and sends it via the oneshot channel

The busy mutex ensures only one agent run executes at a time, preventing concurrent execution on the same agent instance.

## HTTP Transport Usage

Both blocking and SSE HTTP modes use `dispatcher.submit()` to queue the request. Blocking mode awaits the oneshot directly; SSE mode spawns a separate task to await it while streaming events. Legacy HTTP request fields named `req_id` are bridged into `AgentRequest.run_id` at the transport boundary [[run-id-unification]].

## Related Concepts

- [[run-id-unification]]: Dispatcher request/result identity now uses `run_id`.
- [[agent-router]]: Routes requests to dispatchers and cancels across dispatchers by run id.
- [[run-context]]: Receives the dispatcher's chosen run id through `ReActAgent::run_with_id()`.
