//! Remote JSON-RPC WebSocket connection to an agent service.
//!
//! Implements both `AgentConnection` and `FileOperations` for remote mode.
//! Auto-reconnects with exponential backoff on disconnection (max 5 retries, 1s-30s).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use jsonrpsee::core::client::ClientT;
use jsonrpsee::core::params::ObjectParams;
use serde::Deserialize;
use tokio::sync::{mpsc, RwLock};

use crate::connection::{AgentConnection, FileEntry, FileOperations, LogRunInfo, SessionInfo};
use crate::state::UiEvent;

/// Connection state for tracking.
struct ConnectionState {
    ws_url: String,
}

/// Provider and model used for the run — mirrors
/// `vol_llm_agent_protocol::agent_server_protocol::ProviderInfo`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderInfo {
    pub name: String,
    pub model: String,
}

/// Response from `agent.submit` — mirrors `AgentPayload::SubmitResult`.
///
/// The server sends a single SubmitResult response (no more Ack+Result pair)
/// carrying `run_id`, the `accepted` flag, provider info, and the effective
/// tool/MCP/skill capability lists instead of the old `response` value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubmitResult {
    pub run_id: String,
    pub accepted: bool,
    pub provider: ProviderInfo,
    pub tools: Vec<String>,
    pub mcps: Vec<String>,
    pub skills: Vec<String>,
}

/// Parse a flat `agent.submit` JSON-RPC result into the new `SubmitResult` shape.
fn parse_submit_result(value: &serde_json::Value) -> anyhow::Result<SubmitResult> {
    let run_id = value
        .get("run_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("missing run_id"))?
        .to_string();
    let accepted = value
        .get("accepted")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    let provider = ProviderInfo {
        name: value
            .get("provider")
            .and_then(|p| p.get("name"))
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string(),
        model: value
            .get("provider")
            .and_then(|p| p.get("model"))
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string(),
    };
    let str_list = |key: &str| {
        value
            .get(key)
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|e| e.as_str().map(ToString::to_string))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    };
    Ok(SubmitResult {
        run_id,
        accepted,
        provider,
        tools: str_list("tools"),
        mcps: str_list("mcps"),
        skills: str_list("skills"),
    })
}

/// Remote JSON-RPC WebSocket connection to an agent service.
///
/// Implements both `AgentConnection` and `FileOperations` for remote mode.
/// Auto-reconnects with exponential backoff on disconnection (max 5 retries, 1s-30s).
pub struct RemoteConnection {
    state: RwLock<ConnectionState>,
    connected: Arc<AtomicBool>,
}

impl RemoteConnection {
    /// Create a new remote connection.
    pub fn new(ws_url: &str) -> Self {
        Self {
            state: RwLock::new(ConnectionState {
                ws_url: ws_url.to_string(),
            }),
            connected: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Send a JSON-RPC request and wait for the response.
    async fn rpc_call<T>(&self, method: &str, params: ObjectParams) -> anyhow::Result<T>
    where
        T: for<'de> Deserialize<'de>,
    {
        use jsonrpsee::ws_client::WsClientBuilder;

        let url = {
            let state = self.state.read().await;
            state.ws_url.clone()
        };

        // Connect and send
        let client = WsClientBuilder::default().build(&url).await?;
        self.connected.store(true, Ordering::SeqCst);

        let response: T = client.request(method, params).await?;
        Ok(response)
    }
}

#[async_trait]
impl AgentConnection for RemoteConnection {
    async fn submit(&self, input: String) -> anyhow::Result<mpsc::Receiver<UiEvent>> {
        let (tx, rx) = mpsc::channel(256);
        let url = {
            let state = self.state.read().await;
            state.ws_url.clone()
        };
        let connected = self.connected.clone();
        let input_clone = input.clone();

        // Spawn event listener task
        tokio::spawn(async move {
            use jsonrpsee::ws_client::WsClientBuilder;

            let mut retry = 0u32;
            let max_retries = 5u32;

            while retry <= max_retries {
                let result = async {
                    let client = WsClientBuilder::default().build(&url).await?;
                    connected.store(true, Ordering::SeqCst);

                    // Simple request-response pattern: agent.submit now returns
                    // a single SubmitResult { run_id, accepted, provider,
                    // tools, mcps, skills } — no more Ack+Result pair.
                    let mut params = ObjectParams::new();
                    params.insert("input", &input_clone)?;
                    let response: serde_json::Value =
                        client.request("agent.submit", params).await?;

                    let submit_result = match parse_submit_result(&response) {
                        Ok(sr) => sr,
                        Err(e) => {
                            let _ = tx
                                .send(UiEvent::AgentError {
                                    run_id: String::new(),
                                    message: format!("agent.submit: invalid response: {e}"),
                                })
                                .await;
                            return anyhow::Result::<()>::Ok(());
                        }
                    };

                    if submit_result.accepted {
                        tracing::info!(
                            "agent.submit accepted: run_id={} provider={}/{} tools={} mcps={} skills={}",
                            submit_result.run_id,
                            submit_result.provider.name,
                            submit_result.provider.model,
                            submit_result.tools.len(),
                            submit_result.mcps.len(),
                            submit_result.skills.len(),
                        );
                        let _ = tx
                            .send(UiEvent::AgentStart {
                                run_id: submit_result.run_id,
                                input: input_clone.clone(),
                            })
                            .await;
                    } else {
                        let _ = tx
                            .send(UiEvent::AgentError {
                                run_id: submit_result.run_id,
                                message: "agent.submit was rejected by the server".to_string(),
                            })
                            .await;
                    }

                    anyhow::Result::<()>::Ok(())
                }
                .await;

                match result {
                    Ok(()) => break,
                    Err(e) => {
                        connected.store(false, Ordering::SeqCst);
                        retry += 1;
                        if retry <= max_retries {
                            let delay = std::cmp::min(1000 * 2_u64.pow(retry - 1), 30000);
                            tokio::time::sleep(std::time::Duration::from_millis(delay)).await;
                        } else {
                            let _ = tx
                                .send(UiEvent::AgentError {
                                    run_id: String::new(),
                                    message: format!(
                                        "Connection failed after {retry} retries: {e}"
                                    ),
                                })
                                .await;
                        }
                    }
                }
            }
        });

        Ok(rx)
    }

    async fn approve_tool(
        &self,
        req_id: String,
        approved: bool,
        reason: Option<String>,
    ) -> anyhow::Result<()> {
        let mut params = ObjectParams::new();
        params.insert("req_id", &req_id)?;
        params.insert("approved", approved)?;
        params.insert("reason", &reason)?;
        let _response: serde_json::Value = self.rpc_call("agent.approve", params).await?;
        Ok(())
    }

    async fn cancel(&self, req_id: String) -> anyhow::Result<()> {
        let mut params = ObjectParams::new();
        params.insert("req_id", &req_id)?;
        let _response: serde_json::Value = self.rpc_call("agent.cancel", params).await?;
        Ok(())
    }

    fn is_connected(&self) -> bool {
        self.connected.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl FileOperations for RemoteConnection {
    async fn list_files(&self, path: &str) -> anyhow::Result<Vec<FileEntry>> {
        let mut params = ObjectParams::new();
        params.insert("path", path)?;
        let response: serde_json::Value = self.rpc_call("file.list", params).await?;
        let entries = response
            .get("entries")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|e| {
                        Some(FileEntry {
                            name: e.get("name")?.as_str()?.to_string(),
                            is_dir: e.get("is_dir")?.as_bool()?,
                            size: e.get("size")?.as_u64()?,
                        })
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        Ok(entries)
    }

    async fn read_file(&self, path: &str) -> anyhow::Result<String> {
        let mut params = ObjectParams::new();
        params.insert("path", path)?;
        let response: serde_json::Value = self.rpc_call("file.read", params).await?;
        Ok(response
            .get("content")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string())
    }

    async fn list_logs(&self) -> anyhow::Result<Vec<LogRunInfo>> {
        let params = ObjectParams::new();
        let response: serde_json::Value = self.rpc_call("log.list", params).await?;
        #[allow(clippy::cast_possible_truncation)]
        let logs = response
            .get("runs")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|e| {
                        Some(LogRunInfo {
                            run_id: e.get("id")?.as_str()?.to_string(),
                            timestamp: e.get("timestamp")?.as_str()?.to_string(),
                            event_count: e.get("count")?.as_u64()? as usize,
                        })
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        Ok(logs)
    }

    async fn list_sessions(&self) -> anyhow::Result<Vec<SessionInfo>> {
        let params = ObjectParams::new();
        let response: serde_json::Value = self.rpc_call("session.list", params).await?;
        #[allow(clippy::cast_possible_truncation)]
        let sessions = response
            .get("sessions")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|e| {
                        Some(SessionInfo {
                            session_id: e.get("id")?.as_str()?.to_string(),
                            entry_count: e.get("entry_count")?.as_u64()? as usize,
                            created_at: e.get("created_at")?.as_str()?.to_string(),
                        })
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        Ok(sessions)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_remote_connection_new_not_connected() {
        let conn = RemoteConnection::new("ws://localhost:3000");
        assert!(!conn.is_connected());
    }

    #[tokio::test]
    async fn test_remote_connection_submit_returns_receiver() {
        let conn = RemoteConnection::new("ws://localhost:9999"); // Won't connect
        let rx = conn.submit("test".into()).await.unwrap();
        // Receiver should exist; actual connection will fail in background
        drop(rx);
    }

    #[test]
    fn parse_submit_result_parses_new_shape() {
        let value = serde_json::json!({
            "run_id": "run-1",
            "accepted": true,
            "provider": { "name": "anthropic", "model": "claude-sonnet-5" },
            "tools": ["bash", "read"],
            "mcps": ["k8s"],
            "skills": ["code-review"],
        });
        let sr = parse_submit_result(&value).unwrap();
        assert_eq!(sr.run_id, "run-1");
        assert!(sr.accepted);
        assert_eq!(sr.provider.name, "anthropic");
        assert_eq!(sr.provider.model, "claude-sonnet-5");
        assert_eq!(sr.tools, vec!["bash", "read"]);
        assert_eq!(sr.mcps, vec!["k8s"]);
        assert_eq!(sr.skills, vec!["code-review"]);
    }

    #[test]
    fn parse_submit_result_defaults_missing_capability_lists() {
        // Minimal response — capability lists are optional on the wire.
        let value = serde_json::json!({
            "run_id": "run-2",
            "accepted": false,
            "provider": { "name": "unknown", "model": "unknown" },
        });
        let sr = parse_submit_result(&value).unwrap();
        assert_eq!(sr.run_id, "run-2");
        assert!(!sr.accepted);
        assert!(sr.tools.is_empty());
        assert!(sr.mcps.is_empty());
        assert!(sr.skills.is_empty());
    }

    #[test]
    fn parse_submit_result_missing_run_id_errors() {
        let value = serde_json::json!({ "accepted": true, "tools": [] });
        assert!(parse_submit_result(&value).is_err());
    }
}
