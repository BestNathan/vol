# Design: SSH Sandbox + Sandbox Abstraction

## Requirements Traceability

This design implements requirements from [`2026-06-07-ssh-sandbox-requirement.md`](../requirement/2026-06-07-ssh-sandbox-requirement.md).

| Requirement Goal | Design Section |
|-----------------|----------------|
| G1: Sandbox trait as single I/O abstraction | §2 Sandbox trait, §5 Tool migration |
| G2: SSH sandbox implementation | §3 SSHSandbox internals |
| G3: Sandbox registry | §4 SandboxRegistry |
| G4: Per-tool sandbox selection | §6 Agent & Tool Wiring |
| G5: Local sandbox always available | §4 Registry (hardcoded local entry) |
| G6: Idle-timeout connection management | §3 Connection state machine |
| G7: Backward compatibility | §5 Tool migration |

## 1. Crate Structure

```
crates/
├── vol-llm-core/              # (modified) Remove sandbox.rs module
├── vol-llm-sandbox/           # (NEW) All sandbox types and implementations
│   ├── src/
│   │   ├── lib.rs             # Sandbox trait, SandboxError, CommandRequest/Output,
│   │   │                      #   DirEntry, FileMetadata, SandboxRef type alias
│   │   ├── local.rs           # LocalSandbox
│   │   ├── ssh.rs             # SSHSandbox (public API)
│   │   ├── ssh/
│   │   │   ├── connection.rs  # Connection pool, idle timeout state machine
│   │   │   ├── channel.rs     # SSH channel multiplexing
│   │   │   └── session.rs     # Session lifecycle (connect, auth, disconnect)
│   │   └── registry.rs        # SandboxRegistry
│   └── Cargo.toml
├── vol-llm-tool/              # (modified) Depends on vol-llm-sandbox
│                              #   ToolContext.sandbox: Option<SandboxRef> → Arc<dyn Sandbox>
└── vol-llm-tools-builtin/     # (modified) Tools call sandbox methods instead of OS I/O
    
    vol-llm-runtime/            # (modified) Initializes SandboxRegistry, wires into AgentRuntime

Dependency chain:
vol-llm-sandbox               (leaf — async_trait, ssh2, tokio, serde)
    ↑
vol-llm-tool                  (ToolContext holds Arc<dyn Sandbox>)
    ↑
vol-llm-tools-builtin          (tools use ctx.sandbox().X())
    ↑
vol-llm-runtime                (registry init, AgentRuntime has Arc<SandboxRegistry>)
```

### Crate Dependencies

**`vol-llm-sandbox` Cargo.toml**:
```toml
[dependencies]
async-trait.workspace = true
ssh2.workspace = true
tokio.workspace = true
serde.workspace = true
serde_json.workspace = true
thiserror.workspace = true
tracing.workspace = true
```

**`vol-llm-tool` changes**: Add `vol-llm-sandbox` dependency; remove `vol-llm-core` sandbox re-export usage.

**`vol-llm-core` changes**: Remove `pub mod sandbox;` and its re-export. Remove SandboxError, SandboxResult, Sandbox, SandboxRef from core's public API.

## 2. Sandbox Trait

Location: `crates/vol-llm-sandbox/src/lib.rs`

```rust
use std::path::{Path, PathBuf};
use std::collections::HashMap;
use std::time::Duration;
use std::sync::Arc;

/// Reference to a sandbox instance.
pub type SandboxRef = Arc<dyn Sandbox>;

/// Trait for isolated execution environments.
/// All tool I/O goes through this trait — tools never call OS APIs directly.
#[async_trait]
pub trait Sandbox: Send + Sync {
    // -- identity --
    fn kind(&self) -> &str;            // "local", "ssh"
    fn name(&self) -> &str;            // registry key, e.g. "devbox"

    // -- lifecycle --
    async fn start(&self) -> SandboxResult<()>;
    async fn cleanup(&self) -> SandboxResult<()>;

    // -- path --
    fn root_path(&self) -> &Path;
    fn resolve_path(&self, rel: &str) -> SandboxResult<PathBuf>;

    // -- command execution --
    async fn execute(&self, req: CommandRequest) -> SandboxResult<CommandOutput>;

    // -- file I/O --
    /// Read file content as raw bytes. Tools decode to String as needed.
    async fn read_file(
        &self, path: &Path, offset: Option<u64>, limit: Option<u64>
    ) -> SandboxResult<Vec<u8>>;

    /// Write bytes to a file. Parent directories must exist (call `create_dir_all` first).
    async fn write_file(&self, path: &Path, content: &[u8]) -> SandboxResult<()>;

    /// Create directory and all parents inside the sandbox root.
    async fn create_dir_all(&self, path: &Path) -> SandboxResult<()>;

    // -- directory & metadata --
    async fn read_dir(&self, path: &Path) -> SandboxResult<Vec<DirEntry>>;
    async fn metadata(&self, path: &Path) -> SandboxResult<FileMetadata>;
}

#[derive(Debug, Clone)]
pub struct CommandRequest {
    pub program: String,
    pub args: Vec<String>,
    pub env: HashMap<String, String>,
    pub cwd: Option<PathBuf>,    // relative to sandbox root
    pub stdin: Option<Vec<u8>>,
    pub timeout: Duration,
}

#[derive(Debug, Clone)]
pub struct CommandOutput {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub exit_code: i32,
    pub killed_by_signal: Option<i32>,
}

#[derive(Debug, Clone)]
pub struct DirEntry {
    pub name: String,
    pub is_dir: bool,
    pub is_file: bool,
}

#[derive(Debug, Clone)]
pub struct FileMetadata {
    pub size: u64,
    pub mtime: u64,      // unix timestamp millis
    pub is_dir: bool,
    pub is_file: bool,
}

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
    #[error("SSH error: {0}")]
    Ssh(String),
    #[error("Command timed out after {0:?}")]
    Timeout(Duration),
    #[error("Unknown sandbox type: {0}")]
    UnknownType(String),
    #[error("Sandbox '{0}' already registered")]
    DuplicateName(String),
    #[error("Local sandbox cannot be overridden by config")]
    LocalOverride,
}

pub type SandboxResult<T> = Result<T, SandboxError>;
```

## 3. SSHSandbox Internals

### 3.1 Connection State Machine

```
                    ┌─────────────────────────────────────────┐
                    │                                         │
                    ▼                                         │
              ┌──────────┐    sandbox call arrives     ┌───────────┐
              │          │◄───────────────────────────│           │
     ┌───────►│  Idle    │                            │ Disconn-  │
     │        │          │──────────┐                 │  ected    │
     │        └──────────┘          │                 └───────────┘
     │                              │                       ▲
     │   idle_timeout elapsed       │                       │
     │   (background timer)         │                       │
     │                              ▼                       │
     │                        ┌───────────┐   handshake    │
     │  ┌──────────┐          │           │    failure      │
     │  │          │          │ Connect-  │────────────────┘
     └──│  Discon- │◄─────────│   ing     │
        │  necting │  graceful │           │
        │          │  close    └───────────┘
        └──────────┘                │
             │                      │ handshake ok
             ▼                      ▼
        ┌──────────┐          ┌───────────┐
        │          │          │           │
        │ Disconn- │          │ Connected │──┐
        │  ected   │          │  (Idle)   │  │ execute() call
        └──────────┘          └───────────┘  │
              ▲                      ▲        │
              │                      └────────┘
              │                   execute() completes,
              │                   idle timer resets
              │
              └── cleanup() called
```

**Key rules:**
- Idle timer counts time **between** commands, not command duration
- Background tokio task checks idle timer every second; fires disconnect when threshold exceeded
- `start()` is idempotent — if already connected, returns Ok immediately
- `cleanup()` while command is running: terminates command (SIGTERM → wait 5s → SIGKILL) then disconnects
- On SSH disconnect (network drop), next sandbox call triggers automatic reconnect

### 3.2 Channel Multiplexing

Multiple concurrent `execute()` calls share one TCP connection via SSH channel multiplexing:

```rust
pub struct SshConnection {
    session: Mutex<Option<ssh2::Session>>,
    // ... config, idle timer, etc.
}

impl SshConnection {
    async fn execute(&self, req: CommandRequest) -> SandboxResult<CommandOutput> {
        // 1. Lock mutex briefly, ensure session exists (reconnect if needed), open channel
        let mut channel = {
            let mut guard = self.session.lock().await;
            let session = guard.as_mut().ok_or(SandboxError::NotStarted)?;
            session.channel_session()?
        };
        // 2. Mutex released — channel executes independently
        channel.exec(&command_line)?;

        // 3. Read stdout/stderr, check exit
        // 4. Notify idle timer of activity
    }
}
```

- `ssh2::Channel` handles SSH protocol multiplexing natively
- Mutex held only during channel open/close + connection setup, NOT during command execution
- Actual concurrency limited by SSH server's `MaxSessions` config

### 3.3 File I/O via SFTP

All file operations use `ssh2::Sftp` (not shell commands). `Sftp` provides binary-safe, structured access:

| Sandbox Method | SFTP Call |
|---------------|-----------|
| `read_file(path)` | `sftp.open(path)` → `read()` |
| `write_file(path, content)` | `sftp.create(path)` → `write()` |
| `create_dir_all(path)` | `sftp.mkdir()` per component |
| `read_dir(path)` | `sftp.readdir(path)` → map to `DirEntry` |
| `metadata(path)` | `sftp.stat(path)` → map to `FileMetadata` |

SFTP session is opened on-demand (one per concurrent I/O operation) from the shared SSH session.

### 3.4 SSH Host Key Verification

```toml
[sandbox.ssh]
# Option A: known_hosts file
known_hosts_file = "~/.ssh/known_hosts"

# Option B: pinned fingerprint
host_key = "sha256:AAAAbbbbCCCC..."
```

At connection time:
1. If `known_hosts_file` is set, use `ssh2::Session::known_hosts()` to verify
2. If `host_key` is set, compare against server's presented key fingerprint
3. If neither is configured, connection fails with error — **never skip verification**

## 4. SandboxRegistry

### 4.1 Configuration Format

`.agent/sandboxes/*.toml` — only non-local sandboxes need config files.

```toml
# .agent/sandboxes/devbox.toml
[sandbox]
name = "devbox"
type = "ssh"
work_dir = "/home/agent/sandbox"

[sandbox.ssh]
host = "192.168.2.100"
port = 22
user = "agent"
identity_file = "~/.ssh/id_ed25519"
# passphrase = "optional"          # optional
known_hosts_file = "~/.ssh/known_hosts"
# host_key = "sha256:AAAA..."     # alternative to known_hosts_file
idle_timeout_secs = 300
connect_timeout_secs = 10
```

### 4.2 Registry Implementation

```rust
pub struct SandboxRegistry {
    sandboxes: HashMap<String, Arc<dyn Sandbox>>,
    default_name: String,  // always "local"
}

impl SandboxRegistry {
    pub async fn load(sandboxes_dir: &Path) -> SandboxResult<Self> {
        let mut sandboxes: HashMap<String, Arc<dyn Sandbox>> = HashMap::new();

        // 1. Always register LocalSandbox (hardcoded, no config file needed)
        let local = Arc::new(LocalSandbox::new(None)); // None = temp dir
        sandboxes.insert("local".to_string(), local);

        // 2. Load *.toml files from sandboxes_dir
        if sandboxes_dir.exists() {
            for entry in std::fs::read_dir(sandboxes_dir)? {
                let entry = entry?;
                if entry.path().extension() == Some(std::ffi::OsStr::new("toml")) {
                    let config = SandboxConfig::from_file(&entry.path())?;
                    if config.name == "local" {
                        return Err(SandboxError::LocalOverride);
                    }
                    if sandboxes.contains_key(&config.name) {
                        return Err(SandboxError::DuplicateName(config.name));
                    }
                    let sandbox: Arc<dyn Sandbox> = match config.sandbox_type {
                        "ssh" => Arc::new(SSHSandbox::from_config(config)?),
                        other => return Err(SandboxError::UnknownType(other.to_string())),
                    };
                    sandbox.start().await?;
                    sandboxes.insert(config.name.clone(), sandbox);
                }
            }
        }

        Ok(Self { sandboxes, default_name: "local".to_string() })
    }

    pub fn get(&self, name: &str) -> Option<Arc<dyn Sandbox>> {
        self.sandboxes.get(name).cloned()
    }

    pub fn default(&self) -> Arc<dyn Sandbox> {
        self.sandboxes.get(&self.default_name)
            .cloned()
            .expect("LocalSandbox always present")
    }
}
```

### 4.3 Runtime Integration

`AgentRuntimeBuilder::build()` — after MCP/tool registry setup:

```rust
let sandbox_registry = SandboxRegistry::load(
    &working_dir.join(".agent/sandboxes")
).await.map_err(|e| format!("Sandbox init failed: {e}"))?;
let sandbox_registry = Arc::new(sandbox_registry);
```

`AgentRuntime` gains:
```rust
pub sandbox_registry: Arc<SandboxRegistry>,
```

## 5. Tool Migration

### 5.1 ToolContext Change

```rust
// Before
pub struct ToolContext {
    pub sandbox: Option<SandboxRef>,   // Option<Arc<dyn Sandbox>>
    // ...
}

// After
pub struct ToolContext {
    pub sandbox: Arc<dyn Sandbox>,   // Always set (defaults to LocalSandbox)
    // ...
}
```

Tools no longer check `if let Some(sandbox) = ...` — they call `ctx.sandbox().execute()` directly.

### 5.2 Per-Tool Sandbox Selection

Each tool definition (`ToolDef`) gains an optional `sandbox` field:

```rust
pub struct ToolDef {
    // ... existing fields ...
    pub sandbox: Option<String>,  // sandbox registry name
}
```

Resolution order when constructing `ToolContext`:
1. If `ToolDef.sandbox` is set → resolve from registry
2. Else if `AgentConfig.default_sandbox` is set → resolve from registry
3. Else → registry.default() (LocalSandbox)

Resolution happens in the agent's tool execution loop, NOT in individual tools.

### 5.3 Tool-by-Tool Changes

| Tool | Current Implementation | New Implementation |
|------|----------------------|-------------------|
| `bash` | `std::process::Command::new("bash")` with process_group, signal handling | `sandbox.execute(CommandRequest { program: "bash", args: ["-c", cmd], timeout, cwd })` |
| `read_file` | `tokio::fs::read_to_string(path)` | `sandbox.read_file(path, offset, limit)` → `String::from_utf8_lossy` |
| `write_file` | `tokio::fs::create_dir_all` + `tokio::fs::write` | `sandbox.create_dir_all(parent)` + `sandbox.write_file(path, content)` |
| `edit_file` | `tokio::fs::read_to_string` + `tokio::fs::write` | `sandbox.read_file` + `sandbox.write_file` |
| `glob` | `glob::glob(pattern)` + `std::fs::metadata` | `sandbox.read_dir(parent)` + pattern matching in tool + `sandbox.metadata` |
| `grep` | `std::process::Command("rg")` or library search | `sandbox.execute(CommandRequest { program: "rg", ... })`; fallback: `sandbox.read_file` + local regex |
| `web_search` | reqwest HTTP POST | Unchanged (no sandbox I/O) |
| `web_fetch` | reqwest HTTP GET | Unchanged (no sandbox I/O) |
| `task` | FileTaskStore | Unchanged (abstracted behind TaskStore trait) |
| `skill` | SkillLoader (FS reads) | Unchanged (abstracted behind SkillLoader trait) |
| `agent` | Agent dispatch | Unchanged (internal runtime logic) |

**bash security patterns**: The existing `DANGEROUS_PATTERNS` regex checks remain in the bash tool — they run before `sandbox.execute()` is called. The sandbox is a transport layer, not a security boundary replacement.

**bash timeout/signal handling**: `CommandRequest.timeout` is passed to `sandbox.execute()`. The sandbox implementation handles timeout via SSH channel close + re-open. Process group signaling (SIGTERM/SIGKILL) is achieved through `ssh2::Channel::send_eof()` followed by channel close for soft termination, or direct channel close for hard termination.

## 6. Agent & Tool Wiring

### 6.1 AgentConfig Changes

```rust
pub struct AgentConfig {
    // ... existing fields ...
    pub sandbox_registry: Arc<SandboxRegistry>,      // NEW
    pub default_sandbox: Option<String>,              // NEW: registry name
}
```

### 6.2 Tool Execution Flow

```
Agent receives tool call (name + args)
    │
    ▼
Look up tool in ToolRegistry → get ExecutableTool + ToolDef
    │
    ▼
Resolve sandbox:
    tool.sandbox_name = ToolDef.sandbox
                        .or(AgentConfig.default_sandbox)
                        .unwrap_or("local")
    sandbox = registry.get(tool.sandbox_name) ?? registry.default()
    │
    ▼
Build ToolContext { sandbox, messages, agent_def }
    │
    ▼
tool.execute(args, &ctx)
    │
    ▼
Inside tool: ctx.sandbox().execute() / read_file() / write_file() / ...
```

### 6.3 AgentConfigBuilder Changes

```rust
impl AgentConfigBuilder {
    pub fn with_sandbox_registry(mut self, registry: Arc<SandboxRegistry>) -> Self {
        self.sandbox_registry = Some(registry);
        self
    }

    pub fn with_default_sandbox(mut self, name: impl Into<String>) -> Self {
        self.default_sandbox = Some(name.into());
        self
    }
}
```

## 7. Error Handling

### Error Propagation

```
SandboxError (vol-llm-sandbox)
    ↓ 工具将 SandboxError 映射为 ToolError::ExecutionFailed
ToolError (vol-llm-tool)
    ↓ Agent 将 ToolError 作为 ToolResult { success: false, error: "..." } 返回给 LLM
ToolResult (vol-llm-tool)
    ↓ LLM 看到错误文本，决定重试/修正/放弃
```

### Key Error Scenarios

| Scenario | SandboxError | Tool Behavior |
|----------|-------------|---------------|
| SSH host unreachable | `Ssh("connection refused")` | Return error; agent can retry |
| SSH auth failure | `Ssh("authentication failed")` | Return error; agent reports to user |
| Command timeout | `Timeout(duration)` | Same as current bash timeout behavior |
| Path traversal attempted | `PathTraversal(path)` | Return error; path is rejected |
| Remote disk full | `Io(WriteZero)` | Return error; agent can try cleaning up |
| SFTP channel exhausted | `Ssh("channel open failed")` | Return error; agent can retry later |

## 8. Testing Strategy

### Unit Tests (always run)

| Test | Scope |
|------|-------|
| `LocalSandbox` trait methods | Local filesystem I/O, path traversal rejection |
| `SandboxRegistry::load()` | Config parsing, duplicate detection, local override rejection |
| `ToolContext` sandbox resolution | ToolDef override > Agent default > local fallback |
| `CommandRequest` serde | Round-trip serialization |
| `resolve_path()` traversal checks | `../` escapes, absolute paths, symlink scenarios |

### Integration Tests (may be `#[ignore]` in CI, require SSH test host)

| Test | Scope |
|------|-------|
| `SSHSandbox::execute()` | Real SSH connection, execute `echo hello`, verify output |
| `SSHSandbox` idle timeout + reconnect | Short timeout (2s), verify disconnect + auto-reconnect |
| `SSHSandbox` file I/O | write → read → verify content match, glob, grep |
| `SSHSandbox` command timeout | Long-running command, verify kill + error |
| `SSHSandbox` channel multiplexing | 3 concurrent execute() calls, verify all complete correctly |
| `SSHSandbox` host key verification | Missing known_hosts → error; valid known_hosts → connects |

### Test Utilities

- **`#[cfg(test)]` mock sandbox**: Implements `Sandbox` trait with in-memory filesystem and command capture, for tool unit tests
- **SSH test host**: Docker container with OpenSSH server, key-based auth pre-configured, used by integration tests

## 9. Implementation Order

| Phase | What | Depends On |
|-------|------|-----------|
| **P0: Trait + Local** | `vol-llm-sandbox` crate, extended Sandbox trait, updated LocalSandbox | Nothing |
| **P1: ToolContext + Tools** | Change ToolContext, migrate all builtin tools to sandbox trait | P0 |
| **P2: SandboxRegistry** | Registry loading, runtime integration, AgentConfig wiring | P0 |
| **P3: SSHSandbox** | SSHSandbox impl with connection state machine, channel multiplexing, SFTP | P0 |
| **P4: SSH integration tests** | Docker-based SSH test host, integration test suite | P3 |
| **P5: Per-tool sandbox** | ToolDef.sandbox field, per-tool resolution in agent loop | P2 |

P0-P1 can be done together (they form the trait + migration). P2-P3 can be done in parallel (registry vs SSH impl). P4-P5 are sequential on P3/P2 respectively.

## Version History

| Date | Change |
|------|--------|
| 2026-06-07 | Initial design document |
