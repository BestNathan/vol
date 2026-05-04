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

## Dependencies

- `reqwest` (workspace) — HTTP client with rustls TLS
- `tokio` (workspace) — async runtime, mpsc channel, background task
- `serde` / `serde_json` (workspace) — JSON serialization for Loki Push API
