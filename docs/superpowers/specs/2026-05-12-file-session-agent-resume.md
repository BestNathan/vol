# File Session + Agent Resume Design Spec

## Goal

Make `jsonrpc_agent_service` use file-based sessions so the agent's conversation history persists to disk, and add a real `session.resume` RPC that restores a session into the agent's context so the user can continue from where they left off.

## Problem

The example creates `Session` with `InMemoryEntryStore` (line 46 of `jsonrpc_agent_service.rs`), but `JsonRpcConnection` reads sessions from `FileSessionEntryStore` (from `/tmp/vol-llm-store`). These are two disconnected stores — agent writes never reach disk, so `session.list` and `session.entries` always return empty.

Additionally, `handle_session_resume` is a stub. Even with file-based sessions, there's no way to restore a session into the agent's live context.

## Architecture

### Two independent changes that work together

1. **File-based session**: Example uses `FileSessionEntryStore` so agent writes go to `/tmp/vol-llm-store/<session_id>.jsonl`
2. **Agent resume**: `session.resume` RPC swaps the agent's session in-place, so the next `agent.submit` continues from the resumed context

Both share the same `FileSessionEntryStore` — the example creates it, passes it to both the agent's `Session` and `JsonRpcConnection`'s `entry_store`.

## Design

### Part 1: File-based Session in Example

**File:** `crates/vol-llm-agent-channel/examples/jsonrpc_agent_service.rs`

Replace `InMemoryEntryStore` with `FileSessionEntryStore`:

```rust
// Before:
use vol_session::InMemoryEntryStore;
let session = Arc::new(Session::new(Arc::new(InMemoryEntryStore::new())));

// After:
use vol_session::file_store::FileSessionEntryStore;
let entry_store = Arc::new(FileSessionEntryStore::new("/tmp/vol-llm-store"));
let session = Arc::new(Session::new(entry_store));
```

The `JsonRpcConnection` already creates its own `FileSessionEntryStore` from `store_dir` (which is `/tmp/vol-llm-store`). Both now share the same directory.

### Part 2: `ReActAgent::with_session`

**File:** `crates/vol-llm-agent/src/react/agent.rs`

Add method to `ReActAgent`:

```rust
/// Create a new agent with the given session (clones the rest of config).
pub fn with_session(&self, session: Arc<Session>) -> Self {
    Self {
        config: AgentConfig {
            session,
            ..self.config.clone()
        },
    }
}
```

### Part 3: `AgentDispatcher::swap_session`

**File:** `crates/vol-llm-agent-channel/src/dispatcher.rs`

Change `agent` field from `Arc<ReActAgent>` to `std::sync::RwLock<Arc<ReActAgent>>`:

```rust
pub struct AgentDispatcher {
    agent: std::sync::RwLock<Arc<ReActAgent>>,
    state: Arc<DispatcherState>,
}
```

Update `new()` to wrap in `RwLock::new()`. Update `run_loop` to clone from the RwLock read guard.

Add `swap_session` method:

```rust
/// Atomically replace the agent's session.
pub fn swap_session(&self, new_session: Arc<Session>) {
    let old_agent = self.agent.read().unwrap();
    let new_agent = Arc::new(old_agent.with_session(new_session));
    *self.agent.write().unwrap() = new_agent;
}
```

### Part 4: Real `handle_session_resume`

**File:** `crates/vol-llm-agent-channel/src/jsonrpc/connection.rs`

Replace stub:

```rust
async fn handle_session_resume(&self, id: u64, session_id: String) -> String {
    // Swap agent session in all dispatchers
    let entry_store = &self.entry_store;
    let session = match Session::resume(session_id.clone(), entry_store.clone()).await {
        Ok(s) => Arc::new(s),
        Err(e) => {
            return to_jsonrpc_error(Some(id), -32000, format!("Failed to resume session: {e}"));
        }
    };
    for dispatcher in self.dispatchers.values() {
        dispatcher.swap_session(session.clone());
    }

    // Return session info + entries for UI display
    match entry_store.get_entries(&session_id).await {
        Ok(entries) => {
            let json_entries: Vec<serde_json::Value> = entries
                .into_iter()
                .filter_map(|e| serde_json::to_value(e).ok())
                .collect();
            to_jsonrpc_response(id, serde_json::json!({
                "session_id": session_id,
                "entry_count": entries.len(),
                "entries": json_entries,
            }))
        }
        Err(e) => to_jsonrpc_error(Some(id), -32000, format!("Failed to read entries: {e}")),
    }
}
```

Import `Session` from `vol_session` at the top of connection.rs.

### Part 5: Frontend `session_resume` RPC client method

**File:** `crates/vol-llm-ui/src/web/client.rs`

Add method following `session_entries` pattern:

```rust
pub fn session_resume(&self, session_id: &str, cb: impl FnOnce(Result<SessionResumeResponse, String>) + 'static) {
    // Sends session.resume RPC, parses { session_id, entry_count, entries }
}
```

Add `SessionResumeResponse` struct matching the wire format.

### Part 6: SessionsPanel Resume button

**File:** `crates/vol-llm-ui/src/web/components/sessions_panel.rs`

Each session item gets a Resume button. On click:
1. Call `rpc.session_resume(id)` → swaps agent session + returns entries
2. Convert entries to `ConversationEntry` → replace `ConversationState.entries`
3. Switch tab to `Conversation`

Error handling: if resume fails (session not found on disk), show error toast.

## Data Flow

```
disk (jsonl files)
  → FileSessionEntryStore.list_sessions()  (session.list RPC)
  → SessionsPanel displays session list
  → User clicks Resume on a session
  → session.resume RPC:
      a. Session::resume(session_id, entry_store) → new Session from disk
      b. dispatcher.swap_session(session) → agent's context replaced
      c. entry_store.get_entries(session_id) → entries returned to UI
  → Frontend: session_entries_to_conversation(entries)
  → ConversationState.entries = converted entries
  → ActiveTab switches to Conversation
  → User types new input → agent.submit → agent runs with resumed context
  → New messages appended to same jsonl file
```

## Error Handling

- Missing session file on resume: RPC returns error with "Session not found"
- Corrupted jsonl: `from_json` returns None, entries silently skipped
- Empty session list: SessionsPanel shows "No sessions found"

## Testing

- Unit test: `ReActAgent::with_session` creates agent with new session
- Unit test: `AgentDispatcher::swap_session` replaces agent session
- Integration test: `handle_session_resume` swaps session + returns entries
- Manual: start service, submit conversation, verify jsonl written, restart, resume session
