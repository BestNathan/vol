---
type: source
source_type: code
date: 2026-08-28
ingested: 2026-08-28
tags: [serve-loop, parallel, timeout, bugfix, jsonrpc, websocket]
---

# Data-Plane Serve Loop Parallelization + Frontend Timeout

**Authors/Creators:** Claude + user
**Date:** 2026-08-28
**Link:** git diff (vol-agent-server + frontend)

## TL;DR

Fixed three interconnected issues caused by the data-plane `serve_dyn` loop processing messages serially: (1) Sandboxes tab click sent request but got no response; (2) a stuck request blocked all subsequent requests on the same WebSocket connection; (3) confirmed all requests were processed sequentially per connection. Solution: parallelized the serve loop (one spawned task per message, mpsc channel serializing sends), added per-call timeouts to `JsonRpcClient`, reset `isRunning` on WS disconnect, and added warn-level logging for dropped agent events.

## Key Takeaways

- **`serve_dyn` was the bottleneck**: `while recv → handle.await → send` meant any slow handler (sandbox I/O, MCP warmup) blocked the entire connection
- **Agent dispatcher is intentionally serial per agent** (FIFO + busy lock) — that design is correct; the problem was the serve loop above it
- **`ConnectionHolder.listen()` silently dropped events** when the connection was dead — now logs at warn level
- **Frontend `isRunning` had no escape hatch** — if the backend never sent `agent_complete`, the input stayed disabled forever. Now resets on WS disconnect.
- **`JsonRpcClient.call()` had no timeout** — stuck requests hung indefinitely. Now has a 30s default with per-call override; `timeoutMs: 0` disables.

## Detailed Summary

### Backend: parallel serve loop (`data_plane/core.rs`)

- Clone `HandlerRegistry` at serve start (registry is now `#[derive(Clone)]` — handlers are Arc-backed, clone is cheap)
- Create `mpsc::unbounded_channel` for responses
- Spawn a dedicated sender task that drains the channel → `conn.send()`
- Each incoming message → `tokio::spawn` a handler task that calls `registry.dispatch(msg)` and enqueues responses
- Main loop only does `recv()` — never blocks on handler execution
- `drop(send_tx)` + `sender_task.await` on loop exit ensures clean shutdown

### Backend: ConnectionHolder logging (`connection_holder.rs`)

- `let _ = conn.send(msg).await` → `if let Err(e) = conn.send(msg).await { tracing::warn!(...) }`
- Makes dropped events visible in logs for debugging "no response" issues

### Frontend: JsonRpcClient timeout (`jsonrpc-client.ts`)

- Constructor accepts `defaultTimeoutMs` (default 30s); `call()` accepts per-call `timeoutMs`
- `timeoutMs: 0` disables timeout for long-running operations
- Timeout clears on resolve/reject; cleanup via `clearTimeout`
- Disconnect handler in `App.tsx` resets `isRunning`, `runningAgents`, `runMap`, `pendingSubmitAgent`, `approvalPending`

### Control plane: left serial

- Control plane operations are lightweight (register, heartbeat, node_list)
- `is_register` path has ordering-sensitive side effects (storing conn in `node_connections`)
- Parallelizing control plane is deferred — not needed for the reported issue

## Entities Mentioned

- [[vol-agent-server-crate]]: serve_dyn parallelized, ConnectionHolder logging added
- [[vol-llm-agent-protocol-crate]]: HandlerRegistry now Clone

## Concepts Covered

- [[agent-dispatcher]]: intentionally serial per agent (unchanged)
- [[connection-holder]]: now logs send failures at warn level
- [[json-rpc-websocket]]: per-call timeout added, default 30s
- [[agent-server-control-data-plane]]: data-plane serve loop now parallel
- [[frontend-auto-reconnect]]: disconnect now resets isRunning + run state

## Notes

- Pre-existing `sandbox_protocol_integration.rs` test failures (API mismatch from prior refactor) are unrelated to these changes
- 177 lib tests pass for vol-agent-server, 97 for protocol crate, 182 frontend tests pass
- Coverage gate passes at 80% for vol-agent-server
