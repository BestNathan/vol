# Requirements: vol-agent-manager Channel Integration

## Background

`vol-agent-manager` and `vol-llm-agent-channel` are two independent crates with no dependency relationship. `vol-agent-manager` implements its own WebSocket protocol (`ws/protocol.rs` with `WsMessage` and related payload types), WebSocket handler (`ws/handler.rs`), and task dispatcher (`task/dispatcher.rs`). `vol-llm-agent-channel` provides a channel abstraction with a `Connection` trait, `ConnectionHolder` (as AgentPlugin), `AgentDispatcher`, `AgentRouter`, and `InboundMessage`/`OutboundMessage` protocol types.

The two crates have redundant channel-related logic: both define message protocols, both have dispatcher concepts, and both handle agent communication. This creates maintenance burden and inconsistent behavior across the system.

## Goals

1. **Add `vol-llm-agent-channel` as a dependency to `vol-agent-manager`** — establish a unidirectional dependency so the manager reuses the channel crate's abstractions.

2. **Remove duplicate protocol types from `vol-agent-manager`** — delete `ws/protocol.rs` and replace `WsMessage`, `RegisterPayload`, `HeartbeatPayload`, `MetricPayload`, `EventPayload`, `TaskPayload`, `TaskResultPayload` with equivalents from or adapted to `vol-llm-agent-channel`'s `InboundMessage`/`OutboundMessage`.

3. **Refactor `ws/handler.rs` to work with the `Connection` trait** — instead of directly manipulating axum `WebSocket`, the handler should use a `WsConnection` adapter that implements the `Connection` trait from vol-llm-agent-channel.

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
- `ws/protocol.rs` — delete or replace with channel protocol types
- `ws/handler.rs` — refactor to use `Connection` trait
- `ws/mod.rs` — update module exports
- `lib.rs` / `main.rs` — update imports and wiring
- Any tests that reference removed types

**Excluded:**
- `state/`, `metrics/`, `health/`, `events/` modules
- `task/dispatcher.rs` (different concern from channel's AgentDispatcher)
- `ws/server.rs` (routing stays, only handler internals change)

## Constraints

- The protocol types in `vol-llm-agent-channel` (`InboundMessage` with Submit/Cancel) do not directly match vol-agent-manager's message types (register/heartbeat/metric/event/task_result). A protocol adapter or extension is required.
- `vol-llm-agent-channel` depends on `vol-llm-agent` — verify no unwanted transitive dependencies are pulled into vol-agent-manager.
- The existing WebSocket connection flow (auth → register → message loop) must be preserved.

## Success Criteria

1. `vol-agent-manager/Cargo.toml` includes `vol-llm-agent-channel` as a dependency.
2. `ws/protocol.rs` is deleted or reduced to only manager-specific types not covered by channel protocols.
3. `ws/handler.rs` uses `Connection` trait or channel types instead of raw axum `WebSocket`.
4. `cargo check --workspace` passes with no errors.
5. `cargo test --workspace` passes with all tests green.
6. `vol-agent-manager`'s `Cargo.toml` has no unnecessary new transitive dependencies beyond `vol-llm-agent-channel`.

## Edge Cases

- **Protocol mismatch**: `InboundMessage` only has `Submit` and `Cancel` variants. The manager needs register/heartbeat/metric/event/task_result. Options: (a) extend `InboundMessage` in vol-llm-agent-channel, or (b) create a manager-specific adapter type. Decision: create an adapter in vol-agent-manager to avoid modifying channel's core protocol.
- **Connection lifecycle**: `ConnectionHolder` is an `AgentPlugin` designed for `ReActAgent` instances. The manager handles external agent connections, not ReActAgent runs. The `Connection` trait should be used directly, not through `ConnectionHolder`.
- **OutboundMessage serialization**: `OutboundMessage` only derives `Serialize`, not `Deserialize`. This is intentional (never deserialized), but the manager's handler may need bidirectional parsing for incoming register messages.

## Open Questions

None.
