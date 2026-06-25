# Observability Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add full OpenTelemetry observability (traces + metrics + logs) to agent-server, agent-server-dingtalk, and docs-rs-mcp, exporting to the existing OTel Collector in the `observability` namespace.

**Architecture:** Extend `vol-llm-observability` crate with a unified `init()` that sets up OTel traces (→Tempo), metrics (→Prometheus), and logs (→Loki) via gRPC to the OTel Collector. Agent-server and docs-rs-mcp call `init()` at startup. Instrumentation uses `#[tracing::instrument]` for spans and existing `MetricsPlugin`/`LokiPlugin` AgentPlugin hooks for agent-level metrics and structured logs.

**Tech Stack:** Rust, OpenTelemetry SDK 0.29, tracing-opentelemetry 0.30, tower-http 0.5, axum 0.7, Kubernetes

---

## File Structure

| Action | File | Responsibility |
|--------|------|----------------|
| Modify | `crates/vol-agent-server/src/config.rs` | Add `OpenTelemetrySection` config struct + tests |
| Modify | `crates/vol-agent-server/Cargo.toml` | Add `vol-llm-observability`, `tower-http` trace feature |
| Modify | `crates/vol-agent-server/src/main.rs` | Replace tracing init with `vol_llm_observability::otel_init::init()` |
| Modify | `crates/vol-llm-observability/src/otel_init.rs` | Full OTel init: traces + metrics + logs |
| Modify | `crates/vol-llm-observability/src/lib.rs` | Re-export new `init()`, `OtelConfig`, `OtelGuards` |
| Modify | `crates/vol-llm-observability/Cargo.toml` | Add `tracing-opentelemetry` dep |
| Modify | `crates/vol-agent-server/src/app.rs` | Add `TraceLayer` to axum router |
| Modify | `crates/vol-llm-agent/src/react/agent.rs` | Add `#[instrument]` spans to `run_input()`, iteration loop, `execute_tool()` |
| Modify | `crates/vol-llm-mcp/src/manager.rs` | Add `#[instrument]` span to `call_tool()` |
| Modify | `crates/vol-llm-mcp/Cargo.toml` | Ensure `tracing` dep present |
| Modify | `crates/vol-mcp-servers/Cargo.toml` | Add `vol-llm-observability` dep |
| Modify | `crates/vol-mcp-servers/src/bin/docs_rs.rs` | Replace tracing init with OTel init |
| Modify | `Cargo.toml` (workspace) | Add `"metrics"` feature to `opentelemetry_sdk` and `opentelemetry-otlp` |
| Modify | `k8s/agent-server/configmap.yaml` | Add `[opentelemetry]` section |
| Modify | `k8s/agent-server/deployment.yaml` | Add `OTEL_SERVICE_NAME` env var |

---

### Task 1: Workspace Cargo.toml — Enable metrics features

**Files:**
- Modify: `Cargo.toml:99-100`

- [ ] **Step 1: Add "metrics" feature to workspace OTel deps**

In root `Cargo.toml`, change:

```toml
opentelemetry_sdk = { version = "0.29", features = ["tokio", "trace", "logs", "rt-tokio"] }
opentelemetry-otlp = { version = "0.29", features = ["tokio", "grpc-tonic", "logs"] }
```

To:

```toml
opentelemetry_sdk = { version = "0.29", features = ["tokio", "trace", "logs", "metrics", "rt-tokio"] }
opentelemetry-otlp = { version = "0.29", features = ["tokio", "grpc-tonic", "logs", "metrics"] }
```

- [ ] **Step 2: Verify workspace compiles**

Run: `cargo check -p vol-llm-observability 2>&1 | tail -5`
Expected: No errors (just feature flag additions, no code changes yet)

- [ ] **Step 3: Commit**

```bash
git add Cargo.toml
git commit -m "chore: enable metrics feature for opentelemetry workspace deps"
```

---

### Task 2: Add OpenTelemetrySection to agent-server config

**Files:**
- Modify: `crates/vol-agent-server/src/config.rs`
- Test: `crates/vol-agent-server/src/config.rs` (tests module, lines 303–603)

- [ ] **Step 1: Write failing test for OpenTelemetrySection defaults**

Add to `crates/vol-agent-server/src/config.rs` in the `mod tests` block (after the existing tests):

```rust
#[test]
fn test_opentelemetry_defaults() {
    let config = ServerConfig::default();
    assert!(!config.opentelemetry.enabled);
    assert_eq!(
        config.opentelemetry.endpoint,
        "http://otel-collector.observability.svc.cluster.local:4317"
    );
    assert_eq!(config.opentelemetry.service_name, "agent-server");
    assert_eq!(config.opentelemetry.service_namespace, "vol-agent");
    assert_eq!(config.opentelemetry.deployment_environment, "production");
    assert_eq!(config.opentelemetry.sample_rate, 1.0);
    assert_eq!(config.opentelemetry.batch_max_export_timeout_millis, 5000);
}

#[test]
fn test_parse_opentelemetry_toml() {
    let toml_str = r#"
[opentelemetry]
enabled = true
endpoint = "http://localhost:4317"
service_name = "test-agent"
sample_rate = 0.5
"#;
    let config: ServerConfig = toml::from_str(toml_str).unwrap();
    assert!(config.opentelemetry.enabled);
    assert_eq!(config.opentelemetry.endpoint, "http://localhost:4317");
    assert_eq!(config.opentelemetry.service_name, "test-agent");
    assert_eq!(config.opentelemetry.sample_rate, 0.5);
    // Defaults preserved for unset fields
    assert_eq!(config.opentelemetry.service_namespace, "vol-agent");
    assert_eq!(config.opentelemetry.deployment_environment, "production");
    assert_eq!(config.opentelemetry.batch_max_export_timeout_millis, 5000);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vol-agent-server --lib config::tests::test_opentelemetry_defaults 2>&1 | tail -10`
Expected: FAIL — `no field opentelemetry on type ServerConfig`

- [ ] **Step 3: Add OpenTelemetrySection struct and defaults**

Add to `crates/vol-agent-server/src/config.rs` after the `TracingSection` struct (around line 89):

```rust
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

fn default_otel_endpoint() -> String {
    "http://otel-collector.observability.svc.cluster.local:4317".to_string()
}
fn default_otel_service_name() -> String {
    "agent-server".to_string()
}
fn default_otel_service_namespace() -> String {
    "vol-agent".to_string()
}
fn default_otel_deployment_env() -> String {
    "production".to_string()
}
fn default_sample_rate() -> f64 {
    1.0
}
fn default_max_export_timeout_millis() -> u64 {
    5000
}

impl Default for OpenTelemetrySection {
    fn default() -> Self {
        Self {
            enabled: false,
            endpoint: default_otel_endpoint(),
            service_name: default_otel_service_name(),
            service_namespace: default_otel_service_namespace(),
            deployment_environment: default_otel_deployment_env(),
            sample_rate: default_sample_rate(),
            batch_max_export_timeout_millis: default_max_export_timeout_millis(),
        }
    }
}
```

- [ ] **Step 4: Add opentelemetry field to ServerConfig**

In `ServerConfig` struct (line 9–21), add the new field:

```rust
#[derive(Debug, Clone, Deserialize, Default)]
pub struct ServerConfig {
    #[serde(default)]
    pub server: ServerSection,
    #[serde(default)]
    pub control_plane: ControlPlaneSection,
    #[serde(default)]
    pub data_plane: DataPlaneSection,
    #[serde(default)]
    pub runtime: RuntimeSection,
    #[serde(default)]
    pub tracing: TracingSection,
    #[serde(default)]
    pub opentelemetry: OpenTelemetrySection,
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p vol-agent-server --lib config::tests::test_opentelemetry 2>&1 | tail -10`
Expected: Both `test_opentelemetry_defaults` and `test_parse_opentelemetry_toml` PASS

- [ ] **Step 6: Run all config tests to check no regressions**

Run: `cargo test -p vol-agent-server --lib config::tests 2>&1 | tail -15`
Expected: All tests PASS

- [ ] **Step 7: Commit**

```bash
git add crates/vol-agent-server/src/config.rs
git commit -m "feat(config): add OpenTelemetrySection to agent-server config"
```

---

### Task 3: Expand vol-llm-observability init() for full OTel stack

**Files:**
- Modify: `crates/vol-llm-observability/Cargo.toml`
- Modify: `crates/vol-llm-observability/src/otel_init.rs`
- Modify: `crates/vol-llm-observability/src/lib.rs`

- [ ] **Step 1: Add tracing-opentelemetry dep to vol-llm-observability**

In `crates/vol-llm-observability/Cargo.toml`, add to `[dependencies]`:

```toml
tracing-opentelemetry = { workspace = true }
```

- [ ] **Step 2: Rewrite otel_init.rs with full init()**

Replace the entire content of `crates/vol-llm-observability/src/otel_init.rs` with:

```rust
//! Full OTel initialization: traces + metrics + logs.
//!
//! Provides `init()` which sets up the complete tracing-subscriber stack
//! with OTel trace export (→Tempo), metrics export (→Prometheus), and
//! log export (→Loki), plus console and rolling-file layers.

use std::sync::OnceLock;

use opentelemetry::global;
use opentelemetry::trace::TracerProvider as _;
use opentelemetry::KeyValue;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::logs::SdkLoggerProvider;
use opentelemetry_sdk::metrics::SdkMeterProvider;
use opentelemetry_sdk::trace::{Sampler, SdkTracerProvider};
use opentelemetry_sdk::Resource;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{
    fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Layer, Registry,
};

static INITIALIZED: OnceLock<()> = OnceLock::new();

/// Configuration for OTel initialization.
#[derive(Debug, Clone)]
pub struct OtelConfig {
    pub enabled: bool,
    pub endpoint: String,
    pub service_name: String,
    pub service_namespace: String,
    pub deployment_environment: String,
    pub sample_rate: f64,
    pub batch_max_export_timeout_millis: u64,
}

/// Guards that keep OTel providers alive. Must be held for the app's lifetime.
/// Call `shutdown()` on app exit for graceful flush.
pub struct OtelGuards {
    pub tracer_provider: Option<SdkTracerProvider>,
    pub logger_provider: Option<SdkLoggerProvider>,
    pub meter_provider: Option<SdkMeterProvider>,
}

impl OtelGuards {
    /// Shutdown all providers gracefully, flushing pending spans/logs/metrics.
    pub fn shutdown(self) {
        if let Some(tp) = self.tracer_provider {
            let _ = tp.shutdown();
        }
        if let Some(lp) = self.logger_provider {
            let _ = lp.shutdown();
        }
        if let Some(mp) = self.meter_provider {
            let _ = mp.shutdown();
        }
        global::shutdown_tracer_provider();
    }
}

/// Initialize the full OTel stack: traces + metrics + logs.
///
/// Sets up:
/// 1. Console layer (stdout, colored)
/// 2. Rolling file layer (JSON, hourly rotation, 7-day retention)
/// 3. OTel trace layer (→Tempo via OTel Collector gRPC)
/// 4. OTel log layer (→Loki via OTel Collector gRPC)
/// 5. Global MeterProvider (→Prometheus via OTel Collector)
///
/// When `config.enabled` is false, only console + file layers are set up.
///
/// Returns `OtelGuards` that must be kept alive for the app's lifetime.
pub fn init(
    config: &OtelConfig,
    log_level: &str,
) -> Result<OtelGuards, Box<dyn std::error::Error + Send + Sync>> {
    if INITIALIZED.get().is_some() {
        return Ok(OtelGuards {
            tracer_provider: None,
            logger_provider: None,
            meter_provider: None,
        });
    }

    let resolved_endpoint = std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT")
        .unwrap_or_else(|_| config.endpoint.clone());
    let resolved_service_name = std::env::var("OTEL_SERVICE_NAME")
        .unwrap_or_else(|_| config.service_name.clone());
    let resolved_sample_rate: f64 = std::env::var("OTEL_SAMPLE_RATE")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(config.sample_rate);

    let resource = Resource::builder()
        .with_service_name(resolved_service_name.clone())
        .with_attributes([
            KeyValue::new("service.namespace", config.service_namespace.clone()),
            KeyValue::new("deployment.environment", config.deployment_environment.clone()),
        ])
        .build();

    let timeout = std::time::Duration::from_millis(config.batch_max_export_timeout_millis);

    // Console layer
    let console_layer = fmt::layer()
        .with_target(true)
        .with_file(true)
        .with_line_number(true)
        .with_ansi(true)
        .with_writer(std::io::stdout);

    // Rolling file layer (JSON)
    let file_appender = RollingFileAppender::builder()
        .rotation(Rotation::HOURLY)
        .filename_prefix("agent")
        .filename_suffix("log")
        .max_log_files(168) // 7 days
        .build(".")
        .unwrap_or_else(|_| {
            RollingFileAppender::builder()
                .rotation(Rotation::HOURLY)
                .filename_prefix("agent")
                .filename_suffix("log")
                .build("/tmp")
                .expect("Failed to create file appender in /tmp")
        });
    let file_layer = fmt::layer()
        .with_ansi(false)
        .with_target(true)
        .with_thread_ids(true)
        .with_file(true)
        .with_line_number(true)
        .json()
        .with_current_span(true)
        .with_writer(file_appender);

    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(log_level));

    if config.enabled && resolved_sample_rate > 0.0 {
        // OTel trace exporter
        let span_exporter = opentelemetry_otlp::SpanExporter::builder()
            .with_tonic()
            .with_endpoint(&resolved_endpoint)
            .with_timeout(timeout)
            .build()?;

        let sampler = if resolved_sample_rate >= 1.0 {
            Sampler::AlwaysOn
        } else {
            Sampler::TraceIdRatioBased(resolved_sample_rate)
        };

        let tracer_provider = SdkTracerProvider::builder()
            .with_sampler(sampler)
            .with_resource(resource.clone())
            .with_batch_exporter(span_exporter)
            .build();

        let tracer = tracer_provider.tracer(resolved_service_name.clone());
        let otel_trace_layer = tracing_opentelemetry::layer()
            .with_tracer(tracer)
            .with_location(true)
            .with_threads(true);

        global::set_tracer_provider(tracer_provider.clone());

        // OTel log exporter
        let log_exporter = opentelemetry_otlp::LogExporter::builder()
            .with_tonic()
            .with_endpoint(&resolved_endpoint)
            .with_timeout(timeout)
            .build()?;

        let logger_provider = SdkLoggerProvider::builder()
            .with_resource(resource.clone())
            .with_batch_exporter(log_exporter)
            .build();

        let otel_log_layer =
            opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge::new(&logger_provider);

        // OTel metrics exporter
        let metrics_exporter = opentelemetry_otlp::MetricExporter::builder()
            .with_tonic()
            .with_endpoint(&resolved_endpoint)
            .with_timeout(timeout)
            .build()?;

        let meter_provider = SdkMeterProvider::builder()
            .with_resource(resource.clone())
            .with_periodic_exporter(metrics_exporter)
            .build();

        global::set_meter_provider(meter_provider.clone());

        Registry::default()
            .with(env_filter)
            .with(console_layer)
            .with(file_layer)
            .with(otel_trace_layer)
            .with(otel_log_layer)
            .init();

        tracing::info!(
            endpoint = %resolved_endpoint,
            service = %resolved_service_name,
            sample_rate = resolved_sample_rate,
            "OpenTelemetry enabled: traces + metrics + logs"
        );

        INITIALIZED.get_or_init(|| ());

        Ok(OtelGuards {
            tracer_provider: Some(tracer_provider),
            logger_provider: Some(logger_provider),
            meter_provider: Some(meter_provider),
        })
    } else {
        // OTel disabled — console + file only
        type OtelLogLayer = opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge<
            SdkLoggerProvider,
            opentelemetry_sdk::logs::SdkLogger,
        >;

        Registry::default()
            .with(env_filter)
            .with(console_layer)
            .with(file_layer)
            .with(Option::<OtelLogLayer>::None)
            .init();

        tracing::info!("OpenTelemetry disabled — console + file logging only");

        INITIALIZED.get_or_init(|| ());

        Ok(OtelGuards {
            tracer_provider: None,
            logger_provider: None,
            meter_provider: None,
        })
    }
}

/// Backward-compatible init_otel_logs — delegates to init() with logs-only config.
/// Deprecated: prefer init() for new code.
pub fn init_otel_logs(
    endpoint: &str,
    service_name: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let config = OtelConfig {
        enabled: true,
        endpoint: endpoint.to_string(),
        service_name: service_name.to_string(),
        service_namespace: "vol-agent".to_string(),
        deployment_environment: "development".to_string(),
        sample_rate: 1.0,
        batch_max_export_timeout_millis: 5000,
    };
    let _guards = init(&config, "info")?;
    // Note: guards dropped here — providers shut down immediately.
    // This is the legacy behavior; new code should use init() and hold the guards.
    Ok(())
}
```

- [ ] **Step 3: Update lib.rs to export new types**

Replace `crates/vol-llm-observability/src/lib.rs` with:

```rust
//! vol-llm-observability: JSONL event logging and observability for LLM agents.
//!
//! Provides:
//! - A `LoggerPlugin` that writes structured run logs as JSONL files
//! - An `init()` function for full OTel initialization (traces + metrics + logs)
//! - A `LokiPlugin` that sends agent events to OTel via tracing macros
//! - A `MetricsPlugin` that records OTel metrics from agent events

pub mod loki_plugin;
pub mod metrics_plugin;
pub mod otel_init;
pub mod plugin;
pub mod run_log;

pub use loki_plugin::LokiPlugin;
pub use metrics_plugin::MetricsPlugin;
pub use otel_init::{init, init_otel_logs, OtelConfig, OtelGuards};
pub use plugin::LoggerPlugin;
pub use run_log::{append_log, LogEntry};
```

- [ ] **Step 4: Verify it compiles**

Run: `cargo check -p vol-llm-observability 2>&1 | tail -20`
Expected: Compiles without errors. (If `SdkMeterProvider` or `MetricExporter` don't resolve, the workspace Cargo.toml "metrics" feature from Task 1 wasn't applied.)

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-observability/
git commit -m "feat(observability): expand init() for full OTel traces + metrics + logs"
```

---

### Task 4: Wire agent-server main.rs to use OTel init

**Files:**
- Modify: `crates/vol-agent-server/Cargo.toml`
- Modify: `crates/vol-agent-server/src/main.rs`

- [ ] **Step 1: Add deps to agent-server Cargo.toml**

In `crates/vol-agent-server/Cargo.toml`, add to `[dependencies]`:

```toml
vol-llm-observability = { path = "../vol-llm-observability" }
```

- [ ] **Step 2: Replace tracing init in main.rs**

Replace the tracing init block in `crates/vol-agent-server/src/main.rs` (lines 37–51):

```rust
    // --- Init tracing ---
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(&config.tracing.level));

    match config.tracing.format.as_str() {
        "json" => {
            tracing_subscriber::fmt()
                .with_env_filter(env_filter)
                .json()
                .init();
        }
        _ => {
            tracing_subscriber::fmt().with_env_filter(env_filter).init();
        }
    }
```

With:

```rust
    // --- Init tracing + OTel ---
    let otel_config = vol_llm_observability::OtelConfig {
        enabled: config.opentelemetry.enabled,
        endpoint: config.opentelemetry.endpoint.clone(),
        service_name: config.opentelemetry.service_name.clone(),
        service_namespace: config.opentelemetry.service_namespace.clone(),
        deployment_environment: config.opentelemetry.deployment_environment.clone(),
        sample_rate: config.opentelemetry.sample_rate,
        batch_max_export_timeout_millis: config.opentelemetry.batch_max_export_timeout_millis,
    };
    let otel_guards = vol_llm_observability::init(&otel_config, &config.tracing.level)
        .expect("Failed to initialize tracing");
```

- [ ] **Step 3: Add graceful shutdown on app exit**

At the end of `main()`, after the `app::run(config).await` call, add shutdown. Change:

```rust
    if let Err(err) = app::run(config).await {
        tracing::error!("Server error: {}", err);
        std::process::exit(1);
    }
```

To:

```rust
    let result = app::run(config).await;
    otel_guards.shutdown();
    if let Err(err) = result {
        tracing::error!("Server error: {}", err);
        std::process::exit(1);
    }
```

- [ ] **Step 4: Verify it compiles**

Run: `cargo check -p vol-agent-server 2>&1 | tail -10`
Expected: Compiles without errors

- [ ] **Step 5: Run existing tests**

Run: `cargo test -p vol-agent-server --lib 2>&1 | tail -15`
Expected: All existing tests still pass

- [ ] **Step 6: Commit**

```bash
git add crates/vol-agent-server/
git commit -m "feat(agent-server): wire OTel init in main.rs with graceful shutdown"
```

---

### Task 5: Add tower-http TraceLayer to axum router

**Files:**
- Modify: `Cargo.toml:63` (workspace — add "trace" feature to tower-http)
- Modify: `crates/vol-agent-server/Cargo.toml`
- Modify: `crates/vol-agent-server/src/app.rs`

- [ ] **Step 1: Add "trace" feature to workspace tower-http dep**

In root `Cargo.toml`, change:

```toml
tower-http = { version = "0.5", features = ["cors", "fs"] }
```

To:

```toml
tower-http = { version = "0.5", features = ["cors", "fs", "trace"] }
```

- [ ] **Step 2: Add TraceLayer to router in app.rs**

In `crates/vol-agent-server/src/app.rs`, add the import at the top:

```rust
use tower_http::trace::TraceLayer;
```

In the `run()` function, after the `mount_ws_routes()` call (around line 397) and before `TcpListener::bind`, add:

```rust
    app = app.layer(TraceLayer::new_for_http());
```

So the section reads:

```rust
    app = mount_ws_routes(
        app,
        ws_owner,
        control_core,
        data_core,
        &config.control_plane.client_ws_path,
        &config.control_plane.node_ws_path,
    )?;

    app = app.layer(TraceLayer::new_for_http());

    let addr = format!("{}:{}", config.server.host, config.server.port);
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo check -p vol-agent-server 2>&1 | tail -10`
Expected: Compiles without errors

- [ ] **Step 4: Run tests**

Run: `cargo test -p vol-agent-server --lib 2>&1 | tail -10`
Expected: All tests pass

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml crates/vol-agent-server/
git commit -m "feat(agent-server): add tower-http TraceLayer for HTTP request spans"
```

---

### Task 6: Add #[instrument] spans to agent run loop and tool execution

**Files:**
- Modify: `crates/vol-llm-agent/src/react/agent.rs:284`
- Modify: `crates/vol-llm-agent/src/react/run_context.rs:287`

- [ ] **Step 1: Add #[instrument] to run_input()**

In `crates/vol-llm-agent/src/react/agent.rs`, find the `run_input` method signature (line 284):

```rust
    pub async fn run_input(&self, input: AgentInput) -> Result<AgentResponse, crate::AgentError> {
```

Change to:

```rust
    #[tracing::instrument(skip(self, input), fields(agent.run_id))]
    pub async fn run_input(&self, input: AgentInput) -> Result<AgentResponse, crate::AgentError> {
```

Then inside the function body, after `run_id` is assigned (after line 306 where `let run_id = input.run_id.clone().unwrap_or_else(...)`) add:

```rust
        tracing::Span::current().record("agent.run_id", &run_id);
```

- [ ] **Step 2: Add #[instrument] to execute_tool()**

In `crates/vol-llm-agent/src/react/run_context.rs`, find `execute_tool` (line 287):

```rust
    pub async fn execute_tool(
        &self,
        call: &vol_llm_core::ToolCall,
        ctx: &vol_llm_tool::ToolContext,
    ) -> vol_llm_tool::Result<vol_llm_tool::ToolResult> {
```

Change to:

```rust
    #[tracing::instrument(skip(self, ctx), fields(tool.name = %call.name))]
    pub async fn execute_tool(
        &self,
        call: &vol_llm_core::ToolCall,
        ctx: &vol_llm_tool::ToolContext,
    ) -> vol_llm_tool::Result<vol_llm_tool::ToolResult> {
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo check -p vol-llm-agent 2>&1 | tail -10`
Expected: Compiles without errors

- [ ] **Step 4: Run agent tests**

Run: `cargo test -p vol-llm-agent 2>&1 | tail -10`
Expected: All tests pass

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-agent/
git commit -m "feat(agent): add tracing instrument spans for agent.run and tool.call"
```

---

### Task 7: Add #[instrument] span to MCP call_tool

**Files:**
- Modify: `crates/vol-llm-mcp/src/manager.rs`

- [ ] **Step 1: Add #[instrument] to call_tool()**

In `crates/vol-llm-mcp/src/manager.rs`, find `call_tool` (around line 390):

```rust
    pub async fn call_tool(
        &self,
        server: &str,
        tool_name: &str,
        args: serde_json::Value,
    ) -> Result<String, McpError>
```

Add the instrument attribute:

```rust
    #[tracing::instrument(skip(self, args), fields(mcp.server = server, mcp.tool = tool_name))]
    pub async fn call_tool(
        &self,
        server: &str,
        tool_name: &str,
        args: serde_json::Value,
    ) -> Result<String, McpError>
```

- [ ] **Step 2: Verify tracing is in deps**

Run: `grep 'tracing' crates/vol-llm-mcp/Cargo.toml`
Expected: Should show `tracing` in dependencies. If missing, add `tracing = { workspace = true }`.

- [ ] **Step 3: Verify it compiles**

Run: `cargo check -p vol-llm-mcp 2>&1 | tail -10`
Expected: Compiles without errors

- [ ] **Step 4: Run MCP tests**

Run: `cargo test -p vol-llm-mcp 2>&1 | tail -10`
Expected: All tests pass

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-mcp/
git commit -m "feat(mcp): add tracing instrument span for MCP call_tool"
```

---

### Task 8: Wire MetricsPlugin + LokiPlugin into agent-server

**Files:**
- Modify: `crates/vol-agent-server/src/data_plane/core.rs:260`

- [ ] **Step 1: Register MetricsPlugin and LokiPlugin in register_agent()**

In `crates/vol-agent-server/src/data_plane/core.rs`, find the `register_agent` method. At line 260, just before `let agent = vol_llm_agent::ReActAgent::new(config);`, add plugin registration:

```rust
        // Register observability plugins
        config.plugin_registry.register(vol_llm_observability::MetricsPlugin::new());
        config.plugin_registry.register(vol_llm_observability::LokiPlugin::new());

        let agent = vol_llm_agent::ReActAgent::new(config);
```

The `PluginRegistry::register<P: AgentPlugin + 'static>(&mut self, plugin: P)` method accepts any type implementing `AgentPlugin`. Both `MetricsPlugin` and `LokiPlugin` implement this trait. `MetricsPlugin::new()` creates with a default OTel meter; `LokiPlugin::new()` creates a stateless plugin that emits structured log events.

- [ ] **Step 2: Verify it compiles**

Run: `cargo check -p vol-agent-server 2>&1 | tail -10`
Expected: Compiles without errors

- [ ] **Step 3: Commit**

```bash
git add crates/vol-agent-server/
git commit -m "feat(agent-server): register MetricsPlugin and LokiPlugin on agent runs"
```

---

### Task 9: Update docs-rs-mcp to use OTel init

**Files:**
- Modify: `crates/vol-mcp-servers/Cargo.toml`
- Modify: `crates/vol-mcp-servers/src/bin/docs_rs.rs`

- [ ] **Step 1: Add vol-llm-observability dep**

In `crates/vol-mcp-servers/Cargo.toml`, add to `[dependencies]`:

```toml
vol-llm-observability = { path = "../vol-llm-observability" }
```

- [ ] **Step 2: Replace tracing init in docs_rs.rs**

In `crates/vol-mcp-servers/src/bin/docs_rs.rs`, replace:

```rust
    tracing_subscriber::fmt().with_writer(std::io::stderr).init();
```

With:

```rust
    let otel_config = vol_llm_observability::OtelConfig {
        enabled: std::env::var("OTEL_ENABLED").map(|v| v == "true").unwrap_or(false),
        endpoint: std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT")
            .unwrap_or_else(|_| "http://otel-collector.observability.svc.cluster.local:4317".to_string()),
        service_name: "docs-rs-mcp".to_string(),
        service_namespace: "vol-agent".to_string(),
        deployment_environment: std::env::var("OTEL_ENV").unwrap_or_else(|_| "production".to_string()),
        sample_rate: 1.0,
        batch_max_export_timeout_millis: 5000,
    };
    let _otel_guards = vol_llm_observability::init(&otel_config, "info")
        .expect("Failed to initialize tracing");
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo check -p vol-mcp-servers 2>&1 | tail -10`
Expected: Compiles without errors

- [ ] **Step 4: Commit**

```bash
git add crates/vol-mcp-servers/
git commit -m "feat(docs-rs-mcp): wire OTel init for traces + metrics + logs"
```

---

### Task 10: Full workspace compile + test

- [ ] **Step 1: Full workspace check**

Run: `cargo check --workspace 2>&1 | tail -20`
Expected: All crates compile without errors

- [ ] **Step 2: Full workspace tests**

Run: `cargo test --workspace 2>&1 | tail -30`
Expected: All tests pass

- [ ] **Step 3: Fix any issues found**

If compilation or tests fail, fix inline and re-run.

- [ ] **Step 4: Commit any fixes**

```bash
git add -A
git commit -m "fix: resolve compilation/test issues from observability integration"
```

---

### Task 11: Update K8s ConfigMaps and Deployments

**Files:**
- Modify: `k8s/agent-server/configmap.yaml`
- Modify: `k8s/agent-server/deployment.yaml`

- [ ] **Step 1: Add [opentelemetry] section to ConfigMap**

In `k8s/agent-server/configmap.yaml`, add after the `[tracing]` section:

```yaml
    [opentelemetry]
    enabled = true
    endpoint = "http://otel-collector.observability.svc.cluster.local:4317"
    service_name = "agent-server"
    service_namespace = "vol-agent"
    deployment_environment = "production"
    sample_rate = 1.0
```

- [ ] **Step 2: Add OTEL env vars to Deployment**

In `k8s/agent-server/deployment.yaml`, add to the container's `env:` section:

```yaml
        - name: OTEL_SERVICE_NAME
          value: "agent-server"
        - name: OTEL_EXPORTER_OTLP_ENDPOINT
          value: "http://otel-collector.observability.svc.cluster.local:4317"
```

- [ ] **Step 3: Verify YAML syntax**

Run: `kubectl apply -f k8s/agent-server/configmap.yaml --dry-run=client 2>&1`
Run: `kubectl apply -f k8s/agent-server/deployment.yaml --dry-run=client 2>&1`
Expected: Both succeed with "configured (dry run)"

- [ ] **Step 4: Commit**

```bash
git add k8s/agent-server/
git commit -m "feat(k8s): add OpenTelemetry config to agent-server ConfigMap and Deployment"
```

---

### Task 12: Build Docker image and deploy to cluster

- [ ] **Step 1: Build and push Docker image**

Run:
```bash
docker build -f dockers/vol-agent-server.Dockerfile -t vol-agent-server:otel .
docker tag vol-agent-server:otel crpi-ck06yio90i1ttwlz.cn-beijing.personal.cr.aliyuncs.com/n_common/vol-agent-server:otel
docker push crpi-ck06yio90i1ttwlz.cn-beijing.personal.cr.aliyuncs.com/n_common/vol-agent-server:otel
```

- [ ] **Step 2: Deploy to cluster**

Run:
```bash
kubectl apply -f k8s/agent-server/configmap.yaml
kubectl apply -f k8s/agent-server/deployment.yaml
kubectl rollout restart deployment/agent-server -n vol-agent-system
```

- [ ] **Step 3: Verify pods start successfully**

Run: `kubectl get pods -n vol-agent-system -w`
Expected: All pods reach Running status. Check logs for "OpenTelemetry enabled" message.

- [ ] **Step 4: Verify traces in Tempo**

Open Grafana at `http://<node-ip>:31149`, go to Explore → Tempo, search for traces from `agent-server`.

- [ ] **Step 5: Verify logs in Loki**

In Grafana Explore → Loki, query `{service_name="agent-server"}` — should see log entries.

- [ ] **Step 6: Verify metrics in Prometheus**

In Grafana Explore → Prometheus, query `otel_agent_run_total` or similar — should see metric data.
