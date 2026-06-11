---
type: source
source_type: report
date: 2026-06-10
ingested: 2026-06-10
tags: [agent-server, boundary-tests, role-modes, control-plane, data-plane]
---

# Agent Server Boundary and Role-Mode Verification

**Authors/Creators:** Claude Code
**Date:** 2026-06-10
**Link:** /Users/admin/Documents/learn/vol-agent/crates/vol-agent-server/tests/role_modes.rs; /Users/admin/Documents/learn/vol-agent/scripts/check-agent-boundaries.sh

## TL;DR
Task 10 added final boundary and role-mode verification for the agent-server control/data-plane implementation. A shell boundary check fails if either [[vol-llm-agent-channel-crate]] or [[vol-llm-runtime-crate]] depends on [[vol-agent-server-crate]], and an integration test verifies `/ws` ownership plus config rejection when both server roles are disabled.

## Key Takeaways
- `/Users/admin/Documents/learn/vol-agent/scripts/check-agent-boundaries.sh` runs `cargo tree` checks for `vol-llm-agent-channel` and `vol-llm-runtime` and prints `agent boundary checks passed` on success.
- The boundary script is executable and prevents lower-level crates from depending upward on `vol-agent-server`.
- `/Users/admin/Documents/learn/vol-agent/crates/vol-agent-server/tests/role_modes.rs` verifies `ws_owner(false, true) == DataPlane` for standalone data-plane mode.
- The same integration test verifies `ws_owner(true, false)` and `ws_owner(true, true) == ControlPlane`, preserving control-plane priority for `/ws`.
- TOML config validation rejects `[server.roles] control_plane = false` and `data_plane = false` through `ServerConfig::load`.
- No `lib.rs` export changes were required because `config` and `routes` were already exported.

## Detailed Summary
Task 10 converted previously local role-routing assertions into integration-level coverage and added an explicit dependency-boundary script. The integration test imports `ServerConfig`, `ws_owner`, and `WsOwner` through the crate library API, which validates that the minimal public surface already supports external tests without over-exporting internals.

The boundary script checks two architectural invariants from the control/data-plane split:

1. `vol-llm-agent-channel` remains the protocol/transport abstraction crate and must not depend on `vol-agent-server`.
2. `vol-llm-runtime` remains the runtime resource owner and capability source and must not depend on `vol-agent-server`.

Verification passed:
- `/Users/admin/Documents/learn/vol-agent/scripts/check-agent-boundaries.sh`
- `cargo test -p vol-agent-server --test role_modes`
- `cargo check -p vol-agent-server`
- `cargo fmt --check`

`cargo check` completed without errors, with existing warnings in unrelated crates.

## Entities Mentioned
- [[vol-agent-server-crate]]: owns the boundary script target tests and role-mode integration tests.
- [[vol-llm-agent-channel-crate]]: checked to ensure it does not depend on `vol-agent-server`.
- [[vol-llm-runtime-crate]]: checked to ensure it does not depend on `vol-agent-server`.

## Concepts Covered
- [[agent-server-control-data-plane]]: Task 10 verifies role-mode behavior and crate dependency boundaries for the split architecture.

## Notes
The working tree already contained many unrelated changes from earlier implementation tasks. Task 10 code changes were limited to the new script and integration test file.
