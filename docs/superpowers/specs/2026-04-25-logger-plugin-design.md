# LoggerPlugin Design

**Date**: 2026-04-25
**Status**: Draft

## Summary

Replace the monolithic `ObservabilityPlugin` with a focused `LoggerPlugin` that only writes event logs to JSONL. Parameters are `store_dir` and `agent_id`; the base directory `{store_dir}/{agent_id}/` is derived on init. Each run writes to `{base_dir}/{run_id}.jsonl`.

## 1. Current Problem

`ObservabilityPlugin` conflates three concerns:
1. **JSONL run log** — writes events to disk
2. **Metrics** — TTFT, tool latency, call counts
3. **Tracing** — creates OpenTelemetry spans

Only the logging part is used by CodingAgent. Metrics and tracing add complexity and dependencies without delivering value.

## 2. LoggerPlugin

```rust
pub struct LoggerPlugin {
    base_dir: PathBuf,  // {store_dir}/logs/{agent_id}/
}

impl LoggerPlugin {
    pub fn new(store_dir: PathBuf, agent_id: String) -> Self {
        let base_dir = store_dir.join("logs").join(&agent_id);
        std::fs::create_dir_all(&base_dir).ok();
        Self { base_dir }
    }
}
```

### File Layout

```
{store_dir}/logs/{agent_id}/
├── {run_id_1}.jsonl          # AgentStreamEvent logs
├── {run_id_2}.jsonl
├── my_plugin/
│   └── {run_id_1}.jsonl      # PluginEvent logs for "my_plugin"
└── another_plugin/
    └── {run_id_2}.jsonl      # PluginEvent logs for "another_plugin"
```

### Plugin Event Handling

`PluginEvent` events have a `name` field identifying the source plugin.
When a `PluginEvent` is received, it is written to `{store_dir}/logs/{agent_id}/{plugin_name}/{run_id}.jsonl`
instead of the main run log. Other events go to `{store_dir}/logs/{agent_id}/{run_id}.jsonl`.

Each line is a `LogEntry`:

```json
{"timestamp":"2026-04-25T12:00:00Z","run_id":"abc","agent_id":"coding-agent","event":"AgentStart","data":{"input":"fix the bug"}}
```

### Plugin Behavior

- **`intercept()`**: returns `Continue` (passive, no blocking)
- **`listen()`**: 
  - For `PluginEvent { name, .. }`: writes to `{store_dir}/logs/{agent_id}/{name}/{run_id}.jsonl`
  - For all other events: writes to `{store_dir}/logs/{agent_id}/{run_id}.jsonl`
  - Creates directories on demand
  - Direct file I/O via `fs::OpenOptions::append` — no async spawn
  - Errors logged via `tracing::warn!` and swallowed (never block other plugins)

### LogEntry

```rust
#[derive(Serialize, Deserialize)]
pub struct LogEntry {
    pub timestamp: DateTime<Utc>,
    pub run_id: String,
    pub agent_id: String,
    pub event: String,
    pub data: Value,
}
```

Reuses the same `create_log_entry(event, run_id, agent_id)` function from the current `ObservabilityPlugin` — all 19 event variants mapped to JSON.

## 3. Files Changed

| File | Change |
|------|--------|
| `vol-llm-observability/src/lib.rs` | Export `LoggerPlugin`, `LogEntry`; remove `ObservabilityPlugin`, `ObservabilityConfig`, `MetricsCollector` |
| `vol-llm-observability/src/plugin.rs` | **Replace** `ObservabilityPlugin` with `LoggerPlugin` |
| `vol-llm-observability/src/run_log/logger.rs` | Keep `LogEntry`, simplify `RunLogLogger` (or inline into plugin) |
| `vol-llm-observability/src/config.rs` | **Delete** (no config struct needed) |
| `vol-llm-observability/src/metrics/` | **Delete** |
| `vol-llm-observability/src/tracing/` | **Delete** |
| `vol-llm-agents/src/coding/agent.rs` | Update any `ObservabilityPlugin` references to `LoggerPlugin` |
| `vol-llm-tui/src/main.rs` | Update any `ObservabilityPlugin` references |
| `vol-llm-agents/src/coding/tests.rs` | Update tests referencing ObservabilityPlugin |

## 4. Dependencies Removed

- `MetricsCollector` and all its sub-modules (`state.rs`, `summary.rs`)
- `tracing` spans module
- `ObservabilityConfig`
- `chrono` retained (for timestamps)
- `serde_json` retained (for JSONL)

## 5. Migration

Callers that used:
```rust
ObservabilityPlugin::new(agent_id, log_base_path)
```
Become:
```rust
LoggerPlugin::new(store_dir, agent_id)
```

Where `store_dir` is the parent of the previous `log_base_path` (the `logs/` subdir is auto-created).
