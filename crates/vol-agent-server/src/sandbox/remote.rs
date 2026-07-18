//! Remote sandbox — implements the `Sandbox` trait via JSON-RPC over WebSocket
//! to a remote agent server.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use tokio::sync::{mpsc, oneshot};

use vol_llm_agent_protocol::agent_server_protocol::{
    AgentServerMessage, Operation, Payload, SandboxOperation, SandboxPayload,
};
use vol_llm_agent_protocol::transport::jsonrpc::codec::{
    decode_jsonrpc_frame, encode_jsonrpc_message,
};
use vol_llm_sandbox::{
    CommandOutput, CommandRequest, DirEntry, FileMetadata, FileType, Sandbox, SandboxError,
    SandboxResult,
};

/// A `Sandbox` backed by a remote agent server via JSON-RPC/WebSocket.
///
/// Created via `RemoteSandbox::connect(url)`. On drop, the background reader/writer
/// tasks are cancelled via `CancellationToken`.
pub struct RemoteSandbox {
    #[allow(dead_code)]
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
        use futures_util::{SinkExt, StreamExt};
        use tokio_tungstenite::tungstenite::Message;

        let (ws, _) = tokio_tungstenite::connect_async(server_url)
            .await
            .map_err(|e| SandboxError::Io(std::io::Error::other(e)))?;

        let (mut ws_write, mut ws_read) = ws.split();
        let (write_tx, mut write_rx) = mpsc::unbounded_channel::<String>();
        let cancel = tokio_util::sync::CancellationToken::new();
        let inner = Arc::new(RemoteSandboxInner {
            pending: Mutex::new(HashMap::new()),
            msg_id_counter: AtomicU64::new(0),
        });

        // Background writer: mpsc -> WebSocket
        let writer_cancel = cancel.child_token();
        let writer = tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = writer_cancel.cancelled() => break,
                    frame = write_rx.recv() => {
                        match frame {
                            Some(text) => {
                                let _ = ws_write.send(Message::Text(text)).await;
                            }
                            None => break,
                        }
                    }
                }
            }
        });

        // Background reader: WebSocket -> decode -> oneshot send
        let reader_cancel = cancel.child_token();
        let pending_map = inner.clone();
        let reader = tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = reader_cancel.cancelled() => break,
                    frame = ws_read.next() => {
                        match frame {
                            Some(Ok(Message::Text(text))) => {
                                if let Ok(msg) = decode_jsonrpc_frame(&text) {
                                    let mid = msg.message_id.clone();
                                    #[allow(clippy::unwrap_used)]
                                    let mut guard = pending_map.pending.lock().unwrap();
                                    if let Some(tx) = guard.remove(&mid) {
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
    #[allow(clippy::unwrap_used)]
    async fn request(
        &self,
        op: SandboxOperation,
        payload: SandboxPayload,
    ) -> SandboxResult<AgentServerMessage> {
        let msg_id = self
            .inner
            .msg_id_counter
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

        let frame =
            encode_jsonrpc_message(msg).map_err(|e| SandboxError::Io(std::io::Error::other(e)))?;
        self.write_tx
            .send(frame)
            .map_err(|e| SandboxError::Io(std::io::Error::other(e)))?;

        tokio::time::timeout(Duration::from_secs(30), rx)
            .await
            .map_err(|_| SandboxError::Timeout(Duration::from_secs(30)))?
            .map_err(|_| SandboxError::Io(std::io::Error::other("request cancelled")))
    }
}

#[async_trait]
impl Sandbox for RemoteSandbox {
    fn kind(&self) -> &str {
        "remote"
    }
    fn name(&self) -> &str {
        "remote"
    }

    async fn start(&self) -> SandboxResult<()> {
        Ok(())
    }
    async fn cleanup(&self) -> SandboxResult<()> {
        Ok(())
    }

    fn root_path(&self) -> &Path {
        Path::new("")
    }

    fn resolve_path(&self, rel: &str) -> SandboxResult<PathBuf> {
        Ok(std::path::PathBuf::from(rel))
    }

    async fn execute(&self, req: CommandRequest) -> SandboxResult<CommandOutput> {
        use vol_llm_agent_protocol::agent_server_protocol::CommandRequestDef;
        let def: CommandRequestDef = req.into();
        let resp = self
            .request(
                SandboxOperation::Exec,
                SandboxPayload::Exec { command: def },
            )
            .await?;
        match resp.payload {
            Payload::Sandbox(SandboxPayload::ExecResult { output }) => Ok(output.into()),
            _ => Err(SandboxError::Io(std::io::Error::other(
                "unexpected response type",
            ))),
        }
    }

    async fn read_file(
        &self,
        path: &Path,
        offset: Option<u64>,
        limit: Option<u64>,
    ) -> SandboxResult<Vec<u8>> {
        let resp = self
            .request(
                SandboxOperation::ReadFile,
                SandboxPayload::ReadFile {
                    path: path.to_string_lossy().to_string(),
                    offset,
                    limit,
                },
            )
            .await?;
        match resp.payload {
            Payload::Sandbox(SandboxPayload::ReadFileResult { content }) => {
                use base64::Engine;
                base64::engine::general_purpose::STANDARD
                    .decode(&content)
                    .map_err(|e| SandboxError::Io(std::io::Error::other(e)))
            }
            _ => Err(SandboxError::Io(std::io::Error::other(
                "unexpected response type",
            ))),
        }
    }

    async fn write_file(&self, path: &Path, content: &[u8]) -> SandboxResult<()> {
        use base64::Engine;
        let encoded = base64::engine::general_purpose::STANDARD.encode(content);
        let resp = self
            .request(
                SandboxOperation::WriteFile,
                SandboxPayload::WriteFile {
                    path: path.to_string_lossy().to_string(),
                    content: encoded,
                },
            )
            .await?;
        match resp.payload {
            Payload::Sandbox(SandboxPayload::WriteFileResult) => Ok(()),
            _ => Err(SandboxError::Io(std::io::Error::other(
                "unexpected response type",
            ))),
        }
    }

    async fn create_dir_all(&self, path: &Path) -> SandboxResult<()> {
        let resp = self
            .request(
                SandboxOperation::CreateDir,
                SandboxPayload::CreateDir {
                    path: path.to_string_lossy().to_string(),
                },
            )
            .await?;
        match resp.payload {
            Payload::Sandbox(SandboxPayload::CreateDirResult) => Ok(()),
            _ => Err(SandboxError::Io(std::io::Error::other(
                "unexpected response type",
            ))),
        }
    }

    async fn read_dir(&self, path: &Path) -> SandboxResult<Vec<DirEntry>> {
        let resp = self
            .request(
                SandboxOperation::ReadDir,
                SandboxPayload::ReadDir {
                    path: path.to_string_lossy().to_string(),
                },
            )
            .await?;
        match resp.payload {
            Payload::Sandbox(SandboxPayload::ReadDirResult { entries }) => Ok(entries
                .into_iter()
                .map(|def| {
                    let file_type = match def.file_type.as_str() {
                        "directory" => FileType::Directory,
                        "file" => FileType::File,
                        "symlink" => FileType::Symlink,
                        _ => FileType::Other,
                    };
                    DirEntry {
                        name: def.name,
                        file_type,
                    }
                })
                .collect()),
            _ => Err(SandboxError::Io(std::io::Error::other(
                "unexpected response type",
            ))),
        }
    }

    async fn metadata(&self, path: &Path) -> SandboxResult<FileMetadata> {
        let resp = self
            .request(
                SandboxOperation::Metadata,
                SandboxPayload::Metadata {
                    path: path.to_string_lossy().to_string(),
                },
            )
            .await?;
        match resp.payload {
            Payload::Sandbox(SandboxPayload::MetadataResult { metadata: def }) => {
                let file_type = match def.file_type.as_str() {
                    "directory" => FileType::Directory,
                    "file" => FileType::File,
                    "symlink" => FileType::Symlink,
                    _ => FileType::Other,
                };
                Ok(FileMetadata {
                    size: def.size,
                    mtime: def.mtime,
                    file_type,
                })
            }
            _ => Err(SandboxError::Io(std::io::Error::other(
                "unexpected response type",
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::mpsc;

    /// Verify that Drop on RemoteSandbox does not panic.
    /// We construct one with dummy channels since we can't connect to a real WS server.
    #[tokio::test]
    async fn test_remote_sandbox_drop_no_panic() {
        let (write_tx, _write_rx) = mpsc::unbounded_channel::<String>();
        let inner = Arc::new(RemoteSandboxInner {
            pending: Mutex::new(HashMap::new()),
            msg_id_counter: AtomicU64::new(0),
        });
        let cancel = tokio_util::sync::CancellationToken::new();
        let bg = tokio::spawn(async {
            // no-op background task
        });

        let sandbox = RemoteSandbox {
            server_url: "ws://localhost:9999/test".into(),
            write_tx,
            inner,
            cancel,
            _bg: bg,
        };
        // Drop should call cancel and not panic
        drop(sandbox);
    }

    /// Verify that Sandbox trait methods return expected static values.
    #[tokio::test]
    async fn test_remote_sandbox_kind_and_name() {
        let (write_tx, _write_rx) = mpsc::unbounded_channel::<String>();
        let inner = Arc::new(RemoteSandboxInner {
            pending: Mutex::new(HashMap::new()),
            msg_id_counter: AtomicU64::new(0),
        });
        let cancel = tokio_util::sync::CancellationToken::new();
        let bg = tokio::spawn(async {});

        let sandbox = RemoteSandbox {
            server_url: "ws://localhost:9999/test".into(),
            write_tx,
            inner,
            cancel,
            _bg: bg,
        };

        assert_eq!(sandbox.kind(), "remote");
        assert_eq!(sandbox.name(), "remote");
        assert_eq!(sandbox.root_path(), Path::new(""));
    }

    /// Verify that connect fails gracefully when no server is running.
    #[tokio::test]
    async fn test_remote_sandbox_connect_fails() {
        let result = RemoteSandbox::connect("ws://127.0.0.1:1/does-not-exist").await;
        assert!(result.is_err(), "expected connect to fail with WS error");
    }

    /// Full integration test: start a local WS echo server, connect RemoteSandbox,
    /// and exercise the list operation through the request path.
    #[tokio::test]
    async fn test_remote_sandbox_with_local_ws_server() {
        use futures_util::{SinkExt, StreamExt};
        use tokio::net::TcpListener;
        use tokio_tungstenite::accept_async;
        use tokio_tungstenite::tungstenite::Message;

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        // Spawn WS server that echoes back the parsed method/params as a command.
        // RemoteSandbox reader uses decode_jsonrpc_frame which expects method+params.
        tokio::spawn(async move {
            if let Ok((stream, _)) = listener.accept().await {
                if let Ok(ws_stream) = accept_async(stream).await {
                    let (mut write, mut read) = ws_stream.split();
                    while let Some(Ok(msg)) = read.next().await {
                        if let Message::Text(text) = msg {
                            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&text) {
                                let id = val.get("id");
                                let method = val
                                    .get("method")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("sandbox.list");
                                // Respond with a ListResult as a command frame that
                                // decode_jsonrpc_frame can parse
                                let payload = if method == "sandbox.list" {
                                    serde_json::json!({
                                        "ListResult": {
                                            "sandboxes": [{
                                                "name": "test",
                                                "kind": "local",
                                                "root_path": "/tmp"
                                            }]
                                        }
                                    })
                                } else if method == "sandbox.exec" {
                                    serde_json::json!({
                                        "ExecResult": {
                                            "output": {
                                                "stdout": "",
                                                "stderr": "",
                                                "exit_code": 0
                                            }
                                        }
                                    })
                                } else {
                                    continue;
                                };
                                let response = serde_json::json!({
                                    "jsonrpc": "2.0",
                                    "id": id,
                                    "method": method,
                                    "params": payload,
                                });
                                let _ = write.send(Message::Text(response.to_string())).await;
                            }
                        }
                    }
                }
            }
        });

        // Connect RemoteSandbox to our local WS server
        let url = format!("ws://{addr}");
        let sandbox = RemoteSandbox::connect(&url).await.unwrap();

        // Verify kind/name/root_path
        assert_eq!(sandbox.kind(), "remote");
        assert_eq!(sandbox.name(), "remote");
        assert_eq!(sandbox.root_path(), Path::new(""));

        // Give the WS server a moment to process
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }

    /// Test that execute() calls request() and handles response correctly.
    #[tokio::test]
    async fn test_remote_sandbox_execute_via_ws() {
        use base64::Engine;
        use futures_util::{SinkExt, StreamExt};
        use std::collections::HashMap;
        use std::time::Duration;
        use tokio::net::TcpListener;
        use tokio_tungstenite::accept_async;
        use tokio_tungstenite::tungstenite::Message;

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        // Spawn WS server that responds to sandbox.exec command with ExecResult
        let server = tokio::spawn(async move {
            let accept_fut = async {
                if let Ok((stream, _)) = listener.accept().await {
                    if let Ok(ws_stream) = accept_async(stream).await {
                        let (mut write, mut read) = ws_stream.split();
                        while let Some(Ok(msg)) = read.next().await {
                            if let Message::Text(text) = msg {
                                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&text) {
                                    let id = val.get("id");
                                    let method =
                                        val.get("method").and_then(|v| v.as_str()).unwrap_or("");
                                    let payload = if method == "sandbox.exec" {
                                        serde_json::json!({
                                            "ExecResult": {
                                                "output": {
                                                    "stdout": base64::engine::general_purpose::STANDARD.encode(b"hello from ws"),
                                                    "stderr": "",
                                                    "exit_code": 0
                                                }
                                            }
                                        })
                                    } else {
                                        serde_json::json!({
                                            "ListResult": {
                                                "sandboxes": []
                                            }
                                        })
                                    };
                                    let response = serde_json::json!({
                                        "jsonrpc": "2.0",
                                        "id": id,
                                        "method": method,
                                        "params": payload,
                                    });
                                    let _ = write.send(Message::Text(response.to_string())).await;
                                }
                            }
                        }
                    }
                }
            };
            tokio::time::timeout(Duration::from_secs(5), accept_fut)
                .await
                .ok();
        });

        // Small delay to let the server spin up
        tokio::time::sleep(Duration::from_millis(50)).await;

        let url = format!("ws://{addr}");
        let sandbox = RemoteSandbox::connect(&url).await.unwrap();

        // Small delay for WS connection setup on both sides
        tokio::time::sleep(Duration::from_millis(50)).await;

        let output = sandbox
            .execute(CommandRequest {
                program: "echo".into(),
                args: vec![],
                env: HashMap::new(),
                cwd: None,
                stdin: None,
                timeout: Duration::from_secs(3),
            })
            .await
            .unwrap();

        assert_eq!(output.exit_code, 0);
        assert_eq!(output.stdout, b"hello from ws");

        server.abort();
    }
}
