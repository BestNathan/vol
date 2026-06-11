---
type: source
source_type: design
date: 2026-06-10
ingested: 2026-06-10
tags: [agent-server, control-plane, data-plane, architecture, runtime, mcp, tools, json-rpc, channel]
---

# Agent Server Control Plane / Data Plane Architecture

**Authors/Creators:** Claude
**Date:** 2026-06-10
**Link:** `docs/superpowers/architectures/2026-06-10-agent-server-control-data-plane.md`
**Feishu/Lark:** https://my.feishu.cn/docx/K0mGdhW5UoKL9IxVBwHcQmsxn9c

## TL;DR

Designs a control/data-plane architecture without adding a new control-plane crate. `vol-llm-agent-channel` owns all protocol definitions, JSON-RPC over WebSocket transport, connection abstraction, handler registry, and generic JSON-RPC service abstraction. `vol-agent-server` owns concrete server implementations: `DataPlaneServerCore` for local agent execution and `ControlPlaneServerCore` for node registry, capability indexing, routing, leases, and events. Both client-facing and node-facing APIs use JSON-RPC 2.0 over WebSocket.

## Key Takeaways

- No new `vol-agent-control-plane` crate; control-plane implementation lives under [[vol-agent-server-crate]].
- [[vol-llm-agent-channel-crate]] owns wire protocol: `Operation`, `Payload`, `control.*` methods, JSON-RPC codec, `Connection`, `DomainHandler`, `HandlerRegistry`, and a generic `JsonRpcMessageService` abstraction.
- [[vol-agent-server-crate]] owns concrete cores: `DataPlaneServerCore` and `ControlPlaneServerCore`.
- Current `AgentServerCore` behavior should move out of channel into `vol-agent-server::data_plane`, because it is concrete data-plane execution implementation rather than protocol.
- JSON-RPC over WebSocket is the only application protocol; HTTP is reserved for `/health` and `/metrics`.
- Runtime resources remain owned by [[vol-llm-runtime-crate]], and data-plane capability snapshots are derived from runtime state.
- `vol-agent-server` role config supports standalone data-plane, standalone control-plane, and combined control+data-plane modes.

## Detailed Summary

The final boundary separates protocol from concrete server behavior. `vol-llm-agent-channel` should no longer own a concrete `AgentServerCore`. Instead, it should expose reusable protocol/transport abstractions: `AgentServerMessage`, `Operation`, `Payload`, `ControlOperation`, `ControlPayload`, JSON-RPC frame encode/decode, `JsonRpcConnection`, generic `JsonRpcServer<S>`, `DomainHandler`, `HandlerRegistry`, and `JsonRpcMessageService`.

`vol-agent-server` should define concrete server cores. `DataPlaneServerCore` replaces the current channel-crate `AgentServerCore` as the owner of local agent execution, runtime construction, data-plane handlers, `AgentRouter`, `AgentDispatcher`, and `ConnectionHolder`. `ControlPlaneServerCore` owns `ControlPlaneState`, including `NodeRegistry`, `CapabilityIndex`, `LeaseManager`, `ControlRouter`, `EventBus`, and run/command state.

The same `vol-agent-server` binary should compose roles from config. In standalone data-plane mode, `/ws` mounts to `DataPlaneServerCore`. In standalone control-plane mode, `/ws` mounts to `ControlPlaneServerCore` for client requests and `/control/v1/ws` accepts node links. In combined mode, the local data plane registers with the local control-plane endpoint, preferably via loopback JSON-RPC in the MVP.

## Entities Mentioned

- [[vol-agent-server-crate]]: concrete server implementation crate for data-plane and control-plane cores plus role composition.
- [[vol-llm-agent-channel-crate]]: protocol, JSON-RPC transport, connection, handler, registry, and generic service abstractions.
- [[vol-llm-runtime-crate]]: authoritative owner of execution resources in every data-plane node.

## Concepts Covered

- [[agent-server-control-data-plane]]: single server crate with split data/control cores and shared channel protocol.
- [[agent-router]]: moves conceptually into data-plane execution, below distributed `ControlRouter`.
- [[tool-registry]]: source of native tool capability snapshots.
- [[mcp-manager-lifecycle]]: source of MCP server/tool/resource/prompt capability state.
- [[skill-system]]: source of skill capability snapshots.
- [[runtime-task-store-configuration]]: durable task store remains runtime-owned rather than duplicated by the control plane.
- [[runtime-session-store-configuration]]: durable session store remains runtime-owned rather than duplicated by the control plane.

## Notes

- The design supersedes both the older removed `vol-agent-manager` direction and the earlier draft that introduced a separate `vol-agent-control-plane` crate.
- `vol-agent-server` must not define wire-level protocol types; any type crossing JSON-RPC must live in `vol-llm-agent-channel`.
- First phase excludes HA, RBAC, multi-tenancy, cross-node session migration, and exactly-once command execution across crashes.
