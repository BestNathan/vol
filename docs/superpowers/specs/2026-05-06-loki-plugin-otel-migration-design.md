# Design: LokiPlugin → OTel SDK via opentelemetry-appender-tracing

## Architecture Overview

LokiPlugin is rewritten to use `tracing::info!` macros for structured logging. The `tracing-subscriber` stack, extended with `opentelemetry-appender-tracing`, routes these logs through the OTel SDK to the OTel Collector. LokiPlugin itself holds no state, no endpoint, and makes no HTTP calls.

```
tracing::info!(...) ─────┐
                         │
   tracing-subscriber    │
   ┌──────────────────┐  │
   │  fmt::layer      │──┼→ stdout + JSONL files (existing, unchanged)
   │                  │  │
   │  OTel log layer  │──┘
   └───────┬──────────┘
           │
   OTel SDK BatchLogProcessor
           │
   opentelemetry-otlp (gRPC)
           │
   OTel Collector (endpoint from env)
```

## Crate Structure

### vol-monitor (initialization)

**Role:** Central OTel initialization point. Already contains `tracing_setup.rs` which sets up console, file, and OTel trace layers.

**Changes:**
- `src/tracing_setup.rs` — extend `init()` to call new `init_otel_logs()`
- New `init_otel_logs()` function creates OTel log exporter and integrates with tracing stack
- `Cargo.toml` — add `opentelemetry-appender-tracing`

### vol-llm-observability (LokiPlugin)

**Role:** Agent plugin for observability.

**Changes:**
- `src/loki/plugin.rs` — rewrite `listen()` to call `tracing::info!` with structured fields
- `src/loki/client.rs` — **removed**
- `src/loki/config.rs` — **removed**
- `src/loki/labels.rs` — **removed**
- `src/loki/mod.rs` — remove re-exports of removed modules
- `Cargo.toml` — remove `reqwest`, add `opentelemetry`, `opentelemetry_sdk`

### vol-llm-agent (RunContext)

**Role:** Agent execution context.

**Changes:**
- `src/react/run_context.rs` — add `pub model: String` field
- `src/react/agent.rs` — pass `llm.model()` into `RunContext::new()`
- `Cargo.toml` — no change (already has `vol-llm-core` for LLMClient trait)

### vol-config (configuration)

**Role:** Config types for tracing/OTel.

**Changes:**
- `src/tracing.rs` — `OpenTelemetryConfig` already has all needed fields (endpoint, service_name, etc.)
- May add `logs_enabled: bool` field for explicit control

## LokiPlugin Implementation

The plugin is stateless — no fields, no writer, no config:

```rust
pub struct LokiPlugin;

impl LokiPlugin {
    pub fn new() -> Self {
        Self
    }

    pub fn should_send(event: &AgentStreamEvent) -> bool {
        // Same logic: skip ThinkingDelta, ContentDelta, ToolCallArgumentDelta
        !matches!(event, ...)
    }
}

#[async_trait]
impl AgentPlugin for LokiPlugin {
    fn id(&self) -> String { "loki".to_string() }
    fn priority(&self) -> u32 { 20 }
    async fn intercept(&self, _: &AgentStreamEvent, _: &RunContext) -> PluginDecision {
        PluginDecision::Continue
    }
    async fn listen(&self, event: &AgentStreamEvent, ctx: &RunContext) {
        if !Self::should_send(event) { return; }
        let event_json = serde_json::to_string(event).unwrap_or_default();
        let def = ctx.config.def.as_ref();
        let agent_type = def.map(|d| &d.r#type).map_or("unknown", |v| v.as_str());
        let agent_id = def.map(|d| &d.name).map_or("unknown", |v| v.as_str());
        tracing::info!(
            namespace = "agent",
            session_id = ctx.session_id,
            agent_id = agent_id,
            agent_type = agent_type,
            run_id = ctx.run_id,
            model = ctx.model,
            event = %event_json,
            "agent_event"
        );
    }
}
```

## OTel Log Initialization

`init_otel_logs()` in `tracing_setup.rs`:

1. Read `OTEL_EXPORTER_OTLP_ENDPOINT` from env, fallback to `config.opentelemetry.endpoint` (default `http://localhost:4317`)
2. Create `OtlpLogExporter` with gRPC transport via `opentelemetry-otlp`
3. Create `LoggerProvider` with `BatchLogProcessor` and resource attributes (service.name, service.namespace, deployment.environment)
4. Set up `tracing-appender` + `tracing-opentelemetry` log layer
5. Integrate into the existing `tracing_subscriber` Registry

The function is called from the existing `init()` after trace setup, before subscriber initialization.

## RunContext.model Field

```rust
pub struct RunContext {
    // ... existing fields ...
    /// Model used for this run, from LLM config.
    pub model: String,
}
```

The `RunContext::new()` constructor gains a `model: String` parameter. The agent `run()` method extracts `self.config.llm.model()` and passes it. All test helpers that construct `RunContext` directly need updating.

## Dependencies

### Workspace `Cargo.toml`

```toml
# Before:
opentelemetry = "0.21"
opentelemetry_sdk = { version = "0.21", features = ["tokio", "trace", "rt-tokio"] }
opentelemetry-otlp = { version = "0.14", features = ["tokio", "grpc-tonic"] }
tracing-opentelemetry = "0.22"

# After:
opentelemetry = "0.27"
opentelemetry_sdk = { version = "0.27", features = ["tokio", "trace", "logs", "rt-tokio"] }
opentelemetry-otlp = { version = "0.27", features = ["tokio", "grpc-tonic", "logs"] }
opentelemetry-appender-tracing = "0.27"
tracing-opentelemetry = "0.30"
```

### Crate-level changes

| Crate | Remove | Add |
|-------|--------|-----|
| vol-monitor | - | `opentelemetry-appender-tracing` |
| vol-llm-observability | `reqwest` | `opentelemetry`, `opentelemetry_sdk` |
| vol-config | - | no change |
| vol-llm-agent | - | no change |

## Error Handling

- **OTel Collector unavailable:** OTel `BatchLogProcessor` buffers and drops on timeout; does not block agent execution
- **OTel not initialized:** `tracing::info!` falls through to console/file layers; no error from plugin
- **Model field empty:** `RunContext::new()` normalizes empty string to `"unknown"`

## Testing

- LokiPlugin tests updated to verify structured fields via `tracing` output inspection
- `init_otel_logs()` unit test: verifies env var reading, exporter creation
- RunContext tests: verify model field propagation
