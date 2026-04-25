# TUI Agent/Session/Logger Optimization Design

**Date**: 2026-04-25
**Status**: Approved

## Summary

Add session management, agent caching, and log viewing to vol-llm-tui. Three focused improvements: session list with resume capability, agent config caching to avoid per-run reconstruction, and a new Logs tab to browse JSONL event logs from LoggerPlugin.

## 1. Session List & Resume

### Problem
TUI creates a fresh session on startup. Old sessions accumulate in `{store_dir}/sessions/*.jsonl` but are never shown or resumed.

### Design

**1.1 Add `list_sessions()` to `FileSessionEntryStore`**

Scan `{entry_dir}/*.jsonl`, parse session IDs from filenames, return `(session_id, created_at, entry_count)`. Not added to the trait — concrete method on `FileSessionEntryStore` since it's file-system-specific.

**1.2 Add `SessionManager` struct to TUI**

```rust
pub struct SessionManager {
    entry_store: Arc<FileSessionEntryStore>,
    current_session: Arc<Session>,
}
```

Methods:
- `create_session()` → creates new session, returns `Arc<Session>`
- `resume_session(session_id)` → calls `Session::resume()`, returns `Arc<Session>`
- `list_sessions()` → delegates to `entry_store.list_sessions()`
- `delete_session(session_id)` → delegates to `entry_store.delete_session()`
- `current_session()` → returns current session Arc

**1.3 TUI session list dialog**

Triggered by `/sessions` command or `Ctrl+S` shortcut. Inline overlay (not a new tab):

```
Sessions (Ctrl+S to dismiss)
───────────────────────────────────────
> abc123...    142 entries    2 hours ago
  def456...     28 entries    yesterday
  ghi789...      5 entries    3 days ago

[n] New session
[Enter] Resume  [d] Delete  [Esc] Cancel
```

On resume: session is swapped, `session_id` in status bar updates, conversation history is preserved in AppState (old entries remain visible but are from the previous session).

## 2. Agent Cache

### Problem
Every `spawn_agent()` call rebuilds the entire agent — tool config from env vars, builder chain, LoggerPlugin. Most of this is identical between runs.

### Design

**2.1 `AgentCache` struct in TUI**

```rust
pub struct AgentCache {
    working_dir: PathBuf,
    store_dir: PathBuf,
    tool_config: ToolConfig,
}
```

Built once in `main()` before entering the event loop. `CodingAgentBuilder` is used per-run but populated from the cache:

```rust
let agent = CodingAgentBuilder::new()
    .working_dir(cache.working_dir.clone())
    .store_dir(cache.store_dir.clone())
    .max_iterations(10)
    .session(session)
    .hitl_enabled(true)
    .unsafe_mode(unsafe_mode)
    .approval_handler(handler)
    .tool_config(cache.tool_config.clone())
    .with_logger()
    .build()
    .await?;
```

**2.2 Tool config built once**

`TAVILY_API_KEY` and `WEB_FETCH_MAX_LENGTH` env vars read once at startup. `AgentCache` holds the constructed `ToolConfig`.

**Tradeoff:** Tool config changes between runs won't be picked up until restart. Acceptable — env vars are static during a TUI session.

## 3. Log Viewer Tab

### Problem
LoggerPlugin writes JSONL to `{store_dir}/logs/{run_id}.jsonl` but nothing in the TUI displays these logs.

### Design

**3.1 New `ActiveTab::Logs` variant**

Third tab in the right panel, accessible via Tab cycling: `Conversation → Workspace → Logs → Conversation`. Ctrl+3 direct switch.

**3.2 State structs in `app.rs`**

```rust
pub struct LogViewer {
    pub run_logs: Vec<LogRunSummary>,
    pub selected_run: Option<String>,  // run_id
    pub entries: Vec<LogLine>,
    pub scroll: u16,
    pub auto_scroll: bool,
}

pub struct LogRunSummary {
    pub run_id: String,
    pub event_count: usize,
    pub last_event: String,
    pub last_event_time: String,
}

pub struct LogLine {
    pub event_type: String,
    pub summary: String,
    pub timestamp: String,
}
```

**3.3 Lazy loading**

When the Logs tab is first focused:
- Scan `{store_dir}/logs/*.jsonl` (not plugin subdirectories)
- For each file, count lines, parse last line to get `last_event` and `last_event_time`
- Populate `LogViewer.run_logs`

When a run is selected:
- Parse all lines from `{store_dir}/logs/{run_id}.jsonl`
- Use `LogEntry::format_event_summary()` for each line's summary
- Populate `LogViewer.entries`

**3.4 Color-coded rendering**

Reuse conversation view color scheme:
- `AgentStart`/`AgentComplete` = green
- `ToolCallBegin`/`ToolCallComplete` = yellow
- `ToolCallError` = red
- `AgentAborted` = red
- Other events = default

## Files Changed

| File | Change |
|------|--------|
| `crates/vol-session/src/file_store.rs` | Add `list_sessions()` method |
| `crates/vol-llm-tui/src/main.rs` | Add SessionManager, AgentCache, migrate spawn_agent |
| `crates/vol-llm-tui/src/app.rs` | Add ActiveTab::Logs, LogViewer, LogRunSummary, LogLine, session list state |
| `crates/vol-llm-tui/src/render.rs` | Render Logs tab, session list dialog |
| `crates/vol-llm-tui/src/ui.rs` | Route to Logs tab render, update tab switching |

## Dependencies

- `vol_llm_observability::LogEntry` — already available, used for parsing JSONL logs
- `vol_session::FileSessionEntryStore` — extended with `list_sessions()`
