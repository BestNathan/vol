# Observability Service Design

## Overview

An independent observability service that collects ReAct Agent runtime events, routes them to Loki (structured logs) and TDengine (time-series metrics), and exposes them to Grafana dashboards for real-time monitoring and aggregated analysis.

## Architecture

```
Agent (vol-llm-agent)
  └── ObservabilityPlugin (AgentPlugin)
       └── POST events → vol-observability service (HTTP /api/v1/events)
            ├── → Loki (structured logs, POST /loki/api/v1/push)
            └── → TDengine (metrics, via vol-tdengine client)

Grafana
  ├── Data Source: Loki → Dashboard A (Agent Run view)
  └── Data Source: TDengine → Dashboard A + Dashboard B (aggregated metrics)
```

### Data Flow

```
Agent listen() hook
  └── should_log() filters delta events (ThinkingDelta, ContentDelta, ToolCallArgumentDelta)
  └── serialize → tokio mpsc channel → background batch task
       └── POST /api/v1/events (batch, every 10 items or 500ms)

vol-observability service
  └── Axum HTTP handler → tokio mpsc → two BatchWriter tasks
       ├── LokiBatchWriter (50 items or 200ms → POST /loki/api/v1/push)
       └── TDengineBatchWriter (100 items or 500ms → bulk INSERT)
```

### Design Principles

- **Decoupled**: Agent and observability service are separate processes, independently deployable.
- **Non-blocking**: Agent events enter a bounded channel; if full, events are dropped (warn log).
- **No degradation**: ObservabilityPlugin does NOT fall back to local JSONL. LoggerPlugin and ObservabilityPlugin have independent responsibilities. Failed writes log ERROR and drop the batch.

## Agent-Side Integration

### ObservabilityPlugin

New plugin in `vol-llm-observability` crate, alongside existing `LoggerPlugin`:

- Implements `AgentPlugin` trait (`listen()` hook)
- Filters delta events via existing `should_log()` logic
- Serializes events with metadata and pushes to bounded `tokio::sync::mpsc` channel
- Background task batches and HTTP POSTs to ingest service

### Event Format

```json
{
  "run_id": "uuid",
  "session_id": "session-1",
  "agent_id": "agent-uuid",
  "agent_type": "CodingAgent",
  "timestamp": "2026-04-29T10:00:00Z",
  "event": "ToolCallComplete",
  "data": { "tool_call_id": "c1", "tool_name": "bash", "result": "...", "duration_ms": 150 }
}
```

### Metadata Fields

- `agent_id` — unique identifier for the agent instance
- `agent_type` — type name (CodingAgent, AdviceAgent, QaAgent, PptAgent)
- `run_id` — agent run UUID
- `session_id` — session identifier

These are injected by the plugin from `RunContext` / `AgentConfig`.

### Error Handling (Agent Side)

| Scenario | Behavior |
|----------|----------|
| Channel full | WARN log, drop new event |
| HTTP push fails | ERROR log, drop batch |
| Serialization fails | ERROR log, skip event |

## Ingest Service

### Crate Structure

New crate at `crates/vol-observability/`:

```
vol-observability/
├── src/
│   ├── main.rs          # Binary entrypoint, config loading, task startup
│   ├── lib.rs           # Public types
│   ├── config.rs        # TOML config structs
│   ├── ingest.rs        # Axum routes and handlers
│   ├── loki_writer.rs   # Loki batch writer
│   ├── tdengine_writer.rs # TDengine batch writer
│   └── event.rs         # Event types (can reuse AgentStreamEvent from vol-llm-core)
├── dashboards/          # Grafana dashboard JSON files
│   ├── agent-run.json
│   └── agent-metrics.json
└── Cargo.toml
```

### Dependencies

```toml
[dependencies]
axum = "0.7"
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
reqwest = { version = "0.12", features = ["json"] }
tracing = "0.1"
vol-llm-core = { path = "../vol-llm-core" }   # AgentStreamEvent types
vol-tdengine = { path = "../vol-tdengine" }    # TDengine client
vol-config = { path = "../vol-config" }        # TOML config loading
chrono = { version = "0.4", features = ["serde"] }
```

### HTTP API

| Method | Path | Description |
|--------|------|-------------|
| POST | `/api/v1/events` | Receive batch of agent events |
| GET | `/health` | Health check (returns 200 when all writers are healthy) |

### Configuration

```toml
[observability]
listen_addr = "0.0.0.0:3030"

[observability.loki]
enabled = true
url = "http://localhost:3100"
batch_size = 50
flush_interval_ms = 200

[observability.tdengine]
enabled = true
dsn = "taos://localhost:6030"
database = "vol_observability"
batch_size = 100
flush_interval_ms = 500
```

### Agent-Side Configuration (in vol-llm-agent config)

```toml
[agent.observability]
enabled = true
ingest_url = "http://localhost:3030/api/v1/events"
channel_capacity = 1000
batch_size = 10
flush_interval_ms = 500
```

## Loki Log Writing

### Label Strategy

Loki streams are identified by labels. The following fields are used as labels:

| Label | Source | Cardinality |
|-------|--------|-------------|
| `run_id` | Agent run UUID | Medium |
| `session_id` | Session identifier | Medium |
| `event_type` | Event name (ToolCallComplete, LLMCallStart, etc.) | Low |
| `agent_id` | Agent instance unique ID | Low-Medium |
| `agent_type` | Agent type (CodingAgent, AdviceAgent, etc.) | Low |
| `tool_name` | Only present on tool-related events | Low |

The following fields go into the JSON log line body (queryable via `| json` pipe in Grafana):
- All `data` field contents
- `input`, `response`, `thinking`, `content` (long text)
- `error` details

### Push API Format

```json
{
  "streams": [
    {
      "stream": {
        "run_id": "abc-123",
        "session_id": "session-1",
        "agent_id": "agent-uuid",
        "agent_type": "CodingAgent",
        "event_type": "ToolCallComplete",
        "tool_name": "bash"
      },
      "values": [
        ["1714370000000000000", "{\"tool_call_id\":\"c1\",\"result\":\"ok\",\"duration_ms\":150}"]
      ]
    }
  ]
}
```

### BatchWriter

- In-memory buffer, flushes on 50 items or 200ms (whichever comes first)
- Merges streams with identical labels within a single flush batch
- On failure: retry once, then drop. Log ERROR.

## TDengine Metrics

### Super Tables

```sql
-- Agent Run metrics
CREATE STABLE IF NOT EXISTS agent_run (
  ts TIMESTAMP,
  duration_ms BIGINT,
  iterations INT,
  tool_calls INT,
  final_answer_len INT,
  status TINYINT
) TAGS (run_id NCHAR(64), session_id NCHAR(64), agent_id NCHAR(64), agent_type NCHAR(64));

-- LLM Call metrics
CREATE STABLE IF NOT EXISTS llm_call (
  ts TIMESTAMP,
  duration_ms BIGINT,
  iteration INT,
  input_tokens INT,
  output_tokens INT,
  total_tokens INT
) TAGS (run_id NCHAR(64), session_id NCHAR(64), agent_id NCHAR(64), agent_type NCHAR(64), model NCHAR(64));

-- Tool Call metrics
CREATE STABLE IF NOT EXISTS tool_call (
  ts TIMESTAMP,
  duration_ms BIGINT,
  status TINYINT
) TAGS (run_id NCHAR(64), session_id NCHAR(64), agent_id NCHAR(64), agent_type NCHAR(64), tool_name NCHAR(128));
```

### Event-to-Metric Mapping

| Event | Table | Extracted Fields |
|-------|-------|-----------------|
| `AgentComplete` / `AgentAborted` | `agent_run` | duration_ms, iterations, tool_calls, status (0=complete, 1=aborted) |
| `LLMCallComplete` | `llm_call` | duration_ms (calculated from paired events), iteration, tokens from `usage` |
| `LLMCallError` | `llm_call` | status=error (e.g., -1) |
| `ToolCallComplete` | `tool_call` | duration_ms, status=0 (success) |
| `ToolCallError` | `tool_call` | duration_ms, status=1 (error) |

### BatchWriter

- Same pattern as Loki writer: memory buffer, flushes on 100 items or 500ms
- Uses TDengine's multi-row INSERT for efficient batch writes
- On failure: log ERROR, drop batch

## Error Handling (Service Side)

| Scenario | Behavior |
|----------|----------|
| Loki unreachable | ERROR log, drop batch |
| TDengine unreachable | ERROR log, drop batch |
| Serialization failure | ERROR log, skip event |

## Grafana Dashboards

### Dashboard A: Agent Run (Real-Time View)

| Panel | Type | Data Source | Description |
|-------|------|-------------|-------------|
| Run Info | Stat | TDengine `agent_run` | run_id, agent_type, status, total duration |
| Event Timeline | Timeline | Loki | Events as timeline bars, color-coded by event_type |
| Tool Call Table | Table | Loki | tool_name, args preview, result, duration, status |
| Thinking Content | Log | Loki | Filtered complete thinking text |
| Content Output | Log | Loki | Final output content |
| LLM Call Latency | Time series | TDengine `llm_call` | Per-call latency within a run |
| Token Usage | Bar gauge | TDengine `llm_call` | Input/output/total tokens per LLM call |

Variables: `run_id`, `agent_id`

### Dashboard B: Agent Aggregated Metrics

| Panel | Type | Description |
|-------|------|-------------|
| LLM Latency Trend | Time series | P50/P95/P99 aggregated by `$__interval` |
| LLM Call Volume | Time series | QPS grouped by model |
| Tool Success Rate | Stat | Success rate grouped by tool_name |
| Tool Error Top N | Table | Most failed tools in last N hours |
| Agent Run Success Rate | Stat | Grouped by agent_type |
| Agent Iteration Distribution | Histogram | Iteration count distribution |
| Token Consumption Trend | Time series | Total tokens by model |

Variables: time range, `agent_type`, `agent_id`, `session_id`

### Dashboard Provisioning

Dashboard JSON files live in `dashboards/` directory. Grafana provisioning config (YAML) auto-loads them on startup.
