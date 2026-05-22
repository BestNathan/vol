---
type: source
source_type: code
date: 2026-05-22
ingested: 2026-05-22
tags: [jsonrpc, transport, refactoring, agent-channel]
---

# JSON-RPC Transport Consolidation

**Authors/Creators:** BestNathan
**Date:** 2026-05-22
**Link:** docs/superpowers/specs/2026-05-22-jsonrpc-transport-consolidation-design.md

## TL;DR

Moved all JSON-RPC transport code from scattered `jsonrpc/` and `gateway/` modules into `transport/jsonrpc/`. Renamed `gateway/jsonrpc_ws.rs` to `codec.rs` (named for what it does: encode/decode JSON-RPC frames). Removed the empty old directories. No public API breakage — `JsonRpcServer` stays re-exported from the crate root.

## Key Takeaways

- `jsonrpc/{server,connection,serde_helpers}.rs` moved to `transport/jsonrpc/`
- `gateway/jsonrpc_ws.rs` moved to `transport/jsonrpc/codec.rs`
- Old `jsonrpc/mod.rs` and `gateway/mod.rs` deleted
- 3 internal import paths updated in `connection.rs`
- 2 test files updated with new import paths
- `JsonRpcServer` re-export from crate root unchanged — zero downstream impact

## Detailed Summary

Consolidated all JSON-RPC transport code under `transport/jsonrpc/`, consistent with how `transport/http.rs`, `transport/ws.rs`, and `transport/memory.rs` are organized. The resulting structure:

```
transport/
  mod.rs
  http.rs
  memory.rs
  ws.rs
  jsonrpc/
    mod.rs
    codec.rs
    connection.rs
    serde_helpers.rs
    server.rs
```

`transport/jsonrpc/mod.rs` declares all submodules public and re-exports `decode_jsonrpc_frame`, `encode_jsonrpc_message`, and `JsonRpcServer`.

## Entities Mentioned

- [[vol-llm-agent-channel-crate]]: jsonrpc and gateway modules consolidated under transport/

## Concepts Covered

- [[connection-trait]]: `JsonRpcConnection` now lives under `transport::jsonrpc`
- [[jsonrpc-transport]]: transport code consolidated from two scattered modules into one

## Notes

- The `codec.rs` file has pre-existing unused import warnings that are out of scope for this change.
