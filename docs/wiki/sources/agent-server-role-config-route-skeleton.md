---
type: source
source_type: code
date: 2026-06-10
ingested: 2026-06-10
tags: [agent-server, control-plane, data-plane, config, routes, health]
---

# Agent Server Role Config and Route Skeleton

**Authors/Creators:** Claude
**Date:** 2026-06-10
**Link:** `crates/vol-agent-server/src/config.rs`, `crates/vol-agent-server/src/app.rs`, `crates/vol-agent-server/src/routes.rs`, `crates/vol-agent-server/src/health.rs`

## TL;DR

Task 3 of [[agent-server-control-data-plane-implementation-plan]] added role-aware server configuration to [[vol-agent-server-crate]] and introduced a minimal Axum route skeleton with `/health`, without wiring the new app runner into `main.rs` yet.

## Key Takeaways

- `ServerConfig` now parses `[server.roles]`, `[control_plane]`, and `[data_plane]` TOML sections.
- Default roles are `control_plane=false` and `data_plane=true`, preserving standalone data-plane behavior until later route composition work.
- `ServerConfig::validate` rejects configs where both roles are disabled with `at least one server role must be enabled`.
- `health.rs`, `routes.rs`, and `app.rs` provide the minimal route skeleton for future role composition.
- TDD verification covered both parsing role config and rejecting the invalid all-disabled role mode.

## Detailed Summary

`crates/vol-agent-server/src/config.rs` now includes `ServerRoles`, `ControlPlaneSection`, and `DataPlaneSection`. The control-plane section contains auth token and endpoint/lease defaults for `/ws`, `/control/v1/ws`, 90-second leases, and 15-second scans. The data-plane section contains node identity, control URL/auth token, 15-second heartbeat, and snapshot-on-connect defaults.

The new route skeleton is intentionally small: `health::health` returns `Json(HealthResponse { status: "ok" })`, `routes::base_router()` mounts `/health`, and `app::run(ServerConfig)` binds the base router to the configured host/port. `main.rs` only declares the modules; it does not switch startup to the new app runner in this task.

Verification used the valid Cargo equivalent `cargo test -p vol-agent-server roles` because Cargo accepts only one test-name filter after package options. The initial RED run failed because `ServerSection.roles` and `ServerConfig.control_plane` did not exist. The final focused tests passed, and `cargo check -p vol-agent-server` plus `cargo fmt --check` passed.

## Entities Mentioned

- [[vol-agent-server-crate]]: owns the new role config sections and route skeleton.

## Concepts Covered

- [[agent-server-control-data-plane]]: role config and base route skeleton for the staged control/data-plane architecture.

## Notes

This task deliberately does not implement Task 7 role route composition, does not move data-plane core behavior, and does not wire `app::run` into `main.rs`.
