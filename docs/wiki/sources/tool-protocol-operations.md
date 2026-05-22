---
type: source
source_type: code
date: 2026-05-22
ingested: 2026-05-22
tags: [tool, protocol, json-rpc, agent-channel]
---

# Tool Protocol Operations

**Authors/Creators:** BestNathan
**Date:** 2026-05-22
**Link:** docs/superpowers/specs/2026-05-22-tool-protocol-design.md

## TL;DR

Added `ToolOperation`/`ToolPayload` to the agent server protocol with `tool.list` and `tool.call` JSON-RPC methods. Created `ToolHandler` domain handler backed by `ToolRegistry`. Frontend tools panel updated with system tool listing and direct invocation.

## Key Takeaways

- New protocol domain: `ToolOperation::List`/`Call`, `ToolPayload::List`/`ListResult`/`Call`/`CallResult`
- `ToolHandler` exposes `ToolRegistry::definitions()` via `tool.list` and `ToolRegistry::execute()` via `tool.call`
- Frontend `JsonRpcClient` gained `tool_list()` and `tool_call()` methods
- Tools panel updated with "Fetch Tools" button, tool list with "Run" per tool, and call result display

## Detailed Summary

The `ToolOperation` enum adds two new operations to the protocol: `List` (method `tool.list`) and `Call` (method `tool.call`). The `ToolPayload` enum carries the request/result data. These are added as `Tool(ToolOperation)` and `Tool(ToolPayload)` variants on the `Operation` and `Payload` enums respectively.

`ToolHandler` implements `DomainHandler`, registered in `AgentServerCore::build()`. It wraps `Arc<ToolRegistry>`:

- `tool.list` calls `registry.definitions()` and returns tool name, description, and parameters as JSON
- `tool.call` builds a `ToolCall` from the request params, executes via `registry.execute()`, and returns the result (success, content, error, data)

The frontend `JsonRpcClient` gained `tool_list(cb)` and `tool_call(name, args, cb)` methods. The tools panel component now has a "System Tools" section with a fetch button, tool listing with per-tool "Run" button, and a call result display area.

## Entities Mentioned

- [[vol-llm-agent-channel-crate]]: new ToolOperation, ToolPayload, ToolHandler
- [[vol-llm-ui-crate]]: updated client and tools panel

## Concepts Covered

- [[tool-registry]]: ToolRegistry exposed via protocol operations
- [[agent-server-protocol]]: new tool domain added
