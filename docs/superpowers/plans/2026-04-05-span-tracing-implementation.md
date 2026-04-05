# Span + Tracing 规范化实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans 来逐步实施此计划。步骤使用复选框（`- [ ]`）语法进行跟踪。

**Goal:** 统一项目中的 tracing 实现，使用 `.instrument()` trait 传递 span，规范 trace_id 生成和使用。

**Architecture:** 按数据流顺序逐个模块重构：vol-datasource → vol-engine → vol-notification，每步独立验证。

**Tech Stack:** Rust, tokio, tracing, tracing-opentelemetry, uuid, vol-tracing 工具库

---

## 文件结构

### 已修改的文件
- `crates/vol-tracing/src/lib.rs` - 已添加 `new_trace_id()`, `current_trace_id()`, re-export `Instrument`
- `crates/vol-tracing/Cargo.toml` - 已添加 `uuid` 依赖

### 待修改的文件
- `crates/vol-datasource/src/volatility.rs` - 数据源入口生成 trace_id，send 操作使用 `.instrument()`
- `crates/vol-datasource/src/portfolio.rs` - 同上
- `crates/vol-engine/src/engine.rs` - rule.evaluate() 使用 `.instrument()`
- `crates/vol-notification/src/feishu.rs` - send_message() 使用 `.instrument()`
- `crates/vol-notification/src/stdout.rs` - 保持 `.enter()`（同步操作）

### 依赖检查
- `crates/vol-datasource/Cargo.toml` - 需添加 `vol-tracing` 依赖
- `crates/vol-engine/Cargo.toml` - 已有 `vol-tracing` 依赖
- `crates/vol-notification/Cargo.toml` - 已有 `vol-tracing` 依赖

---

## 任务分解

### Task 1: vol-datasource/volatility.rs 重构

**Files:**
- Modify: `crates/vol-datasource/src/volatility.rs:182-206`
- Test: `cargo check -p vol-datasource`

- [ ] **Step 1: 更新 import**

修改 `volatility.rs` 第 12 行：
```rust
// 原代码
use vol_tracing::{WithSpan, record_tags};

// 新代码
use vol_tracing::{WithSpan, record_tags, new_trace_id, Instrument};
```

- [ ] **Step 2: 修改数据接收逻辑**

修改 `volatility.rs` 第 182-206 行：
```rust
// 原代码
if let Some(vol_data) = option.to_volatility_data_with_index(index_price) {
    // Create tracing span and extract OTel TraceId
    let span = info_span!("datasource_receive", source = "deribit");

    // Extract trace_id from span context while span is current
    let trace_id_hex = {
        let _guard = span.enter();
        let ctx = tracing::Span::current().context();
        let trace_id = ctx.span().span_context().trace_id();
        format!("tr_{}", trace_id.to_string())
    };

    span.record("trace_id", &trace_id_hex);
    record_tags!(span, vol_data, iv, symbol, dte);
    span.record("index_price", &vol_data.index_price);
    span.record("option_type", &vol_data.option_type.to_string());

    let traced_event = WithSpan::new(vol_data, span);
    if let Err(e) = internal_tx.send(traced_event).await {
        error!(
            instrument = %option.instrument_name,
            error = %e,
            "Failed to send volatility data"
        );
    }
}

// 新代码
if let Some(vol_data) = option.to_volatility_data_with_index(index_price) {
    // 生成 trace_id 并创建 span
    let trace_id = new_trace_id();
    let span = info_span!(
        "datasource_receive",
        source = "deribit",
        trace_id = %trace_id,
        iv = %vol_data.iv,
        symbol = %vol_data.symbol,
        dte = vol_data.dte,
        index_price = %vol_data.index_price,
        option_type = %vol_data.option_type,
    );

    // 发送时用 .instrument() 绑定 span
    let traced_event = WithSpan::new(vol_data, span.clone());
    if let Err(e) = internal_tx.send(traced_event).instrument(span).await {
        error!(
            instrument = %option.instrument_name,
            error = %e,
            "Failed to send volatility data"
        );
    }
}
```

- [ ] **Step 3: 清理未使用的 import**

删除第 13-14 行：
```rust
// 删除这些行
use opentelemetry::trace::TraceContextExt;
use tracing_opentelemetry::OpenTelemetrySpanExt;
```

- [ ] **Step 4: 运行编译检查**

```bash
cargo check -p vol-datasource
```
预期：编译通过

- [ ] **Step 5: 提交**

```bash
git add crates/vol-datasource/src/volatility.rs
git commit -m "refactor(tracing): use .instrument() in volatility datasource

- Generate trace_id at entry point using new_trace_id()
- Use .instrument() trait for async send operations
- Remove manual span.enter() and OpenTelemetry imports
"
```

---

### Task 2: vol-datasource/portfolio.rs 重构

**Files:**
- Modify: `crates/vol-datasource/src/portfolio.rs:140-160`
- Test: `cargo check -p vol-datasource`

- [ ] **Step 1: 查看当前代码**

读取 `portfolio.rs` 第 140-160 行，定位 span 创建和 send 操作的位置。

- [ ] **Step 2: 更新 import**

```rust
// 添加
use vol_tracing::{new_trace_id, Instrument};
```

- [ ] **Step 3: 修改 snapshot 发送逻辑**

找到类似 volatility.rs 的模式，修改为：
```rust
let trace_id = new_trace_id();
let span = info_span!(
    "portfolio_snapshot",
    source = "deribit_portfolio",
    trace_id = %trace_id,
    currency = %snapshot.currency,
);

let traced_event = WithSpan::new(event, span.clone());
tx.send(traced_event).instrument(span).await?;
```

- [ ] **Step 4: 清理旧代码**

删除 `generate_trace_id()` 函数和 `TRACE_ID_COUNTER`（已被 `new_trace_id()` 替代）。

- [ ] **Step 5: 运行编译检查**

```bash
cargo check -p vol-datasource
```
预期：编译通过

- [ ] **Step 6: 提交**

```bash
git add crates/vol-datasource/src/portfolio.rs
git commit -m "refactor(tracing): use .instrument() in portfolio datasource

- Replace custom generate_trace_id() with new_trace_id()
- Use .instrument() trait for async operations
"
```

---

### Task 3: vol-engine/engine.rs 重构

**Files:**
- Modify: `crates/vol-engine/src/engine.rs:153-225`
- Test: `cargo check -p vol-engine`

- [ ] **Step 1: 更新 import**

修改第 9 行：
```rust
// 原代码
use vol_tracing::{record_tags, WithSpan};

// 新代码
use vol_tracing::{WithSpan, Instrument};
```

删除第 10-11 行：
```rust
// 删除这些行
use tracing_opentelemetry::OpenTelemetrySpanExt;
use opentelemetry::trace::TraceContextExt;
```

- [ ] **Step 2: 修改 rule_evaluate span 创建**

修改第 164-187 行：
```rust
// 原代码
let span = info_span!(
    "rule_evaluate",
    rule_id = %rule_id,
    rule_type = %rule_type,
    event_type = ?event.event_type()
);

if let Some(parent) = parent_span {
    span.follows_from(parent.id());
    let parent_trace_id = parent.context().span().span_context().trace_id();
    span.record("parent_trace_id", &parent_trace_id.to_string());
}

span.record("event.timestamp", &event.timestamp());
span.record("event.source", event.source());

let _guard = span.enter();
let alerts = rule_clone.evaluate(&event).await;
drop(_guard);

// 新代码
let span = info_span!(
    "rule_evaluate",
    rule_id = %rule_id,
    rule_type = %rule_type,
    event_type = ?event.event_type(),
    event_timestamp = %event.timestamp(),
    event_source = %event.source(),
);

if let Some(parent) = parent_span {
    span.follows_from(parent.id());
}

let alerts = rule_clone.evaluate(&event).instrument(span).await;
```

- [ ] **Step 3: 修改 alert_generated span**

修改第 191-224 行：
```rust
// 原代码
for alert in alerts {
    let alert_span = info_span!(
        "alert_generated",
        alert_type = %alert.alert_type,
        tenor = ?alert.tenor,
        symbol = %alert.symbol
    );

    let rule_trace_id = span.context().span().span_context().trace_id();
    alert_span.record("trace_id", &rule_trace_id.to_string());

    alert_span.record("iv", &iv);
    alert_span.record("index_price", &index_price);
    // ...

    if let Err(e) = tx.send(alert).await {
        error!(error = %e, "Failed to send alert");
        break;
    }
}

// 新代码
for alert in alerts {
    let alert_span = info_span!(
        "alert_generated",
        alert_type = %alert.alert_type,
        tenor = ?alert.tenor,
        symbol = %alert.symbol,
        iv = %alert.iv,
        dte = alert.dte,
        index_price = %alert.index_price,
    );

    if let Err(e) = tx.send(alert).instrument(alert_span).await {
        error!(error = %e, "Failed to send alert");
        break;
    }
}
```

- [ ] **Step 4: 运行编译检查**

```bash
cargo check -p vol-engine
```
预期：编译通过

- [ ] **Step 5: 提交**

```bash
git add crates/vol-engine/src/engine.rs
git commit -m "refactor(tracing): use .instrument() in rule engine

- Use .instrument() for rule.evaluate() async call
- Use .instrument() for alert send operations
- Remove manual trace_id extraction and record()
- Clean up unused OpenTelemetry imports
"
```

---

### Task 4: vol-notification/feishu.rs 重构

**Files:**
- Modify: `crates/vol-notification/src/feishu.rs:300-345`
- Test: `cargo check -p vol-notification`

- [ ] **Step 1: 更新 import**

修改第 11-14 行：
```rust
// 原代码
use tracing::{info, warn, info_span};
use tracing_opentelemetry::OpenTelemetrySpanExt;
use opentelemetry::trace::TraceContextExt;
use opentelemetry::Context;

// 新代码
use vol_tracing::current_trace_id;
use tracing::{info, warn, info_span, Instrument};
```

- [ ] **Step 2: 修改 send 方法**

修改第 300-345 行：
```rust
// 原代码
async fn send(&self, alert: &Alert) -> Result<()> {
    let span = info_span!(
        "notification_send",
        channel = "feishu",
        alert_type = %alert.alert_type,
        tenor = ?alert.tenor,
        symbol = %alert.symbol,
        iv = %alert.iv
    );

    span.record("alert.dte", &alert.dte);
    span.record("alert.index_price", &alert.index_price);

    let _guard = span.enter();

    let trace_id_prefix = get_trace_id_prefix();
    // ... send message ...

    let trace_id = tracing::Span::current()
        .context()
        .span()
        .span_context()
        .trace_id();

    tracing::info!(
        trace_id = %trace_id,
        recipient = %self.receive_id,
        "notification sent"
    );

    Ok(())
}

// 新代码
async fn send(&self, alert: &Alert) -> Result<()> {
    let span = info_span!(
        "notification_send",
        channel = "feishu",
        alert_type = %alert.alert_type,
        tenor = ?alert.tenor,
        symbol = %alert.symbol,
        iv = %alert.iv,
        dte = alert.dte,
        index_price = %alert.index_price,
    );

    async {
        // 提取 trace_id 用于 Feishu 消息前缀
        let trace_id_prefix = format!("[tr_{}]", &current_trace_id()[..8]);
        
        // 发送消息
        let card_content = self.format_interactive_card(alert, &trace_id_prefix);
        let text_content = self.format_message(alert, &trace_id_prefix);

        if let Err(e) = self.send_message("interactive", &card_content).await {
            warn!("Interactive card failed, falling back to text: {:?}", e);
            self.send_message("text", &json!({ "text": text_content }).to_string()).await?;
        }

        tracing::info!(
            recipient = %self.receive_id,
            "notification sent to feishu"
        );

        Ok(())
    }
    .instrument(span)
    .await
}
```

- [ ] **Step 3: 检查 get_trace_id_prefix 函数**

如果 `get_trace_id_prefix()` 函数不再使用，删除它（第 274-292 行）。

- [ ] **Step 4: 运行编译检查**

```bash
cargo check -p vol-notification
```
预期：编译通过

- [ ] **Step 5: 提交**

```bash
git add crates/vol-notification/src/feishu.rs
git commit -m "refactor(tracing): use .instrument() in feishu notification

- Use current_trace_id() for extracting trace_id
- Use .instrument() for async send_message operations
- Remove get_trace_id_prefix() helper
- Clean up unused OpenTelemetry imports
"
```

---

### Task 5: vol-notification/stdout.rs 保持现状

**Files:**
- Read: `crates/vol-notification/src/stdout.rs`
- Decision: 保持 `.enter()` 模式（同步操作）

- [ ] **Step 1: 确认 stdout.rs 使用 .enter() 是正确的**

因为 `stdout.rs` 的 `send()` 方法是同步操作（println），使用 `.enter()` 是正确的：
```rust
let _guard = span.enter();
info!("{}", message);  // 同步操作，不需要 .instrument()
```

- [ ] **Step 2: 可选 - 使用 current_trace_id()**

如果想统一 trace_id 提取方式，可以修改第 48-53 行：
```rust
// 原代码
let trace_id = tracing::Span::current()
    .context()
    .span()
    .span_context()
    .trace_id();

// 新代码
let trace_id = current_trace_id();
```

但这需要添加 import：
```rust
use vol_tracing::current_trace_id;
```

- [ ] **Step 3: 提交（如果有修改）**

```bash
git add crates/vol-notification/src/stdout.rs
git commit -m "chore(tracing): use current_trace_id() in stdout notification
"
```

---

### Task 6: 全 Workspace 验证

**Files:**
- All workspace crates

- [ ] **Step 1: 运行 workspace 编译检查**

```bash
cargo check --workspace
```
预期：编译通过，无警告（除了 pre-existing warnings）

- [ ] **Step 2: 运行测试（如有）**

```bash
cargo test --workspace
```
预期：所有测试通过

- [ ] **Step 3: 运行 release 构建**

```bash
cargo build --release
```
预期：构建成功

- [ ] **Step 4: 验证 vol-monitor 二进制**

```bash
./target/release/vol-monitor --help
```
预期：显示帮助信息

---

### Task 7: 文档更新

**Files:**
- Modify: `docs/tracing.md`
- Create: `docs/superpowers/specs/2026-04-05-span-tracing-patterns.md`

- [ ] **Step 1: 更新 docs/tracing.md**

添加 `.instrument()` 使用示例章节。

- [ ] **Step 2: 创建 Span Tracing Patterns 文档**

```markdown
# Span Tracing Patterns

## TraceId 生成和使用

```rust
// 数据源入口 - 生成 trace_id
let trace_id = vol_tracing::new_trace_id();
let span = info_span!("datasource_receive",
    source = "deribit",
    trace_id = %trace_id,
);
```

## 跨异步操作

```rust
// 使用 .instrument() trait
async_operation()
    .instrument(span)
    .await;
```

## 跨 Channel 传播

```rust
// 发送端
let traced = WithSpan::new(event, span);
tx.send(traced).await?;

// 接收端
let traced = rx.recv().await?;
traced.enter_span(info_span!("rule_evaluate"), |span| {
    span.follows_from(parent_span.id());
});
```
```

- [ ] **Step 3: 提交**

```bash
git add docs/tracing.md docs/superpowers/specs/2026-04-05-span-tracing-patterns.md
git commit -m "docs: add span tracing patterns documentation
"
```

---

## 完成标准

- [ ] 所有 task 完成
- [ ] `cargo check --workspace` 无错误
- [ ] `cargo build --release` 成功
- [ ] 文档更新完成

---

## 回滚策略

如果某个 task 出现问题：
```bash
# 回滚到上一个已知良好的 commit
git log --oneline  # 找到好的 commit
git revert <bad-commit-hash>  # 回滚特定 commit
```

---

**Plan complete. Two execution options:**

**1. Subagent-Driven (recommended)** - Dispatch a fresh subagent per task, review between tasks, fast iteration

**2. Inline Execution** - Execute tasks in this session using executing-plans, batch execution with checkpoints

**Which approach?**
