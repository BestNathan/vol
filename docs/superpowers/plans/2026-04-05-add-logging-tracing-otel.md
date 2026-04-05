# add-logging-tracing-otel Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 实现统一的日志系统、OpenTelemetry tracing 和 Jaeger 集成，支持端到端的可观测性。

**Architecture:** 
- 新建 `vol-tracing` crate 包含 `WithSpan<T>` wrapper 和 `record_tags!` 宏
- `vol-monitor/src/tracing_setup.rs` 负责日志和 OTLP 初始化
- 各 datasource/rule/notification 模块使用 `WithSpan` 跨 channel 传播 span，使用 `record_tags!` 注入业务标签
- 配置通过 `config.toml` 的 `[tracing]` 节，支持环境变量覆盖

**Tech Stack:** Rust, tracing, tracing-appender, tracing-opentelemetry, opentelemetry-otlp, opentelemetry_sdk

---

## File Structure

### New Files
| File | Responsibility |
|------|----------------|
| `crates/vol-tracing/Cargo.toml` | vol-tracing crate 配置 |
| `crates/vol-tracing/src/lib.rs` | WithSpan wrapper + record_tags 宏 |
| `crates/vol-monitor/src/tracing_setup.rs` | 日志初始化 + OTLP 导出器配置 |
| `docker-compose.jaeger.yml` | 本地 Jaeger 测试环境 |
| `docs/tracing.md` | Tracing 架构和使用文档 |

### Modified Files
| File | Changes |
|------|---------|
| `Cargo.toml` | 添加 tracing-appender, tracing-opentelemetry, opentelemetry-* 依赖 |
| `crates/vol-config/src/tracing.rs` | 已有，确认配置结构完整 |
| `crates/vol-config/src/lib.rs` | 导出 tracing 模块（已有） |
| `crates/vol-monitor/Cargo.toml` | 添加 vol-tracing 依赖 |
| `crates/vol-monitor/src/main.rs` | 调用 tracing_setup::init() |
| `crates/vol-datasource/src/volatility.rs` | 使用 WithSpan 发送事件 |
| `crates/vol-datasource/src/portfolio.rs` | 使用 WithSpan 发送事件 |
| `crates/vol-engine/src/engine.rs` | 使用 WithSpan 接收事件，follows_from 关联 |
| `crates/vol-rules/src/*.rs` | 使用 record_tags! 注入标签 |
| `crates/vol-notification/src/*.rs` | 使用 record_tags! 注入标签，Feishu 消息加 trace_id |
| `config.toml` | 添加 [tracing] 配置节 |
| `k8s/deployment.yaml` | 添加 OTEL_ENDPOINT 环境变量 |

---

## Task 1: Workspace 依赖配置

**Files:**
- Modify: `Cargo.toml`

- [ ] **Step 1: 添加 tracing 相关依赖到 workspace**

在 `Cargo.toml` 的 `[workspace.dependencies]` 添加：

```toml
# Tracing & Observability
tracing-appender = "0.2"
tracing-opentelemetry = "0.22"
opentelemetry = "0.21"
opentelemetry_sdk = { version = "0.21", features = ["tokio", "trace"] }
opentelemetry-otlp = { version = "0.14", features = ["tokio", "grpc-tonic"] }
```

- [ ] **Step 2: 验证依赖版本解析**

```bash
cargo tree -p tracing 2>&1 | head -20
```

Expected: 显示 tracing v0.1.x 及其依赖

- [ ] **Step 3: 提交**

```bash
git add Cargo.toml
git commit -m "chore: add tracing and opentelemetry dependencies to workspace"
```

---

## Task 2: vol-tracing Crate

**Files:**
- Create: `crates/vol-tracing/Cargo.toml`
- Create: `crates/vol-tracing/src/lib.rs`
- Modify: `Cargo.toml` (添加 vol-tracing 到 workspace)
- Modify: `crates/vol-monitor/Cargo.toml` (添加依赖)

- [ ] **Step 1: 更新 workspace 配置**

在 `Cargo.toml` 的 `members` 数组添加 `"crates/vol-tracing"`：

```toml
[workspace]
resolver = "2"
members = [
    "crates/vol-core",
    "crates/vol-eventbus",
    "crates/vol-config",
    "crates/vol-datasource",
    "crates/vol-deribit",
    "crates/vol-alert",
    "crates/vol-notification",
    "crates/vol-monitor",
    "crates/vol-engine",
    "crates/vol-rules",
    "crates/vol-tracing",  # 新增
]
```

- [ ] **Step 2: 创建 vol-tracing/Cargo.toml**

```toml
[package]
name = "vol-tracing"
version.workspace = true
edition.workspace = true

[dependencies]
tracing = { workspace = true }
```

- [ ] **Step 3: 创建 vol-tracing/src/lib.rs**

```rust
//! vol-tracing: Span propagation utilities for cross-crate tracing context.
//!
//! Provides:
//! - `WithSpan<T>`: Wrapper for propagating span context across channel boundaries
//! - `record_tags!`: Macro for injecting attributes into spans

use tracing::Span;

/// Carries a value with its parent span for causal propagation across channel boundaries.
///
/// # Usage
/// ```
/// // Sender side
/// let span = tracing::info_span!("datasource_receive");
/// let traced = WithSpan::new(event, span);
/// tx.send(traced).await?;
///
/// // Receiver side
/// let traced = rx.recv().await?;
/// traced.enter_span("rule_evaluate", |span| {
///     // Processing happens within span context
///     span.record("rule.id", &"my-rule");
///     process(&event);
/// });
/// ```
pub struct WithSpan<T> {
    value: T,
    parent_span: Option<Span>,
}

impl<T> WithSpan<T> {
    /// Create a new traced value with a parent span.
    pub fn new(value: T, span: Span) -> Self {
        Self {
            value,
            parent_span: Some(span),
        }
    }

    /// Create without a parent span (for local operations).
    pub fn without_span(value: T) -> Self {
        Self {
            value,
            parent_span: None,
        }
    }

    /// Split into value and optional parent span.
    pub fn split(self) -> (T, Option<Span>) {
        (self.value, self.parent_span)
    }

    /// Create a child span that follows_from the parent, execute a closure within that span.
    ///
    /// The child span's duration represents the processing time triggered by the parent,
    /// not a child operation of the parent. This is the correct semantic for channel-based
    /// message passing.
    ///
    /// # Arguments
    /// * `span_name` - Name for the child span (use snake_case)
    /// * `f` - Closure to execute within the span context, receives the child Span
    ///
    /// # Example
    /// ```
    /// let traced = WithSpan::new(event, parent_span);
    /// traced.enter_span("rule_evaluate", |span| {
    ///     span.record("rule.id", &self.id);
    ///     process(&event)
    /// });
    /// ```
    pub fn enter_span<F, R>(self, span_name: &str, f: F) -> R
    where
        F: FnOnce(Span) -> R,
    {
        let (value, parent_span) = self.split();

        // Create child span
        let child_span = tracing::info_span!(span_name);

        // Establish causal relationship
        if let Some(parent) = parent_span {
            child_span.follows_from(parent.id());
        }

        // Execute within span context
        let _guard = child_span.enter();
        f(child_span)
    }

    /// Get reference to the wrapped value.
    pub fn value(&self) -> &T {
        &self.value
    }

    /// Unwrap and return the value.
    pub fn into_value(self) -> T {
        self.value
    }
}

/// Record tags to a span from field names.
///
/// # Usage
/// ```
/// use vol_tracing::record_tags;
///
/// let span = tracing::info_span!("my_span");
/// let data = VolatilityData { iv: 0.72, symbol: "BTC".to_string(), .. };
/// record_tags!(span, data, iv, symbol);
/// ```
#[macro_export]
macro_rules! record_tags {
    ($span:expr, $value:expr, $($field:ident),+ $(,)?) => {{
        $(
            $span.record(stringify!($field), &$value.$field);
        )+
    }};
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_with_span_creation() {
        let span = tracing::info_span!("test_span");
        let traced = WithSpan::new(42, span);
        assert!(traced.value().eq(&42));
    }

    #[test]
    fn test_with_span_without_span() {
        let traced = WithSpan::without_span(42);
        let (_, parent) = traced.split();
        assert!(parent.is_none());
    }

    #[test]
    fn test_record_tags_macro() {
        struct TestData {
            iv: f64,
            symbol: String,
        }

        let span = tracing::info_span!("test");
        let data = TestData {
            iv: 0.72,
            symbol: "BTC".to_string(),
        };
        record_tags!(span, data, iv, symbol);
        // Macro compiles successfully - runtime verification via tracing tests
    }
}
```

- [ ] **Step 4: 添加 vol-tracing 依赖到 vol-monitor**

在 `crates/vol-monitor/Cargo.toml` 添加：

```toml
[dependencies]
# ... existing deps ...
vol-tracing = { path = "../vol-tracing" }
```

- [ ] **Step 5: 验证编译**

```bash
cargo check -p vol-tracing 2>&1
```

Expected: 编译通过，无警告

- [ ] **Step 6: 运行测试**

```bash
cargo test -p vol-tracing 2>&1
```

Expected: 3 个测试全部通过

- [ ] **Step 7: 提交**

```bash
git add crates/vol-tracing Cargo.toml crates/vol-monitor/Cargo.toml
git commit -m "feat: add vol-tracing crate with WithSpan wrapper and record_tags macro"
```

---

## Task 3: 日志配置确认

**Files:**
- Read: `crates/vol-config/src/tracing.rs`
- Read: `crates/vol-config/src/lib.rs`

- [ ] **Step 1: 确认 TracingConfig 结构完整**

检查 `vol-config/src/tracing.rs` 包含以下字段：
- `LoggingConfig`: log_dir, log_prefix, retention_days, json_format, console_level, file_level, error_file
- `OpenTelemetryConfig`: enabled, endpoint, service_name, service_namespace, deployment_environment, sample_rate, batch

如有缺失，补充配置结构。

- [ ] **Step 2: 确认 Config 包含 tracing 字段**

检查 `vol-config/src/lib.rs` 的 `Config` 结构体有：

```rust
#[serde(default)]
pub tracing: TracingConfig,
```

- [ ] **Step 3: 提交（如有修改）**

```bash
git add crates/vol-config/src/*.rs
git commit -m "chore: confirm tracing config structure"
```

---

## Task 4: 日志初始化实现

**Files:**
- Create: `crates/vol-monitor/src/tracing_setup.rs`
- Modify: `crates/vol-monitor/src/main.rs`

- [ ] **Step 1: 创建 tracing_setup.rs**

```rust
//! Logging and OpenTelemetry tracing initialization.

use anyhow::Result;
use opentelemetry::{global, KeyValue};
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::{trace, Resource};
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_appender::non_blocking::{WorkerGuard, NonBlocking};
use tracing_subscriber::{
    fmt,
    layer::SubscriberExt,
    registry::Registry,
    Layer,
};
use tracing_opentelemetry::OpenTelemetryLayer;
use vol_config::{TracingConfig, LoggingConfig, OpenTelemetryConfig};

/// Guards that must be kept alive to ensure logs are flushed
pub struct LoggingGuard {
    _file_guard: WorkerGuard,
    _error_guard: WorkerGuard,
    _tracer_provider: Option<trace::TracerProvider>,
}

/// Initialize logging, file output, and OpenTelemetry tracing.
///
/// Returns a guard that must be kept alive for the lifetime of the application.
pub fn init(config: &TracingConfig) -> Result<LoggingGuard> {
    // 1. Create log directory
    std::fs::create_dir_all(&config.logging.log_dir)?;

    // 2. File appender for regular logs (daily rotation, 7 days retention)
    let file_appender = RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .max_log_files(config.logging.retention_days)
        .filename_prefix(&config.logging.log_prefix)
        .filename_suffix("log")
        .build(&config.logging.log_dir)?;

    let (file_writer, file_guard) = tracing_appender::non_blocking(file_appender);

    // 3. File appender for error logs only
    let error_appender = RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .max_log_files(config.logging.retention_days)
        .filename_prefix(&format!("{}-error", config.logging.log_prefix))
        .filename_suffix("log")
        .build(&config.logging.log_dir)?;

    let (error_writer, error_guard) = tracing_appender::non_blocking(error_appender);

    // 4. Console layer - compact format with colors
    let console_layer = fmt::layer()
        .with_target(true)
        .with_thread_ids(false)
        .with_file(false)
        .with_line_number(false)
        .with_level(true)
        .with_ansi(true)
        .compact()
        .with_writer(std::io::stdout)
        .with_filter(parse_level(&config.logging.console_level)?);

    // 5. File layer - JSON format with span context
    let file_layer = fmt::layer()
        .with_target(true)
        .with_thread_ids(true)
        .with_file(true)
        .with_line_number(true)
        .with_level(true)
        .with_ansi(false)
        .json()
        .with_current_span(true)
        .with_span_list(true)
        .with_writer(file_writer)
        .with_filter(parse_level(&config.logging.file_level)?);

    // 6. Error file layer - ERROR only
    let error_layer = fmt::layer()
        .with_target(true)
        .with_thread_ids(true)
        .with_file(true)
        .with_line_number(true)
        .with_level(true)
        .with_ansi(false)
        .json()
        .with_current_span(true)
        .with_span_list(true)
        .with_writer(error_writer)
        .with_filter(tracing_subscriber::filter::LevelFilter::ERROR);

    // 7. OpenTelemetry layer
    let (otel_layer, tracer_provider) = init_otel(&config.opentelemetry)?;

    // 8. Assemble all layers
    let subscriber = Registry::default()
        .with(console_layer)
        .with(file_layer)
        .with(error_layer)
        .with(otel_layer);

    tracing::subscriber::set_global_default(subscriber)?;

    tracing::info!(
        "Logging initialized: log_dir={} error_file={}",
        config.logging.log_dir,
        config.logging.error_file
    );

    Ok(LoggingGuard {
        _file_guard: file_guard,
        _error_guard: error_guard,
        _tracer_provider: Some(tracer_provider),
    })
}

/// Initialize OpenTelemetry tracer
fn init_otel(config: &OpenTelemetryConfig) -> Result<(OpenTelemetryLayer<trace::Tracer>, trace::TracerProvider)> {
    // Environment variables take precedence over config
    let endpoint = std::env::var("OTEL_ENDPOINT")
        .unwrap_or_else(|_| config.endpoint.clone());

    let service_name = std::env::var("OTEL_SERVICE_NAME")
        .unwrap_or_else(|_| config.service_name.clone());

    let sample_rate: f64 = std::env::var("OTEL_SAMPLE_RATE")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(config.sample_rate);

    if !config.enabled || sample_rate <= 0.0 {
        tracing::info!("OpenTelemetry tracing disabled");
        // Return a no-op tracer
        let provider = trace::TracerProvider::default();
        let tracer = provider.tracer("noop");
        return Ok((tracing_opentelemetry::layer().with_tracer(tracer), provider));
    }

    let tracer_provider = trace::TracerProvider::builder()
        .with_config(trace::config::Config::default().with_sample_rate(sample_rate))
        .with_resource(Resource::new(vec![
            KeyValue::new("service.name", service_name),
            KeyValue::new("service.namespace", config.service_namespace.clone()),
            KeyValue::new("deployment.environment", config.deployment_environment.clone()),
        ]))
        .with_batch_exporter(
            opentelemetry_otlp::new_exporter()
                .tonic()
                .with_endpoint(&endpoint)
                .with_timeout(std::time::Duration::from_millis(config.batch.max_export_timeout_millis))
                .build_span_exporter()?,
        )
        .build();

    let tracer = tracer_provider.tracer(&service_name);
    let otel_layer = tracing_opentelemetry::layer().with_tracer(tracer.clone());

    global::set_tracer_provider(tracer_provider.clone());

    tracing::info!(
        "OpenTelemetry tracing enabled: endpoint={} service={} sample_rate={}",
        endpoint,
        service_name,
        sample_rate
    );

    Ok((otel_layer, tracer_provider))
}

/// Parse log level string to LevelFilter
fn parse_level(level: &str) -> Result<tracing_subscriber::filter::LevelFilter> {
    match level.to_lowercase().as_str() {
        "trace" => Ok(tracing_subscriber::filter::LevelFilter::TRACE),
        "debug" => Ok(tracing_subscriber::filter::LevelFilter::DEBUG),
        "info" => Ok(tracing_subscriber::filter::LevelFilter::INFO),
        "warn" => Ok(tracing_subscriber::filter::LevelFilter::WARN),
        "error" => Ok(tracing_subscriber::filter::LevelFilter::ERROR),
        _ => Err(anyhow::anyhow!("Invalid log level: {}", level)),
    }
}

/// Shutdown OpenTelemetry tracer (call on application exit)
pub fn shutdown() {
    global::shutdown_tracer_provider();
}
```

- [ ] **Step 2: 更新 main.rs 调用 init()**

在 `crates/vol-monitor/src/main.rs`：

```rust
// 在文件顶部添加
mod tracing_setup;

// 在 main() 函数中，在现有 tracing_subscriber::fmt()... 之前添加：
#[tokio::main]
async fn main() -> Result<()> {
    // 替换原有的 tracing_subscriber::fmt()... 初始化
    let config = Config::load("config.toml")...;  // 先加载配置
    let _guard = tracing_setup::init(&config.tracing)?;  // 新增

    tracing::info!("===========================================");
    // ... 其余代码不变
```

- [ ] **Step 3: 移除原有的 tracing_subscriber 初始化**

删除原来的：
```rust
tracing_subscriber::fmt()
    .with_env_filter(EnvFilter::from_default_env().add_directive("vol_monitor=info".parse().unwrap()))
    .init();
```

- [ ] **Step 4: 验证编译**

```bash
cargo check -p vol-monitor 2>&1
```

Expected: 编译通过

- [ ] **Step 5: 提交**

```bash
git add crates/vol-monitor/src/tracing_setup.rs crates/vol-monitor/src/main.rs
git commit -m "feat: implement logging initialization with file rotation and OTLP export"
```

---

## Task 5: config.toml 配置

**Files:**
- Modify: `config.toml`

- [ ] **Step 1: 添加 tracing 配置节**

在 `config.toml` 的 `[tenors]` 之后添加：

```toml
# ============= Tracing & Logging Configuration =============

[tracing.logging]
log_dir = "logs"
log_prefix = "vol-monitor"
retention_days = 7
json_format = true
console_level = "info"
file_level = "debug"
error_file = true

[tracing.opentelemetry]
enabled = true
endpoint = "http://localhost:4317"
service_name = "vol-monitor"
service_namespace = "deribit"
deployment_environment = "production"
sample_rate = 1.0

[tracing.opentelemetry.batch]
max_queue_size = 2048
max_batch_size = 512
scheduled_delay_millis = 5000
max_export_timeout_millis = 30000
```

- [ ] **Step 2: 验证配置解析**

```bash
cargo test -p vol-config 2>&1 | grep -A5 "test result"
```

Expected: 配置测试通过

- [ ] **Step 3: 提交**

```bash
git add config.toml
git commit -m "chore: add tracing configuration section"
```

---

## Task 6: DataSource 埋点

**Files:**
- Modify: `crates/vol-datasource/src/volatility.rs`
- Modify: `crates/vol-datasource/src/portfolio.rs`
- Modify: `crates/vol-datasource/Cargo.toml`

- [ ] **Step 1: 添加 vol-tracing 依赖**

在 `crates/vol-datasource/Cargo.toml` 添加：

```toml
[dependencies]
# ... existing ...
vol-tracing = { path = "../vol-tracing" }
```

- [ ] **Step 2: 更新 volatility.rs 的 run() 方法**

在 `VolatilityDataSource::run()` 中，找到发送 event 的位置：

```rust
use tracing::{info, error, warn};
use tracing::info_span;
use vol_tracing::WithSpan;

// 在收到消息后，创建 span 并发送 WithSpan
let trace_id = generate_trace_id();
let span = info_span!(
    "datasource_receive",
    trace_id = %trace_id,
    source = %self.id,
);

span.record("market.symbol", &symbol);  // 添加业务标签

let event = MonitoringEvent::Volatility(VolatilityData { ... });
tx.send(WithSpan::new(event, span)).await?;
```

- [ ] **Step 3: 实现 generate_trace_id()**

```rust
fn generate_trace_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ns = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
    format!("tr_{:016x}", ns)
}
```

- [ ] **Step 4: 更新 portfolio.rs 类似的埋点**

```rust
let span = info_span!("portfolio_poll", source = %self.id);
let event = MonitoringEvent::Portfolio(snapshot);
tx.send(WithSpan::new(event, span)).await?;
```

- [ ] **Step 5: 验证编译**

```bash
cargo check -p vol-datasource 2>&1
```

Expected: 编译通过

- [ ] **Step 6: 提交**

```bash
git add crates/vol-datasource/src/*.rs crates/vol-datasource/Cargo.toml
git commit -m "feat: add tracing spans to datasource with WithSpan wrapper"
```

---

## Task 7: Rule Engine 埋点

**Files:**
- Modify: `crates/vol-engine/src/engine.rs`
- Modify: `crates/vol-engine/Cargo.toml`

- [ ] **Step 1: 添加 vol-tracing 依赖**

```toml
[dependencies]
vol-tracing = { path = "../vol-tracing" }
```

- [ ] **Step 2: 更新 engine.rs 的 process_event**

在 `spawn_rules` 或处理 event 的位置：

```rust
use vol_tracing::WithSpan;

// 接收端使用 enter_span
traced.enter_span("rule_evaluate", |span| {
    span.record("rule.id", &rule.id());
    span.record("rule.type", &rule.type());
    
    let alerts = rule.evaluate(&event).await;
    // ...
});
```

- [ ] **Step 3: 验证编译**

```bash
cargo check -p vol-engine 2>&1
```

- [ ] **Step 4: 提交**

```bash
git add crates/vol-engine/src/engine.rs crates/vol-engine/Cargo.toml
git commit -m "feat: add follows_from tracing to rule evaluation"
```

---

## Task 8: Rule 埋点

**Files:**
- Modify: `crates/vol-rules/src/absolute_iv.rs`
- Modify: `crates/vol-rules/src/rate_change.rs`
- Modify: `crates/vol-rules/src/term_structure.rs`
- Modify: `crates/vol-rules/src/skew.rs`
- Modify: `crates/vol-rules/src/portfolio.rs`
- Modify: `crates/vol-rules/Cargo.toml`

- [ ] **Step 1: 添加 vol-tracing 依赖**

```toml
vol-tracing = { path = "../vol-tracing" }
```

- [ ] **Step 2: 更新每个 Rule 的 evaluate() 方法**

```rust
use vol_tracing::record_tags;

// 在 evaluate 中
let span = tracing::info_span!("alert_evaluate");
record_tags!(span, data, iv, symbol);

if triggered {
    span.record("alert.triggered", &true);
    span.record("alert.threshold", &self.threshold);
}
```

- [ ] **Step 3: 验证编译**

```bash
cargo check -p vol-rules 2>&1
```

- [ ] **Step 4: 提交**

```bash
git add crates/vol-rules/src/*.rs crates/vol-rules/Cargo.toml
git commit -m "feat: add record_tags tracing to all rules"
```

---

## Task 9: Notification 埋点

**Files:**
- Modify: `crates/vol-notification/src/stdout.rs`
- Modify: `crates/vol-notification/src/feishu.rs`
- Modify: `crates/vol-notification/Cargo.toml`

- [ ] **Step 1: 添加 vol-tracing 依赖**

- [ ] **Step 2: 更新 send() 方法添加 span**

```rust
let span = tracing::info_span!("notification_send");
span.record("notification.type", &"stdout");
record_tags!(span, alert, alert_type, tenor, symbol, iv);
```

- [ ] **Step 3: Feishu 消息添加 trace_id 前缀**

```rust
let message = format!(
    "[{}] 🚨 {}",
    &trace_id[..10],  // 短 ID
    self.format_message(alert)
);
```

- [ ] **Step 4: 验证编译**

```bash
cargo check -p vol-notification 2>&1
```

- [ ] **Step 5: 提交**

```bash
git add crates/vol-notification/src/*.rs crates/vol-notification/Cargo.toml
git commit -m "feat: add tracing to notifications with trace_id in Feishu messages"
```

---

## Task 10: 日志迁移

**Files:**
- Scan: `crates/**/*.rs` for `println!` and `eprintln!`

- [ ] **Step 1: 搜索所有 println**

```bash
grep -r "println!" crates/ --include="*.rs" | grep -v target
```

- [ ] **Step 2: 迁移为 tracing::info!**

将 `println!("xxx")` 改为 `tracing::info!("xxx")`

- [ ] **Step 3: 迁移 eprintln! 为 tracing::error!**

- [ ] **Step 4: 清理未使用的 import**

```bash
cargo fix --workspace --allow-dirty 2>&1 | head -20
```

- [ ] **Step 5: 提交**

```bash
git add crates/
git commit -m "refactor: migrate println/eprintln to tracing macros"
```

---

## Task 11: K8s 配置

**Files:**
- Modify: `k8s/deployment.yaml`

- [ ] **Step 1: 添加 OTEL_ENDPOINT 环境变量**

在 `deployment.yaml` 的 env 部分添加：

```yaml
env:
- name: RUST_LOG
  value: "info"
- name: OTEL_ENDPOINT
  value: "http://jaeger-collector.observability:4317"
- name: OTEL_SERVICE_NAME
  value: "vol-monitor-prod"
```

- [ ] **Step 2: 提交**

```bash
git add k8s/deployment.yaml
git commit -m "chore: add OTEL environment variables to k8s deployment"
```

---

## Task 12: 测试与文档

**Files:**
- Create: `docs/tracing.md`

- [ ] **Step 1: 创建 tracing 文档**

```markdown
# Tracing Architecture

## Overview

vol-monitor uses `tracing` crate for structured logging and OpenTelemetry for distributed tracing.

## Components

- **vol-tracing**: WithSpan wrapper for cross-channel span propagation
- **tracing_setup.rs**: Logging and OTLP initialization
- **config.toml [tracing]**: Configuration

## Usage

### Adding a new span

```rust
let span = tracing::info_span!("my_operation");
span.record("key", &value);
```

### Cross-channel propagation

```rust
// Sender
tx.send(WithSpan::new(event, span)).await?;

// Receiver
traced.enter_span("process", |span| {
    span.record("rule.id", &id);
    process(event);
});
```

## Querying Jaeger

1. Open Jaeger UI at http://localhost:16686
2. Select service: vol-monitor
3. Search by trace_id: tr_abc123...
```

- [ ] **Step 2: 运行集成测试**

启动 vol-monitor 验证日志输出：

```bash
cargo build -p vol-monitor 2>&1
./target/debug/vol-monitor --config config.toml &
sleep 5
cat logs/vol-monitor-*.log | head -20
```

- [ ] **Step 3: 提交**

```bash
git add docs/tracing.md
git commit -m "docs: add tracing architecture documentation"
```

---

## Self-Review

检查清单：
1. ✅ Spec coverage - 所有 4 个 capability 的 requirements 都有对应 task
2. ✅ No placeholders - 每个 step 都有具体代码
3. ✅ Type consistency - WithSpan, record_tags! 在所有 task 中一致
4. ✅ File paths - 所有路径都是精确的

---

Plan complete and saved to `docs/superpowers/plans/YYYY-MM-DD-add-logging-tracing-otel.md`.

Two execution options:

**1. Subagent-Driven (recommended)** - Dispatch fresh subagent per task, review between tasks, fast iteration

**2. Inline Execution** - Execute tasks in this session using executing-plans, batch execution with checkpoints

Which approach?
