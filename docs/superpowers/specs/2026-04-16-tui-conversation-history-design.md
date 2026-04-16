# TUI Conversation History Persistence Design

**Goal:** Make conversation history persist across multiple `agent.run()` calls within the same TUI REPL loop, so the coding agent remembers previous Q&As in a session.

**Root cause:** Each REPL iteration creates a new `CodingAgent`, and `CodingAgent::run()` creates a new `Session` with `InMemorySessionStore` and a random session ID. History is never loaded because the store is always empty.

**Approach:** Create one `Session` backed by `FileMessageStore` at TUI startup. Pass the shared session to each `CodingAgent` via `CodingAgentConfig.session`. `CodingAgent::run()` reuses it if provided.

---

## Architecture

### Current Flow

```
TUI REPL loop (main.rs):
  each iteration:
    1. Create CodingAgent::new(config)       // fresh tools, config
    2. agent.run(input) creates:
       - Session(InMemorySessionStore)       // empty, new ID each time
       - SessionListener saves to it
    3. RunContext::init_messages():
       - session.get_messages() → empty      // nothing to load
    4. Agent runs with no prior context
    5. Agent dropped → InMemoryStore destroyed
```

### New Flow

```
TUI startup (main.rs):
  1. Create FileMessageStore("./.vol-sessions/<id>.jsonl")
  2. Create Session(FileMessageStore)        // shared, disk-backed
  3. Store in CodingAgentConfig { session: Some(session), .. }

TUI REPL loop (main.rs):
  each iteration:
    1. Create CodingAgent::new(config)       // config carries shared session
    2. agent.run(input) uses config.session  // no new InMemory session
    3. RunContext::init_messages():
       - session.get_messages() → loads prior Q&As from FileMessageStore
    4. Agent runs with full context
    5. SessionListener appends new messages to FileMessageStore (disk)
    6. Agent dropped → FileMessageStore data persists on disk
```

### Key Existing Components Used

| Component | Role | Already Exists? |
|-----------|------|-----------------|
| `FileMessageStore` | JSONL file-based message storage | Yes (`vol-session/src/file_store.rs`) |
| `Session` | Manages message lifecycle | Yes (`vol-session/src/session.rs`) |
| `CodingAgentConfig.session` | Pass session to agent | Yes (task #1248) |
| `ReActAgent::with_session()` | Set session on agent | Yes (task #1249) |
| `RunContext::init_messages()` | Loads history from session | Yes (`run_context.rs:214-239`) |
| `SessionListener` | Persists messages to MessageStore | Yes (`vol-session/src/listener.rs`) |

---

## Changes

### File 1: `crates/vol-llm-tui/src/main.rs`

**What changes:** Add session creation before REPL loop, pass session to CodingAgentConfig each iteration.

**New imports:**
```rust
use vol_llm_agents::coding::Session;
use vol_session::FileMessageStore;
```

**Before REPL loop, create shared session:**
```rust
// Create persistent session for this TUI run
let session_dir = std::env::current_dir()?.join(".vol-sessions");
std::fs::create_dir_all(&session_dir)?;
let session_id = format!("tui_{}", chrono::Utc::now().format("%Y%m%d_%H%M%S"));
let message_store = Arc::new(FileMessageStore::new(&session_dir, &session_id));
let session_store = Arc::new(vol_session::InMemorySessionStore::new());
let session = Arc::new(Session::new(
    session_id.clone(),
    session_store,
    message_store,
));
print_colored(Color::Green, &format!("Session: {}\n", session_id));
```

**Each REPL iteration, pass session to config:**
```rust
let config = CodingAgentConfig {
    max_iterations: 10,
    working_dir: std::env::current_dir()?,
    hitl_enabled: true,
    verbose: false,
    html_report_path: None,
    session: Some(session.clone()),  // NEW: pass shared session
    tool_config,
    ..Default::default()
};
```

### File 2: `crates/vol-llm-agents/src/coding/agent.rs`

**What changes:** `CodingAgent::run()` uses `config.session` if provided, falls back to InMemory if not.

**Replace lines 187-193 (session creation) with:**
```rust
// Create session for this run
let session = match &self.config.session {
    Some(s) => s.clone(),  // Use shared session from config
    None => {
        // Fallback: create ephemeral in-memory session (backward compatible)
        use vol_llm_agent::session::{InMemorySessionStore, InMemoryMessageStore};
        Arc::new(Session::new(
            format!("coding_{}", chrono::Utc::now().format("%Y%m%d_%H%M%S")),
            Arc::new(InMemorySessionStore::new()),
            Arc::new(InMemoryMessageStore::new()),
        ))
    }
};
```

### Exports (2 files)

- `crates/vol-llm-agents/src/coding/mod.rs` — re-export `Session` from vol-llm-agent
- `crates/vol-session/src/lib.rs` — verify `FileMessageStore` and `InMemorySessionStore` are public (likely already are)

---

## Data Flow

```
User types "question 1" → agent.run() → SessionListener saves messages to FileMessageStore → .vol-sessions/tui_xxx.jsonl
User types "question 2" → agent.run() → init_messages() loads Q1/A1 from FileMessageStore → agent has context → saves Q2/A2
User types "question 3" → agent.run() → init_messages() loads Q1/A1/Q2/A2 → agent has full context → saves Q3/A3
```

The JSONL file on disk accumulates all messages from the session. `max_history_messages` (currently 20) caps how many are loaded into the agent's message array.

---

## Error Handling

- If `.vol-sessions/` directory creation fails in `main.rs`: log error, fall back to `InMemorySessionStore` (no history, but TUI continues)
- If `FileMessageStore::get_by_session()` reads a corrupted JSONL file: return `Err` → `RunContext::init_messages()` has `.unwrap_or_default()` → loads empty history (safe fallback)
- `CodingAgent::run()` always has a fallback: if `config.session` is `None`, creates a fresh `InMemorySession` (backward compatible)

---

## Testing

- Unit test: `CodingAgent::run()` uses config.session when provided
- Unit test: `CodingAgent::run()` falls back to InMemorySessionStore when session is None
- Integration test: Two consecutive `agent.run()` calls with shared session → second run loads first run's messages
