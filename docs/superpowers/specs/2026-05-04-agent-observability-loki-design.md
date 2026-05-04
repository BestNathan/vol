# Design Spec: Agent Observability → Loki Integration

## Architecture Overview

Agent events flow through the existing `AgentPlugin` system. The new `LokiPlugin` implements the same `listen()` hook as `LoggerPlugin`, creating a dual-write pattern: events go to both local JSONL and Loki simultaneously.

```
AgentStreamEvent → PluginRegistry
                     ├── LoggerPlugin.listen()  → local JSONL file
                     └── LokiPlugin.listen()    → mpsc → background task → HTTP POST → Loki
```

## Components

### vol-llm-observability crate (new loki/ module)

```
src/loki/
├── mod.rs       # Module exports: LokiConfig, LokiPlugin
├── config.rs    # LokiConfig: URL, batch_size, flush_interval
├── client.rs    # LokiWriter: background tokio task + batched HTTP
├── labels.rs    # LokiLabels: namespace, agent, agent_id
└── plugin.rs    # LokiPlugin: implements AgentPlugin trait
```

### LokiWriter (background batched writer)

- Spawned via `LokiWriter::spawn(url, batch_size, flush_interval)` — returns a `LokiWriter` with an mpsc sender
- Background task buffers entries and flushes when buffer reaches `batch_size` (default 50) or after `flush_interval_ms` (default 1000ms)
- Groups entries by label set for Loki Push API (`/loki/api/v1/push`)
- On channel close (all clones dropped), flushes remaining entries and exits
- No retry on failure — logs error via `tracing::error!`

### LokiPlugin (AgentPlugin implementation)

- `id()`: `"loki"`, `priority()`: 20 (runs after LoggerPlugin at priority 10)
- `listen()`: filters delta events, creates `LokiEntry`, sends to background writer (non-blocking)
- `intercept()`: no-op, returns `Continue`

### Labels Design

| Label | Value | Cardinality |
|-------|-------|-------------|
| `namespace` | `"agent"` (fixed) | 1 |
| `agent` | Agent type (e.g., `"coding"`) | Low (~4) |
| `agent_id` | Agent instance ID | Medium |
| `model` | LLM model name (only on LLMCallComplete) | Low |

High-cardinality fields (`run_id`, `session_id`) are placed in the log line content, not as labels, to avoid Loki performance issues. They are queried via LogQL line filters: `{namespace="agent"} |= "run-abc"`.

### Log Line Format

Each log entry is a compact JSON object:

```json
{
  "timestamp": "2026-05-04T12:00:00Z",
  "event": "ToolCallBegin",
  "run_id": "run-abc-123",
  "session_id": "sess-xyz",
  "agent_id": "agent_001",
  "tool_name": "bash",
  "tool_call_id": "c1",
  "arguments": "{}"
}
```

### Configuration

- `LOKI_URL` environment variable (primary)
- `LokiConfig::from_env()` returns `None` if not set
- `LokiConfig::with_url(url)` for explicit override
- Default batch_size=50, flush_interval=1000ms (configurable via builder)

### CodingAgentBuilder Integration

- `.with_loki()` method: reads `LOKI_URL` from env, creates and registers `LokiPlugin` if configured
- If env var not set, no plugin is registered (agent runs with local JSONL only)

```rust
CodingAgentBuilder::new()
    .with_logger()   // always: local JSONL
    .with_loki()     // conditional: Loki (only if LOKI_URL is set)
    .build()
```

## Error Handling

- Loki HTTP failure: log error via `tracing::error!`, drop the batch, continue
- Channel full: entry dropped with `tracing::warn!`
- No retry, no fallback, no circuit breaker
- Unconfigured Loki: `with_loki()` is a no-op

## Refactoring: Self-Bootstrapping LokiPlugin (2026-05-04)

### Problem

`LokiPlugin` stores `agent_type` at construction time and passes 5 separate parameters to `create_loki_entry()`. The type at construction (`"coding"`) may not match the actual `AgentDef.r#type` if the agent is dispatched from a file definition. All of `run_id`, `session_id`, `agent_id`, and `agent_type` are available from `RunContext`.

### Changes

#### LokiPlugin struct — remove `agent_type`

```rust
pub struct LokiPlugin {
    writer: Arc<LokiWriter>,
}
```

Constructor becomes `LokiPlugin::new(config: LokiConfig)` — no `agent_type` parameter.

#### `create_loki_entry` — derive from RunContext

```rust
pub fn create_loki_entry(event: &AgentStreamEvent, ctx: &RunContext) -> LokiEntry
```

Internally derive:
- `agent_type`: `ctx.config.def.as_ref().map(|d| &d.r#type).unwrap_or("unknown")`
- `agent_id`: `ctx.config.def.as_ref().map(|d| &d.name).unwrap_or("unknown")`
- `run_id`: `&ctx.run_id`
- `session_id`: `&ctx.session_id`

#### `listen()` simplifies

```rust
async fn listen(&self, event: &AgentStreamEvent, ctx: &RunContext) {
    if !Self::should_send(event) { return; }
    let entry = Self::create_loki_entry(event, ctx);
    self.writer.send(entry).await;
}
```

#### Call site update

```rust
// Before: vol_llm_observability::loki::LokiPlugin::new(config, "coding")
// After:  vol_llm_observability::loki::LokiPlugin::new(config)
```

`CodingAgentBuilder::with_loki()` and any other agent builders that register LokiPlugin need their call site updated.

#### Tests

`create_loki_entry` now takes `&RunContext`. Tests should build a minimal `RunContext` to exercise the real derivation path. Alternatively, keep a secondary `from_parts` variant for unit testing if constructing a full `RunContext` is too heavy.

### Affected files

| File | Change |
|------|--------|
| `crates/vol-llm-observability/src/loki/plugin.rs` | Remove `agent_type` field, refactor `new()`, `create_loki_entry()`, `listen()` |
| `crates/vol-llm-agents/src/coding/agent.rs` | Update `.with_loki()` call site |
| `crates/vol-llm-agents/src/advice/agent.rs` | Update if `.with_loki()` exists |
| `crates/vol-llm-agents/src/qa/agent.rs` | Update if `.with_loki()` exists |
| `crates/vol-llm-agents/src/ppt/agent.rs` | Update if `.with_loki()` exists |

### Risk

Low — internal refactoring only, no behavioral change to log output or Loki interaction.

## Dependencies

- `reqwest` (workspace) — HTTP client with rustls TLS
- `tokio` (workspace) — async runtime, mpsc channel, background task
- `serde` / `serde_json` (workspace) — JSON serialization for Loki Push API
