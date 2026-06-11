---
type: source
source_type: design
date: 2026-06-10
ingested: 2026-06-10
tags: [agent-server, control-plane, data-plane, addendum, json-rpc, migration, capability-index]
---

# Agent Server Control/Data Plane Addendum

**Authors/Creators:** Claude
**Date:** 2026-06-10
**Link:** `docs/superpowers/specs/2026-06-10-agent-server-control-data-plane-addendum.md`
**Feishu/Lark:** https://my.feishu.cn/docx/Rk11ddyFJoC6q2x8HOjcrwuQn4c

## TL;DR

Design addendum for [[agent-server-control-data-plane-architecture]] that refines implementation-critical details: endpoint role allowlists, `control.command` vs agent run semantics, capability snapshot revision rules, `NodeRecord` vs `NodeSession`, combined-mode lifecycle, runtime capability facade, capability policy hints, subscription shape, error-code ownership, migration constraints, and boundary verification tests.

## Key Takeaways

- `/ws` and `/control/v1/ws` use the same JSON-RPC wire protocol but different endpoint roles and method allowlists.
- `control.command` response means accepted/rejected; long-running lifecycle and terminal status should flow through `control.event` and `control.command_result`.
- `CommandRecord` and `RunRecord` are separate because not every command creates an agent run.
- Capability full snapshots use per-node monotonic revision and replace semantics; deltas require `base_revision`.
- `NodeRecord` stores node state; `NodeSession` stores live connection state and generation.
- Combined mode should initially use loopback JSON-RPC registration to exercise the real node endpoint path.
- `RuntimeCapabilitySource` should hide runtime internals from reporter code.
- Protocol/domain error code vocabulary belongs in [[vol-llm-agent-channel-crate]]; server fills contextual detail.
- Moving current `AgentServerCore` out of channel is workspace-internal breaking work and should not be papered over by channel re-exporting server types.

## Detailed Summary

The addendum fills gaps left by the high-level architecture. It makes endpoint roles explicit: client `/ws` may call catalog/execution/status methods, while node `/control/v1/ws` may call registration/reporting methods and receive `control.command` requests. Wrong-role method calls should return `method_not_allowed_for_role`.

It separates command lifecycle from run lifecycle. `control.command` is a control-plane instruction to a node and should return quickly with accepted/rejected status. Longer agent execution progress should use notifications. `CommandRecord` tracks all commands, while `RunRecord` tracks agent runs only.

Capability consistency is defined by node-local monotonic revisions. Full snapshots replace all capabilities for that node. Deltas must include `base_revision`, and stale deltas should be rejected in favor of requesting a full snapshot.

Node state and live connection state are separated. `NodeRecord` can later be persisted, while `NodeSession` is an active connection handle with a generation counter. Reconnect replaces session state and requires a new full snapshot.

The addendum also clarifies combined-mode lifecycle, runtime snapshot facade, capability policy hints for sensitive tools, future `control.subscribe` topics, error-code ownership, migration constraints, and boundary tests.

## Entities Mentioned

- [[vol-agent-server-crate]]: owns concrete data/control cores, lifecycle, stores, and role composition.
- [[vol-llm-agent-channel-crate]]: owns protocol, JSON-RPC codec, service abstraction, and shared error vocabulary.
- [[vol-llm-runtime-crate]]: remains execution resource owner and source of capability snapshots.

## Concepts Covered

- [[agent-server-control-data-plane]]: refined with endpoint roles, command/run semantics, snapshot consistency, session lifecycle, and migration/testing constraints.
- [[agent-router]]: remains local data-plane execution routing under distributed `ControlRouter`.
- [[tool-registry]]: source of tool capability and sensitivity/policy hints.
- [[mcp-manager-lifecycle]]: source of MCP capabilities and reconnect-driven snapshot changes.
- [[runtime-task-store-configuration]]: remains runtime-owned, not control-plane owned.
- [[runtime-session-store-configuration]]: remains runtime-owned, not control-plane owned.

## Notes

Open decisions left in the addendum:

1. Whether `control.command_result` is always notification-only or sometimes folded into synchronous `control.command` result for short operations.
2. Whether combined mode later adds an in-process shortcut in addition to loopback JSON-RPC.
3. Whether `control.subscribe` is needed in MVP or can wait until UI requires node/capability event streams.
