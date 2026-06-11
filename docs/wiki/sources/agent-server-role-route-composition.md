---
type: source
source_type: code
date: 2026-06-10
ingested: 2026-06-10
tags: [agent-server, routes, control-plane, data-plane, json-rpc]
---

# Agent Server Role Route Composition

**Authors/Creators:** Nathan + Claude
**Date:** 2026-06-10
**Link:** `/Users/admin/Documents/learn/vol-agent/crates/vol-agent-server/src/app.rs`, `/Users/admin/Documents/learn/vol-agent/crates/vol-agent-server/src/routes.rs`, `/Users/admin/Documents/learn/vol-agent/crates/vol-agent-server/src/main.rs`

## TL;DR
Task 7 wires the previously introduced control/data-plane roles into active server startup. `vol-agent-server` now uses a pure `ws_owner(control_plane, data_plane)` decision to determine `/ws` ownership, builds `ControlPlaneServerCore` and/or `DataPlaneServerCore` from config, mounts JSON-RPC WebSocket routes accordingly, and delegates binary startup from `main.rs` to `app::run` after config/tracing setup.

## Key Takeaways
- `routes.rs` defines `WsOwner::{DataPlane, ControlPlane}` and `ws_owner(control_plane, data_plane)` with unit tests for data-plane-only, control-plane-enabled, and both-disabled cases.
- `app::run` expands `~` paths, starts from `routes::base_router()`, builds `ControlPlaneServerCore` when the control role is enabled, and builds/discovers `DataPlaneServerCore` when the data role is enabled.
- `/ws` is owned by the control plane whenever `control_plane=true`; otherwise `/ws` is owned by the data plane when available.
- The control-plane node endpoint is mounted from `config.control_plane.node_ws_path`, defaulting to `/control/v1/ws`; the client path comes from `config.control_plane.client_ws_path`, defaulting to `/ws`.
- `main.rs` now preserves config loading and tracing setup while delegating runtime composition and bind/serve to `app::run(config)`.

## Detailed Summary
Task 7 completes the first active role-based composition pass for [[vol-agent-server-crate]]. The route decision is intentionally pure and covered by `cargo test -p vol-agent-server routes::tests`, making `/ws` ownership explicit before involving Axum or server cores.

In `app.rs`, startup now validates route ownership with `routes::ws_owner`, builds the base health router, conditionally creates `ControlPlaneServerCore::new(Arc::new(ControlPlaneState::new()))`, and conditionally creates `DataPlaneServerCore::builder(...).with_task_store_config(...).with_session_store_config(...).build().await` followed by `discover_agents().await`. It then mounts `JsonRpcServer` routes by role: control mode mounts the control core on the configured client and node paths, while standalone data-plane mode mounts the data core on the configured client path.

This task deliberately does not implement Task 8 follow-ups such as `DataPlaneReporter`, loopback registration, snapshots, or command execution. Combined mode currently builds both cores and gives `/ws` ownership to the control core; actual local data-plane registration remains a future task.

## Entities Mentioned
- [[vol-agent-server-crate]]: Owns role-aware startup, route ownership tests, and concrete control/data-plane core composition.
- [[vol-llm-agent-channel-crate]]: Provides the generic `JsonRpcServer` transport consumed by both server cores.

## Concepts Covered
- [[agent-server-control-data-plane]]: Task 7 implements the route composition step described by the architecture and plan.
- [[jsonrpc-transport]]: The configured JSON-RPC WebSocket server path is used for both control and data-plane cores.

## Notes
Verification passed:
- `cargo test -p vol-agent-server routes::tests`
- `cargo check -p vol-agent-server`
- `cargo fmt --check`

`cargo check` emitted existing workspace warnings outside the Task 7 files, but no errors.
