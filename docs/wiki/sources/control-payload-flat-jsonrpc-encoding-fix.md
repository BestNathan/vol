---
type: source
source_type: code
date: 2026-06-10
ingested: 2026-06-10
tags: [agent-channel, json-rpc, control-plane, serialization, tests]
---

# ControlPayload Flat JSON-RPC Encoding Fix

**Authors/Creators:** Claude Code
**Date:** 2026-06-10
**Link:** /Users/admin/Documents/learn/vol-agent/crates/vol-llm-agent-channel/src/agent_server_protocol.rs

## TL;DR

Removed internal serde tagging from `ControlPayload` so `Payload::data_json()` can strip the externally tagged enum variant and emit flat JSON-RPC `params`/`result` objects for `control.*` messages, matching the existing flat decode arms.

## Key Takeaways

- `ControlPayload` now serializes like the other payload enums instead of as `{ "type": "Register", "data": {...} }`.
- Encoded `control.register` commands now expose `params.node_id`, `params.name`, and `params.version` directly.
- Encoded `RegisterAck` results now expose `result.node_id`, `result.accepted`, and `result.generation` directly.
- Regression tests assert that encoded control command/result payloads do not include `type` or `data` wrappers.

## Detailed Summary

The Task 2 control protocol added `ControlPayload` with internal serde tagging. That differed from other protocol payload enums and bypassed the `Payload::data_json()` assumption that enum payloads serialize as a single-key externally tagged object whose wrapper can be stripped for JSON-RPC.

The fix removes `#[serde(tag = "type", content = "data")]` from `ControlPayload` in `agent_server_protocol.rs`. With default externally tagged enum serialization, `Payload::data_json()` receives a single variant wrapper such as `Register`, strips it, and produces the flat JSON-RPC payload shape expected by control decode logic.

Additional tests in `transport/jsonrpc/codec.rs` cover command and result encoding for `control.register` and `RegisterAck`, including explicit absence checks for `type` and `data` fields.

Verification run:

- `cargo test -p vol-llm-agent-channel encode_control` passed 2 tests.
- Combined cargo filter for both decode tests matched 0 tests because cargo test filtering is substring-based in this context.
- `cargo test -p vol-llm-agent-channel decode_control_register` passed.
- `cargo test -p vol-llm-agent-channel decode_control_heartbeat_notification` passed.
- `cargo fmt --check` passed.

## Entities Mentioned

- [[vol-llm-agent-channel-crate]]: owns the protocol payload enums and JSON-RPC codec changed by this fix.

## Concepts Covered

- [[agent-server-control-data-plane]]: depends on flat `control.*` JSON-RPC message shapes between control and data planes.

## Notes

This was a code-quality compatibility fix for Task 2 control protocol serialization. It did not change control decode semantics; it aligned encoding with the existing flat decode contract.
