# Design: JSON-RPC Transport Consolidation

## Summary

Consolidate all JSON-RPC transport code currently scattered across `jsonrpc/` and `gateway/` into a single `transport/jsonrpc/` module. Rename `gateway/jsonrpc_ws.rs` to `codec.rs` (named for what it does: encode/decode JSON-RPC frames).

## File moves

| Current | Target |
|---------|--------|
| `jsonrpc/server.rs` | `transport/jsonrpc/server.rs` |
| `jsonrpc/connection.rs` | `transport/jsonrpc/connection.rs` |
| `jsonrpc/serde_helpers.rs` | `transport/jsonrpc/serde_helpers.rs` |
| `gateway/jsonrpc_ws.rs` | `transport/jsonrpc/codec.rs` |
| `jsonrpc/mod.rs` | deleted |
| `gateway/mod.rs` | deleted |

## New module: `transport/jsonrpc/mod.rs`

```rust
pub mod codec;
pub mod connection;
pub mod server;
pub mod serde_helpers;

pub use codec::{decode_jsonrpc_frame, encode_jsonrpc_message};
pub use server::JsonRpcServer;
```

## Import updates (internal only)

| Old import | New import |
|-----------|-----------|
| `crate::gateway::jsonrpc_ws::decode_jsonrpc_frame` | `crate::transport::jsonrpc::codec::decode_jsonrpc_frame` |
| `crate::gateway::jsonrpc_ws::encode_jsonrpc_message` | `crate::transport::jsonrpc::codec::encode_jsonrpc_message` |
| `super::serde_helpers` (in connection.rs) | unchanged (stays relative) |

## lib.rs changes

- Remove `pub mod gateway;`
- Remove `pub mod jsonrpc;`
- Change `pub use jsonrpc::JsonRpcServer;` to `pub use transport::jsonrpc::JsonRpcServer;`

## transport/mod.rs changes

- Add `pub mod jsonrpc;`

## Public API

`JsonRpcServer` remains re-exported from crate root — no downstream breakage. `decode_jsonrpc_frame` and `encode_jsonrpc_message` remain accessible via `transport::jsonrpc`.

## Test file

`tests/jsonrpc_ws_gateway_test.rs` — update import from `gateway::jsonrpc_ws` to `transport::jsonrpc::codec`.

## Resulting structure

```
transport/
  mod.rs
  http.rs
  memory.rs
  ws.rs
  jsonrpc/
    mod.rs
    server.rs
    connection.rs
    codec.rs
    serde_helpers.rs
```

All JSON-RPC code lives under one directory, consistent with `transport/http.rs`, `transport/ws.rs`, etc.
