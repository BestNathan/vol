# Requirements: SSH Sandbox + Sandbox Abstraction

## Background

The vol-agent project already has a `Sandbox` trait (`vol-llm-core/src/sandbox.rs`) and a single `LocalSandbox` implementation. The trait is minimal—only `kind()`, `start()`, `cleanup()`, `root_path()`, `resolve_path()`—and does not expose the capabilities (command execution, file I/O, directory listing) that builtin tools actually need. Tools currently bypass the sandbox abstraction for these operations: `bash` uses `std::process::Command` directly, file tools use `tokio::fs` directly.

The goal is to make Sandbox a first-class abstraction that encapsulates **all** execution-environment interactions, then build an SSH-backed sandbox as the first non-local implementation. This enables commands and file operations to transparently execute on a remote host, with tool code unchanged.

## Goals

1. **Sandbox trait is the single abstraction for execution environment** — every I/O operation that touches "the outside world" (filesystem, process execution) goes through the sandbox trait, not through direct OS calls
2. **SSH sandbox** implements the trait, routing command execution and file I/O to a remote host over SSH
3. **Sandbox registry** loads named sandbox configurations from `.agent/sandboxes/`, making sandboxes discoverable and selectable
4. **Per-tool sandbox selection** — tools can specify which sandbox they execute on; agent provides a default, tools override
5. **Local sandbox is always available** — the execution environment itself, using the current `work_dir` as root
6. **Idle-timeout connection management** — SSH connections stay alive while in use, auto-disconnect after configurable idle period
7. **Backward compatibility** — existing tools work unchanged; the sandbox wiring is transparent

## Non-Goals

- Container sandbox (Docker/K8s) — out of scope for this iteration, but the trait must be designed to accommodate future container backends
- Sandbox filesystem synchronization/rsync — files are transferred on-demand per operation, not pre-synced
- Multi-sandbox orchestration (e.g., one agent using two sandboxes concurrently for different tools) — single sandbox per tool invocation
- Remote sandbox provisioning/creation — the SSH sandbox connects to a pre-existing remote host; it does not create VMs or containers

## Scope

### In Scope

| Item | Description |
|------|-------------|
| Extended `Sandbox` trait | `execute()`, `read_file()`, `write_file()`, `create_dir_all()`, `read_dir()`, `metadata()`, lifecycle hooks |
| `CommandRequest` / `CommandOutput` types | Structured input/output for command execution (args, env, cwd, stdin, timeout → stdout, stderr, exit_code, signal) |
| `SandboxRegistry` | Loads from `.agent/sandboxes/*.toml`, resolves by name, holds `Arc<dyn Sandbox>` instances |
| `LocalSandbox` (updated) | Implements the extended trait using local OS calls |
| `SSHSandbox` | Implements the trait over SSH connection with idle-timeout management |
| Per-tool sandbox wiring | `ToolContext` carries sandbox reference; tools use `ctx.sandbox()` for I/O |
| Agent default sandbox | Agent config specifies a default sandbox name; `ToolContext` resolves it from registry |
| Sandbox config format | TOML files under `.agent/sandboxes/` with type, connection params, work_dir, idle_timeout |

### Out of Scope

- Pre-flight checks for remote tool availability (e.g., checking `bash`/`rg` exist on remote)
- File streaming for large files (initial version transfers entire file content)
- Sandbox health monitoring / metrics
- Configuration hot-reload (restart required to pick up sandbox config changes)

## Constraints

- **Rust async runtime**: All sandbox I/O methods are `async`, compatible with the existing tokio runtime
- **Sandbox trait must be object-safe**: `dyn Sandbox` is used via `Arc<dyn Sandbox>` throughout the system
- **Path sandboxing preserved**: All path operations enforce sandbox boundaries — no access outside `root_path()` (same as current `LocalSandbox`)
- **SSH**: Use native Rust SSH library (e.g., `ssh2` or `openssh` crate); avoid shelling out to `ssh` CLI
- **TOML config**: Sandbox registry configs use TOML, consistent with the project's existing config format
- **Minimal tool changes**: Tool `execute()` signatures remain unchanged. Tools access sandbox through the existing `ToolContext` API; the context type changes internally but tools only call methods on it — no per-tool trait changes needed
- **Security**:
  - SSH sandbox MUST verify remote host key before connecting (via `known_hosts` file or explicit `host_key` fingerprint in config). No auto-accept of unknown hosts.
  - Key-based auth required; password auth optional and explicitly opt-in.
  - Private key paths read from config, not embedded. Encrypted keys supported via `ssh-agent` or `passphrase` config field.
  - If no host key verification is configured, connection MUST fail with a clear error — never skip verification.

## Success Criteria

| # | Criteria | Verification |
|---|----------|-------------|
| SC1 | `Sandbox` trait defines all methods needed to support `bash`, `read_file`, `write_file`, `edit_file`, `glob`, `grep` without direct OS calls | Code review: trait method set covers all I/O used by builtin tools |
| SC2 | `LocalSandbox` implements the extended trait, all existing tool tests pass | `cargo test -p vol-llm-tools-builtin` passes |
| SC3 | `SSHSandbox` connects to a remote host via SSH key, executes `bash -c <cmd>`, returns stdout/stderr/exit_code | Integration test (may be `#[ignore]` in CI, runnable with SSH test host configured) |
| SC4 | SSH connection idle timeout works: connection closes after N seconds of inactivity, reconnects transparently on next use | Integration test with short idle_timeout (e.g., 2s) to verify disconnect + reconnect |
| SC5 | `read_file`/`write_file`/`edit_file` tools transparently operate on remote filesystem when using SSH sandbox | Integration test: write via sandbox → read back → verify content matches |
| SC6 | `glob`/`grep` tools work against remote filesystem through SSH sandbox | Integration test: create test files on remote → glob/grep → verify results |
| SC7 | Sandbox registry loads from `.agent/sandboxes/`, resolves sandboxes by name | Unit test: load config, get sandbox by name, verify kind/params |
| SC8 | Agent can specify `default_sandbox = "ssh-remote"`; tools that don't override use it | Integration test: agent with SSH sandbox → bash tool → executes on remote |
| SC9 | Tool can specify a different sandbox than agent default | Unit test: tool with explicit sandbox name overrides agent default |
| SC10 | Path traversal protection works on SSH sandbox (e.g., `../../../etc/passwd` rejected) | Unit test: resolve_path with `../` escapes returns error |
| SC11 | Local sandbox is always available even when no registry is configured | Unit test: create agent with no sandbox config → tool execution uses LocalSandbox with agent's work_dir |

## Architecture Overview

```
.agent/sandboxes/
├── local.toml          # type = "local", work_dir = "."
├── ssh-dev.toml        # type = "ssh", host = "devbox.local", ...

SandboxRegistry::load(".agent/sandboxes/") → HashMap<Name, Arc<dyn Sandbox>>
                                              ├── "local" → LocalSandbox
                                              └── "ssh-dev" → SSHSandbox

AgentConfig { default_sandbox: Some("ssh-dev") }
    ↓
ToolContext { sandbox: Arc<dyn Sandbox> }
    ↓
bash::execute() → ctx.sandbox().execute(CommandRequest { ... })
read_file()    → ctx.sandbox().read_file(path)
write_file()   → ctx.sandbox().write_file(path) + create_dir_all(parent)
glob()         → ctx.sandbox().read_dir(path) + metadata()
grep()         → ctx.sandbox().execute("rg ...")  OR  read_file + regex
```

## Sandbox Trait Design

```rust
pub trait Sandbox: Send + Sync {
    // -- identity --
    fn kind(&self) -> &str;           // "local", "ssh", "docker", ...
    fn name(&self) -> &str;           // registry name, e.g. "ssh-dev"

    // -- lifecycle --
    async fn start(&self) -> SandboxResult<()>;
    async fn cleanup(&self) -> SandboxResult<()>;
    // Idle-timeout connection management is an implementation detail
    // of SSHSandbox, NOT a trait method — LocalSandbox has no idle timeout.

    // -- path --
    fn root_path(&self) -> &Path;
    fn resolve_path(&self, rel: &str) -> SandboxResult<PathBuf>;

    // -- command execution --
    async fn execute(&self, req: CommandRequest) -> SandboxResult<CommandOutput>;

    // -- file I/O --
    async fn read_file(&self, path: &Path, offset: Option<u64>, limit: Option<u64>)
        -> SandboxResult<Vec<u8>>;  // binary-safe; tools decode to String as needed
    async fn write_file(&self, path: &Path, content: &[u8]) -> SandboxResult<()>;
    async fn create_dir_all(&self, path: &Path) -> SandboxResult<()>;

    // -- directory & metadata --
    async fn read_dir(&self, path: &Path) -> SandboxResult<Vec<DirEntry>>;
    async fn metadata(&self, path: &Path) -> SandboxResult<FileMetadata>;
}
```

## Sandbox Config Format (`.agent/sandboxes/*.toml`)

```toml
# local.toml — always present, built-in
[sandbox]
name = "local"
type = "local"
work_dir = "."               # relative to agent cwd

# ssh-dev.toml — example SSH sandbox
[sandbox]
name = "ssh-dev"
type = "ssh"
work_dir = "/home/agent/sandbox"

[sandbox.ssh]
host = "devbox.local"
port = 22
user = "agent"
identity_file = "~/.ssh/id_ed25519"
# passphrase = "optional-key-passphrase"   # optional, or use ssh-agent
known_hosts_file = "~/.ssh/known_hosts"    # host key verification (required)
# host_key = "sha256:AAAA..."              # alternative: pin specific host key
idle_timeout_secs = 300                    # 5 min idle → disconnect
connect_timeout_secs = 10
```

## Tool → Sandbox Wiring

Current state: `ToolContext.sandbox: Option<SandboxRef>` — tools optionally use `ctx.resolve_path()`. Actual I/O is direct OS calls.

Target state: `ToolContext.sandbox: Option<Arc<dyn Sandbox>>` — tools delegate ALL I/O through sandbox methods.

**Per-tool sandbox selection mechanism**: Each tool definition (`ToolDef`) has an optional `sandbox` field. When a tool is invoked:
1. If the tool's `ToolDef.sandbox` is set to a sandbox name, resolve that sandbox from the registry and use it for this invocation
2. Otherwise, use the agent's `default_sandbox` (set in `AgentConfig`)
3. If neither is set, use the built-in local sandbox

This resolution happens in `ToolContext` construction before `tool.execute()` is called — tools simply call `ctx.sandbox().execute()` etc. without knowing which sandbox they're on.

**Migration strategy**: Each tool gets its sandbox-using code path updated:

| Tool | Current | Target |
|------|---------|--------|
| `bash` | `std::process::Command::new("bash")` | `sandbox.execute(CommandRequest { program: "bash", args: ["-c", cmd], ... })` |
| `read_file` | `tokio::fs::read_to_string(path)` | `sandbox.read_file(path, offset, limit)` |
| `write_file` | `tokio::fs::write(path, content)` | `sandbox.create_dir_all(parent)` + `sandbox.write_file(path, content)` |
| `edit_file` | `tokio::fs::read_to_string` + `tokio::fs::write` | `sandbox.read_file` + `sandbox.write_file` |
| `glob` | `glob::glob` + `std::fs::metadata` | `sandbox.read_dir` + `sandbox.metadata` |
| `grep` | `std::process::Command::new("rg")` or library | `sandbox.execute(CommandRequest { program: "rg", ... })` — execute `rg` on the sandbox side. If `rg` is unavailable on the sandbox, fall back to `sandbox.read_file` + local regex search |

## Edge Cases

| Edge Case | Expected Behavior |
|-----------|-------------------|
| SSH host unreachable | `execute()` returns error; agent surfaces to user. No fallback to local. |
| SSH auth failure | Clear error: "SSH authentication failed for user@host — check identity_file" |
| SSH connection drops mid-command | Command returns error; next invocation triggers reconnect |
| Idle timeout fires during long command | Idle timer only counts time between commands, not command duration |
| Command exceeds timeout | SIGTERM → wait → SIGKILL, same as current bash tool behavior, but over SSH channel |
| Command output > 1MB | Truncate at limit, same as current bash tool |
| Absolute path in sandbox I/O | Reject with error (paths must be relative to root_path) |
| Path traversal (`../`) | Reject with error. Checked **locally** by `resolve_path()` before any remote I/O — path is normalized and verified to stay within `root_path()`. Note: local-side checking cannot detect remote symlinks pointing outside root_path; this is an accepted limitation for v1. |
| Large file read/write | Read entire file into memory (for now); streaming is future work |
| Concurrent commands on same SSH sandbox | Open multiple SSH channels on a single TCP connection (multiplex). Each `execute()` gets its own channel; no head-of-line blocking |
| Remote work_dir doesn't exist | `start()` creates it on sandbox initialization |
| Sandbox config references unknown sandbox type | Error at registry load time: "unknown sandbox type: xxx" |
| Tool specifies sandbox that doesn't exist in registry | Error at tool execution time |
| Sandbox not configured (no registry) | Fall back to built-in local sandbox, using the agent's configured `work_dir` (from `AgentConfig`), or the process CWD if no agent config is available |
| Binary file content | `read_file` returns `Vec<u8>` (binary-safe); tools that need text decode via `String::from_utf8` or similar |
| Concurrent `start()` calls on disconnected sandbox | `start()` is idempotent — if already connected, return Ok immediately. Internal mutex prevents double-connect races |
| `cleanup()` while command is running | Command is terminated (SIGTERM → SIGKILL) before cleanup proceeds; cleanup waits for command to finish |

## Open Questions

*(All resolved)*

1. ~~**Concurrent SSH commands**~~ → **Resolved**: Open multiple SSH channels on a single connection (multiplex). Concurrent `execute()` calls share one TCP connection but use independent channels, avoiding serialization bottlenecks.
2. ~~**File metadata type**~~ → **Resolved**: `FileMetadata` fields: `mtime`, `size`, `is_dir`, `is_file`.

## Version History

| Date | Change |
|------|--------|
| 2026-06-07 | Initial requirements document |
