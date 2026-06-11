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
use vol_llm_sandbox::*;

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
        use tokio_tungstenite::tungstenite::Message;
        use futures_util::{SinkExt, StreamExt};

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
                                let _ = ws_write.send(Message::Text(text.into())).await;
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

        let frame = encode_jsonrpc_message(msg)
            .map_err(|e| SandboxError::Io(std::io::Error::other(e)))?;
        self.write_tx.send(frame)
            .map_err(|e| SandboxError::Io(std::io::Error::other(e)))?;

        tokio::time::timeout(Duration::from_secs(30), rx)
            .await
            .map_err(|_| SandboxError::Timeout(Duration::from_secs(30)))?
            .map_err(|_| SandboxError::Io(std::io::Error::other("request cancelled")))
    }
}

#[async_trait]
impl Sandbox for RemoteSandbox {
    fn kind(&self) -> &str { "remote" }
    fn name(&self) -> &str { "remote" }

    async fn start(&self) -> SandboxResult<()> { Ok(()) }
    async fn cleanup(&self) -> SandboxResult<()> { Ok(()) }

    fn root_path(&self) -> &Path {
        Path::new("")
    }

    fn resolve_path(&self, rel: &str) -> SandboxResult<PathBuf> {
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
