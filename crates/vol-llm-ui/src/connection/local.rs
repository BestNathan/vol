use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::mpsc;
use vol_llm_agents::coding::{CodingAgent, CodingAgentConfig, EventObserver, ObserverError};
use vol_llm_core::AgentStreamEvent;

use crate::connection::{AgentConnection, FileEntry, FileOperations, LogRunInfo, SessionInfo};
use crate::state::{EventBuffer, UiState};
use crate::UiEvent;

/// Local in-process agent connection.
///
/// Implements both `AgentConnection` (agent lifecycle) and `FileOperations`
/// (filesystem access) for local mode. The agent runs in a background tokio
/// task with an `EventObserver` that converts `AgentStreamEvent` into
/// `UiState` mutations via `EventBuffer`.
pub struct LocalConnection {
    agent_config: CodingAgentConfig,
    state: Arc<tokio::sync::Mutex<UiState>>,
    connected: Arc<AtomicBool>,
    cancelled: Arc<AtomicBool>,
}

impl LocalConnection {
    pub fn new(config: CodingAgentConfig, state: Arc<tokio::sync::Mutex<UiState>>) -> Self {
        Self {
            agent_config: config,
            state,
            connected: Arc::new(AtomicBool::new(true)),
            cancelled: Arc::new(AtomicBool::new(false)),
        }
    }

    async fn run_agent(&self, input: String, tx: mpsc::Sender<UiEvent>, cancelled: Arc<AtomicBool>) {
        let state = self.state.clone();

        let observer = Arc::new(LocalEventObserver {
            state: state.clone(),
            event_tx: tx.clone(),
        });

        let config = self.agent_config.clone();
        let agent = match CodingAgent::new(config) {
            Ok(a) => a,
            Err(e) => {
                let _ = tx
                    .send(UiEvent::AgentError {
                        message: format!("Failed to create agent: {}", e),
                    })
                    .await;
                return;
            }
        };
        let agent = agent.with_observer(observer);

        match agent.run(&input).await {
            Ok(response) => {
                if !response.summary.is_empty() {
                    let _ = tx
                        .send(UiEvent::AgentComplete {
                            response: response.summary,
                        })
                        .await;
                }
            }
            Err(e) => {
                // If cancelled, don't report as error
                if !cancelled.load(Ordering::Relaxed) {
                    let _ = tx
                        .send(UiEvent::AgentError {
                            message: format!("{}", e),
                        })
                        .await;
                }
            }
        }
    }

    pub fn clone_for_run(&self) -> Self {
        Self {
            agent_config: self.agent_config.clone(),
            state: self.state.clone(),
            connected: self.connected.clone(),
            cancelled: Arc::new(AtomicBool::new(false)),
        }
    }
}

#[async_trait]
impl AgentConnection for LocalConnection {
    async fn submit(&self, input: String) -> anyhow::Result<mpsc::Receiver<UiEvent>> {
        let (tx, rx) = mpsc::channel(256);

        // Spawn agent run in background.
        // AgentStart is emitted by the agent itself via the observer,
        // so we don't manually apply it here (avoids duplicate entries).
        let conn = self.clone_for_run();
        let cancelled = conn.cancelled.clone();
        tokio::spawn(async move {
            conn.run_agent(input, tx, cancelled).await;
        });

        Ok(rx)
    }

    async fn approve_tool(
        &self,
        _req_id: String,
        approved: bool,
        _reason: Option<String>,
    ) -> anyhow::Result<()> {
        let mut state = self.state.lock().await;
        state.approval_state.response = Some((approved, None));
        Ok(())
    }

    async fn cancel(&self, _req_id: String) -> anyhow::Result<()> {
        self.cancelled.store(true, Ordering::Relaxed);
        let mut state = self.state.lock().await;
        state.is_running = false;
        Ok(())
    }

    fn is_connected(&self) -> bool {
        self.connected.load(Ordering::Relaxed)
    }
}

#[async_trait]
impl FileOperations for LocalConnection {
    async fn list_files(&self, path: &str) -> anyhow::Result<Vec<FileEntry>> {
        let path = Path::new(path);
        let mut entries = Vec::new();

        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            let file_type = entry.file_type()?;
            let metadata = entry.metadata()?;
            entries.push(FileEntry {
                name: entry.file_name().to_string_lossy().to_string(),
                is_dir: file_type.is_dir(),
                size: if file_type.is_file() {
                    metadata.len()
                } else {
                    0
                },
            });
        }

        // Sort: directories first, then by name
        entries.sort_by_key(|e| (!e.is_dir, e.name.clone()));
        Ok(entries)
    }

    async fn read_file(&self, path: &str) -> anyhow::Result<String> {
        Ok(std::fs::read_to_string(path)?)
    }

    async fn list_logs(&self) -> anyhow::Result<Vec<LogRunInfo>> {
        // Local mode: scan log directory for JSONL files.
        // Stub for now — can be implemented by scanning store_dir/logs/.
        Ok(Vec::new())
    }

    async fn list_sessions(&self) -> anyhow::Result<Vec<SessionInfo>> {
        // Local mode: scan session directory.
        // Stub for now — can be implemented by scanning store_dir/sessions/.
        Ok(Vec::new())
    }
}

/// Observer that converts AgentStreamEvent into UiState mutations.
struct LocalEventObserver {
    state: Arc<tokio::sync::Mutex<UiState>>,
    event_tx: mpsc::Sender<UiEvent>,
}

#[async_trait]
impl EventObserver for LocalEventObserver {
    async fn on_event(&self, event: &AgentStreamEvent) -> Result<(), ObserverError> {
        let mut buffer = EventBuffer::new();
        let mut state = self.state.lock().await;
        buffer.apply_stream(event, &mut state);
        Ok(())
    }

    async fn on_complete(&self) -> Result<(), ObserverError> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vol_llm_agents::coding::CodingAgentConfig;

    fn make_connection() -> LocalConnection {
        let state = Arc::new(tokio::sync::Mutex::new(UiState::new(
            "test-session".into(),
            ".",
        )));
        let config = CodingAgentConfig::default();
        LocalConnection::new(config, state)
    }

    #[tokio::test]
    async fn test_local_connection_is_connected() {
        let conn = make_connection();
        assert!(conn.is_connected());
    }

    #[tokio::test]
    async fn test_local_connection_list_files() {
        let conn = make_connection();
        // Create a temp directory with files
        let dir = std::env::temp_dir().join("vol-llm-ui-local-conn-test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("test.txt"), "hello").unwrap();
        std::fs::create_dir(dir.join("subdir")).unwrap();

        let entries = conn.list_files(dir.to_str().unwrap()).await.unwrap();
        assert_eq!(entries.len(), 2);

        // Directory should be first
        assert!(entries[0].is_dir);
        assert_eq!(entries[0].name, "subdir");

        // File should be second
        assert!(!entries[1].is_dir);
        assert_eq!(entries[1].name, "test.txt");
        assert!(entries[1].size > 0);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_local_connection_read_file() {
        let conn = make_connection();
        let dir = std::env::temp_dir().join("vol-llm-ui-read-test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("test.txt");
        std::fs::write(&file, "hello world").unwrap();

        let content = conn.read_file(file.to_str().unwrap()).await.unwrap();
        assert_eq!(content, "hello world");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_local_connection_submit_returns_receiver() {
        let conn = make_connection();
        let rx = conn.submit("test input".into()).await.unwrap();
        // Should get a receiver; agent will fail since no API key but that's ok
        drop(rx);
    }
}
