# Coding Agent Sandbox Integration

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a Sandbox abstraction so CodingAgent runs tools in an isolated workspace, preventing pollution of the development directory.

**Architecture:** Three-layer design — `Sandbox` trait (core abstraction in vol-llm-core), `LocalSandbox` (concrete implementation in vol-llm-agents), and `CodingAgent` integration (inject sandbox into ToolContext). Tools resolve paths through `ToolContext.resolve_path()`, transparently mapping to sandbox root.

**Tech Stack:** Rust, async trait, Arc<dyn Sandbox>, existing ToolContext/ExecutableTool patterns.

---

## Architecture

```
┌─────────────────────────────────────────────────────┐
│  Caller (example / user code)                        │
│                                                      │
│  1. sandbox = LocalSandbox::new("/tmp/sandbox-xxx")  │
│  2. sandbox.start()                                  │
│  3. agent.with_sandbox(sandbox)                      │
│  4. agent.run(task)                                  │
│  5. sandbox.cleanup()  // optional, keep artifacts   │
└──────────────────────┬──────────────────────────────┘
                       │
          ┌────────────▼─────────────┐
          │  Sandbox (trait)          │
          │  - kind() -> &str         │
          │  - start()                │
          │  - cleanup()              │
          │  - root_path() -> &Path   │
          │  - resolve_path(rel)      │
          └────┬──────────┬──────────┘
               │          │
      ┌────────▼──┐  ┌────▼──────────┐
      │LocalSandbox│  │DockerSandbox  │
      │  (now)    │  │  (future)     │
      └───────────┘  └───────────────┘
               │
               ▼
┌──────────────────────────────────┐
│  ToolContext                     │
│  + sandbox: Option<Arc<dyn Sandbox>> │
│  + resolve_path(rel) -> PathBuf  │
└──────────────────────────────────┘
```

## Design Decisions

### 1. Sandbox Lifecycle
- **Created and managed by the caller** (example code, CLI, or higher-level service)
- **CodingAgent receives** the sandbox via `with_sandbox()` builder method
- **Agent passes** the sandbox reference to tools via `ToolContext`
- **Caller decides** whether to cleanup or keep artifacts after agent finishes

### 2. Tool Path Resolution
- Tools call `ctx.resolve_path(&params.file_path)` instead of using the raw path
- If `ctx.sandbox` is `Some`, the path is resolved relative to sandbox root
- If `ctx.sandbox` is `None`, the original path is returned unchanged (backward compatible)
- No changes to tool core logic — only the path acquisition step changes

### 3. No spawn() Abstraction
- ReAct Agent executes tools sequentially, so per-tool isolation is unnecessary
- `spawn()` can be added later if parallel tool execution is needed
- Premature abstraction avoided — concrete use cases (Docker, SSH) can inform the design later

### 4. Why ToolContext (not path rewriting at agent layer)
- Agent-layer rewriting is fragile (needs to know which args are paths)
- ToolContext gives tools explicit awareness of sandbox, enabling future sandbox-specific behavior
- Each tool controls its own path resolution — some tools (like Bash) may use root_path directly rather than resolve_path

## Components

### Sandbox Trait (`vol-llm-core`)
Core abstraction defining sandbox interface. Future implementations: `LocalSandbox`, `DockerSandbox`, `SSHSandbox`.

### LocalSandbox (`vol-llm-agents`)
Concrete implementation using a local directory. Constructor takes an `Option<PathBuf>` — if `Some`, uses that directory; if `None`, creates a temp directory. `start()` creates the directory if it doesn't exist. `cleanup()` deletes the directory only if it was auto-created (temp mode); caller-owned directories are left untouched.

### ToolContext Extension (`vol-llm-tool`)
Add `sandbox: Option<Arc<dyn Sandbox>>` field and `resolve_path()` helper method.

### CodingAgent Integration (`vol-llm-agents`)
Add `with_sandbox()` builder method. Inject sandbox into ToolContext during `run()`.

## Future Extensibility

The Sandbox trait is designed for future implementations without requiring tool changes:

| Future Sandbox | Implementation Notes |
|---------------|---------------------|
| `DockerSandbox` | Mount local directory as volume, exec commands inside container |
| `SSHSandbox` | SSH connection to remote host, SFTP for file operations, same session for state |
| `CompositeSandbox` | Route different file types to different sandboxes (e.g., `.rs` files to Docker, configs to local) |

These do not affect existing tool implementations — they only depend on the Sandbox trait.
