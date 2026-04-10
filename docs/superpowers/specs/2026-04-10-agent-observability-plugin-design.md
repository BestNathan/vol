# Agent Observability Plugin Design

**Date:** 2026-04-10  
**Status:** Design Approved

---

## Overview

Design a comprehensive observability plugin for the ReAct Agent that provides:
- Structured JSONL logging to files
- Human-readable stdout output
- Automatic log rotation and retention
- Agent-centric log organization

---

## Requirements

### Functional Requirements

| ID | Requirement | Priority |
|----|-------------|----------|
| R1 | Log all AgentStreamEvent events | High |
| R2 | Write logs in JSONL format to files | High |
| R3 | Print human-readable logs to stdout | High |
| R4 | Organize logs by agent_id | High |
| R5 | Session logs rotate by date (YYYYMMDD) | High |
| R6 | Run logs use run_id as filename | High |
| R7 | Retain session logs for 7 days | High |
| R8 | Retain last 10 run logs | High |
| R9 | Auto-cleanup on agent startup | Medium |

### Non-Functional Requirements

| ID | Requirement | Priority |
|----|-------------|----------|
| N1 | Async log writes (non-blocking) | High |
| N2 | Graceful error handling (no panics) | High |
| N3 | Minimal performance overhead | Medium |

---

## Architecture

### Log Directory Structure

```
logs/agents/
├── vol_advice/                    # agent_id
│   ├── sessions/
│   │   ├── session_<id>_20260410.jsonl
│   │   ├── session_<id>_20260409.jsonl
│   │   └── ...
│   └── runs/
│       ├── run_<run_id>.jsonl
│       └── ...
└── vol_code_assistant/
    └── ...
```

### File Naming Conventions

| Log Type | Pattern | Example |
|----------|---------|---------|
| Session | `session_{session_id}_{YYYYMMDD}.jsonl` | `session_sess_abc123_20260410.jsonl` |
| Run | `run_{run_id}.jsonl` | `run_run_xyz789.jsonl` |

### Log Entry Format (JSONL)

```json
{"timestamp":"2026-04-10T12:34:56.789Z","run_id":"run_abc123","agent_id":"vol_advice","event":"AgentStart","data":{"input":"analyze market"}}
{"timestamp":"2026-04-10T12:34:57.123Z","run_id":"run_abc123","agent_id":"vol_advice","event":"ToolCallBegin","data":{"tool_name":"get_price","arguments":"{\"symbol\":\"BTC\"}"}}
```

### Stdout Format

```
[INFO] [vol_advice] [run_abc123] Agent started - input: "analyze market"
[INFO] [vol_advice] [run_abc123] Tool call: get_price({"symbol":"BTC"})
[INFO] [vol_advice] [run_abc123] Tool result: 69000
[INFO] [vol_advice] [run_abc123] Agent completed - iterations: 1, tools: 1
```

---

## Components

### 1. AgentConfig Extension

```rust
pub struct AgentConfig {
    // Existing fields
    pub max_iterations: u32,
    pub max_history_messages: usize,
    pub prompt_context: PromptContext,
    pub verbose: bool,
    pub plugin_registry: PluginRegistry,
    
    // New fields
    pub agent_id: String,           // Required: passed by user
    pub log_base_path: PathBuf,     // Default: "logs/agents"
}
```

### 2. ObservabilityPlugin

**Location:** `crates/vol-llm-agent/src/observability/plugin.rs`

```rust
pub struct ObservabilityPlugin {
    logger: Arc<ObservabilityLogger>,
}

impl ObservabilityPlugin {
    pub fn new(agent_id: String, log_base_path: PathBuf) -> Self;
}

#[async_trait::async_trait]
impl AgentPlugin for ObservabilityPlugin {
    fn id(&self) -> PluginId;
    fn priority(&self) -> u32;
    async fn intercept(&self, _event: &AgentStreamEvent, _ctx: &RunContext) -> PluginDecision;
    async fn listen(&self, event: &AgentStreamEvent, ctx: &RunContext);
}
```

### 3. ObservabilityLogger

**Location:** `crates/vol-llm-agent/src/observability/logger.rs`

```rust
pub struct ObservabilityLogger {
    agent_id: String,
    log_base_path: PathBuf,
    session_tx: mpsc::Sender<LogEntry>,
    run_tx: mpsc::Sender<LogEntry>,
}

impl ObservabilityLogger {
    // Initialize logger: create directories, spawn writer tasks
    pub fn new(agent_id: String, log_base_path: PathBuf) -> Self;
    
    // Log an event
    pub fn log(&self, entry: LogEntry, log_type: LogType);
}

pub enum LogType {
    Session { session_id: String, date: String },
    Run { run_id: String },
}

pub struct LogEntry {
    pub timestamp: DateTime<Utc>,
    pub run_id: String,
    pub agent_id: String,
    pub event: String,
    pub data: serde_json::Value,
}
```

### 4. Log Cleanup

**Location:** `crates/vol-llm-agent/src/observability/cleanup.rs`

```rust
/// Clean up old session logs (>7 days) and run logs (>10 files)
pub async fn cleanup_old_logs(agent_path: &Path) -> Result<(), LogError>;

/// Delete session logs older than retention_days
pub async fn cleanup_session_logs(sessions_path: &Path, retention_days: u32) -> Result<usize, LogError>;

/// Keep only the last max_runs run logs
pub async fn cleanup_run_logs(runs_path: &Path, max_runs: usize) -> Result<usize, LogError>;
```

**Cleanup Logic:**
- Run on agent startup (once per process)
- Session: parse date from filename, delete if > 7 days old
- Run: sort by filename (timestamp), keep last 10

---

## Data Flow

```
AgentStreamEvent
       │
       ▼
┌──────────────────────┐
│ ObservabilityPlugin  │
│   listen()           │
└──────────────────────┘
       │
       ▼
┌──────────────────────┐
│ ObservabilityLogger  │
│   log()              │
└──────────────────────┘
       │
       ├─────────────────┐
       ▼                 ▼
┌──────────────┐  ┌──────────────┐
│ Session Log  │  │   Run Log    │
│ (by date)    │  │  (by run_id) │
└──────────────┘  └──────────────┘
       │                 │
       ▼                 ▼
┌──────────────┐  ┌──────────────┐
│ JSONL File   │  │  JSONL File  │
│ + Stdout     │  │  + Stdout    │
└──────────────┘  └──────────────┘
```

---

## Error Handling

| Scenario | Behavior |
|----------|----------|
| Directory creation fails | Log error to tracing, continue without file logging |
| File write fails | Log error to tracing, continue (no retry) |
| Channel send fails | Silently ignore (receiver dropped) |
| Cleanup fails | Log error to tracing, continue |

**Principle:** Logging failures should never block or crash the agent.

---

## Testing Strategy

### Unit Tests

1. `test_logger_creates_directories` - Verify directory structure creation
2. `test_session_log_filename` - Verify session log naming pattern
3. `test_run_log_filename` - Verify run log naming pattern
4. `test_cleanup_session_logs` - Verify 7-day retention
5. `test_cleanup_run_logs` - Verify last-10 retention
6. `test_log_entry_format` - Verify JSONL structure
7. `test_stdout_format` - Verify human-readable output

### Integration Tests

1. `test_observability_plugin_logs_all_events` - End-to-end event logging
2. `test_full_agent_run_logged` - Complete agent run produces expected logs

---

## Acceptance Criteria

- [ ] All 8 AgentStreamEvent variants are logged
- [ ] Session logs rotate by date (new file per day)
- [ ] Run logs use run_id as filename
- [ ] Session logs older than 7 days are deleted
- [ ] Only last 10 run logs are retained
- [ ] JSONL format matches specification
- [ ] Stdout output is human-readable
- [ ] Cleanup runs on startup without blocking
- [ ] Logging errors don't crash the agent
- [ ] All tests pass (7 unit + 2 integration)

---

## Design Decisions

### Decision 1: Agent ID Required from User

**Decision:** Agent ID must be passed via `AgentConfig`, no environment variable fallback.

**Rationale:**
- Ensures logs are organized consistently
- User controls log location explicitly
- Enables log aggregation across runs (e.g., `vol_advice`)

### Decision 2: JSONL for File Logs

**Decision:** Use JSONL (JSON Lines) format for file logs.

**Rationale:**
- Easy to parse and process
- Append-only (no file locking issues)
- Compatible with log analysis tools

### Decision 3: Startup Cleanup

**Decision:** Run cleanup once on agent startup, not on every write.

**Rationale:**
- Minimal performance impact
- Simple implementation
- Adequate for most use cases

### Decision 4: Non-Breaking Logging

**Decision:** Logging failures are logged to tracing but never crash the agent.

**Rationale:**
- Logging is auxiliary, not core functionality
- Better to lose logs than lose agent execution

---

## Implementation Plan Reference

Implementation details will be defined in a separate plan document:
`docs/superpowers/plans/YYYY-MM-DD-agent-observability-implementation.md`
