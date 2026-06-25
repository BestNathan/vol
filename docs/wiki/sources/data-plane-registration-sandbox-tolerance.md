---
type: source
source_type: code
date: 2026-06-17
ingested: 2026-06-17
tags: [data-plane, registration, sandbox, fault-tolerance, websocket, heartbeat]
---

# Data-Plane Remote Registration and Sandbox Fault Tolerance

**Authors/Creators:** Claude (Nathan)
**Date:** 2026-06-17
**Link:** `crates/vol-llm-sandbox/src/registry.rs`, `crates/vol-agent-server/src/app.rs`

## TL;DR
Two changes: (1) `SandboxRegistry::load()` now wraps per-sandbox errors in `tracing::warn!` + `continue` instead of propagating with `?`, so individual failing sandbox configs (bad TOML, missing known_hosts, etc.) no longer crash the server. (2) `app.rs` gained a `spawn_data_plane_connector()` function that connects a standalone data-plane to a remote control-plane via WebSocket, sends `control.register` + `capability_snapshot` on connect, maintains periodic heartbeats, and auto-reconnects with exponential backoff (1s to 60s) on disconnect.

## Key Takeaways
- Sandbox fault tolerance: per-file errors are now caught, logged, and the loop continues — server starts even when some sandbox configs are invalid.
- Remote registration: `spawn_data_plane_connector()` is called from `app::run()` when `control_plane=false`, `data_plane=true`, and `config.data_plane.control_url` is set.
- Registration sequence: connect via `tokio-tungstenite`, send JSON-RPC `control.register`, then send `capability_snapshot` with live agent IDs from `data_core.list_agent_ids()`.
- Heartbeat loop: periodic `control.heartbeat` notifications with node_id, status="online", and load (running/queued counts).
- Reconnect: exponential backoff from 1s up to 60s on any connection failure; backoff resets to 1s on successful connect.
- Read loop: messages from control-plane are received (but not yet dispatched to handlers); read errors or stream close trigger reconnect.

## Detailed Summary

### Sandbox Fault Tolerance
`SandboxRegistry::load()` in `crates/vol-llm-sandbox/src/registry.rs` reads sandbox TOML files from a directory. Previously, any parse error, missing SSH config, or `sandbox.start()` failure would propagate via `?` and cause the entire server to crash. Now each individual sandbox operation is wrapped:
- `read_dir` entry errors => `tracing::warn!` + `continue`
- File read failures => `tracing::warn!` + `continue`
- TOML parse failures => `tracing::warn!` + `continue`
- `name == "local"` (reserved) => `tracing::warn!` + `continue`
- Duplicate names => `tracing::warn!` + `continue`
- Missing required config sections (SSH, Firecracker) => `tracing::warn!` + `continue`
- SSH sandbox creation failures => `tracing::warn!` + `continue`
- `sandbox.start()` failures => `tracing::warn!` + `continue`
- Unknown sandbox types => `tracing::warn!` + `continue`

### Remote Data-Plane Registration
`spawn_data_plane_connector()` in `crates/vol-agent-server/src/app.rs` spawns a `tokio::spawn` task that:

1. **Connects** to `control_url` via `tokio_tungstenite::connect_async` with exponential backoff.
2. **Registers** by sending an `AgentServerMessage` with `Operation::Control(ControlOperation::Register)` and `Payload::Control(ControlPayload::Register(...))` — includes `node_id`, `name`, and `version`.
3. **Sends capability snapshot** by querying `data_core.list_agent_ids().await` for live agents, then building a `CapabilitySnapshot` with `node_id`, `revision: 1`, and the agent list.
4. **Maintains heartbeats** on a timer interval (`heartbeat_secs` from config) — sends `NodeHeartbeat` with `node_id`, `status: "online"`, and `NodeLoad { running: 0, queued: 0 }`.
5. **Reads messages** from the WebSocket; errors or stream closure trigger reconnect.
6. **Auto-reconnects** with exponential backoff (1s initial, 2x per attempt, max 60s).

The function is called from `app::run()` when all of these hold:
- `control_plane_enabled == false`
- `data_plane_enabled == true`
- `config.data_plane.control_url.is_some()`

## Entities Mentioned
- [[vol-llm-sandbox-crate]]: modified `SandboxRegistry::load()` for fault-tolerant sandbox init
- [[vol-agent-server-crate]]: added `spawn_data_plane_connector()` and remote registration call in `app::run()`

## Concepts Covered
- [[agent-server-control-data-plane]]: implemented remote data-plane registration and reconnect behavior
- (new concept section for sandbox fault tolerance)

## Notes
- The sandbox fault tolerance also handles the edge case where SSH `known_hosts` is missing in container environments.
- The registration heartbeat loop does not expect server responses (notifications are fire-and-forget).
- The capability snapshot currently reports all agents from `list_agent_ids()` with hardcoded `status: "idle"` — a future improvement could query actual agent states.
