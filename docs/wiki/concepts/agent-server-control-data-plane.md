---
type: concept
category: architecture
tags: [control-plane, data-plane, agent-server, distributed-agents, routing, json-rpc, channel]
created: 2026-06-10
updated: 2026-06-10
source_count: 13
---

# Agent Server Control Plane / Data Plane

## Definition

The agent server control/data-plane architecture separates cluster-wide coordination from local execution while keeping protocol definitions in [[vol-llm-agent-protocol-crate]] and concrete server behavior in [[vol-agent-server-crate]]. Both planes use JSON-RPC 2.0 over WebSocket: `/ws` for client-facing requests and `/control/v1/ws` for data-plane node links.

Sources: [[agent-server-control-data-plane-architecture]], [[agent-server-control-data-plane-addendum]], [[agent-server-control-data-plane-implementation-plan]], [[control-payload-flat-jsonrpc-encoding-fix]], [[agent-server-role-config-route-skeleton]], [[agent-server-data-plane-core-move]], [[agent-server-control-plane-core-handlers]], [[agent-server-role-route-composition]], [[agent-server-health-route-collision-validation]], [[agent-server-data-plane-snapshot-command]], [[agent-server-control-router-mvp]], [[agent-server-boundary-mode-verification]], [[control-plane-behavior-completion-plan]]

## Key Points

- No separate control-plane crate is introduced; [[vol-agent-server-crate]] owns both `DataPlaneServerCore` and `ControlPlaneServerCore`.
- [[vol-llm-agent-protocol-crate]] owns all wire-level protocol definitions and abstractions: `Operation`, `Payload`, `control.*`, JSON-RPC codec, `Connection`, `DomainHandler`, `HandlerRegistry`, and `JsonRpcMessageService`.
- `DataPlaneServerCore` is the final home for current `AgentServerCore` behavior: runtime construction, local data-plane handlers, local agent routing/dispatch, and connection holder event bridging.
- `ControlPlaneServerCore` owns `NodeRegistry`, `CapabilityIndex`, `LeaseManager`, `ControlRouter`, `EventBus`, run state, and concrete control-plane handlers.
- [[vol-llm-runtime-crate]] remains the data-plane single source of truth for runtime resources.
- JSON-RPC over WebSocket is the only application protocol; HTTP is reserved for `/health` and `/metrics`.
- `vol-agent-server` config chooses standalone data-plane, standalone control-plane, or combined mode.

## How It Works

1. `vol-agent-server` loads role config.
2. If data-plane mode is enabled, it builds `DataPlaneServerCore`, which builds [[vol-llm-runtime-crate]] and discovers local agents.
3. If control-plane mode is enabled, it builds `ControlPlaneServerCore` with registry, capability index, lease manager, router, and event bus.
4. Routes are mounted by role:
   - standalone data-plane: `/ws -> DataPlaneServerCore`
   - control-plane: `/ws -> ControlPlaneServerCore` client endpoint and `/control/v1/ws -> ControlPlaneServerCore` node endpoint
   - combined: local data-plane registers to the local control-plane endpoint.
5. Data-plane nodes send `control.register`, `control.capability_snapshot`, `control.heartbeat`, and `control.event` over JSON-RPC.
6. Client execution requests such as `agent.submit` reach the control-plane `/ws`; `ControlRouter` selects a node and sends `control.command` to that node.
7. The selected node executes through local `DataPlaneServerCore` handlers, `AgentRouter`, and `AgentDispatcher`, then returns command results/events.

## Operation Mapping

| Operation kind | JSON-RPC behavior | Concrete owner |
|----------------|-------------------|----------------|
| Protocol decode/encode | `vol-llm-agent-channel` maps JSON-RPC frames to `AgentServerMessage` | [[vol-llm-agent-protocol-crate]] |
| Node registration (`control.register`) | Node-to-control request creates/updates `NodeRecord` | `ControlPlaneServerCore` |
| Node reports (`control.heartbeat`, `control.capability_snapshot`, `control.event`) | Node-to-control notifications update leases/index/events | `ControlPlaneServerCore` |
| Catalog (`agent.list`, `tool.list`, `mcp.list_servers`, `skill.list`) | Client-to-control request reads from `CapabilityIndex` in control mode | `ControlPlaneServerCore` |
| Execution (`agent.submit`, `tool.call`, `mcp.call_tool`) | Client request routes `control.command` to a selected node in control mode | `ControlRouter` + `DataPlaneServerCore` |
| Standalone execution | Existing methods execute locally when control-plane role is disabled | `DataPlaneServerCore` |

## Addendum Details

Source: [[agent-server-control-data-plane-addendum]]

The addendum clarifies implementation-critical semantics:

- `/ws` and `/control/v1/ws` share JSON-RPC framing but enforce different method allowlists.
- `control.command` response means accepted/rejected; long-running progress and terminal results use `control.event` and `control.command_result`.
- `CommandRecord` and `RunRecord` are separate because not every command creates an agent run.
- Capability snapshots use node-local monotonic revisions and full-snapshot replace semantics; deltas require `base_revision`.
- `NodeRecord` stores node state while `NodeSession` stores live connection/generation state.
- Combined mode should initially use loopback JSON-RPC registration so local nodes exercise the same path as remote nodes.
- `RuntimeCapabilitySource` should hide runtime internals from data-plane reporter code.
- Error code vocabulary belongs in [[vol-llm-agent-protocol-crate]], while [[vol-agent-server-crate]] fills contextual error details.
- `control.*` JSON-RPC params/results are flat payload objects; `ControlPayload` must not encode internal `type`/`data` wrappers because decode arms consume flat fields such as `node_id` directly [[control-payload-flat-jsonrpc-encoding-fix]].

## Implementation Plan

Source: [[agent-server-control-data-plane-implementation-plan]]

The staged implementation starts by making [[vol-llm-agent-protocol-crate]] transport service-generic (`JsonRpcMessageService`) and adding `control.*` protocol types. It then adds role config and moves concrete data-plane behavior from channel into [[vol-agent-server-crate]] as `DataPlaneServerCore`. Later stages add in-memory `ControlPlaneServerCore` state/handlers, route composition, data-plane snapshot/command skeletons, `ControlRouter` MVP, dependency-boundary tests, and docs/wiki updates.

Task 3 completed the role-config portion and a base route skeleton [[agent-server-role-config-route-skeleton]]: `[server.roles]` defaults to data-plane-only, `[control_plane]` and `[data_plane]` deserialize with endpoint/lease/heartbeat defaults, both roles disabled is rejected, and `/health` exists in `routes::base_router()` for future composition.

Task 4 completed the data-plane ownership split [[agent-server-data-plane-core-move]]. Concrete standalone execution now lives in `vol-agent-server::data_plane::DataPlaneServerCore`, including local handlers, router, dispatcher, and connection-holder event bridge. [[vol-llm-agent-protocol-crate]] no longer owns concrete execution modules; it remains the protocol/connection/service/transport abstraction crate.

Task 6 implemented the initial control-plane core and handlers [[agent-server-control-plane-core-handlers]]. `ControlPlaneServerCore` registers `ControlHandler`, `NodeHandler`, and `CapabilityHandler`; `control.register` writes a node and returns `RegisterAck`; heartbeat and capability snapshot update in-memory state without replies; node list/get and capability list expose registry/index reads. Route composition is intentionally deferred to Task 7.

Task 7 implemented role-based route composition [[agent-server-role-route-composition]]. A pure `ws_owner(control_plane, data_plane)` function selects `/ws` ownership: control-plane owns when enabled, data-plane owns when standalone. `app::run` expands paths, builds cores by config, mounts control-plane `JsonRpcServer` routes on the configured client and node WebSocket paths, mounts the data-plane `JsonRpcServer` on the client path in standalone mode, binds the listener, and serves the composed router. `main.rs` delegates startup to `app::run` after config/tracing setup.

Task 8 added data-plane reporting primitives [[agent-server-data-plane-snapshot-command]]. `RuntimeCapabilitySource` abstracts capability snapshots and load reporting for future reporter code. `StaticCapabilitySource` returns an empty revision-1 snapshot for a node id, and `accept_control_command` creates accepted `CommandAck` responses with synthetic run ids only for `SubmitAgent` commands.

Task 9 added the control router MVP [[agent-server-control-router-mvp]]. `ControlRouter<'a>` uses `CapabilityIndex` snapshots and `NodeRegistry` status to select online nodes with matching agent capabilities. Explicit targets match `agent_id` or `name`; untargeted routing selects the first online snapshot with any agent; missing capability returns `capability_not_found`.

Task 10 added boundary and mode verification [[agent-server-boundary-mode-verification]]. Integration tests assert `/ws` maps to the data plane only in standalone data-plane mode, maps to the control plane whenever the control-plane role is enabled, and rejects TOML configs where both roles are disabled. The boundary script verifies `vol-llm-agent-channel` and `vol-llm-runtime` do not depend on `vol-agent-server`.

## Follow-up Behavior Completion Plan

Source: [[control-plane-behavior-completion-plan]]

Final implementation review found behavior gaps not covered by the first plan: JSON-RPC notifications without `id`, endpoint role allowlists, minimal client-facing control-plane methods, data-plane `control.command` handling, capability revision sync, `control.run_status`, and combined-mode local node registration. The follow-up plan decomposes these into eight implementation tasks.

## Edge Cases

- If both roles are disabled, config validation fails before binding sockets.
- If a configured active WebSocket path collides with `/health`, config validation fails before Axum route construction to avoid duplicate-route panics [[agent-server-health-route-collision-validation]].
- If both roles are enabled, `/ws` belongs to the control-plane endpoint; local data-plane registers through `/control/v1/ws` loopback by default.
- Existing standalone clients keep current `/ws` behavior when `control_plane=false`.
- Duplicate node IDs are handled as reconnect/supersede only when auth identity matches.
- Heartbeat timeout marks a node `Dead`, excludes it from placement, and keeps capabilities as stale for inspection.
- Protocol types needed by both cores must be added to [[vol-llm-agent-protocol-crate]], never [[vol-agent-server-crate]].

## Related Concepts

- [[vol-agent-server-crate]]
- [[vol-llm-agent-protocol-crate]]
- [[agent-router]]
- [[connection-holder]]
- [[tool-registry]]
- [[mcp-manager-lifecycle]]
- [[skill-system]]
- [[runtime-task-store-configuration]]
- [[runtime-session-store-configuration]]
- [[agent-server-control-data-plane-architecture]]
