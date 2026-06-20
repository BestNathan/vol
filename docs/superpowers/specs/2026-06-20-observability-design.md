# Agent Server & MCP Server Observability Design

**Date**: 2026-06-20  
**Status**: Approved  
**Author**: Claude (brainstorming session)

## Overview

Add full observability (traces + metrics + logs) to the vol-agent-system cluster services:
- agent-server (control-plane and data-plane)
- agent-server-dingtalk
- docs-rs-mcp

All services are Rust-based and will use OpenTelemetry SDK to send data to the existing OTel Collector in the `observability` namespace.

## Goals

1. **Agent run traces**: Full request chain from agent.run → LLM calls → tool calls → MCP calls
2. **Service metrics**: HTTP/WebSocket latency, connection counts, error rates
3. **MCP call tracing**: Latency and success rate for each MCP server/tool
4. **Centralized logs**: All logs flow to Loki via OTel Collector

## Non-Goals

- Custom Grafana dashboards (can be added later)
- Alerting rules (can be added later)
- OTel Operator / Sidecar injection (direct connection is simpler)

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                     vol-agent-system namespace                  │
│                                                                 │
│  ┌─────────────┐  ┌──────────────┐  ┌─────────────────────┐   │
│  │agent-server │  │agent-server- │  │    docs-rs-mcp      │   │
│  │    (CP)     │  │   dingtalk   │  │                     │   │
│  │             │  │              │  │                     │   │
│  │ OTel SDK    │  │  OTel SDK    │  │    OTel SDK         │   │
│  └──────┬──────┘  └──────┬───────┘  └──────────┬──────────┘   │
│         │                │                      │              │
│         └────────────────┼──────────────────────┘              │
│                          │                                      │
│                     gRPC:4317                                   │
│                          │                                      │
└──────────────────────────┼──────────────────────────────────────┘
                           │
                           ▼
┌─────────────────────────────────────────────────────────────────┐
│                    observability namespace                       │
│                                                                 │
│  ┌─────────────────┐                                           │
│  │ otel-collector  │◄── Receives OTLP (gRPC:4317, HTTP:4318)  │
│  └────────┬────────┘                                           │
│           │                                                     │
│    ┌──────┼──────────────┐                                     │
│    │      │              │                                     │
│    ▼      ▼              ▼                                     │
│ ┌──────┐ ┌──────┐  ┌──────┐                                   │
│ │Tempo │ │Loki  │  │Prom  │                                   │
│ │(traces)│ │(logs)│  │(metrics)│                              │
│ └──────┘ └──────┘  └──────┘                                   │
│     │         │           │                                    │
│     └─────────┴───────────┘                                   │
│               │                                                │
│               ▼                                                │
│         ┌──────────┐                                          │
│         │ Grafana  │ (NodePort 31149)                         │
│         └──────────┘                                          │
└─────────────────────────────────────────────────────────────────┘
```

## Design Decisions

### Approach: Extend vol-llm-observability crate

**Chosen**: Extend the existing `vol-llm-observability` crate with full OTel initialization (traces + metrics + logs).

**Rejected alternatives**:
- Per-service independent init: Code duplication, maintenance burden
- vol-config unified config: Too much change to agent-server's existing config system

**Rationale**:
- Single crate provides consistent initialization across all services
- Reuses existing partial implementation (MetricsPlugin, LokiPlugin, init_otel_logs)
- Minimal changes to agent-server codebase

### Connection: Direct to OTel Collector

Services connect directly to `otel-collector.observability.svc.cluster.local:4317` via gRPC.

**Rejected**: OTel Operator + Sidecar injection (more complex, requires additional infrastructure)

## Configuration

### Config File Section

Add to `agent-server.toml`:

```toml
[opentelemetry]
enabled = true
endpoint = "http://otel-collector.observability.svc.cluster.local:4317"
service_name = "agent-server"
service_namespace = "vol-agent"
deployment_environment = "production"
sample_rate = 1.0
batch_max_export_timeout_millis = 5000
```

### Rust Config Structure

```rust
// crates/vol-agent-server/src/config.rs

#[derive(Debug, Clone, Deserialize)]
pub struct OpenTelemetrySection {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_otel_endpoint")]
    pub endpoint: String,
    #[serde(default = "default_otel_service_name")]
    pub service_name: String,
    #[serde(default = "default_otel_service_namespace")]
    pub service_namespace: String,
    #[serde(default = "default_otel_deployment_env")]
    pub deployment_environment: String,
    #[serde(default = "default_sample_rate")]
    pub sample_rate: f64,
    #[serde(default = "default_max_export_timeout_millis")]
    pub batch_max_export_timeout_millis: u64,
}

// Defaults
fn default_otel_endpoint() -> String {
    "http://otel-collector.observability.svc.cluster.local:4317".to_string()
}
fn default_otel_service_name() -> String { "agent-server".to_string() }
fn default_otel_service_namespace() -> String { "vol-agent".to_string() }
fn default_otel_deployment_env() -> String { "production".to_string() }
fn default_sample_rate() -> f64 { 1.0 }
fn default_max_export_timeout_millis() -> u64 { 5000 }
```

### Environment Variable Overrides

Environment variables take precedence over config file values:

| Variable | Overrides |
|----------|-----------|
| `OTEL_EXPORTER_OTLP_ENDPOINT` | `endpoint` |
| `OTEL_SERVICE_NAME` | `service_name` |
| `OTEL_SAMPLE_RATE` | `sample_rate` |

## vol-llm-observability Crate Expansion

### New API

```rust
// crates/vol-llm-observability/src/otel_init.rs

pub struct OtelConfig {
    pub enabled: bool,
    pub endpoint: String,
    pub service_name: String,
    pub service_namespace: String,
    pub deployment_environment: String,
    pub sample_rate: f64,
    pub batch_max_export_timeout_millis: u64,
}

pub struct OtelGuards {
    pub tracer_provider: Option<SdkTracerProvider>,
    pub logger_provider: Option<SdkLoggerProvider>,
}

impl OtelGuards {
    /// Shutdown all providers gracefully on app exit
    pub fn shutdown(self) {
        if let Some(tp) = self.tracer_provider {
            let _ = tp.shutdown();
        }
        if let Some(lp) = self.logger_provider {
            let _ = lp.shutdown();
        }
    }
}

/// Initialize full OTel stack: traces + metrics + logs
///
/// Returns guards that must be kept alive for the duration of the app.
pub fn init(
    config: &OtelConfig,
    log_level: &str,
) -> Result<OtelGuards, Box<dyn Error + Send + Sync>> {
    // Implementation:
    // 1. Build Resource with service attributes
    // 2. If enabled:
    //    a. Create SpanExporter → SdkTracerProvider → tracing_opentelemetry layer
    //    b. Create LogExporter → SdkLoggerProvider → OTel log bridge layer
    //    c. Set global TracerProvider and MeterProvider
    // 3. Console layer (stdout, colored)
    // 4. Combine all layers with EnvFilter
    // 5. Return guards
}
```

### Dependency Changes

```toml
# crates/vol-llm-observability/Cargo.toml (add if missing)
[dependencies]
tracing-opentelemetry = { workspace = true }
opentelemetry = { workspace = true }
opentelemetry_sdk = { workspace = true, features = ["tokio", "trace", "logs", "rt-tokio"] }
opentelemetry-otlp = { workspace = true, features = ["tokio", "grpc-tonic", "logs"] }
opentelemetry-appender-tracing = { workspace = true }
```

agent-server also needs the dependency:

```toml
# crates/vol-agent-server/Cargo.toml (add)
vol-llm-observability = { path = "../vol-llm-observability" }
tower-http = { workspace = true, features = ["trace"] }  # add "trace" feature
```

docs-rs-mcp also needs the dependency:

```toml
# crates/vol-mcp-servers/Cargo.toml (add)
vol-llm-observability = { path = "../vol-llm-observability" }
```

### Data Flow

```
Tracing macros (info!, span!, etc.)
         │
    ┌────┴────┐
    │         │
    ▼         ▼
Console    OTel Layer
(stdout)      │
         ┌────┴────┐
         │         │
         ▼         ▼
      Traces    Logs
   (Tempo)    (Loki)
         │
         ▼
   SpanExporter
   (gRPC→OTel Collector:4317)
```

## Instrumentation

### Traces (Spans)

| Span Name | Location | Attributes |
|-----------|----------|------------|
| `agent.run` | agent handler | run_id, agent_name, iterations, status |
| `agent.iteration` | ReAct loop | iteration, model |
| `llm.call` | provider call | provider, model, input_tokens, output_tokens |
| `tool.call` | tool execution | tool_name, status, error |
| `mcp.call` | MCP client | server_name, tool_name |
| `ws.connection` | WebSocket handler | direction, client_id |

### Metrics

| Metric Name | Type | Labels |
|-------------|------|--------|
| `agent_run_total` | Counter | agent_name, status |
| `agent_run_duration_seconds` | Histogram | agent_name |
| `llm_call_duration_seconds` | Histogram | provider, model |
| `llm_tokens_total` | Counter | provider, direction(in/out) |
| `mcp_call_duration_seconds` | Histogram | server, tool |
| `ws_connections_active` | Gauge | direction |
| `http_request_duration_seconds` | Histogram | method, path, status |

### Implementation Strategy

1. **AgentPlugin reuse**: In `DataPlaneServerCoreBuilder::build()` (or the agent run handler), register `MetricsPlugin` and `LokiPlugin` from `vol-llm-observability` on the ReAct agent. These plugins listen to `AgentStreamEvent` and automatically record OTel metrics / export logs.

2. **Axum HTTP middleware**: Add `tower_http::trace::TraceLayer` to the axum router for automatic HTTP request spans. Apply to all routes via `.layer(TraceLayer::new_for_http())`. For WebSocket handlers, add manual spans since TraceLayer doesn't cover WS upgrades.

3. **MCP client spans**: In `vol-llm-mcp` crate's call path (e.g., `McpManager::call_tool`), wrap each tool call in a `tracing::instrument` span named `mcp.call`.

4. **Span nesting**: `agent.run` → `agent.iteration` → `llm.call` / `tool.call` → `mcp.call`. Use `#[tracing::instrument]` on the relevant functions to achieve automatic parent-child relationships via the tracing context.

### Example Trace (Tempo)

```
agent.run (run_id=abc, 3.2s)
├── agent.iteration (iter=1, 1.5s)
│   ├── llm.call (model=gpt4, 1.2s, tokens=500)
│   └── tool.call (name=search, 0.3s)
│       └── mcp.call (server=docs-rs, 0.25s)
└── agent.iteration (iter=2, 1.7s)
    ├── llm.call (model=gpt4, 1.4s, tokens=600)
    └── tool.call (name=write_file, 0.2s)
```

## K8s Deployment Changes

### ConfigMap Update

Update `k8s/agent-server/configmap.yaml` to include `[opentelemetry]` section:

```yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: agent-server-config
  namespace: vol-agent-system
data:
  agent-server.toml: |
    [server]
    host = "0.0.0.0"
    port = 3001

    [server.roles]
    control_plane = true
    data_plane = false

    [opentelemetry]
    enabled = true
    endpoint = "http://otel-collector.observability.svc.cluster.local:4317"
    service_name = "agent-server-cp"
    sample_rate = 1.0

    [tracing]
    level = "info"
    format = "json"
```

### Deployment Environment Variables

Add to each deployment:

```yaml
env:
  # OTel overrides (per-pod customization)
  - name: OTEL_SERVICE_NAME
    value: "agent-server-cp"  # varies per deployment
  - name: OTEL_EXPORTER_OTLP_ENDPOINT
    value: "http://otel-collector.observability.svc.cluster.local:4317"
```

### Service Name Mapping

| Deployment | OTEL_SERVICE_NAME |
|------------|-------------------|
| agent-server (CP) | `agent-server-cp` |
| agent-server-dp (DP) | `agent-server-dp` |
| agent-server-dingtalk | `agent-server-dingtalk` |
| docs-rs-mcp | `docs-rs-mcp` |

### Network Policy (if applicable)

Ensure `vol-agent-system` pods can reach `observability:4317` (gRPC).

## Migration Steps

1. Add `OpenTelemetrySection` to `vol-agent-server/src/config.rs`
2. Expand `vol-llm-observability/src/otel_init.rs` with full `init()` function
3. Update `vol-agent-server/src/main.rs` to call `init()` and hold guards
4. Enable `MetricsPlugin` and `LokiPlugin` in agent-server runtime
5. Add `tower-http` TraceLayer to axum router
6. Update docs-rs-mcp binary similarly
7. Update K8s ConfigMaps and Deployments
8. Test in cluster, verify traces in Tempo, logs in Loki, metrics in Prometheus

## Testing

- Unit tests for config parsing with `[opentelemetry]` section
- Integration test: start agent-server with OTel enabled, verify spans exported
- Manual verification: query Tempo for traces, Loki for logs, Prometheus for metrics

## Future Enhancements

- Custom Grafana dashboard for agent runs
- Alerting rules (e.g., high error rate, slow LLM calls)
- Distributed tracing context propagation across MCP boundaries
