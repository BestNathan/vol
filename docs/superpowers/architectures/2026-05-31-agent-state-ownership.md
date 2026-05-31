# Architecture: Agent Internal State Ownership & Running Guard

**Date**: 2026-05-31
**Status**: Draft
**Author**: BestNathan
**Source**: Codebase analysis — `ReActAgent`, `AgentDispatcher`, `AgentRuntime`

## Requirements

### Goals

1. **Running state owned by Agent**: `ReActAgent` internally tracks whether `run_input()` is executing. External code only reads via `agent.is_running()`, never writes.
2. **Config interior mutability**: `AgentConfig` stays as one cohesive struct. Fields that need mutation (session) use interior mutability (`RwLock`). Fields that are immutable stay as-is.
3. **Agent gates all mutations**: Agent exposes delegate methods for config mutation (`set_session`), checking internal running state before allowing the write.
4. **External reference-only access**: `AgentConfig` loses `Clone`. External code holds only `&AgentConfig` or cloned `Arc<Session>` handles. No full-config clones.

### Non-Goals

- Changing how `Session` / `SessionEntryStore` works internally
- Changing the builder pattern for initial construction
- Removing `AgentDispatcher::busy` lock (queue serialization is a separate concern)
- Changing the UI-layer `is_running` fields (those are local UI mirrors, not source of truth)

---

## Problem Analysis: Where Running State Lives Today

There are **three independent** running-state systems, and **none** are inside `ReActAgent`:

```
┌──────────────────────────────────────────────────────────────────┐
│ System 1: AgentRuntime::agent_status (server-side map)           │
│   Definition:  vol-llm-runtime/src/lib.rs:25-30                  │
│   Writer:      ConnectionHolder::listen() via plugin events      │
│                connection.rs:90-99 (AgentStart→running,          │
│                AgentComplete/Aborted→idle)                       │
│   Reader:      AgentHandler::List/Status  domain/agent.rs:160-223│
│                AgentRuntime::run() shutdown  lib.rs:161-166      │
│   PROBLEM:     Keyed by connection_id, queried by agent_id       │
│                — mismatch makes Status queries unreliable        │
├──────────────────────────────────────────────────────────────────┤
│ System 2: AgentDispatcher::busy (per-agent queue lock)           │
│   Definition:  dispatcher.rs:17 (busy: Mutex<()>)                │
│   Writer:      run_loop() acquires before processing             │
│                dispatcher.rs:115                                 │
│   Reader:      is_busy() via try_lock  dispatcher.rs:102-105     │
│   STATUS:      Only accurate running indicator at agent level    │
│                But only used internally, not exposed to clients  │
├──────────────────────────────────────────────────────────────────┤
│ System 3: UI-layer is_running: bool (TUI + Web)                  │
│   TUI:   AppState.is_running        tui/app.rs:242               │
│   TUI:   UiState.is_running         state/mod.rs:810             │
│   Web:   GlobalState.is_running     state/mod.rs:415             │
│   STATUS: UI-local mirrors. Manually set on AgentStart/Complete  │
│           events. Not queryable by backend clients.              │
└──────────────────────────────────────────────────────────────────┘
```

**Root cause**: The `ReActAgent` struct is a passive data holder. It has no behavior for state management — all state tracking is bolted on externally, inconsistently.

---

## Architecture

**Move running state INTO `ReActAgent`.** Make `AgentConfig` internally mutable for its mutable fields (session), immutable for the rest. Agent delegates mutation methods and gates them on `is_running`.

### Before (current)

```
External mutation, no guard:

  SessionHandler ──► Router ──► Dispatcher ──► clone Arc<ReActAgent>
                                                   │
                                                   ▼
                              ReActAgent::with_session(&self)
                                └── AgentConfig { session, ..self.config.clone() }
                                     ▲
                                     └── clones EVERYTHING (llm, tools, mcp, plugins...)

  Running state: nowhere in agent. Scattered across 3 external systems.
```

### After (proposed)

```
Internal mutation, agent-gated:

  SessionHandler ──► Router ──► Dispatcher ──► agent.set_session(s)
                                                   │
                                                   ▼
                              ┌─ is_running()? ──► Err(AgentBusyError)
                              │
                              └─ false: config.session.write() = new_session
                                         ▲
                                         └── direct replacement, no clone of config

  Running state: AtomicBool inside ReActAgent. Single source of truth.
```

### Component Diagram

```
┌─────────────────────────────────────────────────────────┐
│                     ReActAgent                           │
│                                                          │
│  ┌──────────────────────────────────────────────────┐   │
│  │ AgentConfig (NO Clone)                            │   │
│  │                                                    │   │
│  │  def: Option<AgentDef>           // immutable      │   │
│  │  llm: Arc<dyn LLMClient>         // immutable      │   │
│  │  tools: Arc<ToolRegistry>        // immutable      │   │
│  │  sandbox: Option<SandboxRef>     // immutable      │   │
│  │  context_builder: ContextBuilder // immutable      │   │
│  │  plugin_registry: PluginRegistry // immutable      │   │
│  │  mcp_manager: Option<Arc<McpManager>> // immutable │   │
│  │  agent_id: String                // immutable      │   │
│  │  working_dir: PathBuf            // immutable      │   │
│  │                                                    │   │
│  │  session: RwLock<Arc<Session>>   // INTERIOR MUT   │   │
│  └──────────────────────────────────────────────────┘   │
│                                                          │
│  run_state: Arc<RunningState>                            │
│    └── is_running: AtomicBool                            │
│    └── current_input: RwLock<Option<String>>             │
│    └── current_run_id: RwLock<Option<String>>            │
│                                                          │
│  Methods:                                                │
│    config() -> &AgentConfig                              │
│    session() -> Arc<Session>                             │
│    is_running() -> bool                                  │
│    set_session(Arc<Session>) -> Result<(), AgentBusyError>│
│    run_state() -> &Arc<RunningState>   // for ext query  │
└─────────────────────────────────────────────────────────┘
```

### Running State Ownership

```
                    BEFORE (external)              AFTER (internal)
                    ─────────────────              ────────────────
Owns state:         AgentRuntime + Dispatcher      ReActAgent
Updated by:         ConnectionHolder plugin        ReActAgent::run_input()
                    (event side-channel)           (direct, in the run method)
Queried by clients: AgentHandler → stale map       AgentHandler → dispatcher
                                                      → agent.is_running()
UI gets it from:    UiEvent::AgentStart/Complete    Same events, but backend
                    (duplicated logic)              query also available
```

`AgentDispatcher::busy` lock stays — it serializes the FIFO queue. The agent's `is_running` guards state mutation. Two different concerns:

| Concern | Mechanism | Owner |
|---------|-----------|-------|
| "Only one run at a time" | `DispatcherState::busy: Mutex<()>` | AgentDispatcher |
| "Can I mutate agent state?" | `ReActAgent::is_running: AtomicBool` | ReActAgent |

---

## Key Types

### `ReActAgent`

```rust
/// ReAct Agent — owns its config and running state.
///
/// Config is immutable after construction except for fields with interior
/// mutability (session). All mutation methods check is_running before
/// allowing writes.
pub struct ReActAgent {
    config: AgentConfig,
    /// Shared running state. Exposed via run_state() for external status queries.
    run_state: Arc<RunningState>,
}

/// Agent running state — shared with AgentRuntime for status tracking.
pub struct RunningState {
    /// true while run_input() is executing.
    pub is_running: std::sync::atomic::AtomicBool,
    /// Current input text (for status display).
    pub current_input: std::sync::RwLock<Option<String>>,
    /// Current run_id (for status display).
    pub current_run_id: std::sync::RwLock<Option<String>>,
}

impl ReActAgent {
    /// Create from config. Extracts run_state for external sharing.
    pub fn new(config: AgentConfig) -> Self { .. }

    // ── Read-only access (always allowed) ──

    /// Immutable reference to config.
    pub fn config(&self) -> &AgentConfig { &self.config }

    /// Cheap clone of the shared session handle.
    pub fn session(&self) -> Arc<Session> {
        self.config.session.read().unwrap().clone()
    }

    /// Whether agent is currently executing run_input().
    pub fn is_running(&self) -> bool {
        self.run_state.is_running.load(Ordering::Acquire)
    }

    /// Shared running state (for AgentRuntime / status queries).
    pub fn run_state(&self) -> &Arc<RunningState> { &self.run_state }

    // ── Mutation methods (gated by is_running) ──

    /// Replace the session. Rejected if agent is running.
    pub fn set_session(&self, session: Arc<Session>) -> Result<(), AgentBusyError> {
        if self.is_running() {
            return Err(AgentBusyError { agent_id: self.config.agent_id.clone() });
        }
        *self.config.session.write().unwrap() = session;
        Ok(())
    }

    // ── Execution ──

    /// Run the ReAct loop. Sets/clears is_running.
    pub async fn run_input(&self, input: AgentInput) -> Result<AgentResponse, AgentError> {
        // CAS: false → true, bail if already running
        if self.run_state.is_running.swap(true, Ordering::AcqRel) {
            return Err(AgentError::AlreadyRunning);
        }
        // Set status metadata
        *self.run_state.current_input.write().unwrap() = Some(input.display_text());
        *self.run_state.current_run_id.write().unwrap() = input.run_id.clone();

        // RAII guard: clears is_running on drop (even on panic)
        let _guard = RunningGuard { run_state: &self.run_state };

        // ... existing run logic, RunContext gets self.session() ...
    }
}

/// RAII guard that resets running state on drop.
struct RunningGuard<'a> {
    run_state: &'a RunningState,
}
impl Drop for RunningGuard<'_> {
    fn drop(&mut self) {
        self.run_state.is_running.store(false, Ordering::Release);
        *self.run_state.current_input.write().unwrap() = None;
        *self.run_state.current_run_id.write().unwrap() = None;
    }
}
```

### `AgentConfig`

```rust
/// Agent configuration — single source of truth.
///
/// Clone is REMOVED. Fields that need runtime mutation use interior mutability.
/// After construction via builder, only &AgentConfig references are handed out.
pub struct AgentConfig {
    // === Immutable after construction ===
    pub def: Option<crate::agent_def::AgentDef>,
    pub llm: Arc<dyn vol_llm_core::LLMClient>,
    pub tools: Arc<vol_llm_tool::ToolRegistry>,
    pub sandbox: Option<SandboxRef>,
    pub context_builder: ContextBuilder,
    pub plugin_registry: PluginRegistry,
    pub mcp_manager: Option<Arc<McpManager>>,
    pub agent_id: String,
    pub working_dir: PathBuf,

    // === Interior mutability ===
    /// Session handle. Read via agent.session(), write via agent.set_session().
    pub(crate) session: std::sync::RwLock<Arc<Session>>,
}
```

### `AgentBusyError`

```rust
/// Returned when mutation is attempted while agent is executing.
#[derive(Debug, thiserror::Error)]
#[error("agent {agent_id} is currently running — state mutation not allowed")]
pub struct AgentBusyError {
    pub agent_id: String,
}
```

### `AgentError` (addition)

```rust
pub enum AgentError {
    // ... existing variants ...
    /// run_input() called while already running.
    #[error("agent is already running")]
    AlreadyRunning,
}
```

---

## Data Flow

### Session Resume (mutation gated by running state)

```
Client           SessionHandler          Router          Dispatcher        ReActAgent
  │                   │                    │                 │                  │
  │ Resume{session_id}│                    │                 │                  │
  │──────────────────►│                    │                 │                  │
  │                   │                    │                 │                  │
  │                   │ load entries       │                 │                  │
  │                   │ Session::resume()  │                 │                  │
  │                   │                    │                 │                  │
  │                   │ swap_session(id,s) │                 │                  │
  │                   │───────────────────►│                 │                  │
  │                   │                    │ swap_session(s) │                  │
  │                   │                    │────────────────►│                  │
  │                   │                    │                 │ set_session(s)   │
  │                   │                    │                 │─────────────────►│
  │                   │                    │                 │                  │
  │                   │                    │                 │  is_running()?   │
  │                   │                    │                 │  ├─ false:        │
  │                   │                    │                 │  │  session.write │
  │                   │                    │                 │  │  = new_session │
  │                   │                    │                 │  └─ true:         │
  │                   │                    │                 │     AgentBusyError│
  │                   │                    │                 │                  │
  │                   │                    │  Ok / Err       │                  │
  │                   │◄───────────────────│◄────────────────│◄─────────────────│
  │                   │                    │                 │                  │
  │ ResumeResult      │                    │                 │                  │
  │ (or error if busy)│                    │                 │                  │
  │◄──────────────────│                    │                 │                  │
```

### Agent Run (running state lifecycle)

```
run_input(input)
  │
  ├─ AtomicBool::swap(false→true)
  │   └─ if was already true → return Err(AlreadyRunning)
  │
  ├─ set current_input, current_run_id in run_state
  │
  ├─ create RunningGuard (RAII — clears state on drop)
  │
  ├─ create RunContext { session: self.session() }  ← cheap Arc clone
  │
  ├─ execute ReAct loop ...
  │   (tool calls, LLM streaming, plugin events)
  │
  ├─ plugins still emit AgentStart / AgentComplete events
  │   (ConnectionHolder listens, but for UI forwarding, not state tracking)
  │
  └─ RunningGuard drops:
       is_running ← false
       current_input ← None
       current_run_id ← None
```

### Status Query (reads from agent)

```
AgentHandler::Status        AgentHandler::List
  │                             │
  │  router.agent_status(id)    │  router.list_agent_statuses()
  │         │                   │         │
  │         ▼                   │         ▼
  │  Dispatcher                 │  for each dispatcher:
  │    └─ agent.is_running()    │    └─ agent.is_running()
  │    └─ agent.run_state()     │    └─ agent.run_state()
  │         │                   │         │
  │         ▼                   │         ▼
  │  RunningState {             │  RunningState { ... }
  │    is_running: bool,        │
  │    current_input: Option<>, │
  │    current_run_id: Option<> │
  │  }                          │
```

---

## Edge Cases

| Edge Case | Behavior |
|-----------|----------|
| `set_session()` while `is_running()` | Returns `Err(AgentBusyError)`. SessionHandler propagates to client. |
| `run_input()` while `is_running()` | `AtomicBool::swap` detects already-true → returns `Err(AlreadyRunning)`. |
| Agent panics during `run_input()` | `RunningGuard::drop` resets `is_running` + metadata. No stuck state. |
| `set_session()` before first run | OK — agent is idle. Session swapped in-place via `RwLock::write()`. |
| Concurrent `set_session()` calls | `RwLock::write()` serializes. Last writer wins. |
| `session()` read during `set_session()` | `RwLock` allows concurrent reads. Reader gets old or new — both valid `Arc<Session>`. |
| `AgentRuntime` shutdown with running agent | `run_state.is_running` is checked via stored `Arc<RunningState>`. Waits up to 30s. |
| `AgentConfig` builder from external crate | Unchanged. Builder produces config, `ReActAgent::new(config)` consumes it. |
| `RunContext` needs session | `agent.session()` → `Arc<Session>` clone. Same behavior as today's `config.session.clone()`. |
| `AgentConfig::default()` for tests | Session field becomes `RwLock::new(Arc::new(Session::new(...)))`. Minor caller adjustment. |

---

## Crate / File Structure

```
crates/vol-llm-agent/src/react/
├── agent.rs              # MODIFIED: +RunningState, +AtomicBool, +RunningGuard
│                         #   +set_session(), +is_running(), +session(), +config()
│                         #   -with_session(), -with_new_session()
├── agent_config.rs       # NEW: AgentConfig extracted, Clone removed
│                         #   session: RwLock<Arc<Session>>
├── config_builder.rs     # MODIFIED: build() wraps session in RwLock
├── running_state.rs      # NEW: RunningState struct + RunningGuard RAII
└── mod.rs                # MODIFIED: re-export RunningState

crates/vol-llm-agent-channel/src/
├── dispatcher.rs         # MODIFIED: swap_session() delegates to agent.set_session()
│                         #   Returns Result; busy lock stays for queue serialization
├── router.rs             # MODIFIED: swap_session() returns Result<(), ChannelError>
│                         #   +is_agent_running(agent_id) -> bool
│                         #   +agent_status(agent_id) -> Option<RunningState ref>
├── domain/
│   ├── agent.rs          # MODIFIED: Status/List query via router → agent.run_state()
│   └── session.rs        # MODIFIED: handle AgentBusyError in resume handler
└── server_core.rs        # MODIFIED: store Arc<RunningState> per agent for shutdown

crates/vol-llm-runtime/src/
└── lib.rs                # MODIFIED: agent_status replaced by RunningState refs
                          #   shutdown loop reads is_running from stored RunningState

crates/vol-llm-agent/src/
└── agent_tool.rs         # MODIFIED: use agent.session() instead of config.session
```

---

## Out of Scope

- Changing `Session` / `SessionEntryStore` internals
- Distributed locking (single-process scope)
- `AgentRuntime` builder or `AgentServerCoreBuilder` resource assembly changes
- Cancellation of in-flight `run_input()` via the `AtomicBool` (trivial to add later)
- Removing UI-layer `is_running` fields (those are UI-local derived state)
- Merging `AgentRuntime` and `AgentServerCore`

---

## Migration Strategy

Each step is independently compilable:

1. **Extract `AgentConfig`** to `agent_config.rs`, remove `Clone` derive
2. **Wrap session** in `RwLock<Arc<Session>>` inside `AgentConfig`, update builder
3. **Add `RunningState`** struct + `RunningGuard` in new file `running_state.rs`
4. **Add fields to `ReActAgent`**: `run_state: Arc<RunningState>`, wire in `new()`
5. **Add methods**: `session()`, `is_running()`, `set_session()`, `run_state()`, `config()`
6. **Update `run_input()`**: CAS is_running, create RunningGuard, use `self.session()`
7. **Remove** `with_session()`, `with_new_session()`
8. **Update `AgentDispatcher::swap_session()`**: call `agent.set_session()`, return `Result`
9. **Update `Router`**: `swap_session()` returns `Result`, add status query methods
10. **Update `SessionHandler`**: handle `AgentBusyError`
11. **Update `AgentHandler`**: Status/List query through router → agent
12. **Update `AgentRuntime`**: use `Arc<RunningState>` for shutdown check
13. **Fix all call sites**: `config.session` → `agent.session()` across all crates
14. **Run full test suite**

---

## Testing Strategy

- **Unit**: `RunningGuard` drop resets state, `AtomicBool::swap` re-entrancy prevention, `set_session()` rejection when running, concurrent `session()` reads during `set_session()`
- **Integration**: Session resume through full channel → agent path, status query returns real running/idle, shutdown waits for running agent, resume rejected while agent is running
- **Existing tests must pass**: `react_agent_integration.rs`, `session_history_test.rs`, `agent_run_tests.rs`, dispatcher tests, `agent_loki_integration.rs`
