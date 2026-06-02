---
type: concept
category: pattern
tags: [mcp, manager, lifecycle, reconnect, connection-state, rmcp]
created: 2026-05-13
updated: 2026-05-15
source_count: 2
---

# MCP Manager Lifecycle

**Category:** Connection lifecycle pattern
**Related:** [[vol-llm-mcp-crate]], [[mcp-client-integration]], [[rmcp-sdk]], [[react-agent-mcp-integration]]

## Definition

Pattern for managing MCP server connection lifecycles with automatic reconnection, state tracking, and full protocol discovery (tools, resources, prompts). `McpManager` replaces the flat `McpSession` data structure with a full lifecycle manager.

## Architecture

```
┌─────────────────────────────────────────────────────┐
│                   McpManager                         │
│  Arc<RwLock<HashMap<String, ServerState>>>          │
│                                                     │
│  ┌──────────────┐  ┌──────────────┐                 │
│  │ ServerState  │  │ ServerState  │  ...            │
│  │ (server-a)   │  │ (server-b)   │                 │
│  │              │  │              │                 │
│  │ status       │  │ status       │                 │
│  │ retry_count  │  │ retry_count  │                 │
│  │ running_svc  │  │ running_svc  │                 │
│  │ cancel_token │  │ cancel_token │                 │
│  │ cached_tools │  │ cached_tools │                 │
│  │ cached_resrc │  │ cached_resrc │                 │
│  │ cached_promt │  │ cached_promt │                 │
│  │ reconnect_h  │  │ reconnect_h  │                 │
│  └──────────────┘  └──────────────┘                 │
└─────────────────────────────────────────────────────┘
```

All public methods take `&self` — internal mutability via `Arc<RwLock<>>`.

## ServerStatus States

| State | Meaning |
|-------|---------|
| `Connected` | Server running, capabilities cached |
| `Connecting` | Connection in progress |
| `Disconnected` | Gracefully disconnected by user |
| `Error(String)` | Connection failed, detail in string |

## Connection Flow

### Initial connect()
For each server, dispatch on `config.transport`:

**Stdio:**
1. Set status → `Connecting`
2. Spawn `TokioChildProcess` (command + args + env)
3. Create `rmcp` transport (stdio)
4. `ClientInfo.serve_with_ct(transport, cancel_token)`
5. Wait for peer ready (10s timeout)
6. Fetch & cache: tools, resources, resource templates, prompts
7. Set status → `Connected`

**Http:**
1. Set status → `Connecting`
2. Build `StreamableHttpClientTransportConfig` from URL + optional headers
3. Create `StreamableHttpClientTransport` (reqwest-based)
4. `ClientInfo.serve_with_ct(transport, cancel_token)`
5. Wait for peer ready (10s timeout)
6. Fetch & cache: tools, resources, resource templates, prompts
7. Set status → `Connected`

On failure:
1. Set status → `Error(detail)`, increment `retry_count`
2. If `retry_count < max_retries`: spawn background reconnect
3. If `retry_count >= max_retries`: clear caches, stop auto-reconnects

### Background Reconnect Loop
Spawned as async task with per-server `CancellationToken`:
1. Wait backoff delay (exponential: 1s-30s)
2. Attempt connect
3. Success → reset retry counter, break loop
4. Failure → increment retry counter, loop or exit if exhausted
5. Cancel → exit loop (triggered by manual reconnect or disconnect)

### Manual reconnect()
1. Cancel in-progress reconnect via `CancellationToken`
2. If `Connected` → disconnect single server first
3. Reset `retry_count` to 0
4. Run full connection flow
5. On failure → auto-reconnect resumes if under limit

### Disconnect()
1. Cancel reconnect `CancellationToken`
2. Stdio: kill `TokioChildProcess`; Http: close `RunningService`
3. Stop `RunningService`
4. Clear caches
5. Set status → `Disconnected`

## Key Behaviors

### Cache Filtering
- `list_all_tools()` / `list_all_resources()` / `list_all_prompts()` return cached results filtered by `Connected` servers only
- On `Connected` → `Disconnected`: caches cleared, tools excluded
- On successful reconnect: caches invalidated and re-fetched
- `call_tool()` / `read_resource()` / `get_prompt()` always query live service

### Edge Cases
- **Empty config:** Valid manager with zero servers. `connect()` is no-op.
- **Partial server failure:** Manager operates normally for connected servers.
- **Tool call on disconnected server:** Returns `McpError::ServerDisconnected` — never panics or hangs.
- **Concurrent reconnects:** `reconnect()` cancels in-progress attempt, starts fresh.
- **Reconnect on Connected server:** Gracefully disconnects first, then reconnects with reset retry counter.

## Configuration

| Parameter | Default | Description |
|-----------|---------|-------------|
| `max_retries` | 5 | Max auto-reconnect attempts |
| `backoff_min` | 1s | Minimum backoff delay |
| `backoff_max` | 30s | Maximum backoff delay |
| `connect_timeout` | 10s | Per-connection attempt timeout |

## Timeline

- **2026-05-11**: `McpSession` — flat data structure, no lifecycle management [[react-agent-mcp-integration]]
- **2026-05-13**: `McpManager` — full lifecycle manager with auto-reconnect and state tracking [[mcp-manager-impl]]
- **2026-05-15**: Multi-transport dispatch — `connect_single` matches on `McpTransport` enum; HTTP via `StreamableHttpClientTransport` [[mcp-multi-transport-config]]
