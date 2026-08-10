# agent.submit 响应合并 & 三段覆盖统一设计

## 概述

1. 合并 `agent.submit` 的双响应（Ack + Result）为单个 `SubmitResult`
2. `SubmitResult` 包含本次 run 实际使用的 provider、tools、skills、mcps
3. Tools / Skills / MCPs 统一三段覆盖逻辑（全局 → AgentDef → Overlay），`ReActAgent` 提供只读方法对外暴露
4. 消除 `RunContext::effective_registry()` 中改写 `skill_injector_filter` 的副作用

---

## 资源的三段覆盖模型

三个资源走同样的模式：

```
Tier 1: 全局全量        Tier 2: AgentDef 过滤        Tier 3: Overlay 覆盖
─────────────────       ──────────────────────       ───────────────────────
ToolRegistry 全量        def.tools (allowlist)        overlay.effective_tools
                         def.disallowed_tools          + effective_mcp_servers

SkillLoader 全量         def.skills (新增)             overlay.effective_skills

McpManager 全量          def.mcps                      overlay.effective_mcp_servers
```

核心原则：
- 每个 resolve 方法返回**同类型子集**（`Arc<ToolRegistry>` / `SkillInjector` / `Arc<McpManager>`）
- Agent loop 和 handler 查询走**同一条 resolve 路径**
- RunContext 在 run 开始时 resolve 好，后续直接用

---

## 变更

### 1. AgentDef — 新增 skills 字段

**文件**: `crates/vol-llm-core/src/agent_def.rs`

```rust
pub struct AgentDef {
    // ... 现有字段不变 ...
    pub skills: Option<Vec<String>>,  // NEW: skill allowlist, None = 全量
}
```

**文件**: `crates/vol-llm-agent/src/agent_def.rs` — `AgentFrontmatter` 新增 `skills` 字段，用于从 agent markdown 的 YAML frontmatter 解析。

**文件**: `crates/vol-agent-server/src/data_plane/handlers/capability.rs` — `update_capabilities` 校验新增 skill 的 AgentDef allowlist 检查（与 tools/mcps 一致）：如果 `def.skills` 为 `Some(list)` 且请求的 skill 不在 list 中 → 拒绝。

### 2. Protocol — SubmitResult 合并 Ack，新增结构化字段

**文件**: `crates/vol-llm-agent-protocol/src/agent_server_protocol.rs`

```rust
pub struct ProviderInfo {
    pub name: String,    // provider().to_string()
    pub model: String,   // model()
}

// SubmitResult 修改
AgentPayload::SubmitResult {
    run_id: String,
    accepted: bool,                         // 从 SubmitAck 合并
    provider: ProviderInfo,
    tools: Vec<String>,                     // tool names
    mcps: Vec<String>,                      // MCP server names
    skills: Vec<String>,                    // skill names
}
```

- `SubmitAck` variant 保留不删（其他场景可能用），但 `agent.submit` handler 不再产生 Ack
- `response: serde_json::Value` 字段移除

### 3. ReActAgent — 三个 resolve 方法 + 四个 public 查询方法

**文件**: `crates/vol-llm-agent/src/react/agent.rs`

新增字段：
```rust
pub struct ReActAgent {
    config: AgentConfig,
    base_tools: Arc<ToolRegistry>,       // 全局全量（未过滤）
    skill_loader: Arc<SkillLoader>,      // 全局全量
    // ... 现有字段 ...
}
```

Public 查询方法（handler 用，读当前 session）：
```rust
impl ReActAgent {
    /// LLM 客户端（handler 自己取 provider/model）
    pub fn llm(&self) -> &Arc<dyn LLMClient> { &self.config.llm }

    /// 本次 run 实际使用的 tools（子集 ToolRegistry）
    pub fn tools(&self) -> Arc<ToolRegistry> {
        self.resolve_tools(&self.current_session_id())
    }

    /// 本次 run 实际使用的 skills（过滤后的 SkillInjector）
    pub fn skills(&self) -> SkillInjector {
        self.resolve_skills(&self.current_session_id())
    }

    /// 本次 run 实际使用的 MCPs（子集 McpManager）
    pub fn mcps(&self) -> Arc<McpManager> {
        self.resolve_mcps(&self.current_session_id())
    }
}
```

Internal resolve 方法（agent loop 也走这里）。

三段覆盖优先级：**Overlay 非空 → 用它；Overlay 空 → 用 AgentDef；AgentDef 无 → 全量**。空列表在 Overlay 中语义为"不覆盖"（即回退到 AgentDef），在 AgentDef 中语义为"空集"（即不允许任何）。

```rust
impl ReActAgent {
    fn resolve_tools(&self, sid: &str) -> Arc<ToolRegistry> {
        let overlay = self.get_overlay(sid);

        // 1. allowlist: overlay (non-empty) > def.tools
        let allowed = overlay.as_ref()
            .and_then(|o| if o.effective_tools.is_empty() { None }
                        else { Some(o.effective_tools.as_slice()) })
            .or_else(|| self.config.def.as_ref().and_then(|d| d.tools.as_deref()));

        // 2. blocklist: always from def
        let disallowed = self.config.def.as_ref()
            .and_then(|d| d.disallowed_tools.as_deref());

        let mut filtered = self.base_tools.filter(allowed, disallowed);

        // 3. MCP tool 过滤: overlay (non-empty) > def.mcps
        let mcps = overlay.as_ref()
            .and_then(|o| if o.effective_mcp_servers.is_empty() { None }
                        else { Some(o.effective_mcp_servers.as_slice()) })
            .or_else(|| self.config.def.as_ref().and_then(|d| d.mcps.as_deref()));
        if let Some(mcp_names) = mcps {
            filtered = Arc::new(filtered.filter_mcp_servers(mcp_names));
        }

        filtered
    }

    fn resolve_skills(&self, sid: &str) -> SkillInjector {
        let overlay = self.get_overlay(sid);

        // overlay non-empty > def.skills
        let filter: Option<Vec<String>> = overlay.as_ref()
            .and_then(|o| if o.effective_skills.is_empty() { None }
                        else { Some(o.effective_skills.clone()) })
            .or_else(|| self.config.def.as_ref().and_then(|d| d.skills.clone()));

        SkillInjector::new(
            self.skill_loader.clone(),
            AttentionAnchor::Head(1),
            filter,
        )
    }

    fn resolve_mcps(&self, sid: &str) -> Arc<McpManager> {
        let overlay = self.get_overlay(sid);

        // overlay non-empty > def.mcps
        let filter: Option<&[String]> = overlay.as_ref()
            .and_then(|o| if o.effective_mcp_servers.is_empty() { None }
                        else { Some(o.effective_mcp_servers.as_slice()) })
            .or_else(|| self.config.def.as_ref().and_then(|d| d.mcps.as_deref()));

        self.config.mcp_manager.as_ref()
            .map(|m| Arc::new(m.filter(filter)))
            .unwrap_or_else(|| Arc::new(McpManager::empty()))
    }

    fn get_overlay(&self, sid: &str) -> Option<CapabilityOverlay> {
        self.config.capability_overlays.as_ref()
            .and_then(|map| map.try_read().ok())
            .and_then(|guard| guard.get(&(self.config.agent_id.clone(), sid.to_string())).cloned())
    }

    fn current_session_id(&self) -> String {
        self.config.session.read().unwrap().id.clone()
    }
}
```

### 4. AgentConfig — 删 skill_injector_filter

**文件**: `crates/vol-llm-agent/src/react/agent.rs`

```rust
pub struct AgentConfig {
    // ... 字段不变 ...
    // skill_injector_filter: Option<Arc<RwLock<Option<Vec<String>>>>>  ← 删除
}
```

`config_builder.rs` 中不再捕获 `skill_injector_filter`。

### 5. RunContext — 简化为预 resolve

**文件**: `crates/vol-llm-agent/src/react/run_context.rs`

- 删除 `effective_registry()` / `effective_tools()` / `execute_tool()` 中复杂的过滤逻辑
- 删除 `skill_injector_filter` 副作用代码
- 删除 `capability_overlays` / `current_overlay_version` 字段（不再需要）

保留：
```rust
pub struct RunContext {
    pub tools: Arc<ToolRegistry>,       // ← run 开始时 resolve 好的子集
    pub mcps: Arc<McpManager>,          // ← run 开始时 resolve 好的子集
    // skills 走 SkillInjector contributor，在 context builder 中替换
    // ...
}
```

Agent loop 直接 `self.tools.definitions()` 不再经过过滤。

### 6. run_input — run 开始时 resolve

**文件**: `crates/vol-llm-agent/src/react/agent.rs`

```rust
pub async fn run_input(&self, input: AgentInput) -> Result<AgentResponse, AgentError> {
    // ... 现有初始化 ...

    let sid = session.id.clone();
    let tools = self.resolve_tools(&sid);
    let mcps = self.resolve_mcps(&sid);
    let skills = self.resolve_skills(&sid);

    let run_ctx = RunContext::new(run_id, user_input, self.config.clone())
        .with_tools(tools)
        .with_mcps(mcps);

    // 用 resolve 好的 SkillInjector 替换 context builder 中的旧 skills contributor
    run_ctx.replace_contributor("skills", Box::new(skills));

    // ... agent loop 直接 run_ctx.tools.definitions() ...
}
```

### 7. SkillInjector — 提供 skill_names()

**文件**: `crates/vol-llm-skill/src/injector.rs`

```rust
impl SkillInjector {
    /// 当前过滤后生效的 skill 名称列表
    pub async fn skill_names(&self) -> Vec<String> {
        let metadata = self.loader.list_metadata().await;
        let filter_guard = self.skill_filter.read().await;
        match &*filter_guard {
            Some(filter) if !filter.is_empty() => {
                let set: HashSet<&str> = filter.iter().map(String::as_str).collect();
                metadata.into_iter()
                    .filter(|m| set.contains(m.name.as_str()))
                    .map(|m| m.name)
                    .collect()
            }
            _ => metadata.into_iter().map(|m| m.name).collect(),
        }
    }
}
```

### 8. McpManager — filter 方法

**文件**: `crates/vol-llm-mcp/src/manager.rs`

```rust
impl McpManager {
    /// 返回仅包含指定 servers 的子集 McpManager。
    /// - None = 全量（self.clone()）
    /// - Some([]) = 空集（无任何 MCP server）
    /// - Some(["k8s", "docs-rs"]) = 仅这两个
    pub fn filter(&self, server_names: Option<&[String]>) -> Self {
        match server_names {
            None => self.clone(),
            Some(names) => {
                let allowed: HashSet<&str> = names.iter().map(String::as_str).collect();
                let servers = self.servers.read().unwrap();
                let filtered: HashMap<String, ServerState> = servers
                    .iter()
                    .filter(|(name, _)| allowed.contains(name.as_str()))
                    .map(|(name, state)| (name.clone(), state.clone()))
                    .collect();
                Self {
                    servers: Arc::new(RwLock::new(filtered)),
                    max_retries: self.max_retries,
                    backoff_min: self.backoff_min,
                    backoff_max: self.backoff_max,
                }
            }
        }
    }

    /// 空 McpManager（无 servers）
    pub fn empty() -> Self { ... }
}
```

`ServerState` 需要实现 `Clone`。

### 9. Bug fix — McpTool 命名

**文件**: `crates/vol-llm-tool/src/mcp_tool.rs`

```rust
// Before: format!("mcp__{}_{}", sanitized_server, sanitized_tool)
// After:  format!("mcp__{}__{}", sanitized_server, sanitized_tool)
```

与 `filter_mcp_servers` 的解析逻辑（`rest.find("__")`）一致。

### 10. Handler — 单响应 + 结构化数据

**文件**: `crates/vol-agent-server/src/data_plane/handlers/agent.rs`

```rust
(AgentOperation::Submit, Payload::Agent(AgentPayload::Submit { input, target })) => {
    // ... resolve target_id, session, run_id ...

    let agent = match self.router.get_agent(&target_id).await {
        Some(a) => a,
        None => { /* fallback or error */ }
    };

    let tools = agent.tools();
    let skills = agent.skills();
    let mcps = agent.mcps();
    let llm = agent.llm();

    match self.router.send(&target_id, request).await {
        Ok(rx) => {
            tokio::spawn(async move { Self::process_run_result(rx, &run_id, &router).await });

            // 单响应
            Ok(vec![AgentServerMessage::new_result(
                message.message_id,
                Operation::Agent(AgentOperation::Submit),
                Payload::Agent(AgentPayload::SubmitResult {
                    run_id: run_id.clone(),
                    accepted: true,
                    provider: ProviderInfo {
                        name: llm.provider().to_string(),
                        model: llm.model().to_string(),
                    },
                    tools: tools.definitions().iter().map(|d| d.name.clone()).collect(),
                    mcps: mcps.server_status().keys().cloned().collect(),
                    skills: skills.skill_names().await,
                }),
            )])
        }
        Err(e) => { /* error response */ }
    }
}
```

### 11. core.rs — 简化

**文件**: `crates/vol-agent-server/src/data_plane/core.rs`

Agent 构建时不再预过滤 tool registry，传入全量 `base_tools`：

```rust
// Before: tool_registry.filter_mcp_servers(def.mcps).filter(def.tools, def.disallowed)
// After: 传全量，过滤交给 ReActAgent 的 resolve 方法

let config = AgentConfig::builder()
    .with_def(def.clone())
    .with_llm(llm)
    .with_tools(self.tool_registry.clone())  // 全量 → 存为 base_tools
    .with_session(session)
    .with_working_dir(agent_dir)
    .build()?;

config.mcp_manager = Some(self.mcp_manager.clone());
config.capability_overlays = Some(self.capability_overlays.clone());

let agent = ReActAgent::new(config, self.tool_registry.clone(), self.skill_loader.clone());
```

`ReActAgent::new` 签名更新为接收 `base_tools` + `skill_loader`。

---

## 受影响的上游调用方

| 文件 | 变更 |
|------|------|
| `vol-agent-server/src/control_plane/handlers/client.rs` | control-plane 的 `agent.submit` 也返回新 SubmitResult |
| `vol-llm-ui/src/connection/remote.rs` | 前端适配新响应结构 |
| `vol-llm-ui/src/web/client.rs` | 同上 |
| `vol-agent-server/tests/` | 更新 Ack/Result 相关断言 |
| `vol-llm-agent-protocol/tests/` | 同上 |
| `vol-llm-agent/tests/` | 更新 RunContext 相关测试 |

---

## 不影响

- `agent.event` 流式推送
- `RunResult` oneshot 机制
- `process_run_result` 日志
- `agent.get_capabilities` / `agent.update_capabilities`（overlay 更新路径不变）
- Plugin 系统
- Sandbox 系统
