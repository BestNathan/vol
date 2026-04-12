# LLM Builtin Tools MVP Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 实现 6 个基础工具（Read, Write, Edit, Glob, Grep, Bash），每个工具作为独立子 crate，提供便捷注册函数。

**Architecture:** Workspace 结构，`vol-llm-tools-builtin` 作为 root crate，每个工具一个子 crate（read-tool, write-tool, edit-tool, glob-tool, grep-tool, bash-tool），root crate 负责 re-export 和统一注册。

**Tech Stack:** Rust 2021, tokio (async), glob crate, grep crate, thiserror, serde_json.

---

## File Structure

### Files to Create

```
crates/vol-llm-tools-builtin/
├── Cargo.toml                    # Workspace root
├── src/lib.rs                    # Re-exports + register_all()
├── read-tool/
│   ├── Cargo.toml
│   └── src/lib.rs                # ReadTool
├── write-tool/
│   ├── Cargo.toml
│   └── src/lib.rs                # WriteTool
├── edit-tool/
│   ├── Cargo.toml
│   └── src/lib.rs                # EditTool
├── glob-tool/
│   ├── Cargo.toml
│   └── src/lib.rs                # GlobTool
├── grep-tool/
│   ├── Cargo.toml
│   └── src/lib.rs                # GrepTool
└── bash-tool/
    ├── Cargo.toml
    └── src/lib.rs                # BashTool
```

### Files to Modify

- `Cargo.toml` (workspace root): 添加 `crates/vol-llm-tools-builtin` 到 members
- `crates/vol-llm-agent/Cargo.toml`: 添加 vol-llm-tools-builtin 依赖（可选）

---

## Phase 1: Project Setup & File Tools (Read, Write, Edit)

### Task 1: Create Workspace Structure

**Files:**
- Create: `crates/vol-llm-tools-builtin/Cargo.toml`
- Create: `crates/vol-llm-tools-builtin/src/lib.rs`

- [ ] **Step 1: Create workspace root Cargo.toml**

```toml
[workspace]
resolver = "2"
members = [
    "read-tool",
    "write-tool",
    "edit-tool",
    "glob-tool",
    "grep-tool",
    "bash-tool",
]

[workspace.package]
version = "0.1.0"
edition = "2021"

[workspace.dependencies]
vol-llm-tool = { path = "../../vol-llm-tool" }
vol-llm-core = { path = "../../vol-llm-core" }
tokio = { workspace = true }
async-trait = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }
tracing = { workspace = true }
glob = "0.3"
grep = "0.2"
walkdir = "2.4"
regex = "1.10"
```

- [ ] **Step 2: Create root lib.rs**

```rust
//! vol-llm-tools-builtin: Built-in tools for LLM Agent.
//!
//! Each tool is a separate sub-crate for optional dependencies.
//! Use `register_all()` to register all tools at once.

pub mod read_tool {
    pub use vol_llm_tools_builtin_read::*;
}

pub mod write_tool {
    pub use vol_llm_tools_builtin_write::*;
}

pub mod edit_tool {
    pub use vol_llm_tools_builtin_edit::*;
}

pub mod glob_tool {
    pub use vol_llm_tools_builtin_glob::*;
}

pub mod grep_tool {
    pub use vol_llm_tools_builtin_grep::*;
}

pub mod bash_tool {
    pub use vol_llm_tools_builtin_bash::*;
}

// Re-export all tools for convenience
pub use read_tool::ReadTool;
pub use write_tool::WriteTool;
pub use edit_tool::EditTool;
pub use glob_tool::GlobTool;
pub use grep_tool::GrepTool;
pub use bash_tool::BashTool;

// Re-export error type
pub use read_tool::BuiltinToolError;

/// Register all built-in tools to a ToolRegistry
pub fn register_all(registry: &mut vol_llm_tool::ToolRegistry) {
    registry.register(ReadTool::new());
    registry.register(WriteTool::new());
    registry.register(EditTool::new());
    registry.register(GlobTool::new());
    registry.register(GrepTool::new());
    registry.register(BashTool::new());
}
```

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-tools-builtin/
git commit -m "feat(tools-builtin): create workspace structure"
```

---

### Task 2: Create ReadTool

**Files:**
- Create: `crates/vol-llm-tools-builtin/read-tool/Cargo.toml`
- Create: `crates/vol-llm-tools-builtin/read-tool/src/lib.rs`
- Test: `crates/vol-llm-tools-builtin/read-tool/tests/read_tool_test.rs`

- [ ] **Step 1: Create read-tool Cargo.toml**

```toml
[package]
name = "vol-llm-tools-builtin-read"
version.workspace = true
edition.workspace = true

[dependencies]
vol-llm-tool = { workspace = true }
vol-llm-core = { workspace = true }
async-trait = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }
tokio = { workspace = true, features = ["fs"] }
```

- [ ] **Step 2: Create ReadTool implementation**

```rust
//! ReadTool - Read file content with line numbers.

use async_trait::async_trait;
use serde_json::{json, Value};
use std::error::Error;
use vol_llm_tool::{ExecutableTool, ToolContext, ToolResult};

pub use crate::error::BuiltinToolError;

/// ReadTool reads file content with optional offset/limit.
pub struct ReadTool;

impl ReadTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ReadTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ExecutableTool for ReadTool {
    fn name(&self) -> &'static str {
        "read_file"
    }

    fn description(&self) -> &'static str {
        "Read a file from the local filesystem. Returns content with line numbers (cat -n format)."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "Absolute path to the file"
                },
                "offset": {
                    "type": "integer",
                    "description": "Line offset to start reading (0-based), default 0"
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum lines to read, default 2000"
                }
            },
            "required": ["file_path"]
        })
    }

    async fn execute(
        &self,
        args: &Value,
        _context: &ToolContext,
    ) -> Result<ToolResult, BuiltinToolError> {
        let file_path = args["file_path"]
            .as_str()
            .ok_or_else(|| BuiltinToolError::InvalidArguments("Missing file_path".to_string()))?;

        let offset = args["offset"].as_u64().unwrap_or(0) as usize;
        let limit = args["limit"].as_u64().unwrap_or(2000) as usize;

        // Read file content
        let content = tokio::fs::read_to_string(file_path)
            .await
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    BuiltinToolError::NotFound(file_path.to_string())
                } else {
                    BuiltinToolError::Io(e)
                }
            })?;

        // Apply offset and limit
        let lines: Vec<&str> = content.lines().collect();
        let end = (offset + limit).min(lines.len());
        let selected_lines = if offset >= lines.len() {
            &lines[lines.len()..]
        } else {
            &lines[offset..end]
        };

        // Format with line numbers (1-based, cat -n style)
        let formatted = selected_lines
            .iter()
            .enumerate()
            .map(|(i, line)| format!("{:4}  | {}", offset + i + 1, line))
            .collect::<Vec<_>>()
            .join("\n");

        Ok(ToolResult::success(formatted))
    }
}

/// Error type for built-in tools
#[derive(Debug, thiserror::Error)]
pub enum BuiltinToolError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Invalid arguments: {0}")]
    InvalidArguments(String),

    #[error("File not found: {0}")]
    NotFound(String),
}
```

- [ ] **Step 3: Write test for ReadTool**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::NamedTempFile;
    use std::io::Write;

    #[tokio::test]
    async fn test_read_file_success() {
        let mut tmp = NamedTempFile::new().unwrap();
        writeln!(tmp, "line 1").unwrap();
        writeln!(tmp, "line 2").unwrap();
        writeln!(tmp, "line 3").unwrap();

        let tool = ReadTool::new();
        let args = json!({
            "file_path": tmp.path().to_str().unwrap()
        });

        let result = tool.execute(&args, &ToolContext::default()).await.unwrap();
        assert!(result.success);
        assert!(result.content.contains("1  | line 1"));
        assert!(result.content.contains("2  | line 2"));
    }

    #[tokio::test]
    async fn test_read_file_with_limit() {
        let mut tmp = NamedTempFile::new().unwrap();
        for i in 1..=10 {
            writeln!(tmp, "line {}", i).unwrap();
        }

        let tool = ReadTool::new();
        let args = json!({
            "file_path": tmp.path().to_str().unwrap(),
            "limit": 5
        });

        let result = tool.execute(&args, &ToolContext::default()).await.unwrap();
        assert!(result.success);
        assert!(result.content.contains("1  | line 1"));
        assert!(result.content.contains("5  | line 5"));
        assert!(!result.content.contains("6  | line 6"));
    }

    #[tokio::test]
    async fn test_read_file_not_found() {
        let tool = ReadTool::new();
        let args = json!({
            "file_path": "/nonexistent/file.txt"
        });

        let result = tool.execute(&args, &ToolContext::default()).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), BuiltinToolError::NotFound(_)));
    }
}
```

- [ ] **Step 4: Run tests**

```bash
cd crates/vol-llm-tools-builtin/read-tool
cargo test
```

Expected: All 3 tests pass.

- [ ] **Step 5: Commit**

```bash
cd /root/nq-deribit
git add crates/vol-llm-tools-builtin/read-tool/
git commit -m "feat(tools-builtin): implement ReadTool with offset/limit support"
```

---

### Task 3: Create WriteTool

**Files:**
- Create: `crates/vol-llm-tools-builtin/write-tool/Cargo.toml`
- Create: `crates/vol-llm-tools-builtin/write-tool/src/lib.rs`
- Test: `crates/vol-llm-tools-builtin/write-tool/tests/write_tool_test.rs`

- [ ] **Step 1: Create write-tool Cargo.toml**

```toml
[package]
name = "vol-llm-tools-builtin-write"
version.workspace = true
edition.workspace = true

[dependencies]
vol-llm-tool = { workspace = true }
vol-llm-core = { workspace = true }
async-trait = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }
tokio = { workspace = true, features = ["fs"] }
tracing = { workspace = true }
```

- [ ] **Step 2: Create WriteTool implementation**

```rust
//! WriteTool - Create or overwrite files.

use async_trait::async_trait;
use serde_json::{json, Value};
use std::error::Error;
use vol_llm_tool::{ExecutableTool, ToolContext, ToolResult};
use tracing::warn;

pub use crate::error::BuiltinToolError;

/// WriteTool creates new files or overwrites existing files.
pub struct WriteTool;

impl WriteTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for WriteTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ExecutableTool for WriteTool {
    fn name(&self) -> &'static str {
        "write_file"
    }

    fn description(&self) -> &'static str {
        "Write content to a file. Creates new file or overwrites existing file."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "Absolute path to the file"
                },
                "content": {
                    "type": "string",
                    "description": "Content to write"
                }
            },
            "required": ["file_path", "content"]
        })
    }

    async fn execute(
        &self,
        args: &Value,
        _context: &ToolContext,
    ) -> Result<ToolResult, BuiltinToolError> {
        let file_path = args["file_path"]
            .as_str()
            .ok_or_else(|| BuiltinToolError::InvalidArguments("Missing file_path".to_string()))?;

        let content = args["content"]
            .as_str()
            .ok_or_else(|| BuiltinToolError::InvalidArguments("Missing content".to_string()))?;

        let path = std::path::Path::new(file_path);

        // Check if parent directory exists
        if let Some(parent) = path.parent() {
            if !parent.exists() {
                return Err(BuiltinToolError::InvalidArguments(
                    format!("Parent directory does not exist: {}", parent.display()),
                ));
            }
        }

        // Check if file exists (for warning)
        if path.exists() {
            warn!("Overwriting existing file: {}", file_path);
        }

        // Write file
        tokio::fs::write(path, content)
            .await
            .map_err(BuiltinToolError::Io)?;

        Ok(ToolResult::success(format!(
            "Successfully wrote {} bytes to {}",
            content.len(),
            file_path
        )))
    }
}

/// Error type for built-in tools
#[derive(Debug, thiserror::Error)]
pub enum BuiltinToolError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Invalid arguments: {0}")]
    InvalidArguments(String),
}
```

- [ ] **Step 3: Write test for WriteTool**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::{NamedTempFile, tempdir};
    use std::io::Write;

    #[tokio::test]
    async fn test_write_new_file() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("new_file.txt");
        let content = "Hello, World!";

        let tool = WriteTool::new();
        let args = json!({
            "file_path": file_path.to_str().unwrap(),
            "content": content
        });

        let result = tool.execute(&args, &ToolContext::default()).await.unwrap();
        assert!(result.success);

        let written = fs::read_to_string(&file_path).unwrap();
        assert_eq!(written, content);
    }

    #[tokio::test]
    async fn test_write_overwrite_file() {
        let mut tmp = NamedTempFile::new().unwrap();
        writeln!(tmp, "old content").unwrap();

        let tool = WriteTool::new();
        let args = json!({
            "file_path": tmp.path().to_str().unwrap(),
            "content": "new content"
        });

        let result = tool.execute(&args, &ToolContext::default()).await.unwrap();
        assert!(result.success);

        let written = fs::read_to_string(tmp.path()).unwrap();
        assert_eq!(written, "new content");
    }

    #[tokio::test]
    async fn test_write_parent_not_exist() {
        let tool = WriteTool::new();
        let args = json!({
            "file_path": "/nonexistent_dir/file.txt",
            "content": "content"
        });

        let result = tool.execute(&args, &ToolContext::default()).await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            BuiltinToolError::InvalidArguments(_)
        ));
    }
}
```

- [ ] **Step 4: Run tests**

```bash
cd crates/vol-llm-tools-builtin/write-tool
cargo test
```

Expected: All 3 tests pass.

- [ ] **Step 5: Commit**

```bash
cd /root/nq-deribit
git add crates/vol-llm-tools-builtin/write-tool/
git commit -m "feat(tools-builtin): implement WriteTool with parent dir validation"
```

---

### Task 4: Create EditTool

**Files:**
- Create: `crates/vol-llm-tools-builtin/edit-tool/Cargo.toml`
- Create: `crates/vol-llm-tools-builtin/edit-tool/src/lib.rs`
- Test: `crates/vol-llm-tools-builtin/edit-tool/tests/edit_tool_test.rs`

- [ ] **Step 1: Create edit-tool Cargo.toml**

```toml
[package]
name = "vol-llm-tools-builtin-edit"
version.workspace = true
edition.workspace = true

[dependencies]
vol-llm-tool = { workspace = true }
vol-llm-core = { workspace = true }
async-trait = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }
tokio = { workspace = true, features = ["fs"] }
```

- [ ] **Step 2: Create EditTool implementation**

```rust
//! EditTool - Precise string replacement in files.

use async_trait::async_trait;
use serde_json::{json, Value};
use std::error::Error;
use vol_llm_tool::{ExecutableTool, ToolContext, ToolResult};

pub use crate::error::BuiltinToolError;

/// EditTool replaces exact strings in files.
pub struct EditTool;

impl EditTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for EditTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ExecutableTool for EditTool {
    fn name(&self) -> &'static str {
        "edit_file"
    }

    fn description(&self) -> &'static str {
        "Replace exact string in a file. Requires reading the file first."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "Absolute path to the file"
                },
                "old_string": {
                    "type": "string",
                    "description": "String to find and replace"
                },
                "new_string": {
                    "type": "string",
                    "description": "String to replace with"
                },
                "replace_all": {
                    "type": "boolean",
                    "description": "Replace all occurrences (default: false)"
                }
            },
            "required": ["file_path", "old_string", "new_string"]
        })
    }

    async fn execute(
        &self,
        args: &Value,
        _context: &ToolContext,
    ) -> Result<ToolResult, BuiltinToolError> {
        let file_path = args["file_path"]
            .as_str()
            .ok_or_else(|| BuiltinToolError::InvalidArguments("Missing file_path".to_string()))?;

        let old_string = args["old_string"]
            .as_str()
            .ok_or_else(|| BuiltinToolError::InvalidArguments("Missing old_string".to_string()))?;

        let new_string = args["new_string"]
            .as_str()
            .ok_or_else(|| BuiltinToolError::InvalidArguments("Missing new_string".to_string()))?;

        let replace_all = args["replace_all"].as_bool().unwrap_or(false);

        // Read file
        let content = tokio::fs::read_to_string(file_path)
            .await
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    BuiltinToolError::NotFound(file_path.to_string())
                } else {
                    BuiltinToolError::Io(e)
                }
            })?;

        // Count occurrences
        let count = content.matches(old_string).count();

        if count == 0 {
            return Err(BuiltinToolError::InvalidArguments(
                "old_string not found in file".to_string(),
            ));
        }

        if count > 1 && !replace_all {
            return Err(BuiltinToolError::NotUnique(format!(
                "old_string appears {} times. Use replace_all=true to replace all occurrences.",
                count
            )));
        }

        // Perform replacement
        let new_content = if replace_all {
            content.replace(old_string, new_string)
        } else {
            content.replacen(old_string, new_string, 1)
        };

        // Write file
        tokio::fs::write(file_path, &new_content)
            .await
            .map_err(BuiltinToolError::Io)?;

        Ok(ToolResult::success(format!(
            "Replaced {} occurrence(s) of old_string",
            count
        )))
    }
}

/// Error type for built-in tools
#[derive(Debug, thiserror::Error)]
pub enum BuiltinToolError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Invalid arguments: {0}")]
    InvalidArguments(String),

    #[error("File not found: {0}")]
    NotFound(String),

    #[error("String not unique: {0}")]
    NotUnique(String),
}
```

- [ ] **Step 3: Write test for EditTool**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::NamedTempFile;
    use std::io::Write;

    #[tokio::test]
    async fn test_edit_unique_string() {
        let mut tmp = NamedTempFile::new().unwrap();
        writeln!(tmp, "line 1").unwrap();
        writeln!(tmp, "old_value").unwrap();
        writeln!(tmp, "line 3").unwrap();

        let tool = EditTool::new();
        let args = json!({
            "file_path": tmp.path().to_str().unwrap(),
            "old_string": "old_value",
            "new_string": "new_value"
        });

        let result = tool.execute(&args, &ToolContext::default()).await.unwrap();
        assert!(result.success);

        let content = fs::read_to_string(tmp.path()).unwrap();
        assert!(content.contains("new_value"));
        assert!(!content.contains("old_value"));
    }

    #[tokio::test]
    async fn test_edit_multiple_replace_all() {
        let mut tmp = NamedTempFile::new().unwrap();
        writeln!(tmp, "old").unwrap();
        writeln!(tmp, "old").unwrap();

        let tool = EditTool::new();
        let args = json!({
            "file_path": tmp.path().to_str().unwrap(),
            "old_string": "old",
            "new_string": "new",
            "replace_all": true
        });

        let result = tool.execute(&args, &ToolContext::default()).await.unwrap();
        assert!(result.success);

        let content = fs::read_to_string(tmp.path()).unwrap();
        assert_eq!(content.matches("new").count(), 2);
    }

    #[tokio::test]
    async fn test_edit_not_unique_error() {
        let mut tmp = NamedTempFile::new().unwrap();
        writeln!(tmp, "old").unwrap();
        writeln!(tmp, "old").unwrap();

        let tool = EditTool::new();
        let args = json!({
            "file_path": tmp.path().to_str().unwrap(),
            "old_string": "old",
            "new_string": "new"
        });

        let result = tool.execute(&args, &ToolContext::default()).await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            BuiltinToolError::NotUnique(_)
        ));
    }

    #[tokio::test]
    async fn test_edit_not_found_error() {
        let mut tmp = NamedTempFile::new().unwrap();
        writeln!(tmp, "line 1").unwrap();

        let tool = EditTool::new();
        let args = json!({
            "file_path": tmp.path().to_str().unwrap(),
            "old_string": "nonexistent",
            "new_string": "new"
        });

        let result = tool.execute(&args, &ToolContext::default()).await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            BuiltinToolError::InvalidArguments(_)
        ));
    }
}
```

- [ ] **Step 4: Run tests**

```bash
cd crates/vol-llm-tools-builtin/edit-tool
cargo test
```

Expected: All 4 tests pass.

- [ ] **Step 5: Commit**

```bash
cd /root/nq-deribit
git add crates/vol-llm-tools-builtin/edit-tool/
git commit -m "feat(tools-builtin): implement EditTool with uniqueness validation"
```

---

## Phase 2: Search Tools (Glob, Grep)

### Task 5: Create GlobTool

**Files:**
- Create: `crates/vol-llm-tools-builtin/glob-tool/Cargo.toml`
- Create: `crates/vol-llm-tools-builtin/glob-tool/src/lib.rs`
- Test: `crates/vol-llm-tools-builtin/glob-tool/tests/glob_tool_test.rs`

- [ ] **Step 1: Create glob-tool Cargo.toml**

```toml
[package]
name = "vol-llm-tools-builtin-glob"
version.workspace = true
edition.workspace = true

[dependencies]
vol-llm-tool = { workspace = true }
vol-llm-core = { workspace = true }
async-trait = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }
glob = "0.3"
```

- [ ] **Step 2: Create GlobTool implementation**

```rust
//! GlobTool - File path pattern matching.

use async_trait::async_trait;
use glob::glob;
use serde_json::{json, Value};
use std::error::Error;
use std::path::{Path, PathBuf};
use vol_llm_tool::{ExecutableTool, ToolContext, ToolResult};

pub use crate::error::BuiltinToolError;

/// GlobTool matches file paths using glob patterns.
pub struct GlobTool;

impl GlobTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for GlobTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ExecutableTool for GlobTool {
    fn name(&self) -> &'static str {
        "glob"
    }

    fn description(&self) -> &'static str {
        "Match file paths using glob patterns (e.g., **/*.rs, src/**/*.ts)."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "Glob pattern (e.g., **/*.rs)"
                },
                "path": {
                    "type": "string",
                    "description": "Root directory to search (default: current directory)"
                }
            },
            "required": ["pattern"]
        })
    }

    async fn execute(
        &self,
        args: &Value,
        _context: &ToolContext,
    ) -> Result<ToolResult, BuiltinToolError> {
        let pattern = args["pattern"]
            .as_str()
            .ok_or_else(|| BuiltinToolError::InvalidArguments("Missing pattern".to_string()))?;

        let search_path = args["path"]
            .as_str()
            .unwrap_or(".");

        // Build full pattern
        let full_pattern = if Path::new(pattern).is_absolute() {
            pattern.to_string()
        } else {
            format!("{}/{}", search_path.trim_end_matches('/'), pattern)
        };

        // Execute glob
        let entries = glob(&full_pattern)
            .map_err(|e| BuiltinToolError::InvalidArguments(format!("Invalid pattern: {}", e)))?;

        // Collect and sort by modification time (newest first)
        let mut files: Vec<(PathBuf, std::time::SystemTime)> = Vec::new();
        for entry in entries {
            if let Ok(path) = entry {
                if let Ok(metadata) = std::fs::metadata(&path) {
                    if let Ok(mtime) = metadata.modified() {
                        files.push((path, mtime));
                    }
                }
            }
        }

        // Sort by modification time (newest first)
        files.sort_by(|a, b| b.1.cmp(&a.1));

        // Format output
        let paths: Vec<String> = files
            .into_iter()
            .map(|(path, _)| path.to_string_lossy().to_string())
            .collect();

        let content = if paths.is_empty() {
            "No files matched the pattern.".to_string()
        } else {
            paths.join("\n")
        };

        Ok(ToolResult::success(content))
    }
}

/// Error type for built-in tools
#[derive(Debug, thiserror::Error)]
pub enum BuiltinToolError {
    #[error("Invalid arguments: {0}")]
    InvalidArguments(String),
}
```

- [ ] **Step 3: Write test for GlobTool**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;
    use std::io::Write;

    #[tokio::test]
    async fn test_glob_basic() {
        let dir = tempdir().unwrap();
        fs::create_dir(dir.path().join("src")).unwrap();

        let mut f1 = fs::File::create(dir.path().join("src/main.rs")).unwrap();
        writeln!(f1, "fn main() {{}}").unwrap();

        let mut f2 = fs::File::create(dir.path().join("src/lib.rs")).unwrap();
        writeln!(f2, "pub fn lib() {{}}").unwrap();

        let tool = GlobTool::new();
        let args = json!({
            "pattern": "**/*.rs",
            "path": dir.path().to_str().unwrap()
        });

        let result = tool.execute(&args, &ToolContext::default()).await.unwrap();
        assert!(result.success);
        assert!(result.content.contains("main.rs"));
        assert!(result.content.contains("lib.rs"));
    }

    #[tokio::test]
    async fn test_glob_no_matches() {
        let dir = tempdir().unwrap();

        let tool = GlobTool::new();
        let args = json!({
            "pattern": "*.nonexistent",
            "path": dir.path().to_str().unwrap()
        });

        let result = tool.execute(&args, &ToolContext::default()).await.unwrap();
        assert!(result.success);
        assert!(result.content.contains("No files matched"));
    }
}
```

- [ ] **Step 4: Run tests**

```bash
cd crates/vol-llm-tools-builtin/glob-tool
cargo test
```

Expected: Tests pass.

- [ ] **Step 5: Commit**

```bash
cd /root/nq-deribit
git add crates/vol-llm-tools-builtin/glob-tool/
git commit -m "feat(tools-builtin): implement GlobTool with mtime sorting"
```

---

### Task 6: Create GrepTool

**Files:**
- Create: `crates/vol-llm-tools-builtin/grep-tool/Cargo.toml`
- Create: `crates/vol-llm-tools-builtin/grep-tool/src/lib.rs`
- Test: `crates/vol-llm-tools-builtin/grep-tool/tests/grep_tool_test.rs`

- [ ] **Step 1: Create grep-tool Cargo.toml**

```toml
[package]
name = "vol-llm-tools-builtin-grep"
version.workspace = true
edition.workspace = true

[dependencies]
vol-llm-tool = { workspace = true }
vol-llm-core = { workspace = true }
async-trait = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }
grep = "0.2"
walkdir = "2.4"
```

- [ ] **Step 2: Create GrepTool implementation**

```rust
//! GrepTool - File content search with regex.

use async_trait::async_trait;
use grep::{regex::RegexMatcher, Searcher};
use grep_searcher::LineStep;
use serde_json::{json, Value};
use std::error::Error;
use std::path::Path;
use walkdir::WalkDir;
use vol_llm_tool::{ExecutableTool, ToolContext, ToolResult};

pub use crate::error::BuiltinToolError;

/// GrepTool searches file content using regex patterns.
pub struct GrepTool;

impl GrepTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for GrepTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ExecutableTool for GrepTool {
    fn name(&self) -> &'static str {
        "grep"
    }

    fn description(&self) -> &'static str {
        "Search file content using regex patterns. Returns matches with optional context."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "Regex pattern to search"
                },
                "path": {
                    "type": "string",
                    "description": "Root directory to search (default: current directory)"
                },
                "glob": {
                    "type": "string",
                    "description": "File pattern filter (e.g., *.rs)"
                },
                "output_mode": {
                    "type": "string",
                    "enum": ["content", "files_with_matches", "count"],
                    "description": "Output format (default: files_with_matches)"
                },
                "context_lines": {
                    "type": "integer",
                    "description": "Lines of context around matches (-C)"
                },
                "case_sensitive": {
                    "type": "boolean",
                    "description": "Case-sensitive search (default: false)"
                }
            },
            "required": ["pattern"]
        })
    }

    async fn execute(
        &self,
        args: &Value,
        _context: &ToolContext,
    ) -> Result<ToolResult, BuiltinToolError> {
        let pattern = args["pattern"]
            .as_str()
            .ok_or_else(|| BuiltinToolError::InvalidArguments("Missing pattern".to_string()))?;

        let search_path = args["path"].as_str().unwrap_or(".");
        let glob_filter = args["glob"].as_str();
        let output_mode = args["output_mode"].as_str().unwrap_or("files_with_matches");
        let case_sensitive = args["case_sensitive"].as_bool().unwrap_or(false);

        // Build regex matcher
        let matcher = if case_sensitive {
            RegexMatcher::new(pattern)
        } else {
            RegexMatcher::new_case_insensitive(pattern)
        }
        .map_err(|e| BuiltinToolError::InvalidArguments(format!("Invalid regex: {}", e)))?;

        // Collect files
        let mut files: Vec<std::path::PathBuf> = Vec::new();
        for entry in WalkDir::new(search_path).into_iter().filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.is_file() {
                if let Some(glob) = glob_filter {
                    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                        if glob_match(glob, name) {
                            files.push(path.to_path_buf());
                        }
                    }
                } else {
                    files.push(path.to_path_buf());
                }
            }
        }

        // Search files
        let mut results: Vec<SearchResult> = Vec::new();
        for file_path in &files {
            let mut searcher = Searcher::new();
            let mut matches = Vec::new();

            searcher.search_path(
                &matcher,
                file_path,
                grep_searcher::Sink::with_step(|step| {
                    matches.push((
                        step.absolute_byte_offset(),
                        step.line_number().unwrap_or(0),
                    ));
                    Ok(true)
                }),
            );

            if !matches.is_empty() {
                results.push(SearchResult {
                    path: file_path.clone(),
                    match_count: matches.len(),
                    line_numbers: matches.into_iter().map(|(_, ln)| ln).collect(),
                });
            }
        }

        // Format output based on mode
        let content = match output_mode {
            "files_with_matches" => {
                let paths: Vec<String> = results
                    .iter()
                    .map(|r| r.path.to_string_lossy().to_string())
                    .collect();
                paths.join("\n")
            }
            "count" => {
                let counts: Vec<String> = results
                    .iter()
                    .map(|r| format!("{}: {}", r.path.display(), r.match_count))
                    .collect();
                counts.join("\n")
            }
            "content" => {
                // For content mode, we'd need to read actual lines
                // MVP: return file list with line numbers
                let lines: Vec<String> = results
                    .iter()
                    .flat_map(|r| {
                        r.line_numbers
                            .iter()
                            .map(|ln| format!("{}:{}", r.path.display(), ln))
                    })
                    .collect();
                lines.join("\n")
            }
            _ => Err(BuiltinToolError::InvalidArguments(format!(
                "Invalid output_mode: {}",
                output_mode
            )))?,
        };

        Ok(ToolResult::success(if content.is_empty() {
            "No matches found.".to_string()
        } else {
            content
        }))
    }
}

/// Simple glob match helper
fn glob_match(pattern: &str, name: &str) -> bool {
    // Simplified: only support * wildcard
    if pattern == "*" {
        return true;
    }
    if pattern.starts_with("*.") {
        let ext = &pattern[1..];
        return name.ends_with(ext);
    }
    pattern == name
}

#[derive(Debug)]
struct SearchResult {
    path: std::path::PathBuf,
    match_count: usize,
    line_numbers: Vec<u64>,
}

/// Error type for built-in tools
#[derive(Debug, thiserror::Error)]
pub enum BuiltinToolError {
    #[error("Invalid arguments: {0}")]
    InvalidArguments(String),
}
```

- [ ] **Step 3: Write test for GrepTool**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;
    use std::io::Write;

    #[tokio::test]
    async fn test_grep_basic() {
        let dir = tempdir().unwrap();
        let mut f1 = fs::File::create(dir.path().join("test.txt")).unwrap();
        writeln!(f1, "hello world").unwrap();
        writeln!(f1, "foo bar").unwrap();
        writeln!(f1, "hello again").unwrap();

        let tool = GrepTool::new();
        let args = json!({
            "pattern": "hello",
            "path": dir.path().to_str().unwrap(),
            "output_mode": "files_with_matches"
        });

        let result = tool.execute(&args, &ToolContext::default()).await.unwrap();
        assert!(result.success);
        assert!(result.content.contains("test.txt"));
    }

    #[tokio::test]
    async fn test_grep_no_matches() {
        let dir = tempdir().unwrap();
        let mut f1 = fs::File::create(dir.path().join("test.txt")).unwrap();
        writeln!(f1, "hello world").unwrap();

        let tool = GrepTool::new();
        let args = json!({
            "pattern": "nonexistent",
            "path": dir.path().to_str().unwrap()
        });

        let result = tool.execute(&args, &ToolContext::default()).await.unwrap();
        assert!(result.success);
        assert!(result.content.contains("No matches"));
    }

    #[tokio::test]
    async fn test_grep_with_glob() {
        let dir = tempdir().unwrap();
        let mut f1 = fs::File::create(dir.path().join("test.rs")).unwrap();
        writeln!(f1, "fn main() {{}}").unwrap();

        let mut f2 = fs::File::create(dir.path().join("test.txt")).unwrap();
        writeln!(f2, "hello").unwrap();

        let tool = GrepTool::new();
        let args = json!({
            "pattern": "fn",
            "path": dir.path().to_str().unwrap(),
            "glob": "*.rs"
        });

        let result = tool.execute(&args, &ToolContext::default()).await.unwrap();
        assert!(result.success);
        assert!(result.content.contains("test.rs"));
        assert!(!result.content.contains("test.txt"));
    }
}
```

- [ ] **Step 4: Run tests**

```bash
cd crates/vol-llm-tools-builtin/grep-tool
cargo test
```

Expected: Tests pass.

- [ ] **Step 5: Commit**

```bash
cd /root/nq-deribit
git add crates/vol-llm-tools-builtin/grep-tool/
git commit -m "feat(tools-builtin): implement GrepTool with regex search"
```

---

## Phase 3: BashTool (with Security)

### Task 7: Create BashTool

**Files:**
- Create: `crates/vol-llm-tools-builtin/bash-tool/Cargo.toml`
- Create: `crates/vol-llm-tools-builtin/bash-tool/src/lib.rs`
- Test: `crates/vol-llm-tools-builtin/bash-tool/tests/bash_tool_test.rs`

- [ ] **Step 1: Create bash-tool Cargo.toml**

```toml
[package]
name = "vol-llm-tools-builtin-bash"
version.workspace = true
edition.workspace = true

[dependencies]
vol-llm-tool = { workspace = true }
vol-llm-core = { workspace = true }
async-trait = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }
tokio = { workspace = true, features = ["process", "time"] }
tracing = { workspace = true }
regex = "1.10"
```

- [ ] **Step 2: Create BashTool implementation**

```rust
//! BashTool - Execute shell commands with security checks.

use async_trait::async_trait;
use regex::Regex;
use serde_json::{json, Value};
use std::error::Error;
use std::time::Duration;
use tokio::process::Command;
use tokio::time::timeout;
use vol_llm_tool::{ExecutableTool, ToolContext, ToolResult};
use tracing::{info, warn};

pub use crate::error::BuiltinToolError;

/// Dangerous command patterns (blocking dangerous combinations)
const DANGEROUS_PATTERNS: &[&str] = &[
    r"rm\s+(-[a-zA-Z]*r[a-zA-Z]*f|[a-zA-Z]*f[a-zA-Z]*r).*\s+/",  // rm -rf /
    r":\(\)\{\s*:\|:&\s*\};",                                     // Fork bomb
    r"mkfs",                                                      // Format disk
    r"dd\s+of=/dev/(zero|sda|nvme)",                              // Write to device
    r">\s*/dev/sd[a-z]",                                          // Redirect to device
    r"curl\s+[^|]*\|\s*(?:bash|sh)",                              // Curl pipe bash
    r"wget\s+[^|]*-O[^|]*\|\s*(?:bash|sh)",                       // Wget pipe bash
    r"nc\s+-e\s+",                                                // Netcat reverse shell
    r"bash\s+-i\s+>&\s+/dev/tcp",                                 // Bash reverse shell
];

/// BashTool executes shell commands with security and timeout controls.
pub struct BashTool {
    dangerous_patterns: Vec<Regex>,
    default_timeout: Duration,
    max_output_size: usize,
}

impl BashTool {
    pub fn new() -> Self {
        let patterns = DANGEROUS_PATTERNS
            .iter()
            .filter_map(|p| Regex::new(p).ok())
            .collect();

        Self {
            dangerous_patterns: patterns,
            default_timeout: Duration::from_secs(120),
            max_output_size: 1024 * 1024, // 1MB
        }
    }

    /// Check if command matches any dangerous pattern
    fn is_dangerous(&self, command: &str) -> bool {
        self.dangerous_patterns
            .iter()
            .any(|pattern| pattern.is_match(command))
    }
}

impl Default for BashTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ExecutableTool for BashTool {
    fn name(&self) -> &'static str {
        "bash"
    }

    fn description(&self) -> &'static str {
        "Execute a shell command with security checks and timeout control."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "Shell command to execute"
                },
                "timeout": {
                    "type": "integer",
                    "description": "Timeout in milliseconds (default: 120000)"
                },
                "working_dir": {
                    "type": "string",
                    "description": "Working directory for command execution"
                },
                "run_in_background": {
                    "type": "boolean",
                    "description": "Run in background (not implemented in MVP)"
                }
            },
            "required": ["command"]
        })
    }

    async fn execute(
        &self,
        args: &Value,
        _context: &ToolContext,
    ) -> Result<ToolResult, BuiltinToolError> {
        let command = args["command"]
            .as_str()
            .ok_or_else(|| BuiltinToolError::InvalidArguments("Missing command".to_string()))?;

        let timeout_ms = args["timeout"].as_u64().unwrap_or(120000);
        let working_dir = args["working_dir"].as_str();

        // Security check
        if self.is_dangerous(command) {
            warn!("Blocked dangerous command: {}", command);
            return Err(BuiltinToolError::SecurityViolation(format!(
                "Command blocked by security filter: {}",
                command
            )));
        }

        info!("Executing command: {}", command);

        // Build command
        let mut cmd = Command::new("sh");
        cmd.arg("-c").arg(command);

        if let Some(dir) = working_dir {
            cmd.current_dir(dir);
        }

        // Execute with timeout
        let output = timeout(Duration::from_millis(timeout_ms), async {
            cmd.output().await
        })
        .await
        .map_err(|_| BuiltinToolError::Timeout(format!("Command timed out after {}ms", timeout_ms)))?
        .map_err(BuiltinToolError::Io)?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        // Truncate output if too large
        let mut result = if stdout.is_empty() {
            stderr.to_string()
        } else if stderr.is_empty() {
            stdout.to_string()
        } else {
            format!("stdout:\n{}\n\nstderr:\n{}", stdout, stderr)
        };

        if result.len() > self.max_output_size {
            result.truncate(self.max_output_size);
            result.push_str("\n\n[Output truncated - exceeded 1MB limit]");
        }

        if output.status.success() {
            Ok(ToolResult::success(result))
        } else {
            Ok(ToolResult::success(format!(
                "Command exited with code {:?}\n{}",
                output.status.code(),
                result
            )))
        }
    }
}

/// Error type for built-in tools
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
}
```

- [ ] **Step 3: Write test for BashTool**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_bash_simple_command() {
        let tool = BashTool::new();
        let args = json!({
            "command": "echo hello"
        });

        let result = tool.execute(&args, &ToolContext::default()).await.unwrap();
        assert!(result.success);
        assert!(result.content.contains("hello"));
    }

    #[tokio::test]
    async fn test_bash_rm_rf_blocked() {
        let tool = BashTool::new();
        let args = json!({
            "command": "rm -rf /"
        });

        let result = tool.execute(&args, &ToolContext::default()).await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            BuiltinToolError::SecurityViolation(_)
        ));
    }

    #[tokio::test]
    async fn test_bash_fork_bomb_blocked() {
        let tool = BashTool::new();
        let args = json!({
            "command": ":(){:|:&}:;"
        });

        let result = tool.execute(&args, &ToolContext::default()).await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            BuiltinToolError::SecurityViolation(_)
        ));
    }

    #[tokio::test]
    async fn test_bash_rm_file_allowed() {
        let tool = BashTool::new();
        let args = json!({
            "command": "rm /tmp/nonexistent_file_test_12345"
        });

        // This should NOT be blocked (rm with specific file, not -rf /)
        let result = tool.execute(&args, &ToolContext::default()).await;
        assert!(!matches!(
            result.unwrap_err(),
            BuiltinToolError::SecurityViolation(_)
        ));
        // Will fail with "No such file" which is expected
    }

    #[tokio::test]
    async fn test_bash_timeout() {
        let tool = BashTool::new();
        let args = json!({
            "command": "sleep 5",
            "timeout": 100  // 100ms
        });

        let result = tool.execute(&args, &ToolContext::default()).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), BuiltinToolError::Timeout(_)));
    }
}
```

- [ ] **Step 4: Run tests**

```bash
cd crates/vol-llm-tools-builtin/bash-tool
cargo test
```

Expected: All tests pass (security blocks work, timeout works).

- [ ] **Step 5: Commit**

```bash
cd /root/nq-deribit
git add crates/vol-llm-tools-builtin/bash-tool/
git commit -m "feat(tools-builtin): implement BashTool with security blacklist"
```

---

## Phase 4: Integration & Documentation

### Task 8: Update Workspace and Add Integration

**Files:**
- Modify: `Cargo.toml` (workspace root)
- Modify: `crates/vol-llm-tools-builtin/src/lib.rs` (if needed)

- [ ] **Step 1: Add vol-llm-tools-builtin to workspace**

Modify root `Cargo.toml`:

```toml
[workspace]
members = [
    # ... existing members ...
    "crates/vol-llm-tools-builtin",
]
```

- [ ] **Step 2: Build workspace**

```bash
cargo build -p vol-llm-tools-builtin-read
cargo build -p vol-llm-tools-builtin-write
cargo build -p vol-llm-tools-builtin-edit
cargo build -p vol-llm-tools-builtin-glob
cargo build -p vol-llm-tools-builtin-grep
cargo build -p vol-llm-tools-builtin-bash
```

Expected: All crates build successfully.

- [ ] **Step 3: Commit**

```bash
git add Cargo.toml Cargo.lock
git commit -m "feat(tools-builtin): add to workspace"
```

---

### Task 9: Write README and Documentation

**Files:**
- Create: `crates/vol-llm-tools-builtin/README.md`

- [ ] **Step 1: Create README**

```markdown
# vol-llm-tools-builtin

Built-in tools for LLM Agent, providing file operations, search, and shell execution capabilities.

## Tools

| Tool | Description |
|------|-------------|
| `read_file` | Read file content with line numbers |
| `write_file` | Create or overwrite files |
| `edit_file` | Precise string replacement |
| `glob` | File path pattern matching |
| `grep` | File content regex search |
| `bash` | Shell command execution |

## Usage

### Register all tools

```rust
use vol_llm_tools_builtin::register_all;

let mut registry = ToolRegistry::new();
register_all(&mut registry);
```

### Register individual tools

```rust
use vol_llm_tools_builtin::read_tool::ReadTool;

let mut registry = ToolRegistry::new();
registry.register(ReadTool::new());
```

## Security

The `bash` tool includes a security blacklist for dangerous commands:
- `rm -rf /` and similar destructive deletes
- Fork bombs
- Disk formatting commands
- Reverse shells

**Note**: The blacklist is not exhaustive. For production use, consider additional safeguards like HITL approval.

## License

MIT
```

- [ ] **Step 2: Commit**

```bash
git add crates/vol-llm-tools-builtin/README.md
git commit -m "docs(tools-builtin): add README"
```

---

## Self-Review Checklist

Before finalizing, verify:

1. **Spec coverage**: Each spec requirement has a corresponding task
2. **No placeholders**: No TBD, TODO, or "add tests" without code
3. **Type consistency**: All tools use consistent error type naming
4. **Test coverage**: Each tool has tests for success, error, and edge cases

---

## Execution Options

Plan complete and saved to `docs/superpowers/plans/llm-builtin-tools-mvp.md`. Two execution options:

**1. Subagent-Driven (recommended)** - I dispatch a fresh subagent per task, review between tasks, fast iteration

**2. Inline Execution** - Execute tasks in this session using executing-plans, batch execution with checkpoints

**Which approach?**
