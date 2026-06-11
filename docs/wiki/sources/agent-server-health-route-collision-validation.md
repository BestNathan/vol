---
type: source
source_type: code
date: 2026-06-10
ingested: 2026-06-10
tags: [agent-server, config, health, routing, axum]
---

# Agent Server Health Route Collision Validation

**Authors/Creators:** Nathan, Claude
**Date:** 2026-06-10
**Link:** `/Users/admin/Documents/learn/vol-agent/crates/vol-agent-server/src/config.rs`

## TL;DR
`ServerConfig::validate` now rejects WebSocket paths that equal `/health` in the active server modes, preventing Axum duplicate-route panics because `routes::base_router()` always registers the health endpoint.

## Key Takeaways
- `control_plane.client_ws_path = "/health"` is rejected when the control-plane role is enabled.
- `control_plane.node_ws_path = "/health"` is rejected when the control-plane role is enabled.
- Data-plane-only mode also rejects `control_plane.client_ws_path = "/health"`, because that configured client path is used for the standalone data-plane WebSocket route.
- The regression test `test_reject_health_route_ws_path_collision` covers all three rejection cases.
- Verification passed: `cargo test -p vol-agent-server -- config`, `cargo check -p vol-agent-server`, and `cargo fmt --check`.

## Detailed Summary
Task 7 route composition made `vol-agent-server` mount role-specific WebSocket endpoints using configured paths while `routes::base_router()` registers `/health` unconditionally. If a user configured a WebSocket path as `/health`, Axum would see duplicate routes and panic during startup rather than returning a configuration error.

The fix adds early validation after the existing both-roles-disabled guard in `ServerConfig::validate`. Control-plane mode checks both `client_ws_path` and `node_ws_path`; standalone data-plane mode checks `client_ws_path`, which is the path used to mount the data-plane JSON-RPC WebSocket. The validation returns explicit `String` errors before runtime/task/session store validation proceeds.

## Entities Mentioned
- [[vol-agent-server-crate]]: owns `ServerConfig`, role configuration, `/health`, and role-based route composition.

## Concepts Covered
- [[agent-server-control-data-plane]]: route composition now includes configuration-time rejection for health route collisions.

## Notes
The optional warning-only node WebSocket dedup check was not needed for this quality fix; the implemented hard failures target the Axum panic caused by duplicate `/health` routes.
