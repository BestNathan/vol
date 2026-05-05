# Requirements: vol-agent-manager Channel Integration

## Background

`vol-agent-manager` and `vol-llm-agent-channel` are two independent crates with no dependency relationship. `vol-agent-manager` implements its own WebSocket protocol (`ws/protocol.rs` with `WsMessage` and related payload types), WebSocket handler (`ws/handler.rs`), and task dispatcher (`task/dispatcher.rs`). `vol-llm-agent-channel` provides a channel abstraction with a `Connection` trait, `ConnectionHolder` (as AgentPlugin), `AgentDispatcher`, `AgentRouter`, and `InboundMessage`/`OutboundMessage` protocol types.

The two crates have redundant channel-related logic: both define message protocols, both have dispatcher concepts, and both handle agent communication. This creates maintenance burden and inconsistent behavior across the system.

## Goals

1. **Add `vol-llm-agent-channel` as a dependency to `vol-agent-manager`** — establish a unidirectional dependency so the manager reuses the channel crate's abstractions.

2. **Delete `ws/protocol.rs` entirely** — remove all manager-specific message types (`WsMessage`, `RegisterPayload`, `HeartbeatPayload`, `MetricPayload`, `EventPayload`, `TaskPayload`, `TaskResultPayload`, `RegisterAckPayload`, `HostInfo`). All communication uses `vol-llm-agent-channel`'s `InboundMessage` (Submit/Cancel) and `OutboundMessage` (Connected/Event/Result/Error) exclusively. No adapter layer — manager adopts channel protocol directly.

3. **Refactor `ws/handler.rs` to use channel protocol types** — the handler processes `InboundMessage` and sends `OutboundMessage` via the `Connection` trait from vol-llm-agent-channel. Manager concepts (register, heartbeat, task_result, metric, event) are mapped into the channel's Submit/Event/Result messages.

4. **Keep `ws/server.rs` in vol-agent-manager** — routing configuration remains the manager's responsibility. The server creates the appropriate `Connection` implementation and passes it to the handler.

5. **Keep `task/dispatcher.rs` in vol-agent-manager** — `TaskDispatcher` tracks multi-agent task state at the management level, which is a different concern from `AgentDispatcher`'s single-agent request queue. No redundancy here.

6. **Ensure the project compiles and all tests pass** after integration.

## Non-Goals

- Do NOT merge the two crates into one — they remain separate crates.
- Do NOT modify `vol-llm-agent-channel`'s core API (the `Connection` trait, `InboundMessage`, `OutboundMessage`) unless absolutely necessary for protocol compatibility.
- Do NOT change the WebSocket endpoint URL (`/ws`) or connection authentication logic.
- Do NOT modify `vol-agent-manager`'s state management, metrics, event bus, or health check modules.

## Scope

**Included:**
- `Cargo.toml` — add `vol-llm-agent-channel` dependency
- `ws/protocol.rs` — delete entirely, zero protocol types remain in vol-agent-manager
- `ws/handler.rs` — refactor to use `Connection` trait
- `ws/mod.rs` — update module exports
- `lib.rs` / `main.rs` — update imports and wiring
- Any tests that reference removed types

**Excluded:**
- `state/`, `metrics/`, `health/`, `events/` modules
- `task/dispatcher.rs` (different concern from channel's AgentDispatcher)
- `ws/server.rs` (routing stays, only handler internals change)

## Constraints

- `InboundMessage` (`Submit`/`Cancel`) is the sole message protocol. Manager concepts (register, heartbeat, metric, event, task_result) are expressed through Submit messages with metadata or through existing channel types.
- The connection lifecycle in vol-agent-manager (auth → register → message loop) maps to: auth check → `OutboundMessage::Connected` → process `InboundMessage` Submit/Cancel.
- Agent registration is expressed via Submit metadata rather than a separate register message.
- `vol-llm-agent-channel` depends on `vol-llm-agent` — verify no unwanted transitive dependencies are pulled into vol-agent-manager.
- The existing WebSocket connection flow (auth → register → message loop) must be preserved.

## Success Criteria

1. `vol-agent-manager/Cargo.toml` includes `vol-llm-agent-channel` as a dependency.
2. `ws/protocol.rs` is deleted entirely — zero protocol types remain in vol-agent-manager.
3. `ws/handler.rs` uses `Connection` trait or channel types instead of raw axum `WebSocket`.
4. `cargo check --workspace` passes with no errors.
5. `cargo test --workspace` passes with all tests green.
6. `vol-agent-manager`'s `Cargo.toml` has no unnecessary new transitive dependencies beyond `vol-llm-agent-channel`.

## Edge Cases

- **All manager message types removed**: `WsMessage`, `RegisterPayload`, `HeartbeatPayload`, `MetricPayload`, `EventPayload`, `TaskPayload`, `TaskResultPayload` are all deleted. Manager uses channel's `InboundMessage` (Submit/Cancel) and `OutboundMessage` (Connected/Event/Result/Error) exclusively.
- **Connection lifecycle**: `ConnectionHolder` is an `AgentPlugin` designed for `ReActAgent` instances. The manager handles external agent connections, not ReActAgent runs. The `Connection` trait should be used directly, not through `ConnectionHolder`.
- **OutboundMessage serialization**: `OutboundMessage` only derives `Serialize`, not `Deserialize`. This is intentional (never deserialized), but the manager's handler may need bidirectional parsing for incoming register messages.

## Open Questions

None.
