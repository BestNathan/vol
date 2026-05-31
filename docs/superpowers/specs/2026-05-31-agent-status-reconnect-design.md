# Agent Status on Reconnect — Design Spec

**Date:** 2026-05-31
**Status:** draft

## Problem

When the WebSocket reconnects and the user selects an agent, the frontend has no way to know whether the agent is still running in the background. Currently `WsConnected` unconditionally resets `is_running = false`, which loses the running state.

## Design

### New RPC: `agent.status`

```
Request:   {"method": "agent.status", "params": {"agent_id": "..."}}
Response:  {"status": "idle" | "running", "run_id": "..." | null}
```

### Frontend Flow: Select Agent After Reconnect

```
User selects agent
  → agent.status(agent_id)     ← check running state first
  → load session entries       ← parallel with above
  ↓
  ├─ idle → normal mode, input enabled
  └─ running → show running banner, input disabled, subscribe(run_id)
```

### Running Banner

A non-dismissible banner at the top of the conversation view:

> ⬤ Agent is currently running. Below is the live conversation.
> [run_id: abc123]

- Appears when `agent.status` returns `status: "running"`
- Disappears when `AgentComplete` / `AgentAborted` / `AgentError` event arrives
- Shows `run_id` so the user can reference it for cancel operations

### Changes

| Layer | File | Change |
|-------|------|--------|
| **Backend** |
| Protocol | `agent_server_protocol.rs` | Add `AgentOperation::Status` variant; add `AgentPayload::Status { status: String, run_id: Option<String> }` |
| Codec | `operation_codec.rs` | Map `"agent.status"` ↔ `AgentOperation::Status` |
| Handler | `domain/agent.rs` | Handle `AgentOperation::Status`: read `agent_status` map, return idle/running + run_id |
| **Frontend** |
| Client | `web/client.rs` | Add `agent_status(agent_id, cb)` RPC method |
| State | `state/mod.rs` | Add `ConversationEntry::RunningBanner { run_id: String }` variant |
| UI | `conversation.rs` | Render `RunningBanner` at top of conversation; clear on AgentComplete/Aborted/Error |
| Logic | `app.rs` | Remove `is_running = false` from WsConnected handler; on agent select: call `agent.status`, if running: push `RunningBanner`, set `is_running = true`, subscribe(run_id) |

### Edge Cases

- **Status fails to load**: Treat as idle (degraded, input available).
- **Agent running but session empty**: Still show banner + subscribe. User sees live events.
- **Agent complete between status check and subscribe**: `AgentComplete` event from subscribe will clear the banner naturally.
- **User selects different agent while current is running**: Clear running state for previous agent, check status for new agent.
