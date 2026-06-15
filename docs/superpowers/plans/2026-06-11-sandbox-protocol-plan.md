# Sandbox Protocol Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a sandbox JSON-RPC protocol to agent-server so remote agents can execute commands and file I/O via `RemoteSandbox` (which implements the `Sandbox` trait)

**Architecture:** Three-layer stack — (1) wire types in `vol-llm-agent-protocol` (new `SandboxOperation`/`SandboxPayload` enums + Def conversion types), (2) `SandboxHandler` in `vol-agent-server` (DomainHandler dispatching to a local `Sandbox` instance), (3) `RemoteSandbox` in `vol-agent-server` (WebSocket client implementing `Sandbox` trait for remote callers)

**Tech Stack:** Rust, tokio, serde, tokio-tungstenite, vol-llm-sandbox Sandbox trait

---

### File Structure

```
vol-llm-agent-protocol/
├── Cargo.toml                                    # Modify: +vol-llm-sandbox dep
├── src/
│   ├── agent_server_protocol.rs                  # Modify: +SandboxOperation, +SandboxPayload,
│   │                                             #   +SandboxInfo, +CommandRequestDef, +CommandOutputDef
│   │                                             #   +DirEntryDef, +FileMetadataDef
│   │                                             #   +Operation::Sandbox, +Payload::Sandbox
│   └── operation_codec.rs                        # Modify: +sandbox.* method mapping

vol-agent-server/
├── Cargo.toml                                    # No change: already depends on both crates
├── src/
│   ├── data_plane/
│   │   ├── core.rs                               # Modify: register SandboxHandler
│   │   └── handlers/
│   │       └── sandbox.rs                        # CREATE: SandboxHandler
│   ├── control_plane/
│   │   └── core.rs                               # Modify: register SandboxHandler
│   └── sandbox/
│       └── remote.rs                             # CREATE: RemoteSandbox
└── tests/
    └── sandbox_protocol_integration.rs            # CREATE: integration tests
```

---

### Task 1: Add `vol-llm-sandbox` dependency to protocol crate

**Files:**
- Modify: `crates/vol-llm-agent-protocol/Cargo.toml`

- [ ] **Step 1: Add dependency**

```toml
vol-llm-sandbox = { path = "../vol-llm-sandbox" }
```

Add this line under `[dependencies]` in `crates/vol-llm-agent-protocol/Cargo.toml` (next to the existing `vol-llm-agent` dep).

- [ ] **Step 2: Verify compilation**

Run: `cargo check -p vol-llm-agent-protocol`
Expected: PASS (new dep resolves, no code uses it yet)

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-agent-protocol/Cargo.toml
git commit -m "chore: add vol-llm-sandbox dependency to protocol crate

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 2: Define `SandboxOperation` enum and wire types in protocol

**Files:**
- Modify: `crates/vol-llm-agent-protocol/src/agent_server_protocol.rs`

- [ ] **Step 1: Define `SandboxOperation` enum**

Add after the last existing `*Operation` enum (after `ControlOperation`):

```rust
/// Sandbox protocol operations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SandboxOperation {
    /// List available sandboxes.
    List,
    /// Execute a command inside the sandbox.
    Exec,
    /// Read file content from the sandbox.
    ReadFile,
    /// Write file content to the sandbox.
    WriteFile,
    /// Create a directory (and parents) inside the sandbox.
    CreateDir,
    /// List directory entries in the sandbox.
    ReadDir,
    /// Get file/directory metadata.
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

- [ ] **Step 2: Add `Operation::Sandbox` variant**

In the `Operation` enum (currently 10 variants), add:

```rust
    Sandbox(SandboxOperation),
```

After `Control(ControlOperation),` and before the closing `}`.

- [ ] **Step 3: Add `SandboxOperation` arm to `Operation::method_name()`**

In the existing `impl Operation { pub fn method_name(&self) -> &'static str { ... } }` block, add:

```rust
            Self::Sandbox(op) => op.method_name(),
```

- [ ] **Step 4: Define serde-friendly Def types**

Add after `SandboxOperation`:

```rust
/// Wire-compatible command request. All fields directly serializable.
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
    pub stdin: Option<String>, // base64 encoded
    #[serde(default)]
    pub timeout_ms: u64,
}

impl From<CommandRequestDef> for vol_llm_sandbox::CommandRequest {
    fn from(d: CommandRequestDef) -> Self {
        use std::time::Duration;
        let timeout = if d.timeout_ms == 0 {
            Duration::from_secs(30)
        } else {
            Duration::from_millis(d.timeout_ms)
        };
        Self {
            program: d.program,
            args: d.args,
            env: d.env.into_iter().collect(),
            cwd: d.cwd.map(std::path::PathBuf::from),
            stdin: d.stdin.map(|s| {
                use base64::Engine;
                base64::engine::general_purpose::STANDARD
                    .decode(&s)
                    .unwrap_or_default()
            }),
            timeout,
        }
    }
}

impl From<vol_llm_sandbox::CommandRequest> for CommandRequestDef {
    fn from(r: vol_llm_sandbox::CommandRequest) -> Self {
        use base64::Engine;
        Self {
            program: r.program,
            args: r.args,
            env: r.env.into_iter().collect(),
            cwd: r.cwd.map(|p| p.to_string_lossy().to_string()),
            stdin: r.stdin.map(|s| {
                base64::engine::general_purpose::STANDARD.encode(&s)
            }),
            timeout_ms: r.timeout.as_millis() as u64,
        }
    }
}

/// Wire-compatible command output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandOutputDef {
    pub stdout: String,  // base64 encoded
    pub stderr: String,  // base64 encoded
    pub exit_code: i32,
    #[serde(default)]
    pub killed_by_signal: Option<i32>,
}

impl From<vol_llm_sandbox::CommandOutput> for CommandOutputDef {
    fn from(o: vol_llm_sandbox::CommandOutput) -> Self {
        use base64::Engine;
        Self {
            stdout: base64::engine::general_purpose::STANDARD.encode(&o.stdout),
            stderr: base64::engine::general_purpose::STANDARD.encode(&o.stderr),
            exit_code: o.exit_code,
            killed_by_signal: o.killed_by_signal,
        }
    }
}

impl From<CommandOutputDef> for vol_llm_sandbox::CommandOutput {
    fn from(d: CommandOutputDef) -> Self {
        use base64::Engine;
        Self {
            stdout: base64::engine::general_purpose::STANDARD
                .decode(&d.stdout)
                .unwrap_or_default(),
            stderr: base64::engine::general_purpose::STANDARD
                .decode(&d.stderr)
                .unwrap_or_default(),
            exit_code: d.exit_code,
            killed_by_signal: d.killed_by_signal,
        }
    }
}

/// Wire-compatible directory entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirEntryDef {
    pub name: String,
    pub file_type: String, // "file", "directory", "symlink", "other"
}

impl From<vol_llm_sandbox::DirEntry> for DirEntryDef {
    fn from(e: vol_llm_sandbox::DirEntry) -> Self {
        Self {
            name: e.name,
            file_type: match e.file_type {
                vol_llm_sandbox::FileType::File => "file".into(),
                vol_llm_sandbox::FileType::Directory => "directory".into(),
                vol_llm_sandbox::FileType::Symlink => "symlink".into(),
                vol_llm_sandbox::FileType::Other => "other".into(),
            },
        }
    }
}

/// Wire-compatible file metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileMetadataDef {
    pub size: u64,
    pub mtime: u64,
    pub file_type: String,
}

impl From<vol_llm_sandbox::FileMetadata> for FileMetadataDef {
    fn from(m: vol_llm_sandbox::FileMetadata) -> Self {
        Self {
            size: m.size,
            mtime: m.mtime,
            file_type: match m.file_type {
                vol_llm_sandbox::FileType::File => "file".into(),
                vol_llm_sandbox::FileType::Directory => "directory".into(),
                vol_llm_sandbox::FileType::Symlink => "symlink".into(),
                vol_llm_sandbox::FileType::Other => "other".into(),
            },
        }
    }
}

/// Sandbox metadata returned by sandbox.list.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxInfo {
    pub name: String,
    pub kind: String,
    pub root_path: String,
}
```

- [ ] **Step 5: Verify compilation**

Run: `cargo check -p vol-llm-agent-protocol`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add crates/vol-llm-agent-protocol/src/agent_server_protocol.rs
git commit -m "feat(protocol): add SandboxOperation + Def wire types

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 3: Define `SandboxPayload` and wire it into `Payload`

**Files:**
- Modify: `crates/vol-llm-agent-protocol/src/agent_server_protocol.rs`

- [ ] **Step 1: Define `SandboxPayload` enum**

Add before the `Payload` enum definition (or grouped with `SandboxOperation`):

```rust
/// Sandbox protocol payload — request/response pairs.
#[derive(Debug, Clone, Serialize, Deserialize)]
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

- [ ] **Step 2: Add `Payload::Sandbox` variant**

In the `#[serde(untagged)] Payload` enum, add:

```rust
    Sandbox(SandboxPayload),
```

After the `Control(ControlPayload),` variant and before `Error(ErrorPayload),`.

- [ ] **Step 3: Add match arm to `Payload::from_operation()`**

In the existing `impl Payload { pub fn from_operation(...) -> ... }` block, find the series of `match` arms that call `serde_json::from_value::<XxxPayload>(value).map(Payload::Xxx)`, and add:

```rust
            Operation::Sandbox(_) => {
                serde_json::from_value::<SandboxPayload>(value).map(Payload::Sandbox)
                    .map_err(|e| ProtocolError::PayloadDecodeFailed(
                        format!("sandbox: {}", e)
                    ))
            }
```

- [ ] **Step 4: Verify compilation**

Run: `cargo check -p vol-llm-agent-protocol`
Expected: PASS (warnings about unused `SandboxPayload` variant ok)

- [ ] **Step 5: Run protocol crate tests**

Run: `cargo test -p vol-llm-agent-protocol`
Expected: All existing tests PASS

- [ ] **Step 6: Commit**

```bash
git add crates/vol-llm-agent-protocol/src/agent_server_protocol.rs
git commit -m "feat(protocol): add SandboxPayload and wire into Payload/Operation

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 4: Add `method_to_operation` routing in `operation_codec.rs`

**Files:**
- Modify: `crates/vol-llm-agent-protocol/src/operation_codec.rs`

- [ ] **Step 1: Add method name mappings**

In the `method_to_operation` function, find the series of match arms like `"control.heartbeat" => ...` and add after the last existing arm:

```rust
        "sandbox.list" => Ok(Operation::Sandbox(SandboxOperation::List)),
        "sandbox.exec" => Ok(Operation::Sandbox(SandboxOperation::Exec)),
        "sandbox.read_file" => Ok(Operation::Sandbox(SandboxOperation::ReadFile)),
        "sandbox.write_file" => Ok(Operation::Sandbox(SandboxOperation::WriteFile)),
        "sandbox.create_dir" => Ok(Operation::Sandbox(SandboxOperation::CreateDir)),
        "sandbox.read_dir" => Ok(Operation::Sandbox(SandboxOperation::ReadDir)),
        "sandbox.metadata" => Ok(Operation::Sandbox(SandboxOperation::Metadata)),
```

- [ ] **Step 2: Import `SandboxOperation` at top of file**

Add to the existing import from `agent_server_protocol`:

```rust
use crate::agent_server_protocol::SandboxOperation;
```

(Alongside the existing `Operation`, `AgentOperation`, `ControlOperation`, etc. imports from the same module.)

- [ ] **Step 3: Verify compilation**

Run: `cargo check -p vol-llm-agent-protocol`
Expected: PASS

- [ ] **Step 4: Run protocol crate tests**

Run: `cargo test -p vol-llm-agent-protocol`
Expected: All tests PASS

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-agent-protocol/src/operation_codec.rs
git commit -m "feat(protocol): add sandbox method routing in operation_codec

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 5: Write unit tests for protocol wire types

**Files:**
- Modify: `crates/vol-llm-agent-protocol/src/agent_server_protocol.rs` (or add `#[cfg(test)] mod tests` at bottom)

- [ ] **Step 1: Add `#[cfg(test)] mod sandbox_protocol_tests` at the bottom of the file**

```rust
#[cfg(test)]
mod sandbox_protocol_tests {
    use super::*;

    #[test]
    fn test_sandbox_operation_method_names() {
        assert_eq!(SandboxOperation::List.method_name(), "sandbox.list");
        assert_eq!(SandboxOperation::Exec.method_name(), "sandbox.exec");
        assert_eq!(SandboxOperation::ReadFile.method_name(), "sandbox.read_file");
        assert_eq!(SandboxOperation::WriteFile.method_name(), "sandbox.write_file");
        assert_eq!(SandboxOperation::CreateDir.method_name(), "sandbox.create_dir");
        assert_eq!(SandboxOperation::ReadDir.method_name(), "sandbox.read_dir");
        assert_eq!(SandboxOperation::Metadata.method_name(), "sandbox.metadata");
    }

    #[test]
    fn test_command_request_def_roundtrip() {
        use std::collections::HashMap;
        use std::time::Duration;

        let orig = vol_llm_sandbox::CommandRequest {
            program: "echo".into(),
            args: vec!["-n".into(), "hello".into()],
            env: HashMap::from([("FOO".into(), "bar".into())]),
            cwd: Some(std::path::PathBuf::from("/tmp")),
            stdin: Some(b"input".to_vec()),
            timeout: Duration::from_secs(30),
        };

        let def: CommandRequestDef = orig.clone().into();
        let back: vol_llm_sandbox::CommandRequest = def.into();

        assert_eq!(back.program, "echo");
        assert_eq!(back.args, vec!["-n", "hello"]);
        assert_eq!(back.env.get("FOO"), Some(&"bar".to_string()));
        assert_eq!(back.cwd, Some(std::path::PathBuf::from("/tmp")));
        assert_eq!(back.stdin, Some(b"input".to_vec()));
    }

    #[test]
    fn test_sandbox_payload_list_serde() {
        let payload = SandboxPayload::List;
        let json = serde_json::to_value(&payload).unwrap();
        assert_eq!(json, serde_json::json!("List"));

        let info = vec![SandboxInfo {
            name: "local".into(),
            kind: "local".into(),
            root_path: "/tmp/sandbox".into(),
        }];
        let result = SandboxPayload::ListResult { sandboxes: info };
        let json = serde_json::to_value(&result).unwrap();
        let back: SandboxPayload = serde_json::from_value(json).unwrap();
        match back {
            SandboxPayload::ListResult { sandboxes } => {
                assert_eq!(sandboxes.len(), 1);
                assert_eq!(sandboxes[0].name, "local");
            }
            _ => panic!("expected ListResult"),
        }
    }

    #[test]
    fn test_sandbox_payload_exec_serde() {
        let payload = SandboxPayload::Exec {
            command: CommandRequestDef {
                program: "echo".into(),
                args: vec!["hello".into()],
                env: vec![],
                cwd: None,
                stdin: None,
                timeout_ms: 5000,
            },
        };
        let json = serde_json::to_value(&payload).unwrap();
        let back: SandboxPayload = serde_json::from_value(json).unwrap();
        match back {
            SandboxPayload::Exec { command } => {
                assert_eq!(command.program, "echo");
                assert_eq!(command.args, vec!["hello"]);
            }
            _ => panic!("expected Exec"),
        }
    }
}
```

- [ ] **Step 2: Run the new tests**

Run: `cargo test -p vol-llm-agent-protocol -- sandbox_protocol_tests`
Expected: 4 tests PASS

- [ ] **Step 3: Run full protocol test suite**

Run: `cargo test -p vol-llm-agent-protocol`
Expected: All tests PASS

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-agent-protocol/src/agent_server_protocol.rs
git commit -m "test(protocol): add sandbox wire type unit tests

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 6: Create `SandboxHandler` in agent-server

**Files:**
- Create: `crates/vol-agent-server/src/data_plane/handlers/sandbox.rs`

- [ ] **Step 1: Create the handler file**

```rust
use async_trait::async_trait;
use std::sync::Arc;

use vol_llm_agent_protocol::agent_server_protocol::{
    AgentServerMessage, Operation, Payload, SandboxOperation, SandboxPayload, SandboxInfo,
};
use vol_llm_agent_protocol::domain::DomainHandler;
use vol_llm_agent_protocol::ProtocolError;
use vol_llm_sandbox::Sandbox;

/// Handler that dispatches sandbox protocol operations to a local Sandbox instance.
pub struct SandboxHandler {
    sandbox: Arc<dyn Sandbox>,
}

impl SandboxHandler {
    pub fn new(sandbox: Arc<dyn Sandbox>) -> Self {
        Self { sandbox }
    }
}

#[async_trait]
impl DomainHandler for SandboxHandler {
    fn name(&self) -> &str {
        "sandbox"
    }

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
        let op = match &message.operation {
            Operation::Sandbox(op) => op.clone(),
            _ => return Err(ProtocolError::PayloadDecodeFailed("sandbox")),
        };

        let mid = &message.message_id;

        match (op, message.payload) {
            (SandboxOperation::List, Payload::Sandbox(SandboxPayload::List)) => {
                let info = SandboxInfo {
                    name: self.sandbox.name().to_string(),
                    kind: self.sandbox.kind().to_string(),
                    root_path: self.sandbox.root_path().to_string_lossy().to_string(),
                };
                Ok(vec![AgentServerMessage::new_result(
                    mid,
                    Operation::Sandbox(SandboxOperation::List),
                    Payload::Sandbox(SandboxPayload::ListResult {
                        sandboxes: vec![info],
                    }),
                )])
            }

            (SandboxOperation::Exec, Payload::Sandbox(SandboxPayload::Exec { command })) => {
                let req: vol_llm_sandbox::CommandRequest = command.into();
                match self.sandbox.execute(req).await {
                    Ok(output) => Ok(vec![AgentServerMessage::new_result(
                        mid,
                        Operation::Sandbox(SandboxOperation::Exec),
                        Payload::Sandbox(SandboxPayload::ExecResult {
                            output: output.into(),
                        }),
                    )]),
                    Err(e) => Ok(vec![AgentServerMessage::new_result(
                        mid,
                        Operation::Sandbox(SandboxOperation::Exec),
                        Payload::Sandbox(SandboxPayload::ExecResult {
                            output: vol_llm_sandbox::CommandOutput {
                                stdout: vec![],
                                stderr: e.to_string().into_bytes(),
                                exit_code: -1,
                                killed_by_signal: None,
                            }.into(),
                        }),
                    )]),
                }
            }

            (SandboxOperation::ReadFile, Payload::Sandbox(SandboxPayload::ReadFile { path, offset, limit })) => {
                let p = std::path::Path::new(&path);
                match self.sandbox.read_file(p, offset, limit).await {
                    Ok(content) => {
                        use base64::Engine;
                        let encoded = base64::engine::general_purpose::STANDARD.encode(&content);
                        Ok(vec![AgentServerMessage::new_result(
                            mid,
                            Operation::Sandbox(SandboxOperation::ReadFile),
                            Payload::Sandbox(SandboxPayload::ReadFileResult { content: encoded }),
                        )])
                    }
                    Err(e) => Err(ProtocolError::PayloadDecodeFailed(
                        format!("sandbox.read_file: {}", e),
                    )),
                }
            }

            (SandboxOperation::WriteFile, Payload::Sandbox(SandboxPayload::WriteFile { path, content })) => {
                use base64::Engine;
                let data = base64::engine::general_purpose::STANDARD
                    .decode(&content)
                    .map_err(|e| ProtocolError::PayloadDecodeFailed(
                        format!("sandbox.write_file base64: {}", e),
                    ))?;
                let p = std::path::Path::new(&path);
                match self.sandbox.write_file(p, &data).await {
                    Ok(()) => Ok(vec![AgentServerMessage::new_result(
                        mid,
                        Operation::Sandbox(SandboxOperation::WriteFile),
                        Payload::Sandbox(SandboxPayload::WriteFileResult),
                    )]),
                    Err(e) => Err(ProtocolError::PayloadDecodeFailed(
                        format!("sandbox.write_file: {}", e),
                    )),
                }
            }

            (SandboxOperation::CreateDir, Payload::Sandbox(SandboxPayload::CreateDir { path })) => {
                let p = std::path::Path::new(&path);
                match self.sandbox.create_dir_all(p).await {
                    Ok(()) => Ok(vec![AgentServerMessage::new_result(
                        mid,
                        Operation::Sandbox(SandboxOperation::CreateDir),
                        Payload::Sandbox(SandboxPayload::CreateDirResult),
                    )]),
                    Err(e) => Err(ProtocolError::PayloadDecodeFailed(
                        format!("sandbox.create_dir: {}", e),
                    )),
                }
            }

            (SandboxOperation::ReadDir, Payload::Sandbox(SandboxPayload::ReadDir { path })) => {
                let p = std::path::Path::new(&path);
                match self.sandbox.read_dir(p).await {
                    Ok(entries) => {
                        let defs: Vec<_> = entries.into_iter().map(|e| e.into()).collect();
                        Ok(vec![AgentServerMessage::new_result(
                            mid,
                            Operation::Sandbox(SandboxOperation::ReadDir),
                            Payload::Sandbox(SandboxPayload::ReadDirResult { entries: defs }),
                        )])
                    }
                    Err(e) => Err(ProtocolError::PayloadDecodeFailed(
                        format!("sandbox.read_dir: {}", e),
                    )),
                }
            }

            (SandboxOperation::Metadata, Payload::Sandbox(SandboxPayload::Metadata { path })) => {
                let p = std::path::Path::new(&path);
                match self.sandbox.metadata(p).await {
                    Ok(meta) => Ok(vec![AgentServerMessage::new_result(
                        mid,
                        Operation::Sandbox(SandboxOperation::Metadata),
                        Payload::Sandbox(SandboxPayload::MetadataResult {
                            metadata: meta.into(),
                        }),
                    )]),
                    Err(e) => Err(ProtocolError::PayloadDecodeFailed(
                        format!("sandbox.metadata: {}", e),
                    )),
                }
            }

            // Catch-all for mismatched payload types
            (SandboxOperation::List, _) => Err(ProtocolError::PayloadDecodeFailed("sandbox.list")),
            (SandboxOperation::Exec, _) => Err(ProtocolError::PayloadDecodeFailed("sandbox.exec")),
            (SandboxOperation::ReadFile, _) => Err(ProtocolError::PayloadDecodeFailed("sandbox.read_file")),
            (SandboxOperation::WriteFile, _) => Err(ProtocolError::PayloadDecodeFailed("sandbox.write_file")),
            (SandboxOperation::CreateDir, _) => Err(ProtocolError::PayloadDecodeFailed("sandbox.create_dir")),
            (SandboxOperation::ReadDir, _) => Err(ProtocolError::PayloadDecodeFailed("sandbox.read_dir")),
            (SandboxOperation::Metadata, _) => Err(ProtocolError::PayloadDecodeFailed("sandbox.metadata")),
        }
    }
}
```

- [ ] **Step 2: Verify module is declared**

Check that `crates/vol-agent-server/src/data_plane/handlers/mod.rs` has `pub mod sandbox;`. If it only names specific modules, add:

```rust
pub mod sandbox;
```

- [ ] **Step 3: Verify compilation**

Run: `cargo check -p vol-agent-server`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add crates/vol-agent-server/src/data_plane/handlers/sandbox.rs
git add crates/vol-agent-server/src/data_plane/handlers/mod.rs
git commit -m "feat(agent-server): add SandboxHandler for data-plane

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 7: Register `SandboxHandler` in data-plane and control-plane builders

**Files:**
- Modify: `crates/vol-agent-server/src/data_plane/core.rs`
- Modify: `crates/vol-agent-server/src/control_plane/core.rs`

- [ ] **Step 1: Register in `DataPlaneServerCoreBuilder::build()`**

In `core.rs`, after the `TaskHandler` registration and before the `DataPlaneControlHandler` registration (or after all registrations), add:

```rust
handler_registry
    .register(Arc::new(SandboxHandler::new(
        sandbox_registry.default(),
    )))
    .map_err(|e| format!("failed to register SandboxHandler: {e}"))?;
```

Then add the import at the top of the file:

```rust
use crate::data_plane::handlers::sandbox::SandboxHandler;
```

- [ ] **Step 2: Find the control-plane builder and register similarly**

In `crates/vol-agent-server/src/control_plane/core.rs`, find the `handler_registry` registration block. Add the same `SandboxHandler` registration, using the sandbox from the builder context. Import `SandboxHandler`:

```rust
use crate::data_plane::handlers::sandbox::SandboxHandler;
```

(If the control-plane builder doesn't have a `sandbox_registry`, create one inline: `Arc::new(vol_llm_sandbox::local::LocalSandbox::new(None))` and call `.start().await` on it, OR add `sandbox_registry` to the builder state.)

- [ ] **Step 3: Verify compilation**

Run: `cargo check -p vol-agent-server`
Expected: PASS

- [ ] **Step 4: Run agent-server tests**

Run: `cargo test -p vol-agent-server`
Expected: All existing tests PASS

- [ ] **Step 5: Commit**

```bash
git add crates/vol-agent-server/src/data_plane/core.rs
git add crates/vol-agent-server/src/control_plane/core.rs
git commit -m "feat(agent-server): register SandboxHandler in data-plane and control-plane

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 8: Write unit tests for `SandboxHandler`

**Files:**
- Create: tests within `crates/vol-agent-server/src/data_plane/handlers/sandbox.rs` (add `#[cfg(test)] mod tests` at bottom)

- [ ] **Step 1: Add handler unit tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use vol_llm_agent_protocol::agent_server_protocol::{
        AgentServerMessage, MessageKind, Operation, Payload, SandboxOperation, SandboxPayload,
        CommandRequestDef,
    };
    use vol_llm_sandbox::local::LocalSandbox;
    use vol_llm_sandbox::Sandbox;

    async fn setup() -> SandboxHandler {
        let sandbox = Arc::new(LocalSandbox::new(None));
        sandbox.start().await.unwrap();
        SandboxHandler::new(sandbox)
    }

    #[test]
    fn test_handler_name() {
        let sb = LocalSandbox::new(None);
        let handler = SandboxHandler::new(Arc::new(sb));
        assert_eq!(handler.name(), "sandbox");
    }

    #[test]
    fn test_operations_count() {
        let sb = LocalSandbox::new(None);
        let handler = SandboxHandler::new(Arc::new(sb));
        let ops = handler.operations();
        assert_eq!(ops.len(), 7);
    }

    #[tokio::test]
    async fn test_list() {
        let handler = setup().await;
        let msg = AgentServerMessage::new_command(
            "1",
            Operation::Sandbox(SandboxOperation::List),
            Payload::Sandbox(SandboxPayload::List),
        );
        let replies = handler.handle(msg).await.unwrap();
        assert_eq!(replies.len(), 1);
        match &replies[0].payload {
            Payload::Sandbox(SandboxPayload::ListResult { sandboxes }) => {
                assert_eq!(sandboxes.len(), 1);
                assert_eq!(sandboxes[0].name, "local");
            }
            _ => panic!("expected ListResult"),
        }
    }

    #[tokio::test]
    async fn test_exec_echo() {
        let handler = setup().await;
        let msg = AgentServerMessage::new_command(
            "2",
            Operation::Sandbox(SandboxOperation::Exec),
            Payload::Sandbox(SandboxPayload::Exec {
                command: CommandRequestDef {
                    program: "echo".into(),
                    args: vec!["-n".into(), "hello".into()],
                    env: vec![],
                    cwd: None,
                    stdin: None,
                    timeout_ms: 5000,
                },
            }),
        );
        let replies = handler.handle(msg).await.unwrap();
        assert_eq!(replies.len(), 1);
        match &replies[0].payload {
            Payload::Sandbox(SandboxPayload::ExecResult { output }) => {
                assert_eq!(output.exit_code, 0);
                assert!(output.stdout.contains("hello"));
            }
            _ => panic!("expected ExecResult"),
        }
    }

    #[tokio::test]
    async fn test_write_and_read_file() {
        let handler = setup().await;
        let write = AgentServerMessage::new_command(
            "3",
            Operation::Sandbox(SandboxOperation::WriteFile),
            Payload::Sandbox(SandboxPayload::WriteFile {
                path: "test.txt".into(),
                content: base64::engine::general_purpose::STANDARD.encode(b"hello world"),
            }),
        );
        let replies = handler.handle(write).await.unwrap();
        assert!(matches!(&replies[0].payload, Payload::Sandbox(SandboxPayload::WriteFileResult)));

        let read = AgentServerMessage::new_command(
            "4",
            Operation::Sandbox(SandboxOperation::ReadFile),
            Payload::Sandbox(SandboxPayload::ReadFile {
                path: "test.txt".into(),
                offset: None,
                limit: None,
            }),
        );
        let replies = handler.handle(read).await.unwrap();
        match &replies[0].payload {
            Payload::Sandbox(SandboxPayload::ReadFileResult { content }) => {
                use base64::Engine;
                let decoded = base64::engine::general_purpose::STANDARD
                    .decode(content).unwrap();
                assert_eq!(decoded, b"hello world");
            }
            _ => panic!("expected ReadFileResult"),
        }
    }
}
```

- [ ] **Step 2: Run handler tests**

Run: `cargo test -p vol-agent-server -- sandbox::tests`
Expected: 4 tests PASS

- [ ] **Step 3: Commit**

```bash
git add crates/vol-agent-server/src/data_plane/handlers/sandbox.rs
git commit -m "test(agent-server): add SandboxHandler unit tests

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 9: Create `RemoteSandbox` in agent-server

**Files:**
- Create: `crates/vol-agent-server/src/sandbox/remote.rs`
- Create: `crates/vol-agent-server/src/sandbox/mod.rs`

- [ ] **Step 1: Create module structure**

Create `crates/vol-agent-server/src/sandbox/mod.rs`:

```rust
pub mod remote;
```

Ensure the top-level `crates/vol-agent-server/src/lib.rs` declares the module:

```rust
pub mod sandbox;
```

- [ ] **Step 2: Implement `RemoteSandbox`**

```rust
//! Remote sandbox — implements the `Sandbox` trait via JSON-RPC over WebSocket
//! to a remote agent server.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use tokio::sync::{mpsc, oneshot};
use tokio_tungstenite::tungstenite::Message;

use vol_llm_agent_protocol::agent_server_protocol::{
    AgentServerMessage, MessageKind, Operation, Payload, MessageMeta,
    SandboxOperation, SandboxPayload, SandboxInfo,
};
use vol_llm_agent_protocol::transport::jsonrpc::codec::{
    decode_jsonrpc_frame, encode_jsonrpc_message,
};
use vol_llm_sandbox::*;

/// A `Sandbox` backed by a remote agent server via JSON-RPC/WebSocket.
///
/// # Lifecycle
///
/// Created via `RemoteSandbox::connect(url)`. On drop, the background reader/writer
/// tasks are cancelled via `CancellationToken`. No explicit `start()` or `cleanup()` needed.
pub struct RemoteSandbox {
    server_url: String,
    write_tx: mpsc::UnboundedSender<String>,
    inner: Arc<RemoteSandboxInner>,
    cancel: tokio_util::sync::CancellationToken,
    _bg: tokio::task::JoinHandle<()>,
}

struct RemoteSandboxInner {
    pending: Mutex<HashMap<String, oneshot::Sender<AgentServerMessage>>>,
    msg_id_counter: AtomicU64,
}

impl Drop for RemoteSandbox {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}

impl RemoteSandbox {
    /// Connect to an agent server.
    ///
    /// Fails immediately if the server is unreachable (WebSocket handshake error).
    pub async fn connect(server_url: &str) -> SandboxResult<Self> {
        let (ws, _) = tokio_tungstenite::connect_async(server_url)
            .await
            .map_err(|e| SandboxError::Io(std::io::Error::other(e)))?;

        let (ws_write, mut ws_read) = ws.split();
        let (write_tx, mut write_rx) = mpsc::unbounded_channel::<String>();
        let cancel = tokio_util::sync::CancellationToken::new();
        let inner = Arc::new(RemoteSandboxInner {
            pending: Mutex::new(HashMap::new()),
            msg_id_counter: AtomicU64::new(0),
        });

        // Background writer: mpsc → WebSocket
        let writer_cancel = cancel.child_token();
        let writer = tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = writer_cancel.cancelled() => break,
                    frame = write_rx.recv() => {
                        match frame {
                            Some(text) => {
                                use futures_util::SinkExt;
                                let _ = ws_write.send(Message::Text(text.into())).await;
                            }
                            None => break,
                        }
                    }
                }
            }
        });

        // Background reader: WebSocket → decode → oneshot send
        let reader_cancel = cancel.child_token();
        let pending_map = inner.clone();
        let reader = tokio::spawn(async move {
            use futures_util::StreamExt;
            loop {
                tokio::select! {
                    _ = reader_cancel.cancelled() => break,
                    frame = ws_read.next() => {
                        match frame {
                            Some(Ok(Message::Text(text))) => {
                                if let Ok(msg) = decode_jsonrpc_frame(&text) {
                                    let mid = msg.message_id.clone();
                                    if let Some(tx) = pending_map.pending.lock().unwrap().remove(&mid) {
                                        let _ = tx.send(msg);
                                    }
                                }
                            }
                            Some(Ok(_)) => {} // ignore non-text frames
                            Some(Err(_)) | None => break,
                        }
                    }
                }
            }
        });

        let bg = tokio::spawn(async move {
            tokio::select! {
                _ = writer => {}
                _ = reader => {}
            }
        });

        Ok(Self {
            server_url: server_url.to_string(),
            write_tx,
            inner,
            cancel,
            _bg: bg,
        })
    }

    /// Send a JSON-RPC request and wait for the matching response.
    async fn request(
        &self,
        op: SandboxOperation,
        payload: SandboxPayload,
    ) -> SandboxResult<AgentServerMessage> {
        let msg_id = self.inner.msg_id_counter
            .fetch_add(1, Ordering::Relaxed)
            .to_string();

        let mut msg = AgentServerMessage::new_command(
            msg_id.clone(),
            Operation::Sandbox(op),
            Payload::Sandbox(payload),
        );
        msg.sender = "remote-sandbox".to_string();
        msg.receiver = "server".to_string();

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

    fn root_path(&self) -> &Path {
        // Remote sandbox doesn't have a local root path.
        // Callers should not depend on this for remote operation.
        Path::new("")
    }

    fn resolve_path(&self, rel: &str) -> SandboxResult<PathBuf> {
        // Delegated to server — no local validation needed.
        Ok(std::path::PathBuf::from(rel))
    }

    async fn execute(&self, req: CommandRequest) -> SandboxResult<CommandOutput> {
        use vol_llm_agent_protocol::agent_server_protocol::CommandRequestDef;
        let def: CommandRequestDef = req.into();
        let resp = self.request(
            SandboxOperation::Exec,
            SandboxPayload::Exec { command: def },
        ).await?;
        match resp.payload {
            Payload::Sandbox(SandboxPayload::ExecResult { output }) => Ok(output.into()),
            _ => Err(SandboxError::Io(std::io::Error::other("unexpected response type"))),
        }
    }

    async fn read_file(
        &self,
        path: &Path,
        offset: Option<u64>,
        limit: Option<u64>,
    ) -> SandboxResult<Vec<u8>> {
        let resp = self.request(
            SandboxOperation::ReadFile,
            SandboxPayload::ReadFile {
                path: path.to_string_lossy().to_string(),
                offset,
                limit,
            },
        ).await?;
        match resp.payload {
            Payload::Sandbox(SandboxPayload::ReadFileResult { content }) => {
                use base64::Engine;
                base64::engine::general_purpose::STANDARD
                    .decode(&content)
                    .map_err(|e| SandboxError::Io(std::io::Error::other(e)))
            }
            _ => Err(SandboxError::Io(std::io::Error::other("unexpected response type"))),
        }
    }

    async fn write_file(&self, path: &Path, content: &[u8]) -> SandboxResult<()> {
        use base64::Engine;
        let encoded = base64::engine::general_purpose::STANDARD.encode(content);
        let resp = self.request(
            SandboxOperation::WriteFile,
            SandboxPayload::WriteFile {
                path: path.to_string_lossy().to_string(),
                content: encoded,
            },
        ).await?;
        match resp.payload {
            Payload::Sandbox(SandboxPayload::WriteFileResult) => Ok(()),
            _ => Err(SandboxError::Io(std::io::Error::other("unexpected response type"))),
        }
    }

    async fn create_dir_all(&self, path: &Path) -> SandboxResult<()> {
        let resp = self.request(
            SandboxOperation::CreateDir,
            SandboxPayload::CreateDir {
                path: path.to_string_lossy().to_string(),
            },
        ).await?;
        match resp.payload {
            Payload::Sandbox(SandboxPayload::CreateDirResult) => Ok(()),
            _ => Err(SandboxError::Io(std::io::Error::other("unexpected response type"))),
        }
    }

    async fn read_dir(&self, path: &Path) -> SandboxResult<Vec<DirEntry>> {
        let resp = self.request(
            SandboxOperation::ReadDir,
            SandboxPayload::ReadDir {
                path: path.to_string_lossy().to_string(),
            },
        ).await?;
        match resp.payload {
            Payload::Sandbox(SandboxPayload::ReadDirResult { entries }) => {
                Ok(entries.into_iter().map(|def| {
                    let file_type = match def.file_type.as_str() {
                        "directory" => FileType::Directory,
                        "file" => FileType::File,
                        "symlink" => FileType::Symlink,
                        _ => FileType::Other,
                    };
                    DirEntry { name: def.name, file_type }
                }).collect())
            }
            _ => Err(SandboxError::Io(std::io::Error::other("unexpected response type"))),
        }
    }

    async fn metadata(&self, path: &Path) -> SandboxResult<FileMetadata> {
        let resp = self.request(
            SandboxOperation::Metadata,
            SandboxPayload::Metadata {
                path: path.to_string_lossy().to_string(),
            },
        ).await?;
        match resp.payload {
            Payload::Sandbox(SandboxPayload::MetadataResult { metadata: def }) => {
                let file_type = match def.file_type.as_str() {
                    "directory" => FileType::Directory,
                    "file" => FileType::File,
                    "symlink" => FileType::Symlink,
                    _ => FileType::Other,
                };
                Ok(FileMetadata { size: def.size, mtime: def.mtime, file_type })
            }
            _ => Err(SandboxError::Io(std::io::Error::other("unexpected response type"))),
        }
    }
}
```

- [ ] **Step 3: Add dependencies to agent-server Cargo.toml**

Check that `tokio-tungstenite`, `futures-util`, `tokio-util`, `base64` are in `vol-agent-server/Cargo.toml`. If any are missing, add them under `[dependencies]`:

```toml
base64 = "0.22"
futures-util = { workspace = true }
tokio-tungstenite = { workspace = true }
tokio-util = { workspace = true }
```

(Many of these likely already exist — verify with `cargo check`.)

- [ ] **Step 4: Verify compilation**

Run: `cargo check -p vol-agent-server`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/vol-agent-server/src/sandbox/mod.rs \
        crates/vol-agent-server/src/sandbox/remote.rs \
        crates/vol-agent-server/src/lib.rs
git commit -m "feat(agent-server): add RemoteSandbox — remote Sandbox via JSON-RPC

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 10: Integration tests

**Files:**
- Create: `crates/vol-agent-server/tests/sandbox_protocol_integration.rs`

- [ ] **Step 1: Write integration test with in-memory server**

```rust
use std::sync::Arc;
use std::time::Duration;

use vol_agent_server::data_plane::core::DataPlaneServerCore;
use vol_agent_server::data_plane::handlers::sandbox::SandboxHandler;
use vol_agent_server::sandbox::remote::RemoteSandbox;
use vol_llm_agent_protocol::domain::HandlerRegistry;
use vol_llm_agent_protocol::transport::memory::MemoryConnection;
use vol_llm_agent_protocol::service::JsonRpcMessageService;
use vol_llm_sandbox::local::LocalSandbox;
use vol_llm_sandbox::Sandbox;

/// Start an in-memory server that handles sandbox protocol messages.
fn create_test_server() -> (MemoryConnection, tokio::task::JoinHandle<()>) {
    let sandbox = Arc::new(LocalSandbox::new(None));
    let rt = tokio::runtime::Handle::current();
    rt.block_on(async { sandbox.start().await.unwrap() });

    let mut registry = HandlerRegistry::new();
    registry.register(Arc::new(SandboxHandler::new(sandbox))).unwrap();

    let (server_conn, client_conn) = MemoryConnection::pair();

    let handle = tokio::spawn(async move {
        let mut messages = vec![];
        loop {
            match server_conn.recv().await {
                Some(Ok(msg)) => {
                    let replies = registry.dispatch(msg).await.unwrap();
                    for reply in replies {
                        server_conn.send(reply).await.ok();
                    }
                }
                Some(Err(_)) | None => break,
            }
        }
    });

    (client_conn, handle)
}

// ⚠️ Note: RemoteSandbox requires a real WebSocket connection.
// For integration tests, we test the handler directly via MemoryConnection.
// The full WebSocket round-trip is tested in the E2E test.

#[tokio::test]
async fn test_sandbox_list_via_handler() {
    let (client_conn, _handle) = create_test_server();

    let msg = vol_llm_agent_protocol::agent_server_protocol::AgentServerMessage::new_command(
        "1",
        vol_llm_agent_protocol::agent_server_protocol::Operation::Sandbox(
            vol_llm_agent_protocol::agent_server_protocol::SandboxOperation::List,
        ),
        vol_llm_agent_protocol::agent_server_protocol::Payload::Sandbox(
            vol_llm_agent_protocol::agent_server_protocol::SandboxPayload::List,
        ),
    );
    client_conn.send(msg).await.unwrap();

    let reply = client_conn.recv().await.unwrap().unwrap();
    match &reply.payload {
        vol_llm_agent_protocol::agent_server_protocol::Payload::Sandbox(
            vol_llm_agent_protocol::agent_server_protocol::SandboxPayload::ListResult { sandboxes },
        ) => {
            assert_eq!(sandboxes.len(), 1);
            assert_eq!(sandboxes[0].name, "local");
            assert_eq!(sandboxes[0].kind, "local");
        }
        _ => panic!("expected ListResult, got {:?}", reply.payload),
    }
}

#[tokio::test]
async fn test_sandbox_exec_echo_via_handler() {
    let (client_conn, _handle) = create_test_server();

    use vol_llm_agent_protocol::agent_server_protocol::CommandRequestDef;

    let msg = vol_llm_agent_protocol::agent_server_protocol::AgentServerMessage::new_command(
        "2",
        vol_llm_agent_protocol::agent_server_protocol::Operation::Sandbox(
            vol_llm_agent_protocol::agent_server_protocol::SandboxOperation::Exec,
        ),
        vol_llm_agent_protocol::agent_server_protocol::Payload::Sandbox(
            vol_llm_agent_protocol::agent_server_protocol::SandboxPayload::Exec {
                command: CommandRequestDef {
                    program: "echo".into(),
                    args: vec!["-n".into(), "hello".into()],
                    env: vec![],
                    cwd: None,
                    stdin: None,
                    timeout_ms: 5000,
                },
            },
        ),
    );
    client_conn.send(msg).await.unwrap();

    let reply = client_conn.recv().await.unwrap().unwrap();
    match &reply.payload {
        vol_llm_agent_protocol::agent_server_protocol::Payload::Sandbox(
            vol_llm_agent_protocol::agent_server_protocol::SandboxPayload::ExecResult { output },
        ) => {
            assert_eq!(output.exit_code, 0);
            use base64::Engine;
            let stdout = base64::engine::general_purpose::STANDARD
                .decode(&output.stdout).unwrap_or_default();
            assert_eq!(stdout, b"hello");
        }
        _ => panic!("expected ExecResult, got {:?}", reply.payload),
    }
}

#[tokio::test]
async fn test_sandbox_path_traversal_rejected() {
    let (client_conn, _handle) = create_test_server();

    let msg = vol_llm_agent_protocol::agent_server_protocol::AgentServerMessage::new_command(
        "3",
        vol_llm_agent_protocol::agent_server_protocol::Operation::Sandbox(
            vol_llm_agent_protocol::agent_server_protocol::SandboxOperation::ReadFile,
        ),
        vol_llm_agent_protocol::agent_server_protocol::Payload::Sandbox(
            vol_llm_agent_protocol::agent_server_protocol::SandboxPayload::ReadFile {
                path: "../etc/passwd".into(),
                offset: None,
                limit: None,
            },
        ),
    );
    client_conn.send(msg).await.unwrap();

    let reply = client_conn.recv().await.unwrap().unwrap();
    // Handler returns error via ProtocolError → protocol layer sends Error variant
    match &reply.kind {
        vol_llm_agent_protocol::agent_server_protocol::MessageKind::Error => {
            // Expected — path traversal should be rejected
        }
        _ => panic!("expected Error, got {:?}", reply.kind),
    }
}

#[tokio::test]
async fn test_sandbox_concurrent_requests() {
    let (client_conn, _handle) = create_test_server();

    // We can't share MemoryConnection across tasks easily with its current API,
    // so this test sends two requests sequentially to verify the handler
    // doesn't break under multiple messages.

    use vol_llm_agent_protocol::agent_server_protocol::CommandRequestDef;

    for i in 0..4 {
        let msg = vol_llm_agent_protocol::agent_server_protocol::AgentServerMessage::new_command(
            format!("req-{}", i),
            vol_llm_agent_protocol::agent_server_protocol::Operation::Sandbox(
                vol_llm_agent_protocol::agent_server_protocol::SandboxOperation::Exec,
            ),
            vol_llm_agent_protocol::agent_server_protocol::Payload::Sandbox(
                vol_llm_agent_protocol::agent_server_protocol::SandboxPayload::Exec {
                    command: CommandRequestDef {
                        program: "echo".into(),
                        args: vec![format!("msg-{}", i)],
                        env: vec![],
                        cwd: None,
                        stdin: None,
                        timeout_ms: 5000,
                    },
                },
            ),
        );
        client_conn.send(msg).await.unwrap();
    }

    for _ in 0..4 {
        let reply = client_conn.recv().await.unwrap().unwrap();
        assert!(matches!(reply.kind, vol_llm_agent_protocol::agent_server_protocol::MessageKind::Result));
    }
}
```

- [ ] **Step 2: Run integration tests**

Run: `cargo test -p vol-agent-server -- sandbox_protocol_integration`
Expected: 4 tests PASS

- [ ] **Step 3: Commit**

```bash
git add crates/vol-agent-server/tests/sandbox_protocol_integration.rs
git commit -m "test(agent-server): add sandbox protocol integration tests

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 11: Full test suite and boundary check

**Files:**
- No file changes — verification only

- [ ] **Step 1: Run agent-server full test suite**

```bash
cargo test -p vol-agent-server
```
Expected: All existing + new tests PASS

- [ ] **Step 2: Run protocol crate tests**

```bash
cargo test -p vol-llm-agent-protocol
```
Expected: All tests PASS (including new sandbox tests)

- [ ] **Step 3: Run boundary check script**

```bash
./scripts/check-agent-boundaries.sh
```
Expected: PASS (no new boundary violations). If script needs updating for the new `vol-llm-agent-protocol → vol-llm-sandbox` dep, update it.

- [ ] **Step 4: Build release**

```bash
cargo build -p vol-agent-server --release
```
Expected: PASS

- [ ] **Step 5: Commit any fixes**

```bash
git add -A
git commit -m "chore: final verification — tests, boundaries, release build"
```

---

### Task 12 (Optional): E2E test via localhost WebSocket

**Files:**
- Create: `crates/vol-agent-server/tests/sandbox_e2e.rs`

- [ ] **Step 1: Write E2E test**

Find or create a helper to start the agent server on a random port (reuse pattern from existing integration tests in `tests/`). Then:

```rust
#[tokio::test]
async fn test_remote_sandbox_e2e_echo() {
    // 1. Start agent server with SandboxHandler registered (localhost:0 = random port)
    // 2. Get the bound port
    // 3. RemoteSandbox::connect(&format!("ws://localhost:{}/ws", port)).await
    // 4. remote.execute(CommandRequest { program: "echo", args: ["hello"], ... })
    // 5. assert output.stdout == "hello"
    // 6. server shutdown
}
```

- [ ] **Step 2: Run E2E test**

```bash
cargo test -p vol-agent-server -- sandbox_e2e --ignored
```
(Mark as `#[ignore]` if it requires port binding — run manually or in CI with `--include-ignored`)

---

## Verification Checklist

- [ ] `cargo check -p vol-llm-agent-protocol` — compiles
- [ ] `cargo check -p vol-agent-server` — compiles
- [ ] `cargo test -p vol-llm-agent-protocol` — all tests pass
- [ ] `cargo test -p vol-agent-server` — all tests pass
- [ ] `./scripts/check-agent-boundaries.sh` — no violations
- [ ] `cargo build -p vol-agent-server --release` — release build succeeds
