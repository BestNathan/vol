---
type: source
category: development
tags: [task, implementation, remote-connection, json-rpc]
created: 2026-05-08
source_type: task
task_number: 6
---

# RemoteConnection Implementation

**Category:** Development task — Task 6

**Related:** [[vol-llm-ui-crate]], [[json-rpc-websocket]], [[remote-agent-connection]], [[vol-llm-agent-channel-crate]]

## Summary

Created `RemoteConnection` in `crates/vol-llm-ui/src/connection/remote.rs` — an implementation of both `AgentConnection` and `FileOperations` traits that connects to a remote agent service via JSON-RPC 2.0 over WebSocket using jsonrpsee 0.26.

## Key Details

### JSON-RPC Methods
| Method | Params | Response | Purpose |
|--------|--------|----------|---------|
| `agent.submit` | `{ input }` | `{ req_id }` | Start agent run |
| `agent.cancel` | `{ req_id }` | `{ ok }` | Cancel agent run |
| `agent.approve` | `{ req_id, approved, reason }` | `{ ok }` | Approve/deny tool execution |
| `file.list` | `{ path }` | `{ entries }` | Directory listing |
| `file.read` | `{ path }` | `{ content }` | Read file content |
| `log.list` | `{}` | `{ runs }` | List log runs |
| `session.list` | `{}` | `{ sessions }` | List sessions |

### Implementation Details
- Uses `jsonrpsee::ws_client::WsClientBuilder` for WebSocket connections
- Uses `ObjectParams` from jsonrpsee-core for named JSON-RPC parameters
- Uses `ClientT` trait for the `request()` method
- Creates a new WebSocket client per `rpc_call()` — connection is established on-demand
- Auto-reconnect with exponential backoff in `submit()`: max 5 retries, 1s to 30s delay
- `submit()` spawns a `tokio::spawn` task that manages the connection lifecycle
- Tracks connection state via `Arc<AtomicBool>` for `is_connected()`

### Code Structure
- `ConnectionState` — holds `ws_url` and `request_id` counter behind `RwLock`
- `RemoteConnection` — main struct with state and connection flag
- `rpc_call<T>()` — generic method for any JSON-RPC request
- `AgentConnection` impl — `submit`, `approve_tool`, `cancel`, `is_connected`
- `FileOperations` impl — `list_files`, `read_file`, `list_logs`, `list_sessions`

### Compilation Notes
- `serde_json::Value` does not implement `ToRpcParams` — must use `ObjectParams`
- `rpc_params!` macro only supports positional parameters, not named ones
- `jsonrpsee::core::client::ClientT` must be imported for `.request()` method
- `jsonrpsee::core::params::ObjectParams` for named parameter objects

## Timeline
- **2026-05-08**: Created with full `AgentConnection` and `FileOperations` implementations, 3 unit tests
