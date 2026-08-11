# Tools & Sandbox 系统设计文档

## 概述

Agent 工具系统的三层架构：**Sandbox（执行基底）→ Tool（工具实现）→ ToolRegistry（注册分发）**。

所有工具的 I/O 必须通过 Sandbox trait 进行，工具永远不直接调用 OS API。

---

## 1. Sandbox 层

### 1.1 `Sandbox` trait 契约

定义在 `crates/vol-llm-sandbox/src/lib.rs`。

```
kind()         → "local" | "ssh" | "firecracker" | "wasm"
name()         → 注册名称（如 "local"、"devbox"）
start()        → 初始化（创建目录/建立连接），幂等
cleanup()      → 清理（删除临时目录/断开连接）
root_path()    → 沙箱文件系统的绝对根路径
resolve_path() → 验证相对路径，返回根路径内的绝对路径
execute()      → 在沙箱内执行命令
read_file()    → 读取文件（调用方必须先调用 resolve_path 验证）
write_file()   → 写入文件（自动创建父目录）
create_dir_all() → 递归创建目录
read_dir()     → 列出目录条目
metadata()     → 获取文件元数据（不跟随符号链接）
```

### 1.2 `resolve_path` 统一契约（所有实现必须遵循）

| 输入 | 行为 |
|-------|----------|
| `"foo/bar.txt"` | 接受 —— 与根路径拼接，规范化 |
| `"./foo"`、`"."` | 接受 —— `.` 组件被规范化 |
| `"/etc/passwd"` | **拒绝** —— 所有实现的 PathTraversal |
| `"~/foo"` | **拒绝** —— 所有实现的 PathTraversal |
| `"../outside"` | **拒绝** —— 转义根路径的 PathTraversal |
| `"foo/../../etc"` | **拒绝** —— 规范化后转义的 PathTraversal |

`ToolContext::resolve_path` 是一个适配层：如果工具接收到一个绝对路径，而该路径恰好在沙箱根目录内，它会在委托给沙箱之前去除根目录前缀。这样工具就可以透明地接收绝对路径（例如从 `tempfile` 传入），而所有沙箱实现始终看到一致的相对路径输入。

### 1.3 实现

#### LocalSandbox (`local.rs`)

- TempDir 支持的沙箱，`std::process::Command` 用于 execute，`std::fs` 用于文件操作
- 通过 `resolve_path` 进行包含检查，防止路径遍历
- `start()` 创建根目录，`cleanup()` 移除（如果是 temp 模式）
- `execute()`：在根目录中设置 cwd 的 `tokio::task::spawn_blocking`，超时时发送 SIGTERM/SIGKILL，支持 stdin
- 适合：开发、测试、单机部署

#### SSHSandbox (`ssh/mod.rs` + `session.rs`)

- 使用 `ssh2` crate 的远程执行，SFTP 用于文件 I/O
- 连接复用，空闲超时断开，自动重连
- 主机密钥验证：`host_key`（指纹）或 `known_hosts_file`
- `resolve_path` 拒绝绝对路径 —— 路径映射通过 `remote_path()`（私有）进行，用于 SFTP 操作
- `execute()`：通过 `channel_exec` 执行命令，参数使用 shell 转义
- `write_file()` 通过 SFTP `create()` 之前调用 `create_dir_all()` 自动创建父目录
- `read_dir()` / `metadata()` 通过 Unix 权限位检测符号链接（`S_IFLNK = 0o120000`）
- 适合：远程开发机、跳板机部署

#### WasmSandbox (`wasm.rs`)

- 使用 `wasmtime` + `wasmtime-wasi`（WASI preview1）的 WebAssembly 运行时
- 模块在构造时从 `.wasm` 文件预编译
- 每个 `execute()` 调用创建一个带有独立内存、stdin/stdout/stderr 管道的新 WASI 实例
- 工作目录被预打开并为 WASM 模块挂载为 `/`
- `read_file` 使用防御性切片（`.get()`）处理越界 offset
- `cleanup()` 删除工作目录
- `expose_as_tool` 配置标志：为 true 时，模块可以作为 Agent 工具注册
- 限制：仅 WASI 模块，不支持网络（preview1），内存限制 128MB（可配置）
- 适合：安全执行不受信任的代码（linter、格式化器、数据验证器），高并发低延迟的工具调用

#### FirecrackerSandbox (`firecracker.rs`)

- MicroVM 隔离（需要 Linux/KVM，`firecracker` 特性）
- 通过 SSH 连接到客户 VM，使用预配置的 rootfs 和内核
- 通过 `FirecrackerPool` 进行 VM 池化管理以快速获取
- 适合：完全隔离的不可信代码执行

### 1.4 `SandboxRegistry` (`registry.rs`)

从 `.agents/sandboxes/*.toml` 管理命名沙箱实例。

```toml
# 示例：.agents/sandboxes/devbox.toml
name = "devbox"
type = "ssh"
work_dir = "/home/agent/sandbox"

[ssh]
host = "dev.example.com"
user = "agent"
identity_file = "~/.ssh/id_ed25519"
known_hosts_file = "~/.ssh/known_hosts"
```

- 始终注册一个内置的 `"local"` LocalSandbox（名称受保护，不可覆盖）
- 容错加载：单个无效的 TOML 文件/沙箱会被跳过并记录警告
- `acquire(name)`：对于基于池的沙箱（Firecracker）创建新实例；对于单例（local、ssh、wasm）返回克隆的 Arc
- `default()` 返回 `"local"` 沙箱

### 1.5 每个工具的沙箱路由

工具可以通过 `ToolConfig` 被分配到特定的沙箱：

```toml
[tools.browser]
sandbox = "docker-sandbox"   # 将 browser 工具路由到特定沙箱
```

解析优先级：`ToolConfig.get_sandbox(tool_name)` > `AgentDef.sandbox` > `"local"`

---

## 2. 工具层

### 2.1 核心 traits（`vol-llm-tool/src/tool.rs`）

#### `ExecutableTool` —— 主要 trait

```rust
pub trait ExecutableTool: Send + Sync {
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn parameters(&self) -> serde_json::Value;        // JSON Schema
    fn sensitivity(&self, args: &Value) -> ToolSensitivity;  // Safe | RequiresApproval
    async fn execute(&self, args: &Value, context: &ToolContext) -> ToolResultType<ToolResult>;
}
```

#### `Tool` —— 遗留/通用 trait

通过 blanket impl 桥接：`impl<T: ExecutableTool> Tool for T`。将字符串参数解析为 JSON，委托给 `ExecutableTool::execute`。生产代码未使用——仅存在于测试中。

#### 关键类型

| 类型 | 用途 |
|------|---------|
| `ToolResult` | `call_id`、`success`、`content`、`error`、`data` |
| `ToolContext` | `messages`、`sandbox: SandboxRef`（始终设置）、`agent_def: Option<AgentDef>` |
| `ToolError` | `InvalidArguments`、`ExecutionFailed`、`NotFound` |
| `ToolSensitivity` | `Safe` \| `RequiresApproval { reason }` |
| `ToolConfig` | 动态的 `HashMap<String, toml::Value>` —— 工具读取 `get::<T>("tool_name")` |

#### `ToolContext::resolve_path` —— 适配层

将绝对路径（在根目录内）转换为相对路径后再委托给 sandbox：

```
输入："/tmp/sandbox_xxx/file.txt" (root = /tmp/sandbox_xxx/)
  → 去除根目录前缀 → "file.txt"
  → sandbox.resolve_path("file.txt") → /tmp/sandbox_xxx/file.txt ✅

输入："/etc/passwd" (root = /tmp/sandbox_xxx/)
  → 不以根目录开头 → 原样透传
  → sandbox.resolve_path("/etc/passwd") → PathTraversal ❌
```

### 2.2 内置工具

所有工具通过 `vol_llm_tools_builtin::register_all()` 注册。

| 工具 | 名称 | 沙箱使用 | 敏感度 | 描述 |
|------|------|-------------|------------|-------------|
| `BashTool` | `bash` | `sandbox.execute()` | `RequiresApproval` | 带安全检查的 Shell 命令执行（阻止 `rm -rf /`、fork 炸弹、反向 shell 等 9 种模式） |
| `ReadTool` | `read_file` | `resolve_path` → `sandbox.read_file()` | `Safe` | 读取带行号格式化输出的文件（支持 offset/limit） |
| `WriteTool` | `write_file` | `resolve_path` → `sandbox.write_file()` | `Safe` | 创建/覆盖文件，自动创建父目录 |
| `EditTool` | `edit_file` | `resolve_path` → `read_file` + `write_file` | `Safe` | 在文件中精确替换字符串；多个出现时必须设置 `replace_all` |
| `GlobTool` | `glob` | `resolve_path` → `read_dir` + `metadata` | `Safe` | 通过 glob 模式匹配文件；支持 `*`、`**`、`?`、`[abc]`、`{a,b}`；输出为 JSON |
| `GrepTool` | `grep` | `resolve_path` → `read_dir` + 内容搜索 | `Safe` | 搜索文件内容；多后端（优先 `rg` CLI，回退到 Rust 库）；支持 glob 过滤 |
| `WebSearchTool` | `web_search` | 无（网络调用） | `Safe` | 通过 Tavily API 进行网页搜索 |
| `WebFetchTool` | `web_fetch` | 无（网络调用） | `Safe` | 获取并提取网页内容 |

### 2.3 系统工具

| 工具 | 名称 | 描述 |
|------|------|------|
| `SkillTool` | `skill` | 按名称加载 skill 指令；列出可用 skills |
| `AgentTool` | `agent` | 分派带有深度限制的子 Agent；子 Agent 继承父 Agent 的工具 |
| Task CLI | `task` | 统一的任务管理 CLI（`create`、`list`、`get`、`update`、`claim`、`stop`、`output`） |
| CLI-as-Tool | 动态（来自 `.agents/cli-tools/*.toml`） | 从 TOML 配置加载的声明式 CLI 包装器 |

### 2.4 MCP 工具代理 (`McpTool`)

通过 `McpManager` 将外部 MCP 服务器工具桥接为 `ExecutableTool`。

- 命名：`mcp__{sanitized_server}__{sanitized_tool}`
- `sanitize_name()` 将特殊字符替换为 `_`（保留字母数字、`_`、`-`）
- `execute()` 委托给 `McpManager::call_tool(server, tool, args)`
- 敏感度始终为 `Safe`（MCP 服务器自行处理审批）
- `ToolRegistry::filter_mcp_servers()` 在保留非 MCP 工具的同时，按服务器名称过滤 MCP 工具

---

## 3. 注册与执行流程

### 3.1 工具注册（`AgentRuntimeBuilder::build()`）

```
1. vol_llm_tools_builtin::register_all()     → 6 个核心工具
2. vol_llm_task::tools::register_cli()       → task CLI 工具
3. cli_tool::register_all()                  → .agents/cli-tools/*.toml 动态 CLI
4. vol_llm_tools_builtin::register_web_all() → web_search, web_fetch
5. SkillTool::new(skill_loader)              → skill 工具
6. register_from_mcp(mcp_manager)            → 所有 MCP 工具
7. Wrapped in Arc<ToolRegistry>
```

### 3.2 每个 Agent 的工具过滤

`ReActAgent::resolve_tools()` 为每个 Agent 会话应用以下过滤器：

```
overlay.allowlist > AgentDef.tools > AgentDef.disallowed_tools
overlay.mcps     > AgentDef.mcps     → filter_mcp_servers()
```

### 3.3 工具执行流程

```
LLM 返回 ToolCall(id, name, arguments: JSON string)
  │
  ├─ ReActAgent::act()
  │   ├─ intercept(ToolCallBegin)         ← HITL 插件决策点
  │   ├─ 解析沙箱: ToolConfig > AgentDef > "local"
  │   ├─ SandboxRegistry::acquire(name)
  │   └─ 构建 ToolContext { sandbox, agent_def }
  │
  ├─ RunContext::execute_tool(call, ctx)
  │   └─ ToolRegistry::execute(call, ctx)
  │       ├─ 按名称查找工具
  │       ├─ 将 JSON 参数字符串解析为 serde_json::Value
  │       ├─ 工具.execute(&args, &context)
  │       │   ├─ 工具调用 context.resolve_path(rel)
  │       │   │   ├─ ToolContext 转换 absolute→relative（如果适用）
  │       │   │   └─ sandbox.resolve_path(relative) → 验证后的 PathBuf
  │       │   └─ 工具通过 context.sandbox 调用文件操作
  │       └─ 使用 call_id 增强 ToolResult
  │
  └─ 记录 ToolCallRecord → 发出 tool_call_complete
      → 将 Message::tool(content, call.id) 追加到会话
      → LLM 在下次推理中看到工具结果
```

---

## 4. 路径解析流程

```
工具接收来自 LLM 参数的 file_path/路径
  │
  ├─ 工具调用 context.resolve_path(raw_path)
  │   │
  │   ├─ ToolContext::resolve_path (适配层)
  │   │   ├─ 如果是相对路径 → 直接传递
  │   │   ├─ 如果是绝对路径 + 在根目录内 → 去除根目录前缀
  │   │   └─ 如果是绝对路径 + 在根目录外 → 传递（沙箱将拒绝）
  │   │
  │   └─ sandbox.resolve_path(relative_path)
  │       ├─ 检查是否为 '/' 或 '~' 前缀 → PathTraversal
  │       ├─ 与 root_path 拼接
  │       ├─ 规范化（解析 . 和 ..）
  │       ├─ 检查是否包含在根目录内 → PathTraversal
  │       └─ 返回绝对 PathBuf
  │
  └─ 工具使用解析后的路径调用 sandbox.read_file / write_file / ...
```

---

## 5. 设计决策

| 决策 | 理由 |
|----------|----------|
| 所有沙箱拒绝绝对路径 | 行为一致；防止沙箱逃逸；`ToolContext` 处理适配 |
| 工具不调用 OS API | 沙箱是可替换的；测试可以使用 LocalSandbox，生产环境可以使用 SSH/Firecracker |
| `ToolContext::for_test()` 根目录为 `/` | 接受来自 `tempfile` 的绝对路径以方便测试；在生产代码中，根目录始终受限 |
| `McpTool` 使用 `Box::leak` 作为 `&'static str` | 工具在启动时注册一次；泄漏可以接受 |
| `ToolSensitivity` 已定义但通过 config 控制 | HITL 审批目前使用工具名称配置；枚举保留用于未来编程式集成 |
| `BashTool` 不调用 `resolve_path` | Bash 命令通过 shell 重定向访问文件；`sandbox.execute()` 仅设置 cwd |
| `EditTool` 在精确字符串匹配失败时拒绝多个出现 | 防止意外替换；`replace_all: true` 启用批量替换 |
| `GlobTool` 结构化 JSON 输出 | 机器可读，以便 LLM 可靠地检查 `truncated`、`total_matched` |

---

## 6. 测试覆盖

| 层 | 测试数 | 关键场景 |
|-------|------|--------------|
| **LocalSandbox** | 37 | 路径解析（`./ . .. ~ 空格 unicode`）、读取/写入/目录/元数据 边缘情况、执行（env、stdin、stderr、非零退出、二进制未找到） |
| **SSHSandbox** | 14 | 路径解析（所有 Local 场景 + 绝对路径拒绝）、构造配置 |
| **WasmSandbox** | 17 | 完整 Sandbox trait 方法、执行（退出 0、stdout、参数/env）、文件 I/O |
| **沙箱集成** | 15 | 沙箱路由、normalize_path、SSH 集成（Docker 测试宿主机，默认忽略） |
| **ToolRegistry** | 12 | execute、dispatch、filter、definitions、contains、tool_sensitivity、错误传播 |
| **McpTool** | 12 | 命名规范、注册表集成、filter_mcp_servers、元数据、错误传播 |
| **内置工具** | ~80 | 每个工具的 execute 路径、错误路径、沙箱集成、工具链（write→read→edit、glob→grep→read 等） |
| **总计** | **~187** | |

### 覆盖率

| Crate | 区域覆盖率 | 行覆盖率 |
|-------|-------------|-----------|
| `vol-llm-tool` | 95.52% | 94.69% |
| `vol-llm-sandbox`（仅 local） | 97.90% | 96.93% |
