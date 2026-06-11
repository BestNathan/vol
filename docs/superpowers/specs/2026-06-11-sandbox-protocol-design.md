# Architecture: Sandbox Protocol for Agent Server

**Date**: 2026-06-11
**Status**: Draft
**Author**: Claude
**Source**: `docs/superpowers/requirement/2026-06-11-sandbox-protocol-requirement.md`

## Requirements

[`2026-06-11-sandbox-protocol-requirement.md`](../requirement/2026-06-11-sandbox-protocol-requirement.md)

**Goals**: 定义 sandbox JSON-RPC 协议 → agent server 实现 handler → 提供 RemoteSandbox（`impl Sandbox`）

**Non-Goals**: 不搞多层转发，不暴露生命周期操作，不做 per-connection 隔离

## Architecture

三层设计：**protocol wire types**（`vol-llm-agent-protocol`）→ **server handler**（`vol-agent-server`）→ **remote proxy**（`vol-agent-server`）。

```
Agent B 内存空间                                Agent Server A 内存空间
───────────────────────────                    ─────────────────────────
RemoteSandbox                                   SandboxHandler (DomainHandler)
  impl Sandbox                                       │
  │                                                  │
  │ ① execute(req)                                  │
  │ ② CommandRequest → CommandRequestDef            │
  │     ↓ JSON-RPC ────────────►  sandbox.exec ────► │
  │                               (WebSocket text)   │
  │                                                   │ ③ LocalSandbox::execute(req)
  │                                                   │ ④ CommandOutput → CommandOutputDef
  │                                                  │
  │ ◄──────── JSON-RPC ────────  sandbox.exec ────── │
  │ ⑤ CommandOutputDef → CommandOutput              │
  │ ⑥ return SandboxResult<CommandOutput>           │
```

**依赖方向（单向无环）：**

```
  vol-llm-sandbox               # Sandbox trait + CommandRequest/CommandOutput/...
        ↑
  vol-llm-agent-protocol        # +vol-llm-sandbox dep; SandboxOperation/Payload wire types
        ↑
  vol-agent-server              # SandboxHandler + RemoteSandbox（两者均已有 protocol + sandbox dep）
```

### Component Breakdown

#### 1. Protocol Wire Types（`vol-llm-agent-protocol`）

- 新增 `SandboxOperation` 枚举（List/Exec/ReadFile/WriteFile/CreateDir/ReadDir/Metadata），每个有 `method_name()` 映射
- 新增 `SandboxPayload` 枚举（request/result pair），`#[serde(untagged)]`，与现有 `FilePayload` 风格一致
- 扩展 `Operation::Sandbox(SandboxOperation)` 和 `Payload::Sandbox(SandboxPayload)`
- `operation_codec.rs` 中新增 `"sandbox.*" → Operation::Sandbox(...)` 映射
- 定义 serde 友好的 "Def" 类型（`CommandRequestDef`、`CommandOutputDef`、`DirEntryDef`、`FileMetadataDef`）避免 `Duration`/`HashMap`/`PathBuf` 序列化问题；通过 `From`/`Into` 转换

#### 2. SandboxHandler（`vol-agent-server`）

- 实现 `DomainHandler` trait
- 持有 `SandboxRef`（直接引用 `LocalSandbox` 实例，不走 `SandboxRegistry`）
- 注册到 data-plane 和 control-plane 的 `HandlerRegistry`
- 每个 handler method 匹配 `(SandboxOperation, SandboxPayload)` → 转换 "Def" 类型 → 调用 `Sandbox` trait → 转换结果 → `AgentServerMessage`

#### 3. RemoteSandbox（`vol-agent-server`）

- 实现 `Sandbox` trait
- `connect(url)` 建立 `tokio_tungstenite` WebSocket 连接
- 内部结构：
  - `write_tx: mpsc::UnboundedSender<String>` 发送端
  - `pending: Mutex<HashMap<msg_id, oneshot::Sender<AgentServerMessage>>>` 请求-响应匹配
  - Background reader task：收帧 → `decode_jsonrpc_frame()` → 查 `pending` → `oneshot::send()`
  - Background writer task：从 mpsc channel 读 → `ws.send()`
- 每个 `Sandbox` trait 方法：构造 `AgentServerMessage` → `encode_jsonrpc_message()` → 发送 → 等待 oneshot 响应 → 解析回 trait 返回类型
- 30s 默认超时，无自动重连
- `start()` / `cleanup()` 均为 no-op

### File Structure

```
vol-llm-agent-protocol/
├── Cargo.toml                                    # +vol-llm-sandbox dep
├── src/
│   ├── agent_server_protocol.rs                  # +SandboxOperation, +SandboxPayload,
│   │                                             #   +CommandRequestDef, +CommandOutputDef, etc.
│   └── operation_codec.rs                        # +sandbox.* method mapping

vol-agent-server/
├── Cargo.toml                                    # 已依赖 vol-llm-sandbox + vol-llm-agent-protocol
├── src/
│   ├── data_plane/
│   │   ├── core.rs                               # register SandboxHandler
│   │   └── handlers/
│   │       └── sandbox.rs                        # NEW
│   ├── control_plane/
│   │   └── core.rs                               # register SandboxHandler
│   └── sandbox/
│       └── remote.rs                             # NEW (RemoteSandbox)
└── tests/
    ├── sandbox_protocol_integration.rs            # NEW
    └── sandbox_e2e_test.rs                       # NEW
```

## Key Types

### `SandboxOperation`（protocol）

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SandboxOperation {
    List,
    Exec,
    ReadFile,
    WriteFile,
    CreateDir,
    ReadDir,
    Metadata,
}

impl SandboxOperation {
    pub fn method_name(&self) -> &'static str {
        match self {
            Self::List => "sandbox.list",
            Self::Exec => "sandbox.exec",
            Self::ReadFile => "sandbox.read_file",
            Self::WriteFile => "sandbox.write_file",
            Self::CreateDir => "sandbox.create_dir",
            Self::ReadDir => "sandbox.read_dir",
            Self::Metadata => "sandbox.metadata",
        }
    }
}
```

### `SandboxPayload`（protocol）

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SandboxPayload {
    // ── List ──
    List,
    ListResult {
        sandboxes: Vec<SandboxInfo>,
    },

    // ── Exec ──
    Exec {
        command: CommandRequestDef,
    },
    ExecResult {
        output: CommandOutputDef,
    },

    // ── ReadFile ──
    ReadFile {
        path: String,
        #[serde(default)]
        offset: Option<u64>,
        #[serde(default)]
        limit: Option<u64>,
    },
    ReadFileResult {
        content: String, // base64
    },

    // ── WriteFile ──
    WriteFile {
        path: String,
        content: String, // base64
    },
    WriteFileResult,

    // ── CreateDir ──
    CreateDir {
        path: String,
    },
    CreateDirResult,

    // ── ReadDir ──
    ReadDir {
        path: String,
    },
    ReadDirResult {
        entries: Vec<DirEntryDef>,
    },

    // ── Metadata ──
    Metadata {
        path: String,
    },
    MetadataResult {
        metadata: FileMetadataDef,
    },
}
```

### Serde-friendly "Def" Types（protocol）

```rust
/// Wire-compatible command request — all fields are directly serializable.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandRequestDef {
    pub program: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: Vec<(String, String)>,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default)]
    pub stdin: Option<String>,     // base64
    #[serde(default)]
    pub timeout_ms: u64,
}

impl From<CommandRequestDef> for CommandRequest {
    fn from(d: CommandRequestDef) -> Self { /* Vec<(K,V)> → HashMap, ms → Duration */ }
}

impl From<CommandRequest> for CommandRequestDef {
    fn from(r: CommandRequest) -> Self { /* HashMap → Vec<(K,V)>, Duration → ms */ }
}

// CommandOutputDef, DirEntryDef, FileMetadataDef — 同理
```

### `SandboxHandler`（agent-server）

```rust
pub struct SandboxHandler {
    sandbox: SandboxRef,
}

impl SandboxHandler {
    pub fn new(sandbox: SandboxRef) -> Self { Self { sandbox } }
}

#[async_trait]
impl DomainHandler for SandboxHandler {
    fn name(&self) -> &str { "sandbox" }

    fn operations(&self) -> Vec<Operation> {
        vec![
            Operation::Sandbox(SandboxOperation::List),
            Operation::Sandbox(SandboxOperation::Exec),
            Operation::Sandbox(SandboxOperation::ReadFile),
            Operation::Sandbox(SandboxOperation::WriteFile),
            Operation::Sandbox(SandboxOperation::CreateDir),
            Operation::Sandbox(SandboxOperation::ReadDir),
            Operation::Sandbox(SandboxOperation::Metadata),
        ]
    }

    async fn handle(
        &self,
        message: AgentServerMessage,
    ) -> Result<Vec<AgentServerMessage>, ProtocolError> {
        let (op, payload) = match (&message.operation, &message.payload) {
            (Operation::Sandbox(op), Payload::Sandbox(p)) => (op, p),
            _ => return Err(ProtocolError::InvalidOperation("not a sandbox operation")),
        };

        let mid = &message.message_id;

        // Dispatch by (op, payload) match → convert Def types → call sandbox.xxx() → convert back
        match (op, payload) {
            (SandboxOperation::Exec, SandboxPayload::Exec { command }) => {
                let req: CommandRequest = command.clone().into();
                match self.sandbox.execute(req).await {
                    Ok(output) => Ok(vec![AgentServerMessage::new_result(
                        mid,
                        Operation::Sandbox(SandboxOperation::Exec),
                        Payload::Sandbox(SandboxPayload::ExecResult { output: output.into() }),
                    )]),
                    Err(e) => Ok(vec![AgentServerMessage::new_error(mid, e.to_string())]),
                }
            }
            // ...其余操作同理
            _ => Ok(vec![AgentServerMessage::new_error(mid, "invalid payload")]),
        }
    }
}
```

### `RemoteSandbox`（agent-server）

```rust
/// A Sandbox that delegates operations to a remote agent server via JSON-RPC/WebSocket.
pub struct RemoteSandbox {
    server_url: String,
    write_tx: mpsc::UnboundedSender<String>,
    inner: Arc<RemoteSandboxInner>,
    _bg: tokio::task::JoinHandle<()>,
}

struct RemoteSandboxInner {
    pending: std::sync::Mutex<HashMap<String, oneshot::Sender<AgentServerMessage>>>,
    msg_id_counter: AtomicU64,
}

impl RemoteSandbox {
    /// Connect to the agent server. Returns immediately if the server is reachable;
    /// errors if the WebSocket handshake fails.
    pub async fn connect(server_url: &str) -> SandboxResult<Self> {
        // 1. Connect via tokio_tungstenite
        // 2. Spawn background writer task (mpsc → ws.send)
        // 3. Spawn background reader task (ws.recv → decode → pending::remove → oneshot::send)
        // 4. Combined _bg task: tokio::select! on writer + reader
    }

    /// Send a JSON-RPC request and await the correlated response.
    async fn request(
        &self,
        op: SandboxOperation,
        payload: SandboxPayload,
    ) -> SandboxResult<AgentServerMessage> {
        let msg_id = self.inner.msg_id_counter
            .fetch_add(1, Ordering::Relaxed)
            .to_string();

        let msg = AgentServerMessage::new_command(
            &msg_id,
            "remote-sandbox", "server",
            Operation::Sandbox(op),
            Payload::Sandbox(payload),
        );

        let (tx, rx) = oneshot::channel();
        self.inner.pending.lock().unwrap().insert(msg_id, tx);

        let frame = encode_jsonrpc_message(&msg)
            .map_err(|e| SandboxError::Io(std::io::Error::other(e)))?;
        self.write_tx.send(frame)
            .map_err(|e| SandboxError::Io(std::io::Error::other(e)))?;

        tokio::time::timeout(Duration::from_secs(30), rx)
            .await
            .map_err(|_| SandboxError::Timeout(Duration::from_secs(30)))?
            .map_err(|_| SandboxError::Io(std::io::Error::other("request cancelled")))?
    }
}

#[async_trait]
impl Sandbox for RemoteSandbox {
    fn kind(&self) -> &str { "remote" }
    fn name(&self) -> &str { "remote" }

    async fn start(&self) -> SandboxResult<()> { Ok(()) }
    async fn cleanup(&self) -> SandboxResult<()> { Ok(()) }
    fn root_path(&self) -> &Path { Path::new("") } // not applicable for remote
    fn resolve_path(&self, rel: &str) -> SandboxResult<PathBuf> { Ok(PathBuf::from(rel)) } // delegated to server

    async fn execute(&self, req: CommandRequest) -> SandboxResult<CommandOutput> {
        let resp = self.request(
            SandboxOperation::Exec,
            SandboxPayload::Exec { command: req.into() },
        ).await?;
        match resp.payload {
            Payload::Sandbox(SandboxPayload::ExecResult { output }) => Ok(output.into()),
            _ => Err(SandboxError::Io(std::io::Error::other("unexpected response"))),
        }
    }

    async fn read_file(&self, path: &Path, offset: Option<u64>, limit: Option<u64>) -> SandboxResult<Vec<u8>> {
        let resp = self.request(
            SandboxOperation::ReadFile,
            SandboxPayload::ReadFile {
                path: path.to_string_lossy().to_string(),
                offset, limit,
            },
        ).await?;
        match resp.payload {
            Payload::Sandbox(SandboxPayload::ReadFileResult { content }) => {
                use base64::Engine;
                base64::engine::general_purpose::STANDARD
                    .decode(&content)
                    .map_err(|e| SandboxError::Io(std::io::Error::other(e)))
            }
            _ => Err(SandboxError::Io(std::io::Error::other("unexpected response"))),
        }
    }

    // write_file / create_dir_all / read_dir / metadata — 同理
}
```

## Data Flow

### Primary Flow: Remote Command Execution

```
1. Agent B 调用 RemoteSandbox::execute(CommandRequest { program: "cargo", args: ["build"], ... })

2. RemoteSandbox::execute():
   - CommandRequest → CommandRequestDef (into)
   - 构造 AgentServerMessage { message_id: "12345",
       operation: Operation::Sandbox(SandboxOperation::Exec),
       payload: Payload::Sandbox(SandboxPayload::Exec { command: ... }) }
   - (msg_id, oneshot::Sender) 插入 pending map
   - encode_jsonrpc_message() → JSON string
   - write_tx.send(json_frame)

3. Background writer task:
   - mpsc channel recv → ws.send(Message::Text(json_frame))

4. Agent Server A 的 JsonRpcConnection:
   - ws.recv → decode_jsonrpc_frame() → AgentServerMessage
   - HandlerRegistry::dispatch("sandbox.exec") → SandboxHandler::handle()

5. SandboxHandler::handle():
   - CommandRequestDef → CommandRequest (into)
   - LocalSandbox::execute(req).await → CommandOutput
   - CommandOutput → CommandOutputDef (into)
   - 返回 AgentServerMessage { kind: Result, payload: ExecResult { output: ... } }

6. JsonRpcConnection:
   - encode_jsonrpc_message() → JSON string
   - ws.send(result_frame)

7. RemoteSandbox background reader task:
   - ws.recv → decode_jsonrpc_frame() → AgentServerMessage
   - pending.lock().remove("12345") → oneshot::Sender
   - tx.send(response_msg)

8. RemoteSandbox::request():
   - rx.await → AgentServerMessage
   - ExecResult::output → CommandOutputDef → CommandOutput (into)
   - return Ok(CommandOutput { stdout: ..., stderr: ..., exit_code: 0, ... })

9. Agent B 得到 CommandOutput
```

## Edge Cases

| Edge Case | Handling |
|-----------|----------|
| Command timeout | `CommandRequest.timeout` propagated; `LocalSandbox::execute()` kills process group after timeout and returns `SandboxError::Timeout` |
| Large file I/O | `read_file` has offset/limit for chunked reads; base64 encoding/decoding handles binary payloads |
| Concurrent sandbox requests | Multiple agents share one `LocalSandbox`; OS process isolation naturally handles concurrent `Command::spawn()` |
| Path traversal | Server-side `Sandbox::resolve_path()` rejects `..` and absolute paths before any file operation |
| Agent server unreachable | `RemoteSandbox::connect()` fails immediately; subsequent requests after disconnect return IO errors |
| Network disconnect mid-request | Oneshot sender dropped → `rx.await` returns `Err(RecvError)` → mapped to IO error; caller decides to reconnect |
| Invalid payload for operation | Handler returns `AgentServerMessage::new_error()` with descriptive message |
| Empty command / stdin | OS-level spawn failure → `SandboxError::Io` |
| sandbox.list on default server | Returns single entry: `{ name: "local", kind: "local", root_path: "..." }` |

## Out of Scope

- Multi-sandbox routing by name — only one shared sandbox per agent server
- Sandbox lifecycle protocol operations (start/cleanup/snapshot/reset)
- Automatic reconnection in RemoteSandbox
- Streaming execution output (only request-response, no events)
- Sandbox resource quotas or rate limiting
- Per-sandbox filesystem namespace isolation

## Testing Strategy

### Unit Tests

| Component | What to Test | Location |
|-----------|-------------|----------|
| `SandboxOperation::method_name()` | All variants return correct dotted names | `vol-llm-agent-protocol` |
| `CommandRequestDef ↔ CommandRequest` round-trip | No data loss in conversion | `vol-llm-agent-protocol` |
| `SandboxPayload` serde | Request/result variants serialize/deserialize correctly | `vol-llm-agent-protocol` |
| `SandboxHandler::operations()` | Returns correct operation list for registry validation | `vol-agent-server` |
| `SandboxHandler` dispatch | Each (operation, payload) pair reaches correct branch | `vol-agent-server` (in-memory `MemoryConnection`) |

### Integration Tests

| Scenario | Description | Location |
|----------|-------------|----------|
| `sandbox.list` round-trip | RemoteSandbox connects to server, calls list, verifies single "local" entry | `vol-agent-server/tests/` |
| `sandbox.exec` round-trip | RemoteSandbox sends `echo hello`, receives stdout="hello", exit_code=0 | `vol-agent-server/tests/` |
| `sandbox.read_file` / `sandbox.write_file` | Write file via RemoteSandbox.write_file, verify via RemoteSandbox.read_file | `vol-agent-server/tests/` |
| `sandbox.create_dir` / `sandbox.read_dir` | Create dir, read dir, verify entries | `vol-agent-server/tests/` |
| `sandbox.metadata` | Query file metadata, verify size > 0, correct FileType | `vol-agent-server/tests/` |
| Command timeout | Execute long-running command, verify timeout error | `vol-agent-server/tests/` |
| Path traversal rejection | Attempt `read_file("../etc/passwd")`, verify error | `vol-agent-server/tests/` |
| Concurrency | Two concurrent RemoteSandbox connections both execute commands successfully | `vol-agent-server/tests/` |

Integration tests use the in-memory server pattern: start `DataPlaneServerCore` with `MemoryConnection`, connect `RemoteSandbox` (with a test-mode path that accepts `MemoryConnection` as transport instead of WebSocket).

### E2E Test

| Scenario | Description |
|----------|-------------|
| Coding agent via RemoteSandbox | Agent constructs `CommandRequest` for `cargo build`, sends via RemoteSandbox to local server, verifies build output in response |
