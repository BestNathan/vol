---
type: source
source_type: code
date: 2026-06-10
ingested: 2026-06-10
tags: [data-plane, snapshot, command, control-plane]
---

# Agent Server Data-Plane Snapshot/Command Skeletons

**Authors/Creators:** Claude (Nathan)
**Date:** 2026-06-10
**Link:** `crates/vol-agent-server/src/data_plane/snapshot.rs`, `crates/vol-agent-server/src/data_plane/command.rs`

## TL;DR
Task 8 of the agent-server control/data-plane plan: added `RuntimeCapabilitySource` trait with `snapshot_capabilities()`/`current_load()` and `StaticCapabilitySource` impl in `snapshot.rs`, plus `accept_control_command()` returning accepted `CommandAck` (with `run_id` for `SubmitAgent`) in `command.rs`.

## Key Takeaways
- `RuntimeCapabilitySource` is the runtime-agnostic facade for capability and load reporting.
- `StaticCapabilitySource` returns a fixed `CapabilitySnapshot` with the configured `node_id`, revision 1, and empty capability vectors.
- `FakeSource` test uses `AgentCapability { agent_id: "coding" }` and verifies `node_id`, `revision`, and `agents[0].agent_id`.
- `accept_control_command` accepts all commands unconditionally but only attaches `run_id` for `SubmitAgent` (as `format!("run_{}", command_id)`).
- Tests pass, `cargo check` passes, `cargo fmt --check` passes.

## Detailed Summary
The two new files are the Task 8 reporting primitives:

**`snapshot.rs`** defines:
- `RuntimeCapabilitySource` trait — two async methods: `snapshot_capabilities() -> CapabilitySnapshot` and `current_load() -> NodeLoad`.
- `StaticCapabilitySource` — struct with `node_id: String`, returns blank snapshot at revision 1 and zero load.
- `FakeSource` test — returns a snapshot with `node-a`, revision 1, one coding `AgentCapability`, verified by `fake_source_returns_snapshot` test.

**`command.rs`** defines:
- `accept_control_command(command: &ControlCommand) -> CommandAck` — always sets `accepted: true`, and for `ControlCommandOperation::SubmitAgent` sets `run_id: Some(format!("run_{}", command_id))`.

**`data_plane/mod.rs`** updated: `pub mod command; pub mod snapshot;` added alongside existing modules.

## Entities Mentioned
- [[vol-agent-server-crate]]: gains `snapshot` and `command` data-plane submodules

## Concepts Covered
- [[agent-server-control-data-plane]]: Task 8 snapshot and command skeleton primer
- [[control-payload-flat-jsonrpc-encoding-fix]]: `CommandAck` etc. already use the flat `CommandAck` struct from channel

## Notes
`StaticCapabilitySource` is a placeholder; Task 8 plan notes "Later tasks can replace StaticCapabilitySource with a real `DataPlaneServerCore` implementation." `accept_control_command` is intentionally simplistic for the MVP; a future implementation will route commands through actual dispatchers.