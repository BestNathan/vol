# Observability 日志系统重构实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 修改 run_id 格式去掉 `run_` 前缀，使用纯 UUID 字符串，并更新相关的 cleanup 逻辑和测试用例。

**Architecture:** 直接在 agent.rs 中修改 run_id 生成逻辑，将 `format!("run_{}", ...)` 改为直接使用 `uuid::Uuid::new_v4().simple().to_string()`。cleanup 逻辑中的文件名匹配模式从 `run_` 前缀改为匹配纯 UUID 格式。

**Tech Stack:** uuid, regex, chrono, tempfile (testing)

---

## 任务分解

### Task 1: 修改 run_id 生成逻辑

**Files:**
- Modify: `crates/vol-llm-agent/src/react/agent.rs:100`
- Test: `cargo test -p vol-llm-agent --lib`

- [ ] **Step 1: 修改 run_id 生成**

在 agent.rs 第 100 行，将：
```rust
let run_id = format!("run_{}", uuid::Uuid::new_v4().simple());
```

修改为：
```rust
let run_id = uuid::Uuid::new_v4().simple().to_string();
```

- [ ] **Step 2: 验证编译**

运行：
```bash
cargo check -p vol-llm-agent
```
期望：编译成功，无错误

- [ ] **Step 3: 运行单元测试**

运行：
```bash
cargo test -p vol-llm-agent --lib
```
期望：所有测试通过（暂时忽略观测性相关测试的断言失败）

- [ ] **Step 4: 提交**

```bash
git add crates/vol-llm-agent/src/react/agent.rs
git commit -m "refactor: remove run_ prefix from run_id generation"
```

---

### Task 2: 更新 cleanup 逻辑

**Files:**
- Modify: `crates/vol-llm-agent/src/observability/cleanup.rs:68`
- Test: `crates/vol-llm-agent/src/observability/cleanup.rs` tests

- [ ] **Step 1: 修改 run log 文件匹配模式**

在 cleanup.rs 第 68 行，将：
```rust
.starts_with("run_")
```

修改为匹配纯 UUID 格式（32 字符十六进制）：
```rust
.len() == 36 && s.chars().all(|c| c.is_ascii_hexdigit())
```

完整上下文（第 65-70 行）：
```rust
.filter(|e| {
    let name = e.file_name().to_string_lossy();
    // Match pure UUID format: 32 hex chars + .jsonl extension
    name.len() == 36 && name.starts_with(|c: char| c.is_ascii_hexdigit()) && name.ends_with(".jsonl")
})
```

更简洁的方案是使用正则：
```rust
let run_pattern = Regex::new(r"^[0-9a-f]{32}\.jsonl$").unwrap();
// 然后在 filter 中使用
.filter(|e| run_pattern.is_match(&e.file_name().to_string_lossy()))
```

但为了避免每次调用都创建 regex，应该在函数外静态定义。最简单的方案：
```rust
.filter(|e| {
    let name = e.file_name().to_string_lossy();
    name.len() == 36 && name.ends_with(".jsonl")
})
```

因为 UUID simple() 格式是 32 个十六进制字符，加上 `.jsonl` 扩展名（6 字符）= 38 字符。

等等，让我重新计算：
- UUID simple() = 32 字符（无连字符的十六进制）
- `.jsonl` = 6 字符
- 总共 = 38 字符

所以 filter 应该是：
```rust
.filter(|e| {
    let name = e.file_name().to_string_lossy();
    name.len() == 38 && name.ends_with(".jsonl")
})
```

- [ ] **Step 2: 更新测试用例中的文件名**

在 cleanup.rs 第 148 行，测试创建文件时，将：
```rust
let file = runs_path.join(format!("run_{:03}.jsonl", i));
```

修改为使用 UUID 格式的测试文件名。为了保持测试的可预测性，使用固定的 UUID 格式：
```rust
let file = runs_path.join(format!("{:032x}.jsonl", i));
```

但这样文件名会是 000...000 格式，不是有效的 UUID。更好的方案是使用递增的 UUID-like 字符串：
```rust
let file = runs_path.join(format!("{:032}.jsonl", i));
```

但这会产生十进制数字而不是十六进制。最简单的方案是保持测试逻辑不变，但修改断言：
```rust
// Create 15 run logs with UUID-like names
for i in 0..15 {
    let file = runs_path.join(format!("{:032x}.jsonl", i));
    fs::write(&file, format!("log {}", i)).unwrap();
}
```

然后更新断言（第 157-159 行）：
```rust
assert!(!runs_path.join(format!("{:032x}.jsonl", 0)).exists());
assert!(runs_path.join(format!("{:032x}.jsonl", 5)).exists());
assert!(runs_path.join(format!("{:032x}.jsonl", 14)).exists());
```

- [ ] **Step 3: 验证编译**

运行：
```bash
cargo check -p vol-llm-agent
```
期望：编译成功

- [ ] **Step 4: 运行 cleanup 测试**

运行：
```bash
cargo test -p vol-llm-agent --lib observability::cleanup
```
期望：所有 cleanup 测试通过

- [ ] **Step 5: 提交**

```bash
git add crates/vol-llm-agent/src/observability/cleanup.rs
git commit -m "fix: update cleanup logic for new run_id format"
```

---

### Task 3: 更新观测性插件测试

**Files:**
- Modify: `crates/vol-llm-agent/src/observability/plugin.rs` tests
- Test: `crates/vol-llm-agent/src/observability/plugin.rs` tests

- [ ] **Step 1: 更新测试中的 run log 文件名断言**

在 plugin.rs 第 148 行，将：
```rust
let run_log_path = runs_path.join("test-run.jsonl");
```

修改为：
```rust
let run_log_path = runs_path.join("test-run.jsonl");  // 测试使用固定的 run_id，保持不变
```

等等，测试中使用的 run_id 是 `"test-run"`（第 116 行），这不是 UUID 格式。测试应该仍然工作，因为文件命名逻辑是 `{}.jsonl` 格式。

让我检查测试实际创建的文件名... 第 148 行：
```rust
let run_log_path = runs_path.join("test-run.jsonl");
```

这个测试应该仍然工作，因为 run_id 是 `"test-run"`，文件名是 `{run_id}.jsonl` = `test-run.jsonl`。

但是为了与生产代码一致，应该更新测试使用 UUID 格式的 run_id。或者保持测试简单，使用固定字符串。

实际上，测试的目的是验证日志被正确写入，run_id 格式不影响这个测试。保持测试不变。

- [ ] **Step 2: 运行观测性插件测试**

运行：
```bash
cargo test -p vol-llm-agent --lib observability::plugin
```
期望：所有测试通过

- [ ] **Step 3: 提交**

如果测试有修改：
```bash
git add crates/vol-llm-agent/src/observability/plugin.rs
git commit -m "test: update plugin tests for new run_id format"
```

---

### Task 4: 验证所有测试通过

**Files:**
- Full test suite

- [ ] **Step 1: 运行完整测试套件**

运行：
```bash
cargo test -p vol-llm-agent
```
期望：所有测试通过

- [ ] **Step 2: 验证 run_id 格式**

运行一个集成测试并检查日志输出：
```bash
cargo test -p vol-llm-agent --test observability_integration -- --nocapture
```
期望：stdout 输出中的 run_id 是纯 UUID 格式（如 `abc123def456...`），无 `run_` 前缀

- [ ] **Step 3: 验证日志文件名**

检查测试创建的日志文件：
```bash
ls -la /tmp/*/runs/
```
期望：文件名格式为 `{uuid}.jsonl`，无 `run_` 前缀

---

## 验收标准

1. ✅ run_id 格式为纯 UUID（如 `abc123...`），无 `run_` 前缀
2. ✅ run log 文件名：`{run_id}.jsonl`
3. ✅ cleanup 逻辑正确匹配新文件名格式
4. ✅ 所有单元测试通过
5. ✅ 所有集成测试通过
6. ✅ cleanup 保留策略正常工作（保留最近 10 个 run log）

---

## 风险与缓解

| 风险 | 缓解 |
|------|------|
| 生产环境中旧日志文件无法被 cleanup | 旧日志文件按旧格式保留，新日志按新格式，cleanup 只影响新文件 |
| 测试断言失败 | 逐个更新测试，确保每个测试通过后再提交 |
| run_id 生成冲突 | UUID v4 冲突概率极低，可忽略 |

---

## 未来增强

1. **完全迁移到 tracing**: 如果需要日志级别控制、采样等功能
2. **OTLP 导出**: 发送到 Jaeger/Tempo 等 tracing 后端
3. **结构化 stdout**: 使用 tracing_subscriber 输出结构化日志
