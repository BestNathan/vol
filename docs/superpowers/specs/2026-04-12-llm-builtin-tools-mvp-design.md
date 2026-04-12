# LLM Builtin Tools MVP Design

**Date**: 2026-04-12
**Author**: Claude Code
**Status**: Draft

---

## Context

当前 vol-llm-agent 已具备工具注册和执行基础设施（ToolRegistry, Tool trait），但缺少通用的文件操作和 shell 执行工具。现有工具均为业务特定工具（PPT 生成、数据分析），无法支持代码理解、文件修改等通用场景。

**约束条件**:
- Rust 技术栈，异步优先（tokio）
- 复用现有 `vol-llm-tool` 框架（Tool trait, ToolRegistry）
- 与 ReActAgent 无缝集成
- MVP 快速验证，后续可扩展

---

## Goals / Non-Goals

**Goals:**
- 实现 6 个核心工具：Read, Write, Edit, Glob, Grep, Bash
- 独立 crate `vol-llm-tools-builtin`，不影响现有代码
- 支持异步执行（async/await）
- 符合 Claude Code CLI 工具行为模式（基于 tool-prompts-complete.md）
- 提供便捷注册函数 `register_builtin_tools()`
- 包含基础安全限制（Bash 命令黑名单）
- 每个工具独立子 crate，便于按需引用和独立演进

**Non-Goals:**
- 复杂权限系统（文件访问控制）
- 高级 Bash 功能（管道、重定向、环境变量管理）
- 增量编辑（diff/patch）
- 符号链接处理
- Windows 跨平台支持（MVP 仅 Linux）

---

## Architecture

### Crate 结构

```
crates/vol-llm-tools-builtin/
├── Cargo.toml (workspace root)
├── src/
│   └── lib.rs (re-export all tools + register_all())
├── read-tool/
│   ├── Cargo.toml
│   └── src/lib.rs
├── write-tool/
│   ├── Cargo.toml
│   └── src/lib.rs
├── edit-tool/
│   ├── Cargo.toml
│   └── src/lib.rs
├── glob-tool/
│   ├── Cargo.toml
│   └── src/lib.rs
├── grep-tool/
│   ├── Cargo.toml
│   └── src/lib.rs
└── bash-tool/
    ├── Cargo.toml
    └── src/lib.rs
```

### 依赖关系

```
┌─────────────────────────────────────────────────────────────┐
│                  vol-llm-tools-builtin                       │
│  (workspace root - re-exports)                               │
└─────────────────────────────────────────────────────────────┘
         │         │         │         │         │
         ▼         ▼         ▼         ▼         ▼
    ┌─────────┬───────┬─────────┬─────────┬─────────┐
    │         │       │         │         │         │
┌───▼───┐ ┌──▼────┐ ┌▼─────┐ ┌▼──────┐ ┌▼──────┐ ┌▼──────┐
│ read  │ │write  │ │ edit │ │ glob  │ │ grep  │ │ bash  │
│ tool  │ │ tool  │ │ tool │ │ tool  │ │ tool  │ │ tool  │
└───────┘ └───────┘ └──────┘ └───────┘ └───────┘ └───────┘
    │         │         │         │         │         │
    └─────────┴─────────┴─────────┴─────────┴─────────┘
                              │
                              ▼
                    ┌─────────────────┐
                    │   vol-llm-tool  │
                    │  (Tool trait)   │
                    └─────────────────┘
```

### 工具参数设计

遵循 Claude Code CLI 模式：

| 工具 | 参数 |
|------|------|
| **ReadTool** | `file_path` (required), `offset?`, `limit?` |
| **WriteTool** | `file_path` (required), `content` (required) |
| **EditTool** | `file_path` (required), `old_string` (required), `new_string` (required), `replace_all?` |
| **GlobTool** | `pattern` (required), `path?` |
| **GrepTool** | `pattern` (required), `path?`, `glob?`, `output_mode?` |
| **BashTool** | `command` (required), `timeout?`, `working_dir?`, `run_in_background?` |

### 输出模式默认值

| 工具 | 默认行为 |
|------|----------|
| **ReadTool** | 读取前 2000 行（`limit=2000`） |
| **GrepTool** | `output_mode=files_with_matches` |
| **BashTool** | `timeout=120000ms` (2 分钟) |

---

## Component Design

### 1. ReadTool

**职责**: 读取文件内容，返回带行号的格式

```rust
pub struct ReadTool;

impl ExecutableTool for ReadTool {
    fn name(&self) -> &'static str { "read_file" }
    
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "file_path": {"type": "string", "description": "Absolute path"},
                "offset": {"type": "integer", "description": "Line offset (0-based)"},
                "limit": {"type": "integer", "description": "Max lines to read"}
            },
            "required": ["file_path"]
        })
    }
}
```

**关键逻辑**:
- 默认 `limit=2000`
- 返回格式：`1  | content line 1\n2  | content line 2...`
- 文件不存在时返回错误，不抛异常

---

### 2. WriteTool

**职责**: 创建或覆盖文件

```rust
pub struct WriteTool;

impl ExecutableTool for WriteTool {
    fn name(&self) -> &'static str { "write_file" }
    
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "file_path": {"type": "string", "description": "Absolute path"},
                "content": {"type": "string", "description": "File content"}
            },
            "required": ["file_path", "content"]
        })
    }
}
```

**关键逻辑**:
- 父目录不存在时返回错误
- 覆盖现有文件时不备份

---

### 3. EditTool

**职责**: 精确字符串替换

```rust
pub struct EditTool;

impl ExecutableTool for EditTool {
    fn name(&self) -> &'static str { "edit_file" }
    
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "file_path": {"type": "string"},
                "old_string": {"type": "string"},
                "new_string": {"type": "string"},
                "replace_all": {"type": "boolean"}
            },
            "required": ["file_path", "old_string", "new_string"]
        })
    }
}
```

**关键逻辑**:
- 验证 `old_string` 唯一性（`replace_all=false` 时）
- 未找到匹配时返回错误
- 保持原有缩进

---

### 4. GlobTool

**职责**: 文件路径模式匹配

```rust
pub struct GlobTool;

impl ExecutableTool for GlobTool {
    fn name(&self) -> &'static str { "glob" }
    
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "pattern": {"type": "string", "description": "Glob pattern (e.g., **/*.rs)"},
                "path": {"type": "string", "description": "Search root directory"}
            },
            "required": ["pattern"]
        })
    }
}
```

**关键逻辑**:
- 使用 `glob` crate
- 结果按修改时间降序排序
- 支持 `**`, `*`, `?`, `[...]` 模式

---

### 5. GrepTool

**职责**: 文件内容搜索

```rust
pub struct GrepTool {
    // 内部使用 grep crate 或 regex + walkdir
}

impl ExecutableTool for GrepTool {
    fn name(&self) -> &'static str { "grep" }
    
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "pattern": {"type": "string", "description": "Regex pattern"},
                "path": {"type": "string"},
                "glob": {"type": "string", "description": "File pattern filter"},
                "output_mode": {"type": "string", "enum": ["content", "files_with_matches", "count"]},
                "context_lines": {"type": "integer", "description": "Lines of context (-C)"},
                "case_sensitive": {"type": "boolean"}
            },
            "required": ["pattern"]
        })
    }
}
```

**关键逻辑**:
- 默认 `output_mode=files_with_matches`
- 支持 `-A`, `-B`, `-C` 上下文
- 支持 `multiline` 跨行匹配

---

### 6. BashTool

**职责**: 安全执行 shell 命令

```rust
pub struct BashTool {
    blacklist: Vec<Regex>,
    default_timeout: Duration,
}

impl ExecutableTool for BashTool {
    fn name(&self) -> &'static str { "bash" }
    
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "command": {"type": "string"},
                "timeout": {"type": "integer", "description": "Timeout in ms"},
                "working_dir": {"type": "string"},
                "run_in_background": {"type": "boolean"}
            },
            "required": ["command"]
        })
    }
}
```

**关键逻辑**:

#### 命令黑名单（正则匹配）
```rust
const DANGEROUS_PATTERNS: &[&str] = &[
    r"rm\s+(-[a-zA-Z]*r[a-zA-Z]*f|[a-zA-Z]*f[a-zA-Z]*r).*\s+/",  // rm -rf /
    r":\(\)\{\s*:\|:&\s*\};",                                     // Fork bomb
    r"mkfs",                                                      // Format disk
    r"dd\s+of=/dev/(zero|sda|nvme)",                              // Write to device
    r">\s*/dev/sd[a-z]",                                          // Redirect to device
    r"curl.*\|\s*(?:bash|sh)",                                    // Curl pipe bash
    r"wget.*-O-.*\|\s*(?:bash|sh)",                               // Wget pipe bash
];
```

#### 超时控制
- 默认 120000ms (2 分钟)
- 超时后强制终止进程

#### 输出截断
- 最大 1MB，超出部分提示已截断

---

## Error Handling

### 统一错误类型

```rust
#[derive(Debug, thiserror::Error)]
pub enum BuiltinToolError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Invalid arguments: {0}")]
    InvalidArguments(String),
    
    #[error("Security violation: {0}")]
    SecurityViolation(String),
    
    #[error("Timeout: {0}")]
    Timeout(String),
    
    #[error("File not found: {0}")]
    NotFound(String),
    
    #[error("String not unique: {0}")]
    NotUnique(String),
    
    #[error("Grep error: {0}")]
    Grep(#[from] grep::Error),
    
    #[error("Glob error: {0}")]
    Glob(#[from] glob::PatternError),
}

pub type Result<T> = std::result::Result<T, BuiltinToolError>;
```

---

## Integration with Agent

### 便捷注册函数

```rust
// vol-llm-tools-builtin/src/lib.rs

pub fn register_all(registry: &mut ToolRegistry) {
    registry.register(read_tool::ReadTool);
    registry.register(write_tool::WriteTool);
    registry.register(edit_tool::EditTool);
    registry.register(glob_tool::GlobTool);
    registry.register(grep_tool::GrepTool);
    registry.register(bash_tool::BashTool);
}

// Optional: AgentBuilder extension
pub trait AgentBuilderExt {
    fn with_builtin_tools(self) -> Self;
}

impl AgentBuilderExt for AgentBuilder {
    fn with_builtin_tools(self) -> Self {
        let mut tools = vec![];
        vol_llm_tools_builtin::register_all(&mut tools);
        self.with_tools(tools)
    }
}
```

---

## Testing Strategy

### 单元测试

每个工具独立测试：
- 参数验证
- 正常路径
- 错误路径

### 安全测试 (BashTool)

```rust
#[test]
fn test_rm_rf_slash_blocked() {
    assert!(BashTool::is_dangerous("rm -rf /"));
    assert!(BashTool::is_dangerous("rm -rf /*"));
    assert!(!BashTool::is_dangerous("rm file.txt"));
}
```

### 集成测试

与 ReActAgent 集成：
- 完整工具调用循环
- 插件拦截验证

---

## Risks / Trade-offs

| Risk | Impact | Mitigation |
|------|--------|------------|
| Bash 黑名单不完整 | 高 | 持续更新，集成 HITL 审批 |
| Grep 性能不足 | 中 | v1.1 升级 ripgrep |
| 文件并发修改冲突 | 低 | 文档说明"先读后改"约束 |
| 大文件内存溢出 | 低 | 强制 offset/limit 分页 |
| Bash 输出过大 | 中 | 输出截断（最大 1MB） |

---

## Implementation Phases

| Phase | Tools | Duration |
|-------|-------|----------|
| **1** | Read, Write, Edit | 1-2 days |
| **2** | Glob, Grep | 1-2 days |
| **3** | Bash (with security) | 2-3 days |
| **4** | Integration, docs | 1 day |

---

## Open Questions (Resolved)

| Question | Decision |
|----------|----------|
| Bash 黑名单粒度 | 仅禁止危险组合 |
| Grep 默认输出模式 | `files_with_matches` |
| ReadTool 默认分页 | 2000 行 |
