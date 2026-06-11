---
type: source
source_type: design
date: 2026-06-10
ingested: 2026-06-10
tags: [agent-server, control-plane, data-plane, implementation-plan, json-rpc, migration]
---

# Agent Server Control/Data Plane Implementation Plan

**Authors/Creators:** Claude
**Date:** 2026-06-10
**Link:** `docs/superpowers/plans/2026-06-10-agent-server-control-data-plane-implementation-plan.md`
**Feishu/Lark:** https://my.feishu.cn/docx/TnKWd2VUeoKHnjxX8FgcIKzEnQ5

## TL;DR

Implementation plan for [[agent-server-control-data-plane-architecture]] and [[agent-server-control-data-plane-addendum]]. It stages the refactor so [[vol-llm-agent-channel-crate]] first gets a generic `JsonRpcMessageService` and `control.*` protocol, then concrete data-plane server behavior moves into [[vol-agent-server-crate]], followed by role config/routes, in-memory control-plane state/handlers, data-plane snapshot/command skeletons, boundary tests, and docs/wiki updates.

## Key Takeaways

- Start by making `JsonRpcServer` generic over `JsonRpcMessageService` while current behavior still works.
- Add `ControlOperation`, `ControlPayload`, node/capability/command models, and `control.*` JSON-RPC mapping in [[vol-llm-agent-channel-crate]].
- Add role config and route composition to [[vol-agent-server-crate]].
- Move current concrete `AgentServerCore`, local router/dispatcher, connection holder, and data-plane handlers from channel to `vol-agent-server::data_plane`.
- Add in-memory `ControlPlaneServerCore` state: `NodeRegistry`, `CapabilityIndex`, `CommandStore`, `RunStore`, and handlers for `control.register`, heartbeat, snapshot, node list, and capability list.
- Add data-plane snapshot and command skeletons, then control-router MVP.
- Add dependency-boundary checks and role-mode routing tests.
- Final implementation docs/wiki updates are included as the last task.

## Detailed Summary

The plan has eleven tasks. Tasks 1-2 make channel protocol/transport ready for both data-plane and control-plane services. Task 1 adds `JsonRpcMessageService` and makes `JsonRpcServer` service-generic. Task 2 adds `control.*` operations and payloads.

Tasks 3-4 shift concrete server ownership into `vol-agent-server`: role config and route skeletons are added first, then data-plane core behavior is moved from channel into `vol-agent-server::data_plane` as `DataPlaneServerCore`.

Tasks 5-7 add an in-memory control plane and role-based routing. `ControlPlaneServerCore` gets state, handlers, and route composition for `/ws` and `/control/v1/ws`.

Tasks 8-10 add MVP data-plane reporting/command skeletons, control routing, and verification checks. Task 11 updates docs, Lark, and wiki.

## Entities Mentioned

- [[vol-llm-agent-channel-crate]]: first migration target; owns generic JSON-RPC service abstraction and `control.*` protocol.
- [[vol-agent-server-crate]]: owns moved data-plane core, new control-plane core, role config/routes, and in-memory state.
- [[vol-llm-runtime-crate]]: remains execution resource owner and capability source.

## Concepts Covered

- [[agent-server-control-data-plane]]: implementation sequencing for the architecture and addendum.
- [[agent-router]]: planned move into `vol-agent-server::data_plane` as local execution routing.
- [[jsonrpc-transport]]: generalized via `JsonRpcMessageService`.
- [[runtime-task-store-configuration]]: passed through `DataPlaneServerCore` builder.
- [[runtime-session-store-configuration]]: passed through `DataPlaneServerCore` builder.

## Implementation Status

- **2026-06-10 Task 1 complete:** `vol-llm-agent-channel` now has `JsonRpcMessageService`, exported from crate root. `JsonRpcServer<S>` is generic over that service and accepts a mounting path such as `/ws` or `/custom/ws`. Current `AgentServerCore` implements the service trait through `serve_dyn(Arc<dyn Connection>)`, preserving existing concrete `serve(conn)` callers. `vol-agent-server` now passes `/ws` explicitly when constructing the server.
- Verification for Task 1 passed: `cargo test -p vol-llm-agent-channel generic_service_tests::jsonrpc_server_accepts_generic_service_and_path`, `cargo test -p vol-llm-agent-channel transport::jsonrpc`, `cargo check -p vol-agent-server`, and `cargo fmt --check`.
- **2026-06-10 Task 3 complete:** `vol-agent-server` now parses `[server.roles]`, `[control_plane]`, and `[data_plane]` sections; validates that at least one role is enabled; and includes `app`, `routes`, and `health` skeleton modules for future route composition. Source: [[agent-server-role-config-route-skeleton]].
- Verification for Task 3 passed: `cargo test -p vol-agent-server roles` (valid equivalent for the two requested test filters), `cargo check -p vol-agent-server`, and `cargo fmt --check`.

## Notes

The plan intentionally leaves full `ControlPlaneClient` send/receive loop and persistent control-plane storage for a later plan. The MVP focuses on compiling boundaries, protocol, route composition, in-memory registry/index, and verification tests.
