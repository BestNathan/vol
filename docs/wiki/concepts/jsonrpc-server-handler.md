---
type: concept
category: framework
tags: [json-rpc, deleted, refactoring]
created: 2026-05-08
updated: 2026-05-09
source_count: 1
---

# JSON-RPC Server Handler (deleted)

**Category:** Historical architecture — deleted

**Related:** [[jsonrpc-transport-refactoring]], [[jsonrpc-transport]]

## Status: DELETED

This page documents the former `JsonRpcHandler`/`JsonRpcContext`/`EventBridgePlugin` architecture in `vol-llm-agent-channel::jsonrpc::handler`. It was deleted on 2026-05-09 and replaced by `JsonRpcConnection` implementing the `Connection` trait [[jsonrpc-transport-refactoring]].

## What Changed

- `JsonRpcHandler` and `JsonRpcContext` → replaced by `JsonRpcConnection` + `JsonRpcServer`
- `EventBridgePlugin` → replaced by `ConnectionHolder` (which already existed)
- `jsonrpsee` crate dependency → removed, replaced by manual JSON-RPC parsing
- File: `src/jsonrpc/handler.rs` (564 lines) → deleted
- New files: `src/jsonrpc/connection.rs`, `src/jsonrpc/server.rs`, `src/jsonrpc/serde_helpers.rs`

## Why It Was Deleted

The old architecture had two parallel event-bridging mechanisms:
1. `ConnectionHolder` forwarded events through the `Connection` trait
2. `EventBridgePlugin` forwarded events through a separate `broadcast::Sender`

This created duplicate event paths, redundant plugin registrations, and code that could not be reused by other transports. The refactoring unified everything under the `Connection` trait.
