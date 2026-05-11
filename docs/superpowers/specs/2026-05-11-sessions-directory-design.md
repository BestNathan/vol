# Sessions Directory Implementation Spec

## Goal

Add a Sessions tab that lists all persisted sessions from disk, lets the user select and load a session's message history into the Conversation view, and displays the complete entry timeline including user messages, assistant responses, tool calls, checkpoints, and summaries.

## Architecture

### Backend: Wire `FileSessionEntryStore` into `JsonRpcConnection`

`JsonRpcConnection` already stores `store_dir: String` but never creates a store instance. Create a `FileSessionEntryStore(store_dir)` and wire it into two handlers:
- `handle_session_list()` → `store.list_sessions()` → returns `{id, entry_count, created_at}` sorted by recency
- `handle_session_entries(session_id)` → `store.get_entries(session_id)` → returns raw `Vec<SessionEntry>`

### Frontend: SessionEntry → ConversationEntry conversion

The RPC returns raw `SessionEntry` objects. The frontend maps them to `ConversationEntry` for display in the Conversation view:

| SessionEntry type | ConversationEntry mapping |
|---|---|
| `Message { role: User }` | `UserInput { text }` |
| `Message { role: Assistant }` | `AgentAnswer { text }` |
| `Message { role: Tool }` | `ToolResult { tool_name, preview, success }` |
| `Summary { summary }` | `RunSummary` (show summary text) |
| `Checkpoint { reason, note }` | Custom `EntryCheckpoint` variant — displayed as `"Checkpoint: {note or reason}"` info line |

### Data Flow

```
disk (jsonl files)
  → FileSessionEntryStore.list_sessions()  (session.list RPC)
  → FileSessionEntryStore.get_entries(sid) (session.entries RPC)
  → SessionEntry[] (WASM deserialization)
  → frontend convert_to_conversation()
  → ConversationState.entries[]
  → ActiveTab::Conversation (auto-switch)
```

## Backend Changes

### 1. `JsonRpcConnection` (connection.rs)

Add `entry_store: FileSessionEntryStore` field. Create it in `new()` using `store_dir`. Wire `store` into handlers.

Replace `handle_session_list()` stub with real `store.list_sessions()` call.

Add `handle_session_entries(session_id: String)` method calling `store.get_entries(session_id)`.

### 2. `JsonRpcRequest` enum (serde_helpers.rs)

Add `SessionEntries { id: u64, session_id: String }` variant.

Add `"session.entries"` parser case.

### 3. Dispatch wiring (connection.rs)

Wire `SessionEntries` into `handle_text_frame` dispatch match.

## Frontend Changes

### 1. `ActiveTab` enum (state/mod.rs)

Add `Sessions` variant. Insert after `Conversation`:
`Conversation, Sessions, Tools, Workspace, Skills, Logs, Agents`

Update `toggle()` accordingly.

### 2. Session-related state types (state/mod.rs)

```rust
/// Session list entry from session.list RPC.
pub struct SessionListEntry {
    pub id: String,
    pub entry_count: usize,
    pub created_at: i64,
}

/// Raw session entry from session.entries RPC (matches wire format).
pub struct SessionListEntry_ {
    pub id: String,
    pub session_id: String,
    pub created_at: i64,
    pub entry_count: usize,
    pub last_entry: String,  // type of last entry
    pub last_entry_time: String,
}

/// Conversation entry for loaded checkpoint display.
pub enum ConversationEntry {
    // ... existing variants ...
    EntryCheckpoint { reason: String, note: Option<String>, created_at: i64 },
}

/// Local state for SessionsPanel.
pub struct SessionsState {
    pub sessions: Vec<SessionListEntry>,
    pub loading: bool,
    pub error: Option<String>,
}
```

### 3. `session_entries()` RPC client method (client.rs)

Add `SessionEntry` struct matching the wire format (serde `SessionEntry` from vol-session).

Add `session_entries(session_id, cb)` callback-based method.

### 4. `SessionsPanel` component (web/components/sessions_panel.rs)

- On mount: set loading=true, call `rpc.session_list()`, populate sessions or error
- Render: table of sessions with id (truncated), entry_count, age
- Click session: call `rpc.session_entries(id)`, convert entries to `ConversationEntry`, replace `ConversationState.entries`, switch tab to `Conversation`
- Refresh button

### 5. Conversation entry converter (web/components/sessions_panel.rs or new module)

```rust
fn session_entries_to_conversation(entries: Vec<SessionEntry>) -> Vec<ConversationEntry> {
    entries.iter().filter_map(|e| match ... {
        // User message → UserInput
        // Assistant message → AgentAnswer
        // Tool message → ToolResult
        // Summary → RunSummary
        // Checkpoint → EntryCheckpoint
        // Skip if deserialization fails
    }).collect()
}
```

### 6. Tab wiring (app.rs)

- Add `SessionsState` signal at App level
- Add `Sessions` tab button to TabBar
- Add `ActiveTab::Sessions => rsx! { SessionsPanel {} }` to TabContent
- Add CSS styles

### 7. Replace existing SessionDialog

The current `SessionDialog` is a modal that doesn't load from the backend. After this feature, it's superseded by the Sessions tab. Remove `SessionDialog` from the UI (keep it in code if still used by TUI, but don't render it in web).

## Error Handling

- Empty session list: show "No sessions found" message
- RPC error: show error with retry button
- Session load failure: show toast-style error in Conversation view
- Missing session file: graceful empty list

## Testing

- Backend: unit test `handle_session_list()` returns correct format from FileSessionEntryStore
- Backend: unit test `handle_session_entries()` returns entries
- Frontend: unit test `session_entries_to_conversation()` mapping
- Integration: manual verification via browser

## Files to Modify

**Backend:**
- `crates/vol-llm-agent-channel/src/jsonrpc/serde_helpers.rs` — add SessionEntries variant + parser
- `crates/vol-llm-agent-channel/src/jsonrpc/connection.rs` — wire FileSessionEntryStore, real handlers
- `crates/vol-llm-agent-channel/src/lib.rs` — export FileSessionEntryStore if needed

**Frontend:**
- `crates/vol-llm-ui/src/state/mod.rs` — add ActiveTab::Sessions, SessionListEntry, SessionsState, EntryCheckpoint
- `crates/vol-llm-ui/src/web/client.rs` — add session_entries() RPC method + SessionEntry types
- `crates/vol-llm-ui/src/web/components/app.rs` — add Sessions tab, CSS, signal
- `crates/vol-llm-ui/src/web/components/sessions_panel.rs` — new component
- `crates/vol-llm-ui/src/web/components/mod.rs` — register sessions_panel module
