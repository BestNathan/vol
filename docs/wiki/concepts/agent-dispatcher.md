---
type: concept
category: framework
tags: [dispatcher, queue, fifo, request]
created: 2026-05-05
updated: 2026-05-05
source_count: 1
---

# Agent Dispatcher

**Category:** Request queueing
**Related:** [[vol-llm-agent-channel-crate]], [[http-transport]], [[react-pattern]]

## Definition

`AgentDispatcher` wraps a `ReActAgent` with a FIFO request queue. It provides immediate return on `submit()` with a oneshot receiver for the result, while a background task processes requests one at a time.

## Key Points
- Created with `AgentDispatcher::new(agent)` which spawns a background `run_loop` task [[http-transport-impl]]
- `submit(request) -> Result<oneshot::Receiver<RunResult>>` pushes to queue and returns immediately [[http-transport-impl]]
- `cancel(req_id) -> bool` removes a request from the queue if still pending [[http-transport-impl]]
- `queue_len() -> usize` returns pending request count [[http-transport-impl]]
- `is_busy() -> bool` checks if currently executing a request [[http-transport-impl]]

## How It Works

1. Caller invokes `submit()`, which enqueues the request and notifies the background loop
2. Background loop (spawned once at creation) waits for notifications
3. On notification, acquires a "busy" mutex (ensures single execution)
4. Pops front of FIFO queue and executes `agent.run(&input)`
5. Wraps result in `RunResult` and sends it via the oneshot channel

The busy mutex ensures only one agent run executes at a time, preventing concurrent execution on the same agent instance.

## HTTP Transport Usage

Both blocking and SSE HTTP modes use `dispatcher.submit()` to queue the request. Blocking mode awaits the oneshot directly; SSE mode spawns a separate task to await it while streaming events.
