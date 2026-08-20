# AgentTool 内置化 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 把 AgentTool 重构进新 crate `vol-llm-agent-tool` 并注册为运行时内置工具：按 `AgentDef.id` 派发内置定义子 agent，深度守卫（默认 1 层），子 agent 会话按 name 持久化可观测，AgentInjector 向上下文贡献可用 agent 列表。

**Architecture:** 新高层 crate `vol-llm-agent-tool`（依赖 vol-llm-agent / vol-session / vol-llm-tool / vol-llm-core / vol-llm-context），runtime 依赖并注册它。parent/depth 记录在 `vol_llm_core::AgentDef` 上，经 `ToolContext.agent_def` 随派发链传递；深度上限读调用方 `tool_config.agent.max_depth`（默认 1）。

**Tech Stack:** Rust workspace（tokio / async-trait / serde），TDD（先写失败测试）。

**Spec:** `docs/superpowers/specs/2026-08-20-agenttool-builtin-design.md`（需求：`docs/superpowers/requirement/2026-08-20-agenttool-builtin-requirement.md`）——计划从 spec 推导，执行者需同时阅读两者。

## Global Constraints

- 覆盖 gate：`just cover-gate <crate> 80`，本计划涉及 vol-llm-core / vol-llm-agent / vol-llm-agent-tool / vol-llm-runtime；例外仅 `main.rs` / `app.rs` / `health.rs`
- 无 doc tests：文档示例用 ` ```text`；执行前跑 `./scripts/check-no-doc-tests.sh`
- 每个新 `pub fn` / handler 至少一个测试
- crate 边界：`vol-llm-runtime` 不得依赖 `vol-agent-server`；`vol-llm-agent-tool` 不得依赖 `vol-llm-runtime`（`./scripts/check-agent-boundaries.sh` 验证）
- 提交信息用 conventional commits（`feat(vol-llm-agent-tool): ...` 等），每任务一个提交
- 最后 wiki-ingest：实现结果摄入 `docs/wiki`
- 不触碰前端、数据面/控制面协议、Docker/K8s

## File Structure

| 文件 | 动作 | 职责 |
|------|------|------|
| `crates/vol-llm-core/src/agent_def.rs` | 修改 | `AgentDef` 增加 `parent_agent` / `depth` 字段 + 默认值 + builder |
| `crates/vol-llm-tools-builtin/tests/fixtures.rs` | 修改 | AgentDef 字面量补新字段 |
| `crates/vol-agent-server/src/data_plane/handlers/capability_tests.rs` | 修改 | 同上 |
| `crates/vol-llm-tool/src/tool.rs` | 修改 | 测试内 AgentDef 字面量补新字段 |
| `crates/vol-llm-agent/src/agent_loader.rs` | 修改 | 字面量补新字段 + 新增 `get_by_id` + 测试 |
| `crates/vol-llm-agent-tool/Cargo.toml` | 创建 | 新 crate 依赖声明 |
| `crates/vol-llm-agent-tool/src/lib.rs` | 创建 | 导出 `AgentTool` / `AgentInjector` |
| `crates/vol-llm-agent-tool/src/agent_tool.rs` | 创建 | AgentTool（迁入 + 重构：id 派发 / depth 守卫 / 持久化 session） |
| `crates/vol-llm-agent-tool/src/injector.rs` | 创建 | AgentInjector（ContextContributor） |
| `crates/vol-llm-agent/src/agent_tool.rs` | 删除 | 迁出 |
| `crates/vol-llm-agent/src/lib.rs` | 修改 | 移除 `pub mod agent_tool;` 与 `pub use agent_tool::AgentTool;` |
| `Cargo.toml` | 修改 | workspace members 增加 `crates/vol-llm-agent-tool` |
| `crates/vol-llm-runtime/Cargo.toml` | 修改 | 增加 `vol-llm-agent-tool` 依赖 |
| `crates/vol-llm-runtime/src/lib.rs` | 修改 | `agent_loader` 字段、build()/for_test() 注册 AgentTool、register_agent 挂 AgentInjector、discover_agents 复用 loader、集成测试 |
| `CLAUDE.md` | 修改 | crates 清单增加 vol-llm-agent-tool |

---

### Task 1: AgentDef 增加 parent_agent / depth（vol-llm-core）

**Files:**
- Modify: `crates/vol-llm-core/src/agent_def.rs`
- Modify（字面量补字段）: `crates/vol-llm-tools-builtin/tests/fixtures.rs:76`、`crates/vol-agent-server/src/data_plane/handlers/capability_tests.rs:51`、`crates/vol-llm-agent/src/agent_loader.rs:93`、`crates/vol-llm-tool/src/tool.rs:300`
- Test: `crates/vol-llm-core/src/agent_def.rs`（tests 模块）

**Interfaces:**
- Consumes: 无
- Produces: `AgentDef.parent_agent: Option<String>`（默认 None）、`AgentDef.depth: u32`（默认 0）、`AgentDef::with_parent_agent(impl Into<String>)`、`AgentDef::with_depth(u32)` —— 后续 Task 2/3/4 使用

- [ ] **Step 1: 写失败测试**（追加到 `crates/vol-llm-core/src/agent_def.rs` 末尾 tests 模块）

```rust
    #[test]
    fn test_agent_def_parent_and_depth_defaults() {
        let def = AgentDef::default();
        assert_eq!(def.parent_agent, None);
        assert_eq!(def.depth, 0);

        let def = AgentDef::new("echo", "prompt");
        assert_eq!(def.parent_agent, None);
        assert_eq!(def.depth, 0);
    }

    #[test]
    fn test_agent_def_parent_and_depth_builders() {
        let def = AgentDef::new("echo", "prompt")
            .with_parent_agent("repo:root")
            .with_depth(2);
        assert_eq!(def.parent_agent, Some("repo:root".to_string()));
        assert_eq!(def.depth, 2);
    }
```

- [ ] **Step 2: 运行确认失败**

Run: `cargo test -p vol-llm-core test_agent_def_parent_and_depth`
Expected: 编译失败——`no field 'parent_agent' on type 'AgentDef'` 等错误

- [ ] **Step 3: 实现**（`crates/vol-llm-core/src/agent_def.rs`）

三处修改：

```rust
// (1) struct 末尾（skills 字段之后）新增两个字段：
    /// Skills allowlist. None = all skills available.
    pub skills: Option<Vec<String>>,
    /// 派发方 agent id（根 agent 为 None）
    pub parent_agent: Option<String>,
    /// 派发层级：根 = 0，每次派发 +1
    pub depth: u32,
}

// (2) impl Default for AgentDef 中 mcps/skills 初始化之后：
            mcps: None,
            skills: None,
            parent_agent: None,
            depth: 0,
        }
    }

// (3) AgentDef::new() 中 mcps/skills 初始化之后：
            mcps: None,
            skills: None,
            parent_agent: None,
            depth: 0,
        }
    }
```

再在 `impl AgentDef` 的 builder 方法区域（`with_tool_config` 之后）追加：

```rust
    /// Set the dispatching parent agent id.
    pub fn with_parent_agent(mut self, id: impl Into<String>) -> Self {
        self.parent_agent = Some(id.into());
        self
    }

    /// Set the dispatch depth (root = 0, +1 per dispatch).
    pub fn with_depth(mut self, depth: u32) -> Self {
        self.depth = depth;
        self
    }
```

- [ ] **Step 4: 修复其余四个 AgentDef 字面量**（每个字面量末尾 `skills: ...` 行后补两行）

`crates/vol-llm-tools-builtin/tests/fixtures.rs:76`、`crates/vol-agent-server/src/data_plane/handlers/capability_tests.rs:51`、`crates/vol-llm-agent/src/agent_loader.rs:93`、`crates/vol-llm-tool/src/tool.rs:300`：

```rust
            parent_agent: None,
            depth: 0,
```

- [ ] **Step 5: 运行确认通过**

Run: `cargo test -p vol-llm-core test_agent_def_parent_and_depth && cargo check --workspace`
Expected: 测试 PASS；workspace 编译通过（无遗漏字面量）

- [ ] **Step 6: 提交**

```bash
git add crates/vol-llm-core/src/agent_def.rs crates/vol-llm-tools-builtin/tests/fixtures.rs crates/vol-agent-server/src/data_plane/handlers/capability_tests.rs crates/vol-llm-agent/src/agent_loader.rs crates/vol-llm-tool/src/tool.rs
git commit -m "feat(vol-llm-core): add parent_agent and depth to AgentDef"
```

---

### Task 2: AgentLoader::get_by_id（vol-llm-agent）

**Files:**
- Modify: `crates/vol-llm-agent/src/agent_loader.rs`
- Test: 同文件 tests 模块

**Interfaces:**
- Consumes: Task 1 的 `AgentDef.id` / `AgentDef.parent_agent` / `AgentDef.depth`
- Produces: `AgentLoader::get_by_id(&self, id: &str) -> Option<Arc<AgentDef>>` —— Task 4 使用

- [ ] **Step 1: 写失败测试**（追加到 agent_loader.rs tests 模块；该文件已有 `#[cfg(test)]` 模块）

```rust
    #[tokio::test]
    async fn test_get_by_id_hit_and_miss() {
        let loader = AgentLoader::new_empty();

        let mut a = AgentDef::new("a", "prompt a");
        a.id = "repo:a".to_string();
        let mut b = AgentDef::new("b", "prompt b");
        b.id = "user:b".to_string();

        loader.register(a).await;
        loader.register(b).await;

        let hit = loader.get_by_id("repo:a").await;
        assert!(hit.is_some());
        assert_eq!(hit.unwrap().name, "a");

        assert!(loader.get_by_id("user:b").await.is_some());
        // scope 前缀不同 → 未命中
        assert!(loader.get_by_id("repo:b").await.is_none());
        assert!(loader.get_by_id("missing").await.is_none());
    }
```

注意：tests 模块内需 `use crate::agent_def::AgentDef;` 与 `use super::*;`（按文件现有测试模块 import 方式补 AgentDef）。

- [ ] **Step 2: 运行确认失败**

Run: `cargo test -p vol-llm-agent test_get_by_id_hit_and_miss`
Expected: 编译失败——`no method named 'get_by_id'`

- [ ] **Step 3: 实现**（`get` 方法之后追加）

```rust
    /// Get full agent definition by unique id ("{scope}:{name}", e.g. "repo:test-runner").
    pub async fn get_by_id(&self, id: &str) -> Option<Arc<AgentDef>> {
        self.ensure_discovered().await;
        self.agents
            .read()
            .await
            .values()
            .find(|def| def.id == id)
            .cloned()
    }
```

- [ ] **Step 4: 运行确认通过**

Run: `cargo test -p vol-llm-agent test_get_by_id_hit_and_miss`
Expected: PASS

- [ ] **Step 5: 提交**

```bash
git add crates/vol-llm-agent/src/agent_loader.rs
git commit -m "feat(vol-llm-agent): add AgentLoader::get_by_id"
```

---

### Task 3: 新 crate vol-llm-agent-tool 骨架 + AgentTool 机械迁移

**Files:**
- Create: `crates/vol-llm-agent-tool/Cargo.toml`、`crates/vol-llm-agent-tool/src/lib.rs`、`crates/vol-llm-agent-tool/src/agent_tool.rs`
- Delete: `crates/vol-llm-agent/src/agent_tool.rs`
- Modify: `Cargo.toml`（workspace members）、`crates/vol-llm-agent/src/lib.rs`

**Interfaces:**
- Consumes: Task 2 的 `AgentLoader`
- Produces: crate `vol-llm-agent-tool`、`vol_llm_agent_tool::AgentTool`（本任务为旧语义占位，Task 4 重构）—— Task 6 使用

- [ ] **Step 1: 创建 Cargo.toml**

```toml
[package]
name = "vol-llm-agent-tool"
version.workspace = true
edition.workspace = true

[lints]
workspace = true

[dependencies]
serde = { workspace = true }
serde_json = { workspace = true }
tokio = { workspace = true }
async-trait = { workspace = true }
vol-llm-core = { path = "../vol-llm-core" }
vol-llm-tool = { path = "../vol-llm-tool" }
vol-session = { path = "../vol-session" }
vol-llm-agent = { path = "../vol-llm-agent" }
vol-llm-context = { path = "../vol-llm-context" }

[dev-dependencies]
tempfile = "3"
```

- [ ] **Step 2: workspace members 增加成员**（`Cargo.toml` 中 `"crates/vol-llm-agent",` 之后加一行）

```toml
    "crates/vol-llm-agent-tool",
```

- [ ] **Step 3: 创建 lib.rs**

```rust
//! vol-llm-agent-tool: AgentTool 派发工具 + AgentInjector 上下文贡献。
//!
//! 高层组合 crate：依赖 vol-llm-agent（ReAct 编排 / AgentLoader）、
//! vol-session（会话持久化）、vol-llm-tool（工具协议）等底层实现，
//! 被 vol-llm-runtime 依赖并注册为内置工具。

pub mod agent_tool;
pub use agent_tool::AgentTool;
```

- [ ] **Step 4: 迁移 agent_tool.rs（纯机械移动，仅改 import）**

把 `crates/vol-llm-agent/src/agent_tool.rs` 全文复制为 `crates/vol-llm-agent-tool/src/agent_tool.rs`，只改头部 import：

```rust
// 原：
use crate::agent_def::{AgentDef, AgentPath};
use crate::agent_loader::AgentLoader;
use crate::react::{AgentConfig, PluginRegistry};
// 改为：
use vol_llm_agent::agent_loader::AgentLoader;
use vol_llm_agent::react::{AgentConfig, PluginRegistry};
use vol_llm_agent::ReActAgent;
use vol_llm_core::agent_def::{AgentDef, AgentPath};
```

文件内其余引用同步替换：
- `crate::react::ReActAgent::new(...)` → `ReActAgent::new(...)`
- tests 模块里 `use crate::agent_def::AgentScope;` → `use vol_llm_core::agent_def::AgentScope;`

- [ ] **Step 5: 从 vol-llm-agent 移除**

`crates/vol-llm-agent/src/lib.rs` 删除两行：`pub mod agent_tool;` 与 `pub use agent_tool::AgentTool;`；删除文件 `crates/vol-llm-agent/src/agent_tool.rs`（`rm`）。

- [ ] **Step 6: 运行确认通过（旧语义、旧测试照常）**

Run: `cargo test -p vol-llm-agent-tool && cargo check --workspace`
Expected: 迁移的 3 个旧测试 PASS；workspace 编译通过（无 crate 还在引用 `vol_llm_agent::AgentTool`）

- [ ] **Step 7: 提交**

```bash
git add Cargo.toml crates/vol-llm-agent-tool crates/vol-llm-agent/src/lib.rs crates/vol-llm-agent/src/agent_tool.rs
git commit -m "refactor(vol-llm-agent-tool): move AgentTool into new crate"
```

---

### Task 4: AgentTool 语义重构（id 派发 / depth 守卫 / 持久化 session）

**Files:**
- Modify: `crates/vol-llm-agent-tool/src/agent_tool.rs`（整体重写，测试全量替换）
- Test: 同文件 tests 模块

**Interfaces:**
- Consumes: Task 2 的 `AgentLoader::get_by_id`、Task 1 的 `AgentDef.parent_agent/depth/tool_config`、`vol_session::SessionManager`（`entry_store_for_agent`）、`vol_llm_tool::ToolContext.agent_def`
- Produces: `AgentTool::new(loader: Arc<AgentLoader>, llm: Arc<dyn LLMClient>, session_manager: Arc<dyn SessionManager>, parent_tools: Weak<ToolRegistry>) -> AgentTool`；工具名 `agent`；参数 `id`/`prompt`/`description`；`pub const DEFAULT_MAX_DEPTH: u32 = 1` —— Task 6 使用

- [ ] **Step 1: 写失败测试**（整个 tests 模块替换为以下内容）

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use vol_llm_agent::agent_loader::AgentLoader;
    use vol_llm_core::{
        ConversationRequest, ConversationResponse, LLMProvider, StreamEvent, StreamEventData,
        StreamReceiver, SupportedParam,
    };
    use vol_session::FileSessionManager;

    /// Mock LLM：返回固定文本，统计调用次数。
    struct MockLlm {
        response_text: String,
        call_count: Arc<AtomicUsize>,
    }

    impl MockLlm {
        fn new(response_text: String) -> Self {
            Self {
                response_text,
                call_count: Arc::new(AtomicUsize::new(0)),
            }
        }

        fn call_count(&self) -> usize {
            self.call_count.load(Ordering::SeqCst)
        }
    }

    #[async_trait]
    impl LLMClient for MockLlm {
        fn provider(&self) -> LLMProvider {
            LLMProvider::Anthropic
        }
        fn model(&self) -> &str {
            "mock-model"
        }
        fn supported_params(&self) -> &[SupportedParam] {
            &[]
        }

        async fn converse(
            &self,
            _request: ConversationRequest,
        ) -> vol_llm_core::Result<ConversationResponse> {
            unimplemented!("Use converse_stream")
        }

        async fn converse_stream(
            &self,
            _request: ConversationRequest,
        ) -> vol_llm_core::Result<StreamReceiver> {
            use tokio::sync::mpsc;
            self.call_count.fetch_add(1, Ordering::SeqCst);
            let (tx, rx) = mpsc::channel(10);
            let text = self.response_text.clone();
            tokio::spawn(async move {
                let _ = tx
                    .send(Ok(StreamEvent {
                        id: "event_1".to_string(),
                        data: StreamEventData::ContentComplete { content: text },
                    }))
                    .await;
            });
            Ok(StreamReceiver::new(rx))
        }
    }

    /// 往临时目录的 `.agents/agents/echo.md` 写入 echo 定义并加载。
    async fn loader_with_echo() -> (tempfile::TempDir, Arc<AgentLoader>) {
        let temp_dir = tempfile::tempdir().unwrap();
        let agents_dir = temp_dir.path().join(".agents").join("agents");
        std::fs::create_dir_all(&agents_dir).unwrap();
        let mut f = std::fs::File::create(agents_dir.join("echo.md")).unwrap();
        writeln!(f, "---").unwrap();
        writeln!(f, "name: echo").unwrap();
        writeln!(f, "type: echo").unwrap();
        writeln!(f, "description: Echoes back the prompt").unwrap();
        writeln!(f, "---").unwrap();
        writeln!(f, "Echo the user's prompt exactly.").unwrap();
        let loader = Arc::new(AgentLoader::new(Some(temp_dir.path().to_path_buf())));
        loader.discover_all().await.unwrap();
        (temp_dir, loader)
    }

    fn caller_def(depth: u32) -> AgentDef {
        let mut def = AgentDef::new("root", "root agent");
        def.id = "repo:root".to_string();
        def.depth = depth;
        def
    }

    #[test]
    fn test_prepare_sub_def_writes_parent_and_depth() {
        let caller = caller_def(2);
        let mut target = AgentDef::new("echo", "prompt");
        target.id = "repo:echo".to_string();
        let sub = AgentTool::prepare_sub_def(Some(&caller), &target);
        assert_eq!(sub.parent_agent, Some("repo:root".to_string()));
        assert_eq!(sub.depth, 3);
        assert_eq!(sub.id, "repo:echo");
        assert_eq!(sub.name, "echo");
    }

    #[test]
    fn test_caller_max_depth_defaults_and_parses() {
        assert_eq!(AgentTool::caller_max_depth(None), 1);
        let def = caller_def(0);
        assert_eq!(AgentTool::caller_max_depth(Some(&def)), 1);

        let mut configured = caller_def(0);
        let mut tool_config = std::collections::HashMap::new();
        tool_config.insert(
            "agent".to_string(),
            serde_json::json!({ "max_depth": 3 }),
        );
        configured.tool_config = Some(tool_config);
        assert_eq!(AgentTool::caller_max_depth(Some(&configured)), 3);

        let mut invalid = caller_def(0);
        let mut bad_config = std::collections::HashMap::new();
        bad_config.insert(
            "agent".to_string(),
            serde_json::json!({ "max_depth": "nope" }),
        );
        invalid.tool_config = Some(bad_config);
        assert_eq!(AgentTool::caller_max_depth(Some(&invalid)), 1);
    }

    #[tokio::test]
    async fn test_agent_tool_depth_guard_default_rejects_nested() {
        let (temp_dir, loader) = loader_with_echo().await;
        let llm = Arc::new(MockLlm::new("ECHO".to_string()));
        let session_manager: Arc<dyn SessionManager> =
            Arc::new(FileSessionManager::new(temp_dir.path().join("sessions")));
        let registry = Arc::new(ToolRegistry::new());
        let tool = AgentTool::new(
            loader,
            llm,
            session_manager,
            Arc::downgrade(&registry),
        );

        // 调用方 depth=1、无 tool_config → 默认 max_depth=1 → 拒绝
        let ctx = ToolContext {
            agent_def: Some(caller_def(1)),
            ..ToolContext::default()
        };
        let args = serde_json::json!({
            "id": "repo:echo",
            "prompt": "help me",
            "description": "get help"
        });
        let result = tool.execute(&args, &ctx).await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("maximum dispatch depth"));
    }

    #[tokio::test]
    async fn test_agent_tool_tool_config_allows_nested() {
        let (temp_dir, loader) = loader_with_echo().await;
        let llm = Arc::new(MockLlm::new("ECHO: nested".to_string()));
        let session_manager: Arc<dyn SessionManager> =
            Arc::new(FileSessionManager::new(temp_dir.path().join("sessions")));
        let registry = Arc::new(ToolRegistry::new());
        let tool = AgentTool::new(
            loader,
            llm,
            session_manager,
            Arc::downgrade(&registry),
        );

        // 调用方 tool_config.agent.max_depth=3、depth=1 → 允许
        let mut caller = caller_def(1);
        let mut tool_config = std::collections::HashMap::new();
        tool_config.insert(
            "agent".to_string(),
            serde_json::json!({ "max_depth": 3 }),
        );
        caller.tool_config = Some(tool_config);
        let ctx = ToolContext {
            agent_def: Some(caller),
            ..ToolContext::default()
        };
        let args = serde_json::json!({
            "id": "repo:echo",
            "prompt": "nested task",
            "description": "nested"
        });
        let result = tool.execute(&args, &ctx).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().content, "ECHO: nested");
    }

    #[tokio::test]
    async fn test_agent_tool_id_not_found_lists_available() {
        let (temp_dir, loader) = loader_with_echo().await;
        let llm = Arc::new(MockLlm::new("unused".to_string()));
        let session_manager: Arc<dyn SessionManager> =
            Arc::new(FileSessionManager::new(temp_dir.path().join("sessions")));
        let registry = Arc::new(ToolRegistry::new());
        let tool = AgentTool::new(
            loader,
            llm,
            session_manager,
            Arc::downgrade(&registry),
        );

        let ctx = ToolContext {
            agent_def: Some(caller_def(0)),
            ..ToolContext::default()
        };
        let args = serde_json::json!({
            "id": "repo:missing",
            "prompt": "do something",
            "description": "test task"
        });
        let result = tool.execute(&args, &ctx).await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("not found"));
        assert!(err.contains("repo:echo"));
    }

    #[tokio::test]
    async fn test_agent_tool_dispatch_persists_session_by_name() {
        let (temp_dir, loader) = loader_with_echo().await;
        let llm = Arc::new(MockLlm::new("ECHO: persisted".to_string()));
        let session_manager: Arc<dyn SessionManager> =
            Arc::new(FileSessionManager::new(temp_dir.path().join("sessions")));
        let registry = Arc::new(ToolRegistry::new());
        let tool = AgentTool::new(
            loader,
            llm.clone(),
            session_manager.clone(),
            Arc::downgrade(&registry),
        );

        let ctx = ToolContext {
            agent_def: Some(caller_def(0)),
            ..ToolContext::default()
        };
        let args = serde_json::json!({
            "id": "repo:echo",
            "prompt": "test prompt",
            "description": "test echo"
        });
        let result = tool.execute(&args, &ctx).await;
        assert!(result.is_ok());
        assert_eq!(llm.call_count(), 1);

        // 会话按 def.name（"echo"）持久化，list_sessions 可查
        let sessions = session_manager.list_sessions(Some("echo")).await.unwrap();
        assert_eq!(sessions.len(), 1);
    }
}
```

- [ ] **Step 2: 运行确认失败**

Run: `cargo test -p vol-llm-agent-tool`
Expected: 编译失败——`AgentTool::prepare_sub_def` / `caller_max_depth` 不存在，构造参数不匹配

- [ ] **Step 3: 重写实现**（`crates/vol-llm-agent-tool/src/agent_tool.rs` 全文替换）

```rust
//! AgentTool — dispatches sub-agents by id, running a full ReAct loop.
//!
//! # Design Notes
//!
//! - 子 agent = `.agents/agents/` 中内置定义，按唯一 `AgentDef.id` 派发
//! - 深度守卫：调用方 `AgentDef.depth >= 调用方 tool_config.agent.max_depth`（默认 1）时拒绝
//! - 子 agent 会话按 `def.name` 经运行时 SessionManager 持久化，可被其他 agent / UI 观测
//! - `parent_tools` 以 `Weak` 持有：运行时注册时 registry 先 Arc 化、AgentTool 后注册，
//!   Weak 避免注册期循环引用（execute 时 upgrade）
//!
//! # YAGNI Notes
//!
//! - `AgentDef.tools` / `disallowed_tools` / `model` 不在本周期（见设计文档 Non-Goals）
//! - Sensitivity 返回 `Safe`：派发器本身无副作用；子 agent 的工具调用仍经各自 sensitivity 评估

use std::sync::{Arc, Weak};

use async_trait::async_trait;
use serde::Deserialize;
use vol_llm_agent::react::{AgentConfig, PluginRegistry};
use vol_llm_agent::{agent_loader::AgentLoader, ReActAgent};
use vol_llm_core::agent_def::AgentDef;
use vol_llm_core::LLMClient;
use vol_llm_tool::{
    ExecutableTool, ToolContext, ToolError, ToolRegistry, ToolResult, ToolResultType,
    ToolSensitivity,
};
use vol_session::{Session, SessionManager};

/// 默认派发深度上限（调用方 tool_config 未配置时）。
pub const DEFAULT_MAX_DEPTH: u32 = 1;

/// Default system prompt for agents with empty body.
const DEFAULT_AGENT_PROMPT: &str =
    "You are a specialized AI agent. Follow the instructions provided.";

/// Parameters for the Agent tool.
#[derive(Debug, Deserialize)]
pub struct AgentToolParams {
    /// 目标 agent 的唯一 id（"{scope}:{name}"，如 "repo:explore"）
    pub id: String,
    /// Full task instructions for the sub-agent
    pub prompt: String,
    /// Short (3-5 word) description of the task
    pub description: String,
}

/// Tool that dispatches sub-agents by id.
pub struct AgentTool {
    loader: Arc<AgentLoader>,
    llm: Arc<dyn LLMClient>,
    session_manager: Arc<dyn SessionManager>,
    parent_tools: Weak<ToolRegistry>,
}

impl AgentTool {
    /// Create a new AgentTool.
    pub fn new(
        loader: Arc<AgentLoader>,
        llm: Arc<dyn LLMClient>,
        session_manager: Arc<dyn SessionManager>,
        parent_tools: Weak<ToolRegistry>,
    ) -> Self {
        Self {
            loader,
            llm,
            session_manager,
            parent_tools,
        }
    }

    /// 读取调用方 tool_config 中的 `agent.max_depth`（缺省/非法 → DEFAULT_MAX_DEPTH）。
    fn caller_max_depth(caller: Option<&AgentDef>) -> u32 {
        caller
            .and_then(|d| d.tool_config.as_ref())
            .and_then(|cfg| cfg.get("agent"))
            .and_then(|v| v.get("max_depth"))
            .and_then(|v| v.as_u64())
            .and_then(|v| u32::try_from(v).ok())
            .unwrap_or(DEFAULT_MAX_DEPTH)
    }

    /// 由调用方与目标定义生成子 agent 定义：写入 parent_agent 与 depth+1。
    fn prepare_sub_def(caller: Option<&AgentDef>, def: &AgentDef) -> AgentDef {
        let mut sub_def = def.clone();
        sub_def.parent_agent = caller.map(|c| c.id.clone());
        sub_def.depth = caller.map(|c| c.depth + 1).unwrap_or(1);
        sub_def
    }

    /// Format an error response with available agents.
    async fn format_id_not_found(&self, id: &str) -> String {
        let metadata = self.loader.list_metadata().await;
        let mut output = format!("Agent id '{id}' not found.\n\n");
        if metadata.is_empty() {
            output.push_str(
                "No agents are defined. Create .md files in .agents/agents/ to define custom agents.",
            );
        } else {
            output.push_str("Available agents:\n");
            for m in &metadata {
                output.push_str(&format!("- {} ({}): {}\n", m.id, m.name, m.description));
            }
        }
        output
    }
}

#[async_trait]
impl ExecutableTool for AgentTool {
    fn name(&self) -> &'static str {
        "agent"
    }

    fn description(&self) -> &'static str {
        "Dispatch a sub-agent (defined in .agents/agents/) by id to handle a task. \
         The sub-agent runs a full ReAct loop and its final result is returned."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "id": {
                    "type": "string",
                    "description": "Id of the agent to dispatch (e.g. repo:explore)"
                },
                "prompt": {
                    "type": "string",
                    "description": "Full task instructions for the sub-agent"
                },
                "description": {
                    "type": "string",
                    "description": "Short (3-5 word) description of the task"
                }
            },
            "required": ["id", "prompt", "description"]
        })
    }

    fn sensitivity(&self, _args: &serde_json::Value) -> ToolSensitivity {
        ToolSensitivity::Safe
    }

    async fn execute(
        &self,
        args: &serde_json::Value,
        context: &ToolContext,
    ) -> ToolResultType<ToolResult> {
        let params: AgentToolParams = serde_json::from_value(args.clone())
            .map_err(|e| ToolError::InvalidArguments(format!("Failed to parse arguments: {e}")))?;

        let caller = context.agent_def.as_ref();
        let caller_depth = caller.map(|c| c.depth).unwrap_or(0);
        let max_depth = Self::caller_max_depth(caller);

        // 深度守卫：默认 1 = 只允许派发一层（根 agent 可派发，depth≥1 拒绝）
        if caller_depth >= max_depth {
            return Err(ToolError::ExecutionFailed(format!(
                "Cannot dispatch: maximum dispatch depth ({}) reached (caller depth {})",
                max_depth, caller_depth
            )));
        }

        // 按 id 查找内置定义
        let def = match self.loader.get_by_id(&params.id).await {
            Some(def) => def,
            None => {
                let error_msg = self.format_id_not_found(&params.id).await;
                return Err(ToolError::ExecutionFailed(error_msg));
            }
        };

        // 生成子 agent 定义：记录 parent_agent 与 depth
        let sub_def = Self::prepare_sub_def(caller, &def);

        let system_prompt = if sub_def.prompt.trim().is_empty() {
            DEFAULT_AGENT_PROMPT.to_string()
        } else {
            sub_def.prompt.clone()
        };

        // 子 agent 会话按 name 持久化（与 register_agent 一致，session.list 可查）
        let parent_tools = self.parent_tools.upgrade().ok_or_else(|| {
            ToolError::ExecutionFailed("tool registry unavailable".to_string())
        })?;
        let session = Arc::new(Session::new(
            self.session_manager.entry_store_for_agent(&sub_def.name),
        ));

        let agent_config = AgentConfig::builder()
            .with_def(sub_def)
            .with_llm(self.llm.clone())
            .with_tools(parent_tools)
            .with_session(session)
            .with_system_prompt(system_prompt)
            .with_plugin_registry(PluginRegistry::new())
            .build()
            .map_err(|e| {
                ToolError::ExecutionFailed(format!("Failed to build agent config: {e}"))
            })?;

        let base_tools = agent_config.tools.clone();
        let skill_loader = agent_config.skill_loader.clone();
        let sub_agent = ReActAgent::new(agent_config, base_tools, skill_loader);

        let response = sub_agent
            .run(&params.prompt)
            .await
            .map_err(|e| ToolError::ExecutionFailed(format!("Sub-agent failed: {e}")))?;

        Ok(ToolResult::success(response.content))
    }
}
```

- [ ] **Step 4: 运行确认通过**

Run: `cargo test -p vol-llm-agent-tool`
Expected: 6 个测试全部 PASS

- [ ] **Step 5: 提交**

```bash
git add crates/vol-llm-agent-tool/src/agent_tool.rs
git commit -m "feat(vol-llm-agent-tool): dispatch by id with depth guard and persisted session"
```

---

### Task 5: AgentInjector（vol-llm-agent-tool）

**Files:**
- Create: `crates/vol-llm-agent-tool/src/injector.rs`
- Modify: `crates/vol-llm-agent-tool/src/lib.rs`（导出）
- Test: `crates/vol-llm-agent-tool/src/injector.rs` tests 模块

**Interfaces:**
- Consumes: `AgentLoader::list_metadata`、`vol_llm_context::{AttentionAnchor, ContextBlock, ContextContributor}`、`vol_llm_core::Message`
- Produces: `AgentInjector::new(loader: Arc<AgentLoader>) -> AgentInjector`（anchor 固定 `Head(1)`）；contributor 名 `"agents"` —— Task 6 使用

- [ ] **Step 1: 写失败测试**（`injector.rs` 底部 tests 模块）

```rust
#[cfg(test)]
mod tests {
    use super::*;

    async fn loader_with_defs() -> (tempfile::TempDir, Arc<AgentLoader>) {
        let temp_dir = tempfile::tempdir().unwrap();
        let agents_dir = temp_dir.path().join(".agents").join("agents");
        std::fs::create_dir_all(&agents_dir).unwrap();
        let mut f = std::fs::File::create(agents_dir.join("explore.md")).unwrap();
        writeln!(f, "---").unwrap();
        writeln!(f, "name: explore").unwrap();
        writeln!(f, "type: explore").unwrap();
        writeln!(f, "description: 搜索代码库").unwrap();
        writeln!(f, "---").unwrap();
        writeln!(f, "You explore the codebase.").unwrap();
        let loader = Arc::new(AgentLoader::new(Some(temp_dir.path().to_path_buf())));
        loader.discover_all().await.unwrap();
        (temp_dir, loader)
    }

    #[tokio::test]
    async fn test_injector_contributes_agent_list() {
        let (temp_dir, loader) = loader_with_defs().await;
        let injector = AgentInjector::new(loader);
        let blocks = injector.contribute().await.unwrap();
        assert_eq!(blocks.len(), 1);
        // ContextBlock.messages 是 pub 字段；用 Debug 输出做稳健断言
        let text = format!("{:?}", blocks[0].messages);
        assert!(text.contains("`agent` tool"), "text: {text}");
        assert!(text.contains("repo:explore"), "text: {text}");
        assert!(text.contains("搜索代码库"), "text: {text}");
    }

    #[tokio::test]
    async fn test_injector_empty_when_no_defs() {
        let injector = AgentInjector::new(Arc::new(AgentLoader::new_empty()));
        let blocks = injector.contribute().await.unwrap();
        assert_eq!(blocks.len(), 1);
        assert!(blocks[0].messages.is_empty());
    }
}
```

注意：`ContextBlock` 的 `messages` / `anchor` 是 pub 字段（`vol-llm-context/src/context_block.rs`），无 getter。

- [ ] **Step 2: 运行确认失败**

Run: `cargo test -p vol-llm-agent-tool test_injector`
Expected: 编译失败——`AgentInjector` 未定义

- [ ] **Step 3: 实现**（`injector.rs`，参考 `vol-llm-skill/src/injector.rs` 的 SkillInjector，去掉 skill_filter）

```rust
//! AgentInjector — 把可用 agent 定义贡献进上下文，提示可用 `agent` 工具派发 subagent。

use std::sync::Arc;

use async_trait::async_trait;
use vol_llm_agent::agent_loader::AgentLoader;
use vol_llm_context::{AttentionAnchor, ContextBlock, ContextContributor};
use vol_llm_core::Message;

/// Formats agent metadata for context injection（参考 vol-llm-skill::SkillInjector）。
pub struct AgentInjector {
    loader: Arc<AgentLoader>,
    anchor: AttentionAnchor,
    cached_size: tokio::sync::Mutex<usize>,
}

impl AgentInjector {
    /// Create an AgentInjector; anchor 固定 Head(1)，跟随 skill 惯例。
    pub fn new(loader: Arc<AgentLoader>) -> Self {
        Self {
            loader,
            anchor: AttentionAnchor::Head(1),
            cached_size: tokio::sync::Mutex::new(0),
        }
    }

    /// Format metadata as prompt string. Returns empty string if no agents are defined.
    pub async fn format_metadata(&self) -> String {
        let metadata = self.loader.list_metadata().await;
        if metadata.is_empty() {
            return String::new();
        }
        let mut output = String::from(
            "You can dispatch sub-agents to handle tasks collaboratively using the `agent` tool (args: id, prompt, description). Available agents:\n",
        );
        for m in &metadata {
            output.push_str(&format!("- {} ({}): {}\n", m.id, m.name, m.description));
        }
        output
    }
}

#[async_trait]
impl ContextContributor for AgentInjector {
    fn name(&self) -> &str {
        "agents"
    }

    async fn contribute(&self) -> Result<Vec<ContextBlock>, vol_llm_context::ContextError> {
        let metadata_text = self.format_metadata().await;
        if metadata_text.is_empty() {
            // 无定义时保持固定 Head 槽位（与 SkillInjector 行为一致）
            *self.cached_size.lock().await = 0;
            return Ok(vec![ContextBlock::new(vec![], self.anchor.clone())]);
        }
        let msg = Message::user(metadata_text);
        let size = vol_llm_context::estimate_tokens(&msg);
        *self.cached_size.lock().await = size;
        Ok(vec![ContextBlock::new(vec![msg], self.anchor.clone())])
    }

    async fn compress(&mut self) {
        // 静态提示内容，无需压缩。
    }
}
```

同时 `lib.rs` 增加：

```rust
pub mod injector;
pub use injector::AgentInjector;
```

- [ ] **Step 4: 运行确认通过**

Run: `cargo test -p vol-llm-agent-tool`
Expected: 全部 PASS（Task 4 的 6 个 + 本任务 2 个）

- [ ] **Step 5: 提交**

```bash
git add crates/vol-llm-agent-tool/src/injector.rs crates/vol-llm-agent-tool/src/lib.rs
git commit -m "feat(vol-llm-agent-tool): add AgentInjector context contributor"
```

---

### Task 6: 运行时接线（vol-llm-runtime）

**Files:**
- Modify: `crates/vol-llm-runtime/Cargo.toml`、`crates/vol-llm-runtime/src/lib.rs`
- Test: `crates/vol-llm-runtime/src/lib.rs` tests 模块

**Interfaces:**
- Consumes: Task 4 的 `AgentTool::new`、Task 5 的 `AgentInjector::new`
- Produces: `AgentRuntime.agent_loader: Arc<AgentLoader>`（pub）；`build()` / `for_test()` 的 registry 含 `agent` 工具；`register_agent` 挂 AgentInjector

- [ ] **Step 1: Cargo.toml 增加依赖**（`vol-llm-sandbox` 行之后）

```toml
vol-llm-agent-tool = { path = "../vol-llm-agent-tool" }
```

- [ ] **Step 2: imports 增加**（`crates/vol-llm-runtime/src/lib.rs` 顶部）

```rust
use vol_llm_agent_tool::{AgentInjector, AgentTool};
```

- [ ] **Step 3: AgentRuntime 增加字段**（`pub skill_loader` 行之后）

```rust
    pub skill_loader: Arc<SkillLoader>,
    /// 共享 AgentLoader：注册定义、AgentTool 派发、AgentInjector 注入同源
    pub agent_loader: Arc<AgentLoader>,
```

- [ ] **Step 4: build() 注册 AgentTool**

（a）在 `let task_store: Arc<dyn TaskStore> = ...` 块之前插入：

```rust
        // 共享 AgentLoader：注册定义、AgentTool 派发、AgentInjector 注入同源
        let agent_loader = Arc::new(AgentLoader::new(Some(self.working_dir.clone())));
```

（b）把工具注册块结尾 `let tool_registry = Arc::new(tool_registry);` 改为 `let mut tool_registry = Arc::new(tool_registry);`，并在其后追加：

```rust
        // Arc 化后注册 AgentTool：Weak 规避注册期循环引用（registry 先 Arc、工具后注册）
        let agent_tool_llm: Arc<dyn vol_llm_core::LLMClient> = {
            let ids = llm_registry.ids();
            let first_id = ids
                .first()
                .ok_or_else(|| "No LLM providers configured".to_string())?;
            let fc = llm_registry
                .get(first_id)
                .ok_or_else(|| "Provider not found".to_string())?;
            create_provider(&fc.to_llm_config())
                .map(Arc::from)
                .map_err(|e| format!("LLM error: {e}"))?
        };
        let agent_tool_weak = Arc::downgrade(&tool_registry);
        Arc::get_mut(&mut tool_registry)
            .expect("sole owner of tool_registry before AgentTool registration")
            .register(AgentTool::new(
                agent_loader.clone(),
                agent_tool_llm,
                session_manager.clone(),
                agent_tool_weak,
            ));
```

注意：`session_manager` 与 `llm_registry` 变量在 build() 中已存在（本块之后原代码继续使用 `tool_registry` Arc，兼容）。

（c）`AgentRuntime { ... }` 字面量（build() 尾部）加 `agent_loader,`。

- [ ] **Step 5: discover_agents 复用共享 loader**

```rust
    /// Discover and register all agents from .agents/agents/ directories.
    pub async fn discover_agents(&self) -> Result<Vec<(String, ReActAgent)>, String> {
        let loader = self.agent_loader.clone();
        loader.discover_all().await.map_err(|e| e.to_string())?;

        let mut registered = Vec::new();
        let agents = loader.list_metadata().await;
        for meta in agents {
            if let Some(def) = loader.get(&meta.name).await {
                let arc_def = Arc::try_unwrap(def).unwrap_or_else(|arc| (*arc).clone());
                let agent = self.register_agent(&meta.name, arc_def).await?;
                registered.push((meta.name, agent));
            }
        }
        Ok(registered)
    }
```

- [ ] **Step 6: register_agent 挂 AgentInjector**

```rust
        let mut config = AgentConfig::builder()
            .with_def(def.clone())
            .with_llm(llm)
            // Full, unfiltered registry — ReActAgent::resolve_tools re-filters
            // from base_tools at run start (def.tools / disallowed_tools / mcps).
            .with_tools(self.tool_registry.clone())
            .with_session(session)
            .with_working_dir(agent_dir)
            .with_contributor(Box::new(AgentInjector::new(self.agent_loader.clone())))
            .build()
            .expect("AgentConfig build failed — all required fields provided");
```

- [ ] **Step 7: for_test() 镜像**（`crates/vol-llm-runtime/src/lib.rs` for_test 内）

（a）fs 注册行之后、`let tool_registry = Arc::new(tool_registry);` 处同样改为 `let mut tool_registry = Arc::new(tool_registry);`，追加：

```rust
        // 内置 agent 工具：for_test 用默认 provider 与空 AgentLoader
        let agent_tool_llm: Arc<dyn vol_llm_core::LLMClient> = {
            let ids = llm_registry.ids();
            let first_id = ids
                .first()
                .expect("ProviderLoader::default() provides at least one provider");
            let fc = llm_registry
                .get(first_id)
                .expect("provider registered in registry");
            create_provider(&fc.to_llm_config())
                .expect("default provider creates")
                .into()
        };
        let agent_tool_weak = Arc::downgrade(&tool_registry);
        Arc::get_mut(&mut tool_registry)
            .expect("sole owner of tool_registry before AgentTool registration")
            .register(AgentTool::new(
                Arc::new(AgentLoader::new_empty()),
                agent_tool_llm,
                session_manager.clone(),
                agent_tool_weak,
            ));
```

（b）for_test 的 `AgentRuntime { ... }` 字面量加 `agent_loader: Arc::new(AgentLoader::new_empty()),`。

- [ ] **Step 8: 测试构造点补字段**

`crates/vol-llm-runtime/src/lib.rs` 中另外两处 `AgentRuntime { ... }` 字面量（tests 模块，约 1144 行与 1177 行）各加：

```rust
            agent_loader: Arc::new(AgentLoader::new_empty()),
```

- [ ] **Step 9: 集成测试**（tests 模块，`for_test_registers_fs_cli_tool` 之后追加）

```rust
    #[tokio::test]
    async fn for_test_registers_agent_tool() {
        let rt = AgentRuntime::for_test().await;
        let names = rt.tool_registry.tool_names();
        assert!(
            names.iter().any(|n| *n == "agent"),
            "agent tool not registered: {names:?}"
        );
    }
```

- [ ] **Step 10: 运行确认通过**

Run: `cargo test -p vol-llm-runtime`
Expected: 全部 PASS（含新测试 `for_test_registers_agent_tool`）

- [ ] **Step 11: 提交**

```bash
git add crates/vol-llm-runtime/Cargo.toml crates/vol-llm-runtime/src/lib.rs
git commit -m "feat(vol-llm-runtime): register builtin agent tool and AgentInjector"
```

---

### Task 7: 质量门、wiki-ingest 与文档收尾

**Files:**
- Modify: `CLAUDE.md`
- Modify: `docs/wiki/**`（wiki-ingest 产出）

- [ ] **Step 1: 全量检查**

Run:

```bash
cargo fmt --all
cargo check --workspace
cargo test -p vol-llm-core -p vol-llm-agent -p vol-llm-agent-tool -p vol-llm-runtime
./scripts/check-no-doc-tests.sh
./scripts/check-agent-boundaries.sh
```

Expected: 全部通过；`check-agent-boundaries.sh` 确认 vol-llm-runtime 未依赖 vol-agent-server、vol-llm-agent-tool 未依赖 vol-llm-runtime

- [ ] **Step 2: 覆盖 gate**

Run: `just cover-gate vol-llm-core 80 && just cover-gate vol-llm-agent 80 && just cover-gate vol-llm-agent-tool 80 && just cover-gate vol-llm-runtime 80`
Expected: 四个 crate 全部 ≥80%（若 vol-llm-agent-tool 不足，补齐 agent_tool.rs / injector.rs 分支测试后重跑）

- [ ] **Step 3: CLAUDE.md 更新 crates 清单**（`vol-llm-agent/` 行之后加）

```markdown
├── vol-llm-agent-tool/    # AgentTool 派发工具 + AgentInjector（高层组合 crate）
```

- [ ] **Step 4: wiki-ingest**

Run: 调用 wiki-ingest skill，摄入 sources：`docs/superpowers/specs/2026-08-20-agenttool-builtin-design.md`、本计划文档、实现 diff 摘要（新 crate、AgentDef 扩展、运行时注册）。
Expected: wiki 索引与相关 entity 页更新（vol-llm-agent-tool-crate、agenttool-builtin 概念等）

- [ ] **Step 5: 提交**

```bash
git add CLAUDE.md docs/wiki
git commit -m "docs(wiki): ingest AgentTool builtin implementation"
```

- [ ] **Step 6: 收尾验证**

Run: `just cover-gate vol-llm-agent-tool 80`（终检）与 `git log --oneline -8`
Expected: 7 个任务提交齐全，无遗漏未提交文件（`git status` 干净，除会话开始前已存在的无关改动）
