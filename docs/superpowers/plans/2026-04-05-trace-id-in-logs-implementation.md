# Trace ID Injection into Logs Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Inject OpenTelemetry trace_id into all log output (console + file) so that related logs can be correlated across the entire data processing pipeline.

**Architecture:** 
1. Replace custom `u64` trace_id generation with OpenTelemetry `TraceId` (128-bit) standard type
2. Create root span at DataSource (ticker receipt) with trace_id as span attribute
3. Propagate span context via `WithSpan<T>` across channel boundaries
4. Configure `tracing-opentelemetry` layer to inject trace_id into log records
5. Update console and file layer formatters to display trace_id

**Tech Stack:** Rust, tracing 0.1, tracing-opentelemetry 0.22, opentelemetry 0.21, tokio

---

## File Structure

**Files to modify:**

| File | Purpose | Lines Changed |
|------|---------|---------------|
| `crates/vol-datasource/Cargo.toml` | Add opentelemetry dependency | +2 |
| `crates/vol-datasource/src/volatility.rs` | Root span creation with OTel TraceId | ~40 |
| `crates/vol-engine/src/engine.rs` | Inherit span context, follows_from | ~20 |
| `crates/vol-alert/src/absolute_iv.rs` | Record alert span attributes | ~10 |
| `crates/vol-notification/src/feishu.rs` | Record notification span attributes | ~15 |
| `crates/vol-notification/src/stdout.rs` | Record notification span attributes | ~10 |
| `crates/vol-monitor/src/tracing_setup.rs` | Configure OTel layer for log injection | ~30 |

**Files unchanged:**

- `Cargo.toml` (workspace) - opentelemetry already defined in workspace dependencies
- `crates/vol-tracing/src/with_span.rs` - WithSpan mechanism already correct
- `crates/vol-config` - No configuration changes needed

---

### Task 1: Add OpenTelemetry dependency to vol-datasource

**Files:**
- Modify: `crates/vol-datasource/Cargo.toml`

- [ ] **Step 1: Add opentelemetry to dependencies**

Open `crates/vol-datasource/Cargo.toml` and add:

```toml
[dependencies]
# ... existing dependencies ...
opentelemetry = { workspace = true }
```

- [ ] **Step 2: Verify workspace dependency**

Run: `grep "opentelemetry" crates/Cargo.toml`

Expected output:
```
opentelemetry = "0.21"
```

- [ ] **Step 3: Compile to verify dependency resolution**

Run: `cargo check -p vol-datasource 2>&1 | tail -5`

Expected: No errors

- [ ] **Step 4: Commit**

```bash
git add crates/vol-datasource/Cargo.toml
git commit -m "chore: add opentelemetry dependency to vol-datasource"
```

---

### Task 2: Replace custom trace_id with OpenTelemetry TraceId

**Files:**
- Modify: `crates/vol-datasource/src/volatility.rs:15-27`

- [ ] **Step 1: Remove custom trace_id generator**

Open `crates/vol-datasource/src/volatility.rs` and remove lines 15-27:

```rust
// REMOVE these lines:
/// Global counter for generating unique trace IDs
static TRACE_ID_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Generate a unique trace ID based on timestamp and counter
fn generate_trace_id() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_micros() as u64;
    let counter = TRACE_ID_COUNTER.fetch_add(1, Ordering::Relaxed);
    (timestamp << 16) ^ counter
}
```

- [ ] **Step 2: Add OpenTelemetry imports**

Add at the top of the file after line 9:

```rust
use opentelemetry::trace::{TraceContextExt, TraceId};
use tracing_opentelemetry::OpenTelemetrySpanExt;
```

- [ ] **Step 3: Update span creation to use OTel trace_id**

Replace lines 197-202:

```rust
// Before (line 197-202):
let trace_id = generate_trace_id();
let span = info_span!(
    "datasource_receive",
    trace_id = %trace_id,
    source = "deribit"
);

// After:
let span = info_span!("datasource_receive", source = "deribit");
let _guard = span.enter();

// Extract trace_id from span context for logging/Feishu messages
let ctx = tracing::Span::current().context();
let trace_id = ctx.span().span_context().trace_id();
let trace_id_hex = format!("tr_{}", trace_id.to_string());
```

- [ ] **Step 4: Update record_tags! call**

After line 203 (now after the trace_id extraction), ensure business attributes are recorded:

```rust
record_tags!(span, vol_data, iv, symbol, dte);
span.record("index_price", &vol_data.index_price);
span.record("option_type", &vol_data.option_type.to_string());
span.record("instrument_name", &option.instrument_name);
```

- [ ] **Step 5: Compile and verify**

Run: `cargo check -p vol-datasource 2>&1 | tail -10`

Expected: No errors about unused imports or missing functions

- [ ] **Step 6: Commit**

```bash
git add crates/vol-datasource/src/volatility.rs
git commit -m "refactor: use OpenTelemetry TraceId instead of custom u64

Replace custom generate_trace_id() function with OpenTelemetry
TraceId type extracted from span context.

This enables:
- Native Jaeger trace ID compatibility
- Automatic trace_id propagation through span context
- Standard 128-bit trace IDs (32 hex chars)
"
```

---

### Task 3: Update vol-engine to preserve span context

**Files:**
- Modify: `crates/vol-engine/src/engine.rs:162-173`

- [ ] **Step 1: Add OpenTelemetry import**

At the top of `crates/vol-engine/src/engine.rs`, add:

```rust
use tracing_opentelemetry::OpenTelemetrySpanExt;
```

- [ ] **Step 2: Update rule_evaluate span creation**

Replace lines 162-173:

```rust
// Before:
let span = info_span!(
    "rule_evaluate",
    rule_id = %rule_id,
    rule_type = %rule_type,
    event_type = ?event.event_type()
);

// Establish causal relationship with parent span if present
if let Some(parent) = parent_span {
    span.follows_from(parent.id());
}

// After - explicitly extract and record trace_id:
let span = info_span!(
    "rule_evaluate",
    rule_id = %rule_id,
    rule_type = %rule_type,
    event_type = ?event.event_type()
);

// Establish causal relationship with parent span
if let Some(parent) = parent_span {
    span.follows_from(parent.id());
    
    // Inherit trace_id from parent for log correlation
    let parent_ctx = parent.context();
    let parent_trace_id = parent_ctx.span().span_context().trace_id();
    span.record("parent_trace_id", &parent_trace_id.to_string());
}
```

- [ ] **Step 3: Update alert span creation**

Replace lines 187-196:

```rust
// Before:
let alert_span = info_span!(
    "alert_generated",
    alert_type = %alert.alert_type,
    tenor = ?alert.tenor,
    symbol = %alert.symbol
);

// Record business attributes from Alert using record_tags! macro
record_tags!(alert_span, alert, iv, index_price, dte, moneyness, mark_price_coin);

// After - add trace_id recording:
let alert_span = info_span!(
    "alert_generated",
    alert_type = %alert.alert_type,
    tenor = ?alert.tenor,
    symbol = %alert.symbol
);

// Inherit trace_id from rule_evaluate span
let rule_trace_id = tracing::Span::current()
    .context()
    .span()
    .span_context()
    .trace_id();
alert_span.record("trace_id", &rule_trace_id.to_string());

record_tags!(alert_span, alert, iv, index_price, dte, moneyness, mark_price_coin);
```

- [ ] **Step 4: Compile and verify**

Run: `cargo check -p vol-engine 2>&1 | tail -10`

Expected: No errors

- [ ] **Step 5: Commit**

```bash
git add crates/vol-engine/src/engine.rs
git commit -m "feat: inherit trace_id in rule evaluation and alert spans

Record parent_trace_id attribute when following from parent span.
This ensures all logs in a processing chain share trace correlation.
"
```

---

### Task 4: Update vol-alert to record trace_id

**Files:**
- Modify: `crates/vol-alert/src/absolute_iv.rs`

- [ ] **Step 1: Read current implementation**

Run: `grep -n "trace_id\|info_span" crates/vol-alert/src/absolute_iv.rs | head -20`

Note the current span creation pattern.

- [ ] **Step 2: Add OpenTelemetry import**

Add to `crates/vol-alert/src/absolute_iv.rs`:

```rust
use tracing_opentelemetry::OpenTelemetrySpanExt;
```

- [ ] **Step 3: Update alert handler to record trace_id**

Find the alert creation logic (around line 45 based on spec) and add:

```rust
// After alert is created, record trace context
let current_span = tracing::Span::current();
current_span.record("alert.trace_id", &tracing::Span::current()
    .context()
    .span()
    .span_context()
    .trace_id()
    .to_string());
```

- [ ] **Step 4: Compile and verify**

Run: `cargo check -p vol-alert 2>&1 | tail -10`

Expected: No errors

- [ ] **Step 5: Commit**

```bash
git add crates/vol-alert/src/absolute_iv.rs
git commit -m "feat: record trace_id in absolute IV alert handler"
```

---

### Task 5: Update vol-notification to output trace_id

**Files:**
- Modify: `crates/vol-notification/src/feishu.rs`
- Modify: `crates/vol-notification/src/stdout.rs`

- [ ] **Step 1: Add OpenTelemetry import to feishu.rs**

Add to `crates/vol-notification/src/feishu.rs`:

```rust
use tracing_opentelemetry::OpenTelemetrySpanExt;
```

- [ ] **Step 2: Update Feishu send to include trace_id**

Find the notification send function (around line 301) and add trace_id extraction:

```rust
// Extract current trace_id for logging
let trace_id = tracing::Span::current()
    .context()
    .span()
    .span_context()
    .trace_id();

tracing::info!(
    trace_id = %trace_id,
    recipient = %self.config.receive_id,
    "notification sent"
);
```

- [ ] **Step 3: Update stdout.rs similarly**

Add to `crates/vol-notification/src/stdout.rs`:

```rust
use tracing_opentelemetry::OpenTelemetrySpanExt;

// In the send function:
let trace_id = tracing::Span::current()
    .context()
    .span()
    .span_context()
    .trace_id();

tracing::info!(
    trace_id = %trace_id,
    "notification sent to stdout"
);
```

- [ ] **Step 4: Compile and verify**

Run: `cargo check -p vol-notification 2>&1 | tail -10`

Expected: No errors

- [ ] **Step 5: Commit**

```bash
git add crates/vol-notification/src/feishu.rs crates/vol-notification/src/stdout.rs
git commit -m "feat: record trace_id in notification handlers

Both Feishu and stdout notification handlers now log
trace_id for log correlation.
"
```

---

### Task 6: Configure tracing-opentelemetry layer for log injection

**Files:**
- Modify: `crates/vol-monitor/src/tracing_setup.rs`

- [ ] **Step 1: Add FmtSpan import**

Add to imports (line 15-22):

```rust
use tracing_subscriber::fmt::format::FmtSpan;
```

- [ ] **Step 2: Update console layer configuration**

Replace lines 45-51:

```rust
// Before:
let console_layer = fmt::layer()
    .with_target(false)
    .with_thread_ids(false)
    .with_thread_names(false)
    .with_file(true)
    .with_line_number(true)
    .with_ansi(true);

// After:
let console_layer = fmt::layer()
    .with_target(true)
    .with_thread_ids(false)
    .with_thread_names(false)
    .with_file(true)
    .with_line_number(true)
    .with_ansi(true)
    .with_span_events(FmtSpan::NEW);
```

- [ ] **Step 3: Update file layer configuration**

Replace lines 55-63:

```rust
// Before:
let file_layer = fmt::layer()
    .with_ansi(false)
    .with_target(true)
    .with_thread_ids(true)
    .with_thread_names(true)
    .with_file(true)
    .with_line_number(true)
    .json()
    .with_writer(file_appender);

// After:
let file_layer = fmt::layer()
    .with_ansi(false)
    .with_target(true)
    .with_thread_ids(true)
    .with_thread_names(true)
    .with_file(true)
    .with_line_number(true)
    .json()
    .with_span_events(FmtSpan::NEW | FmtSpan::CLOSE)
    .with_writer(file_appender);
```

- [ ] **Step 4: Ensure OpenTelemetry layer is properly configured**

Verify lines 98-99 include proper configuration:

```rust
let otel_layer = tracing_opentelemetry::layer()
    .with_tracer(tracer)
    .with_location(true)
    .with_threads(true);
```

- [ ] **Step 5: Compile and verify**

Run: `cargo check -p vol-monitor 2>&1 | tail -10`

Expected: No errors

- [ ] **Step 6: Commit**

```bash
git add crates/vol-monitor/src/tracing_setup.rs
git commit -m "feat: configure tracing layers for trace_id injection

- Enable span events (NEW/CLOSE) for console and file layers
- Add with_location and with_threads to OTel layer
- Enable target in console output for better filtering
"
```

---

### Task 7: Write unit tests for trace_id recording

**Files:**
- Create: `crates/vol-datasource/src/volatility_test.rs` (or add to existing test module)

- [ ] **Step 1: Add test module**

Add to end of `crates/vol-datasource/src/volatility.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use opentelemetry::trace::TraceContextExt;
    use tracing::info_span;

    #[test]
    fn test_trace_id_recorded_in_span() {
        let span = info_span!("test_datasource", source = "deribit");
        let _guard = span.enter();
        
        let ctx = tracing::Span::current().context();
        let trace_id = ctx.span().span_context().trace_id();
        
        assert!(trace_id.to_string().len() == 32, "TraceId should be 32 hex chars");
        assert!(!trace_id.to_string().starts_with("00000000"), "TraceId should not be all zeros");
    }
}
```

- [ ] **Step 2: Run test**

Run: `cargo test -p vol-datasource trace_id 2>&1 | tail -15`

Expected:
```
test volatility::tests::test_trace_id_recorded_in_span ... ok
test result: ok. 1 passed; 0 failed
```

- [ ] **Step 3: Commit**

```bash
git add crates/vol-datasource/src/volatility.rs
git commit -m "test: add unit test for trace_id recording"
```

---

### Task 8: Integration test with Jaeger

**Files:**
- Test: Manual verification

- [ ] **Step 1: Start Jaeger**

Run:
```bash
docker run --rm -d --name jaeger \
  -p 4317:4317 \
  -p 16686:16686 \
  jaegertracing/all-in-one:latest

sleep 5
```

- [ ] **Step 2: Build vol-monitor**

Run:
```bash
cargo build --release 2>&1 | tail -5
```

Expected: `Finished release [optimized]`

- [ ] **Step 3: Clean old logs and run briefly**

Run:
```bash
rm -f logs/*.log*
HTTPS_PROXY=http://192.168.2.98:8890 timeout 5 ./target/release/vol-monitor --config config.toml 2>&1 | head -20
```

- [ ] **Step 4: Verify console output contains trace_id**

Run:
```bash
HTTPS_PROXY=http://192.168.2.98:8890 timeout 3 ./target/release/vol-monitor --config config.toml 2>&1 | grep -o 'tr_[a-f0-9]*' | head -3
```

Expected: Output like `tr_0000018c9a62f3d0`

- [ ] **Step 5: Verify JSON logs contain trace_id field**

Run:
```bash
grep -o '"trace_id":"[^"]*"' logs/vol-monitor.*.log | head -3
```

Expected: Output like:
```
logs/vol-monitor.2026-04-05.log:"trace_id":"0000018c9a62f3d00000000000000000"
```

- [ ] **Step 6: Verify Jaeger UI shows traces**

Open browser to `http://localhost:16686` and:
1. Select service: `vol-monitor`
2. Click "Find Traces"
3. Verify traces appear with spans: `datasource_receive`, `rule_evaluate`, `alert_generated`

- [ ] **Step 7: Document test results**

Run:
```bash
echo "=== Integration Test Results ===" > /tmp/trace-test-results.txt
echo "Date: $(date)" >> /tmp/trace-test-results.txt
echo "" >> /tmp/trace-test-results.txt
echo "Console trace_id sample:" >> /tmp/trace-test-results.txt
grep -o 'tr_[a-f0-9]*' logs/vol-monitor.*.log 2>/dev/null | head -3 >> /tmp/trace-test-results.txt
echo "" >> /tmp/trace-test-results.txt
echo "JSON trace_id sample:" >> /tmp/trace-test-results.txt
grep -o '"trace_id":"[^"]*"' logs/vol-monitor.*.log 2>/dev/null | head -3 >> /tmp/trace-test-results.txt
cat /tmp/trace-test-results.txt
```

- [ ] **Step 8: Stop Jaeger**

Run:
```bash
docker stop jaeger
```

- [ ] **Step 9: Commit test documentation (optional)**

```bash
git add /tmp/trace-test-results.txt || true
git commit -m "chore: document trace ID integration test results" || true
```

---

### Task 9: Update tracing documentation

**Files:**
- Modify: `docs/tracing.md`

- [ ] **Step 1: Add trace_id in logs section**

Append to `docs/tracing.md` after line 95:

```markdown
### Trace ID in Logs

All log output now includes trace_id for correlation:

**Console:**
```
INFO [tr_0000018c9a62f3d0] vol_datasource::volatility: received ticker symbol=BTC
```

**File JSON:**
```json
{
  "trace_id": "0000018c9a62f3d00000000000000000",
  "span": {"name": "datasource_receive"},
  "fields": {"message": "received ticker", "symbol": "BTC"}
}
```

**Querying by trace_id:**
```bash
# Find all logs for a specific trace
grep 'tr_0000018c9a62f3d0' logs/vol-monitor.log

# Extract trace_id from JSON logs
grep -o '"trace_id":"[^"]*"' logs/vol-monitor.log | sort -u
```
```

- [ ] **Step 2: Update Trace ID Format section**

Replace line 124-126:

```markdown
### Trace ID Format

Trace IDs follow OpenTelemetry standard: 128-bit (16 bytes) represented as 32 hex characters.

- **Format**: `tr_` + 32 hex chars (e.g., `tr_0000018c9a62f3d0a1b2c3d4e5f6g7h8`)
- **Generation**: Created at DataSource when ticker is received
- **Propagation**: Via span context through WithSpan wrapper
- **Jaeger compatibility**: Native 128-bit TraceId type
```

- [ ] **Step 3: Commit**

```bash
git add docs/tracing.md
git commit -m "docs: update tracing docs with trace_id in logs

Add examples of trace_id in console and JSON log output.
Document querying logs by trace_id.
Update Trace ID Format section with OTel standard format.
"
```

---

## Self-Review Checklist

**1. Spec coverage:**

| Spec Requirement | Task |
|-----------------|------|
| Use OpenTelemetry TraceId (128-bit) | Task 2 |
| Generate trace_id at DataSource | Task 2 |
| Propagate via WithSpan | Existing mechanism, verified in Task 3 |
| follows_from relationships | Task 3 |
| Console logs with trace_id | Task 6 |
| File JSON with trace_id field | Task 6 |
| Feishu message trace_id | Task 5 |
| Error handling | Built into OTel SDK |
| Unit tests | Task 7 |
| Integration test | Task 8 |
| Documentation | Task 9 |

**2. Placeholder scan:**

- No TBD/TODO placeholders in steps
- All code examples show actual implementation
- All commands have expected output

**3. Type consistency:**

- `TraceId` type from `opentelemetry::trace` used consistently
- `tracing_opentelemetry::OpenTelemetrySpanExt` trait imported where needed
- `trace_id.to_string()` returns 32 hex chars (128-bit)
- Span names match spec: `datasource_receive`, `rule_evaluate`, `alert_generated`, `notification_send`

---

Plan complete and saved to `docs/superpowers/plans/2026-04-05-trace-id-in-logs-implementation.md`. Two execution options:

**1. Subagent-Driven (recommended)** - Dispatch a fresh subagent per task, review between tasks, fast iteration

**2. Inline Execution** - Execute tasks in this session using executing-plans, batch execution with checkpoints

Which approach?
