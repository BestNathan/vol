# Coding Agent Sandbox Integration Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add Sandbox abstraction so CodingAgent runs tools in an isolated workspace, preventing pollution of the development directory.

**Architecture:** Three-layer design — `Sandbox` trait in vol-llm-core, `LocalSandbox` implementation in vol-llm-agents, `ToolContext` extension in vol-llm-tool with sandbox reference and `resolve_path()`. ReActAgent holds optional sandbox, injects it into ToolContext at tool execution time. CodingAgent exposes `with_sandbox()` builder method.

**Tech Stack:** Rust, async trait, `Arc<dyn Sandbox>`, existing `ToolContext`/`ExecutableTool` patterns.

---

### Task 1: Create Sandbox trait in vol-llm-core

**Files:**
- Create: `crates/vol-llm-core/src/sandbox.rs`
- Modify: `crates/vol-llm-core/src/lib.rs`

- [ ] **Step 1: Create sandbox module**

Create `crates/vol-llm-core/src/sandbox.rs`:

```rust
//! Sandbox abstraction for isolated code execution.

use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Sandbox error type
#[derive(Debug, thiserror::Error)]
pub enum SandboxError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Path traversal detected: {0}")]
    PathTraversal(String),
    #[error("Sandbox not started")]
    NotStarted,
    #[error("Sandbox already started")]
    AlreadyStarted,
}

/// Result type for sandbox operations
pub type SandboxResult<T> = Result<T, SandboxError>;

/// Sandbox trait — abstract interface for isolated execution environments.
///
/// Implementations: LocalSandbox (local directory), DockerSandbox (container),
/// SSHSandbox (remote host), etc.
pub trait Sandbox: Send + Sync {
    /// Sandbox type identifier (for logging/debugging)
    fn kind(&self) -> &str;

    /// Start the sandbox (create directory, establish connection, etc.)
    fn start(&self) -> SandboxResult<()>;

    /// Clean up the sandbox (delete temp directory, disconnect, etc.)
    fn cleanup(&self) -> SandboxResult<()>;

    /// Root path of the sandbox
    fn root_path(&self) -> &Path;

    /// Resolve a relative path to an absolute path within the sandbox.
    /// Returns an error if the resolved path escapes the sandbox root.
    fn resolve_path(&self, rel: &str) -> SandboxResult<PathBuf>;
}

/// Type alias for convenience
pub type SandboxRef = Arc<dyn Sandbox>;
```

- [ ] **Step 2: Register module in lib.rs**

In `crates/vol-llm-core/src/lib.rs`, add after `pub mod stream;`:

```rust
pub mod sandbox;
```

And add to the `pub use` block:

```rust
pub use sandbox::*;
```

- [ ] **Step 3: Verify compilation**

Run: `cargo check -p vol-llm-core`
Expected: no errors

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-core/src/sandbox.rs crates/vol-llm-core/src/lib.rs
git commit -m "feat(llm-core): add Sandbox trait for isolated execution environments"
```

---

### Task 2: Create LocalSandbox implementation

**Files:**
- Create: `crates/vol-llm-agents/src/coding/sandbox/mod.rs`
- Create: `crates/vol-llm-agents/src/coding/sandbox/local.rs`
- Modify: `crates/vol-llm-agents/src/coding/mod.rs`
- Test: `crates/vol-llm-agents/tests/sandbox_local_test.rs`

- [ ] **Step 1: Write tests**

Create `crates/vol-llm-agents/tests/sandbox_local_test.rs`:

```rust
use vol_llm_agents::coding::LocalSandbox;
use vol_llm_core::Sandbox;
use std::path::PathBuf;
use tempfile::tempdir;

#[test]
fn test_local_sandbox_new_with_path() {
    let dir = tempdir().unwrap();
    let sandbox = LocalSandbox::new(Some(dir.path().to_path_buf()));
    assert_eq!(sandbox.kind(), "local");
    assert_eq!(sandbox.root_path(), dir.path());
}

#[test]
fn test_local_sandbox_new_temp() {
    let sandbox = LocalSandbox::new(None);
    assert_eq!(sandbox.kind(), "local");
    assert!(sandbox.root_path().to_string_lossy().contains("sandbox"));
}

#[test]
fn test_local_sandbox_start_creates_dir() {
    let dir = tempdir().unwrap();
    let new_path = dir.path().join("new-sandbox");
    let sandbox = LocalSandbox::new(Some(new_path.clone()));
    assert!(!new_path.exists());

    sandbox.start().unwrap();
    assert!(new_path.exists());

    sandbox.cleanup().unwrap();
    assert!(!new_path.exists());
}

#[test]
fn test_local_sandbox_start_existing_dir() {
    let dir = tempdir().unwrap();
    let sandbox = LocalSandbox::new(Some(dir.path().to_path_buf()));

    sandbox.start().unwrap();
    assert!(dir.path().exists());

    sandbox.cleanup().unwrap();
    assert!(dir.path().exists()); // caller-owned dirs NOT deleted
}

#[test]
fn test_local_sandbox_resolve_path() {
    let dir = tempdir().unwrap();
    let sandbox = LocalSandbox::new(Some(dir.path().to_path_buf()));

    let resolved = sandbox.resolve_path("Cargo.toml").unwrap();
    assert_eq!(resolved, dir.path().join("Cargo.toml"));

    let resolved = sandbox.resolve_path("src/main.rs").unwrap();
    assert_eq!(resolved, dir.path().join("src/main.rs"));

    // Absolute paths returned as-is
    let resolved = sandbox.resolve_path("/etc/passwd").unwrap();
    assert_eq!(resolved, PathBuf::from("/etc/passwd"));
}

#[test]
fn test_local_sandbox_resolve_path_traversal_blocked() {
    let dir = tempdir().unwrap();
    let sandbox = LocalSandbox::new(Some(dir.path().to_path_buf()));

    assert!(sandbox.resolve_path("../escape.txt").is_err());
    assert!(sandbox.resolve_path("../../etc/passwd").is_err());
    assert!(sandbox.resolve_path("foo/../../escape.txt").is_err());
}

#[test]
fn test_local_sandbox_temp_cleanup() {
    let sandbox = LocalSandbox::new(None);
    sandbox.start().unwrap();
    let path = sandbox.root_path().to_path_buf();
    assert!(path.exists());

    sandbox.cleanup().unwrap();
    assert!(!path.exists());
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p vol-llm-agents --test sandbox_local_test 2>&1 | tail -5`
Expected: compilation errors — `LocalSandbox` not yet defined

- [ ] **Step 3: Create module files**

Create `crates/vol-llm-agents/src/coding/sandbox/mod.rs`:

```rust
mod local;

pub use local::LocalSandbox;
```

- [ ] **Step 4: Implement LocalSandbox**

Create `crates/vol-llm-agents/src/coding/sandbox/local.rs`:

```rust
use std::path::{Path, PathBuf};
use vol_llm_core::{Sandbox, SandboxResult};

/// A sandbox using a local directory as its root.
///
/// If created with `Some(path)`, the directory is caller-owned and NOT deleted on cleanup.
/// If created with `None`, a temp directory is created and IS deleted on cleanup.
pub struct LocalSandbox {
    root_path: PathBuf,
    is_temp: bool,
}

impl LocalSandbox {
    pub fn new(path: Option<PathBuf>) -> Self {
        let (root_path, is_temp) = match path {
            Some(p) => (p, false),
            None => {
                let timestamp = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis();
                let temp = std::env::temp_dir().join(format!("sandbox_{:x}", timestamp % 0xFFFFFF));
                (temp, true)
            }
        };
        Self { root_path, is_temp }
    }
}

impl Sandbox for LocalSandbox {
    fn kind(&self) -> &str {
        "local"
    }

    fn start(&self) -> SandboxResult<()> {
        std::fs::create_dir_all(&self.root_path).map_err(SandboxError::Io)
    }

    fn cleanup(&self) -> SandboxResult<()> {
        if self.is_temp {
            std::fs::remove_dir_all(&self.root_path).map_err(SandboxError::Io)?;
        }
        Ok(())
    }

    fn root_path(&self) -> &Path {
        &self.root_path
    }

    fn resolve_path(&self, rel: &str) -> SandboxResult<PathBuf> {
        if rel.starts_with('/') {
            return Ok(PathBuf::from(rel));
        }

        let resolved = self.root_path.join(rel);
        let canonical_root = self.root_path.canonicalize().unwrap_or_else(|_| self.root_path.clone());
        let canonical_resolved = resolved.canonicalize().unwrap_or_else(|_| resolved.clone());

        if !canonical_resolved.starts_with(&canonical_root) {
            return Err(SandboxError::PathTraversal(rel.to_string()));
        }

        Ok(resolved)
    }
}
```

- [ ] **Step 5: Export from coding module**

In `crates/vol-llm-agents/src/coding/mod.rs`, add:

```rust
mod sandbox;
```

And in the `pub use` block add:

```rust
pub use sandbox::LocalSandbox;
```

- [ ] **Step 6: Run tests**

Run: `cargo test -p vol-llm-agents --test sandbox_local_test -- --nocapture`
Expected: all 7 tests pass

- [ ] **Step 7: Commit**

```bash
git add crates/vol-llm-agents/src/coding/sandbox/ crates/vol-llm-agents/src/coding/mod.rs crates/vol-llm-agents/tests/sandbox_local_test.rs
git commit -m "feat(llm-agents): add LocalSandbox with path traversal protection"
```

---

### Task 3: Extend ToolContext with sandbox support

**Files:**
- Modify: `crates/vol-llm-tool/src/tool.rs`
- Modify: `crates/vol-llm-tool/Cargo.toml`
- Test: `crates/vol-llm-agents/tests/tool_context_sandbox_test.rs`

- [ ] **Step 1: Write tests**

Create `crates/vol-llm-agents/tests/tool_context_sandbox_test.rs`:

```rust
use vol_llm_tool::ToolContext;
use vol_llm_core::SandboxRef;
use vol_llm_agents::coding::LocalSandbox;
use std::sync::Arc;
use tempfile::tempdir;

#[test]
fn test_resolve_path_without_sandbox() {
    let ctx = ToolContext::default();
    let path = ctx.resolve_path("Cargo.toml").unwrap();
    assert_eq!(path, std::path::PathBuf::from("Cargo.toml"));
}

#[test]
fn test_resolve_path_with_sandbox() {
    let dir = tempdir().unwrap();
    let sandbox: SandboxRef = Arc::new(LocalSandbox::new(Some(dir.path().to_path_buf())));
    let ctx = ToolContext::default().with_sandbox(sandbox);

    let path = ctx.resolve_path("src/main.rs").unwrap();
    assert_eq!(path, dir.path().join("src/main.rs"));
}

#[test]
fn test_resolve_path_traversal_blocked() {
    let dir = tempdir().unwrap();
    let sandbox: SandboxRef = Arc::new(LocalSandbox::new(Some(dir.path().to_path_buf())));
    let ctx = ToolContext::default().with_sandbox(sandbox);

    let result = ctx.resolve_path("../escape.txt");
    assert!(result.is_err());
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p vol-llm-agents --test tool_context_sandbox_test 2>&1 | tail -5`
Expected: compilation errors — `resolve_path` and `with_sandbox` don't exist

- [ ] **Step 3: Extend ToolContext**

Modify `crates/vol-llm-tool/src/tool.rs`.

Add import:

```rust
use vol_llm_core::{Message, SandboxRef};
```

Replace the `ToolContext` struct and add impl block:

```rust
/// Tool execution context
#[derive(Debug, Clone, Default)]
pub struct ToolContext {
    pub messages: Vec<Message>,
    pub sandbox: Option<SandboxRef>,
}

impl ToolContext {
    /// Set the sandbox for this tool context
    pub fn with_sandbox(mut self, sandbox: SandboxRef) -> Self {
        self.sandbox = Some(sandbox);
        self
    }

    /// Resolve a path through the sandbox, or return unchanged if no sandbox.
    pub fn resolve_path(&self, rel: &str) -> Result<std::path::PathBuf, Box<dyn std::error::Error + Send + Sync>> {
        match &self.sandbox {
            Some(sandbox) => Ok(sandbox.resolve_path(rel)?),
            None => Ok(std::path::PathBuf::from(rel)),
        }
    }
}
```

- [ ] **Step 4: Add vol-llm-core dependency**

Modify `crates/vol-llm-tool/Cargo.toml`, add:

```toml
vol-llm-core = { path = "../vol-llm-core" }
```

- [ ] **Step 5: Re-export SandboxRef from vol-llm-tool**

In `crates/vol-llm-tool/src/lib.rs`, add:

```rust
pub use vol_llm_core::SandboxRef;
```

- [ ] **Step 6: Run tests**

Run: `cargo test -p vol-llm-agents --test tool_context_sandbox_test -- --nocapture`
Expected: all 3 tests pass

Also verify: `cargo check -p vol-llm-tool`

- [ ] **Step 7: Commit**

```bash
git add crates/vol-llm-tool/src/tool.rs crates/vol-llm-tool/Cargo.toml crates/vol-llm-tool/src/lib.rs crates/vol-llm-agents/tests/tool_context_sandbox_test.rs
git commit -m "feat(llm-tool): add sandbox support to ToolContext with resolve_path()"
```

---

### Task 4: Add sandbox to ReActAgent

**Files:**
- Modify: `crates/vol-llm-agent/src/react/agent.rs`

- [ ] **Step 1: Add sandbox field to ReActAgent**

Modify `crates/vol-llm-agent/src/react/agent.rs`.

Add import:

```rust
use vol_llm_core::SandboxRef;
```

Add `sandbox` field to struct:

```rust
pub struct ReActAgent {
    llm: Arc<dyn LLMClient>,
    tools: Arc<vol_llm_tool::ToolRegistry>,
    config: AgentConfig,
    session: Arc<Session>,
    sandbox: Option<SandboxRef>,
}
```

Update `new()` to accept sandbox:

```rust
pub fn new(
    llm: Arc<dyn LLMClient>,
    tools: Arc<vol_llm_tool::ToolRegistry>,
    config: AgentConfig,
    session: Arc<Session>,
) -> Self {
    Self {
        llm,
        tools,
        config,
        session,
        sandbox: None,
    }
}
```

Add builder method:

```rust
/// Set the sandbox for tool execution
pub fn with_sandbox(mut self, sandbox: SandboxRef) -> Self {
    self.sandbox = Some(sandbox);
    self
}
```

- [ ] **Step 2: Inject sandbox into ToolContext at tool execution**

In the `run()` method, line ~382, change:

```rust
// Before:
let result = match tools.execute(call, &ToolContext::default()).await {

// After:
let tool_ctx = match &self.sandbox {
    Some(sandbox) => ToolContext::default().with_sandbox(sandbox.clone()),
    None => ToolContext::default(),
};
let result = match tools.execute(call, &tool_ctx).await {
```

- [ ] **Step 3: Verify compilation**

Run: `cargo check -p vol-llm-agent`

Also verify the agent dependency chain: `cargo check -p vol-llm-agents`
(vol-llm-agent needs vol-llm-core for SandboxRef — add it to Cargo.toml if missing)

Check `crates/vol-llm-agent/Cargo.toml` — if `vol-llm-core` is not listed, add:

```toml
vol-llm-core = { path = "../vol-llm-core" }
```

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-agent/src/react/agent.rs crates/vol-llm-agent/Cargo.toml
git commit -m "feat(llm-agent): add sandbox support to ReActAgent"
```

---

### Task 5: Integrate Sandbox into CodingAgent

**Files:**
- Modify: `crates/vol-llm-agents/src/coding/agent.rs`
- Modify: `crates/vol-llm-agents/examples/coding_agent_wordcount.rs`
- Test: Run the wordcount example end-to-end

- [ ] **Step 1: Add sandbox field and builder method**

Modify `crates/vol-llm-agents/src/coding/agent.rs`.

Add `sandbox` field to struct:

```rust
pub struct CodingAgent {
    config: CodingAgentConfig,
    state: Option<CodingAgentState>,
    observer: Option<Arc<dyn EventObserver>>,
    observer_plugin: Option<Arc<ObserverPlugin>>,
    sandbox: Option<vol_llm_core::SandboxRef>,
}
```

Update `new()` to include `sandbox: None`:

```rust
Ok(Self {
    config,
    state: Some(CodingAgentState {
        llm,
        tool_registry: Arc::new(tool_registry),
        agent_config,
    }),
    observer: None,
    observer_plugin: None,
    sandbox: None,
})
```

Add `with_sandbox` method after `with_observer`:

```rust
/// Set the sandbox for tool execution
pub fn with_sandbox(mut self, sandbox: vol_llm_core::SandboxRef) -> Self {
    self.sandbox = Some(sandbox);
    self
}
```

- [ ] **Step 2: Pass sandbox to ReActAgent in run()**

In the `run()` method, after creating the `react_agent`, chain `.with_sandbox()`:

Find the ReActAgent creation block (~line 137-142) and change:

```rust
// Before:
let react_agent = ReActAgent::new(
    state.llm.clone(),
    state.tool_registry.clone(),
    agent_config,
    session,
);

// After:
let mut react_agent = ReActAgent::new(
    state.llm.clone(),
    state.tool_registry.clone(),
    agent_config,
    session,
);

if let Some(ref sandbox) = self.sandbox {
    react_agent = react_agent.with_sandbox(sandbox.clone());
}
```

- [ ] **Step 3: Update example to use sandbox**

Modify `crates/vol-llm-agents/examples/coding_agent_wordcount.rs`.

Replace the agent creation block with sandbox usage:

```rust
use vol_llm_agents::coding::{CodingAgent, CodingAgentConfig, HTMLReporter, LocalSandbox};
use vol_llm_core::Sandbox;
use std::sync::Arc;
```

Before creating the agent, create and start the sandbox:

```rust
let sandbox = Arc::new(LocalSandbox::new(Some(work_dir.clone())));
sandbox.start()?;
```

Chain `with_sandbox` on the agent:

```rust
let agent = agent.with_observer(observer).with_sandbox(sandbox.clone());
```

After `agent.run()` completes, cleanup:

```rust
sandbox.cleanup()?;
```

- [ ] **Step 4: Clean up any leftover files from previous runs**

```bash
rm -rf /tmp/wordcount-work
rm -f crates/vol-llm-agents/coding-report.html
```

- [ ] **Step 5: Run the wordcount example**

```bash
source /root/nq-deribit/.env && cargo run --release -p vol-llm-agents --example coding_agent_wordcount 2>&1
```

Expected: Agent completes successfully, builds the wordcount tool in `/tmp/wordcount-work/`, runs it, and generates HTML report. The worktree root directory should NOT be polluted with Cargo.toml, src/, or target/.

- [ ] **Step 6: Verify no pollution**

```bash
git status --short
```

Expected: No new untracked `src/`, `Cargo.toml`, or `target/` in the worktree root.

- [ ] **Step 7: Commit**

```bash
git add crates/vol-llm-agents/src/coding/agent.rs crates/vol-llm-agents/examples/coding_agent_wordcount.rs
git commit -m "feat(llm-agents): integrate sandbox into CodingAgent with with_sandbox() builder"
```

---

### Task 6: Update built-in tools to use ctx.resolve_path()

**Files:**
- Modify: `crates/vol-llm-tools-builtin/read-tool/src/lib.rs`
- Modify: `crates/vol-llm-tools-builtin/write-tool/src/lib.rs`
- Modify: `crates/vol-llm-tools-builtin/edit-tool/src/lib.rs`

Note: BashTool and GlobTool/GrepTool are NOT modified in this task. BashTool uses `working_dir` param, GlobTool/GrepTool use `path` param — they can be updated later. Read/Write/Edit are the most commonly used file tools and benefit most from sandbox resolution.

- [ ] **Step 1: Update ReadTool to use ctx.resolve_path()**

In `crates/vol-llm-tools-builtin/read-tool/src/lib.rs`, modify the `execute` method:

Change the file reading section from:

```rust
let content = match tokio::fs::read_to_string(&params.file_path).await {
```

To:

```rust
let file_path = match context.sandbox {
    Some(ref sandbox) => sandbox.resolve_path(&params.file_path).map_err(|e| {
        ToolError::ExecutionFailed(format!("Sandbox path resolution failed: {}", e))
    })?,
    None => std::path::PathBuf::from(&params.file_path),
};

let content = match tokio::fs::read_to_string(&file_path).await {
```

- [ ] **Step 2: Update WriteTool to use ctx.resolve_path()**

In `crates/vol-llm-tools-builtin/write-tool/src/lib.rs`, modify the `execute` method:

Change the parent directory check and file write from using `params.file_path` to resolved path:

```rust
let file_path = match context.sandbox {
    Some(ref sandbox) => sandbox.resolve_path(&params.file_path).map_err(|e| {
        ToolError::ExecutionFailed(format!("Sandbox path resolution failed: {}", e))
    })?,
    None => std::path::PathBuf::from(&params.file_path),
};

let parent = file_path.parent()
    .ok_or_else(|| ToolError::ExecutionFailed("Invalid file path".into()))?;

if !parent.as_os_str().is_empty() && !tokio::fs::try_exists(parent).await.unwrap_or(false) {
    return Err(ToolError::ExecutionFailed(format!(
        "Parent directory does not exist: {}",
        parent.display()
    )));
}

tokio::fs::write(&file_path, &params.content)
    .await
    .map_err(|e| ToolError::ExecutionFailed(format!("Failed to write file: {}", e)))?;

let output = format!("Successfully wrote {} bytes to {}", params.content.len(), params.file_path);
Ok(ToolResult::success(output))
```

- [ ] **Step 3: Update EditTool to use ctx.resolve_path()**

In `crates/vol-llm-tools-builtin/edit-tool/src/lib.rs`, modify the `execute` method:

Add path resolution before reading the file:

```rust
let file_path = match context.sandbox {
    Some(ref sandbox) => sandbox.resolve_path(&params.file_path).map_err(|e| {
        ToolError::ExecutionFailed(format!("Sandbox path resolution failed: {}", e))
    })?,
    None => std::path::PathBuf::from(&params.file_path),
};

let content = tokio::fs::read_to_string(&file_path)
    .await
    .map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            ToolError::NotFound(params.file_path.clone())
        } else {
            ToolError::ExecutionFailed(format!("Failed to read file: {}", e))
        }
    })?;
```

And update the write call:

```rust
tokio::fs::write(&file_path, &new_content)
    .await
    .map_err(|e| ToolError::ExecutionFailed(format!("Failed to write file: {}", e)))?;
```

- [ ] **Step 4: Run all tool tests**

```bash
cargo test -p vol-llm-tools-builtin-read 2>&1 | tail -5
cargo test -p vol-llm-tools-builtin-write 2>&1 | tail -5
cargo test -p vol-llm-tools-builtin-edit 2>&1 | tail -5
```

Expected: all tests pass (existing tests use absolute temp paths, so sandbox=None path is taken — backward compatible)

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-tools-builtin/read-tool/src/lib.rs crates/vol-llm-tools-builtin/write-tool/src/lib.rs crates/vol-llm-tools-builtin/edit-tool/src/lib.rs
git commit -m "feat(tools-builtin): use ctx.resolve_path() for sandbox-aware file operations"
```
