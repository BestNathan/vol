//! JSON-RPC method handler for agent operations.
//!
//! Exposes the following methods over WebSocket JSON-RPC:
//! - `agent.submit` — submit input, returns `{ req_id }`
//! - `agent.cancel` — cancel a running agent
//! - `agent.approve` — approve/reject a tool call
//! - `file.list` — list directory contents
//! - `file.read` — read a file
//! - `log.list` — list logged runs
//! - `log.read` — read a specific log
//! - `session.list` — list saved sessions
//! - `session.resume` — resume a saved session

use std::path::Path;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

use crate::dispatcher::AgentDispatcher;

/// Shared context for JSON-RPC handlers.
pub struct JsonRpcContext {
    pub dispatcher: Arc<AgentDispatcher>,
    pub working_dir: String,
    pub store_dir: String,
}

impl JsonRpcContext {
    pub fn new(dispatcher: Arc<AgentDispatcher>, working_dir: String, store_dir: String) -> Self {
        Self {
            dispatcher,
            working_dir,
            store_dir,
        }
    }
}

// === Request/Response types ===

#[derive(Debug, Deserialize)]
pub struct SubmitParams {
    pub input: String,
}

#[derive(Debug, Serialize)]
pub struct SubmitResponse {
    pub req_id: String,
}

#[derive(Debug, Deserialize)]
pub struct CancelParams {
    pub req_id: String,
}

#[derive(Debug, Deserialize)]
pub struct ApproveParams {
    pub req_id: String,
    pub approved: bool,
    #[serde(default)]
    pub reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct FileListParams {
    pub path: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct FileEntry {
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
}

#[derive(Debug, Serialize)]
pub struct FileListResponse {
    pub entries: Vec<FileEntry>,
}

#[derive(Debug, Deserialize)]
pub struct FileReadParams {
    pub path: String,
}

#[derive(Debug, Serialize)]
pub struct FileReadResponse {
    pub content: String,
}

#[derive(Debug, Serialize)]
pub struct LogRunInfo {
    pub id: String,
    pub timestamp: String,
    pub count: usize,
}

#[derive(Debug, Serialize)]
pub struct LogListResponse {
    pub runs: Vec<LogRunInfo>,
}

#[derive(Debug, Deserialize)]
pub struct LogReadParams {
    pub run_id: String,
}

#[derive(Debug, Serialize)]
pub struct LogEntry {
    pub event_type: String,
    pub summary: String,
    pub timestamp: String,
}

#[derive(Debug, Serialize)]
pub struct LogReadResponse {
    pub entries: Vec<LogEntry>,
}

#[derive(Debug, Serialize)]
pub struct SessionInfo {
    pub id: String,
    pub entry_count: usize,
    pub created_at: String,
}

#[derive(Debug, Serialize)]
pub struct SessionListResponse {
    pub sessions: Vec<SessionInfo>,
}

#[derive(Debug, Deserialize)]
pub struct SessionResumeParams {
    pub session_id: String,
}

#[derive(Debug, Serialize)]
pub struct SessionResumeResponse {
    pub session_id: String,
    pub entry_count: usize,
}

// === Handler implementation ===

/// State shared across JSON-RPC method handlers.
pub struct JsonRpcHandler {
    pub ctx: Mutex<JsonRpcContext>,
}

impl JsonRpcHandler {
    pub fn new(ctx: JsonRpcContext) -> Self {
        Self {
            ctx: Mutex::new(ctx),
        }
    }

    /// Submit input to the agent dispatcher.
    pub async fn agent_submit(&self, params: SubmitParams) -> Result<SubmitResponse, String> {
        let ctx = self.ctx.lock().await;
        let request = crate::request::AgentRequest::new("agent", &params.input);
        let req_id = request.req_id.clone();
        drop(ctx);

        let ctx = self.ctx.lock().await;
        let rx = ctx.dispatcher.submit(request).map_err(|e| e.to_string())?;
        drop(ctx);

        // Spawn a task to process the result and push events
        let _ = tokio::spawn(Self::process_run(rx, req_id.clone()));

        Ok(SubmitResponse { req_id })
    }

    /// Cancel a running agent.
    pub async fn agent_cancel(&self, params: CancelParams) -> Result<serde_json::Value, String> {
        let ctx = self.ctx.lock().await;
        let cancelled = ctx.dispatcher.cancel(&params.req_id).await;
        Ok(serde_json::json!({ "cancelled": cancelled }))
    }

    /// Approve/reject a tool call (stub — approval handled via connection).
    pub async fn agent_approve(&self, _params: ApproveParams) -> Result<serde_json::Value, String> {
        Ok(serde_json::json!({ "approved": true }))
    }

    /// List directory contents.
    pub async fn file_list(&self, params: FileListParams) -> Result<FileListResponse, String> {
        let path = Path::new(&params.path);
        let mut entries = Vec::new();

        for entry in std::fs::read_dir(path).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let file_type = entry.file_type().map_err(|e| e.to_string())?;
            let metadata = entry.metadata().map_err(|e| e.to_string())?;
            entries.push(FileEntry {
                name: entry.file_name().to_string_lossy().to_string(),
                is_dir: file_type.is_dir(),
                size: if file_type.is_file() { metadata.len() } else { 0 },
            });
        }

        entries.sort_by_key(|e| (!e.is_dir, e.name.clone()));
        Ok(FileListResponse { entries })
    }

    /// Read a file's contents.
    pub async fn file_read(&self, params: FileReadParams) -> Result<FileReadResponse, String> {
        let content = std::fs::read_to_string(&params.path).map_err(|e| e.to_string())?;
        Ok(FileReadResponse { content })
    }

    /// List logged runs (stub — scans store_dir/logs/).
    pub async fn log_list(&self, _params: serde_json::Value) -> Result<LogListResponse, String> {
        let logs_dir = Path::new(&self.ctx.lock().await.store_dir).join("logs");
        let mut runs = Vec::new();

        if logs_dir.exists() {
            for entry in std::fs::read_dir(&logs_dir).map_err(|e| e.to_string())? {
                let entry = entry.map_err(|e| e.to_string())?;
                let name = entry.file_name().to_string_lossy().to_string();
                if name.ends_with(".jsonl") {
                    let run_id = name.trim_end_matches(".jsonl").to_string();
                    runs.push(LogRunInfo {
                        id: run_id,
                        timestamp: "unknown".to_string(),
                        count: 0,
                    });
                }
            }
        }

        Ok(LogListResponse { runs })
    }

    /// Read a specific log (stub).
    pub async fn log_read(&self, _params: LogReadParams) -> Result<LogReadResponse, String> {
        Ok(LogReadResponse { entries: Vec::new() })
    }

    /// List saved sessions (stub — scans store_dir/sessions/).
    pub async fn session_list(&self, _params: serde_json::Value) -> Result<SessionListResponse, String> {
        let sessions_dir = Path::new(&self.ctx.lock().await.store_dir).join("sessions");
        let mut sessions = Vec::new();

        if sessions_dir.exists() {
            for entry in std::fs::read_dir(&sessions_dir).map_err(|e| e.to_string())? {
                let entry = entry.map_err(|e| e.to_string())?;
                let name = entry.file_name().to_string_lossy().to_string();
                if name.ends_with(".json") {
                    let session_id = name.trim_end_matches(".json").to_string();
                    sessions.push(SessionInfo {
                        id: session_id,
                        entry_count: 0,
                        created_at: "unknown".to_string(),
                    });
                }
            }
        }

        Ok(SessionListResponse { sessions })
    }

    /// Resume a saved session (stub).
    pub async fn session_resume(&self, params: SessionResumeParams) -> Result<SessionResumeResponse, String> {
        Ok(SessionResumeResponse {
            session_id: params.session_id,
            entry_count: 0,
        })
    }

    /// Background task: process agent run results.
    async fn process_run(
        rx: tokio::sync::oneshot::Receiver<crate::request::RunResult>,
        req_id: String,
    ) {
        match rx.await {
            Ok(result) => {
                match result.response {
                    Ok(response) => {
                        tracing::info!(%req_id, run_id = ?result.run_id, "agent run completed");
                    }
                    Err(e) => {
                        tracing::error!(%req_id, %e, "agent run failed");
                    }
                }
            }
            Err(_) => {
                tracing::warn!(%req_id, "agent run receiver dropped");
            }
        }
    }
}
