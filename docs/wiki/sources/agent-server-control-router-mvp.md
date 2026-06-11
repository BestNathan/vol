---
type: source
source_type: code
date: 2026-06-10
ingested: 2026-06-10
tags: [agent-server, control-plane, routing, capability-index]
---

# Agent Server Control Router MVP

**Authors/Creators:** Nathan + Claude Code
**Date:** 2026-06-10
**Link:** `/Users/admin/Documents/learn/vol-agent/crates/vol-agent-server/src/control_plane/router.rs`

## TL;DR
Task 9 added the MVP `ControlRouter<'a>` in `vol-agent-server`, routing agent execution to an online node that advertises a matching agent capability by `agent_id` or `name`, or to the first online node with any agent when no explicit target is provided.

## Key Takeaways
- `ControlRouter<'a>` holds references to `NodeRegistry` and `CapabilityIndex`.
- `route_agent(target)` iterates capability snapshots and ignores nodes not currently `online` in `NodeRegistry`.
- Explicit targets match either `AgentCapability.agent_id` or `AgentCapability.name`.
- Untargeted routing selects the first online snapshot with at least one agent.
- The error string for no route is `capability_not_found`.
- Verification passed for the focused router test, package check, and formatting check.

## Detailed Summary
Task 9 created `crates/vol-agent-server/src/control_plane/router.rs` and exported it from `crates/vol-agent-server/src/control_plane/mod.rs`.

The MVP router is intentionally in-memory and read-only. It depends on existing Task 5 primitives: `NodeRegistry::get` for node liveness/status and `CapabilityIndex::list(None)` for node capability snapshots. It does not mutate leases, command stores, or run records.

The included unit test `route_agent_prefers_node_with_agent_capability` registers `node-a`, applies a capability snapshot containing the `coding` agent, constructs `ControlRouter::new(&nodes, &capabilities)`, and asserts that `route_agent(Some("coding"))` returns `node-a`.

Verification commands:
- `cargo test -p vol-agent-server route_agent_prefers_node_with_agent_capability`
- `cargo check -p vol-agent-server`
- `cargo fmt --check`

## Entities Mentioned
- [[vol-agent-server-crate]]: owns the new control-plane router module.
- [[vol-llm-agent-channel-crate]]: supplies shared `AgentCapability` and `CapabilitySnapshot` protocol models.

## Concepts Covered
- [[agent-server-control-data-plane]]: Task 9 implements the MVP distributed agent placement step in the staged control/data-plane plan.
- [[agent-router]]: contrasts local node routing with the new control-plane router that selects a node.

## Notes
This MVP returns a node id only. Later tasks can attach it to `control.command` dispatch, richer placement policies, stale capability handling, and lease/session-aware delivery.
