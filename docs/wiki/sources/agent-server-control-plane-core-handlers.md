---
type: source
source_type: code
date: 2026-06-10
ingested: 2026-06-10
tags: [agent-server, control-plane, handlers, json-rpc, task-6]
---

# Agent Server Control-Plane Core and Handlers

**Authors/Creators:** Nathan, Claude
**Date:** 2026-06-10
**Link:** `crates/vol-agent-server/src/control_plane/`

## TL;DR

Task 6 adds the in-memory control-plane server core and first concrete control-plane handlers to [[vol-agent-server-crate]]. `ControlPlaneServerCore` owns shared `ControlPlaneState`, registers handlers through channel-owned `HandlerRegistry`, dispatches `AgentServerMessage`s, and implements `JsonRpcMessageService` for JSON-RPC WebSocket connection loops.

## Key Takeaways

- Added `ControlPlaneServerCore` at `crates/vol-agent-server/src/control_plane/core.rs`.
- Added handler modules under `crates/vol-agent-server/src/control_plane/handlers/`.
- `ControlHandler` supports `control.register`, `control.heartbeat`, `control.capability_snapshot`, `control.event`, `control.capability_delta`, and `control.command_result` MVP behavior.
- `NodeHandler` supports `control.node_list` and `control.node_get`.
- `CapabilityHandler` supports `control.capability_list` with optional `node_id` filtering.
- TDD coverage added `control_register_creates_node`; the RED compile failure was observed before implementation.

## Detailed Summary

The implementation creates `ControlPlaneServerCore` with:

- `state: Arc<ControlPlaneState>` for registry, capability index, events, commands, and runs.
- `HandlerRegistry` containing `ControlHandler`, `NodeHandler`, and `CapabilityHandler`.
- `new(state)` for handler registration.
- `handle(message)` for registry dispatch.
- `JsonRpcMessageService::serve_connection` receive/handle/send loop over channel-owned `Connection`.

`ControlHandler` updates existing Task 5 state primitives. Registration writes to `NodeRegistry` and returns `ControlPayload::RegisterAck`; heartbeat updates node load/last-seen with no reply; capability snapshots replace snapshots in `CapabilityIndex`; events are published through `EventBus`; capability deltas and command results are accepted as no-op MVP paths. Unsupported operation/payload pairs return `ProtocolError::PayloadDecodeFailedOwned`.

`NodeHandler` exposes registry reads through `NodeListResult` and `NodeGetResult`. `CapabilityHandler` exposes capability snapshots through `CapabilityListResult`, honoring the optional `CapabilityListRequest.node_id` filter.

Verification commands run:

- `cargo test -p vol-agent-server control_register_creates_node`
- `cargo check -p vol-agent-server`
- `cargo fmt --check -p vol-agent-server`
- `cargo test -p vol-agent-server control_plane`

## Entities Mentioned

- [[vol-agent-server-crate]]: owns the new concrete control-plane core and handlers.
- [[vol-llm-agent-channel-crate]]: provides protocol, `DomainHandler`, `HandlerRegistry`, `Connection`, and `JsonRpcMessageService` abstractions.

## Concepts Covered

- [[agent-server-control-data-plane]]: Task 6 implements the planned control-plane core/handler layer.
- [[jsonrpc-transport]]: the core implements the generic JSON-RPC service interface.

## Notes

Route composition is intentionally not implemented in Task 6; that remains Task 7. Persistent storage, lease scanning, distributed command delivery, and full command result persistence remain future work beyond this MVP handler layer.
