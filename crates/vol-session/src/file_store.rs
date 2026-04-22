//! File-based message store using JSONL format.

use crate::message::SessionMessage;
use crate::store::MessageStore;
use crate::store::{Result, StoreError};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use vol_llm_core::Message;

/// File-based message store using JSONL format.
pub struct FileMessageStore {
    base_path: PathBuf,
    #[allow(dead_code)]
    session_id: String,
    file_path: PathBuf,
}

/// JSONL line format for persistence
#[derive(Clone, Debug, Serialize, Deserialize)]
struct SessionMessageLine {
    event: String,
    data: SessionMessageData,
    session_id: String,
    timestamp: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct SessionMessageData {
    id: String,
    session_id: String,
    message: serde_json::Value,
    parent_id: Option<String>,
    created_at: i64,
    metadata: std::collections::HashMap<String, String>,
}

impl FileMessageStore {
    /// Create a new file message store for a session.
    ///
    /// # Arguments
    /// * `base_path` - Base directory for storing session files
    /// * `session_id` - Session identifier (used as filename)
    pub fn new<P: AsRef<Path>>(base_path: P, session_id: &str) -> Self {
        let base_path = base_path.as_ref().to_path_buf();
        let sessions_dir = base_path.join("sessions");
        let file_path = sessions_dir.join(format!("{}.jsonl", session_id));

        Self {
            base_path,
            session_id: session_id.to_string(),
            file_path,
        }
    }

    /// Ensure the sessions directory exists.
    fn ensure_dir(&self) -> std::io::Result<()> {
        let sessions_dir = self.base_path.join("sessions");
        fs::create_dir_all(&sessions_dir)
    }

    /// Append a line to the JSONL file.
    fn append_line(&self, line: &str) -> std::io::Result<()> {
        self.ensure_dir()?;
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.file_path)?;
        writeln!(file, "{}", line)?;
        Ok(())
    }

    /// Read all lines from the JSONL file.
    fn read_all_lines(&self) -> std::io::Result<Vec<String>> {
        let mut lines = Vec::new();
        if self.file_path.exists() {
            let file = File::open(&self.file_path)?;
            let reader = BufReader::new(file);
            for line in reader.lines() {
                lines.push(line?);
            }
        }
        Ok(lines)
    }

    /// Convert SessionMessage to SessionMessageLine for JSONL storage.
    fn to_line(message: &SessionMessage) -> SessionMessageLine {
        SessionMessageLine {
            event: "SessionMessage".to_string(),
            data: SessionMessageData {
                id: message.id.clone(),
                session_id: message.session_id.clone(),
                message: serde_json::to_value(&message.message).unwrap_or_default(),
                parent_id: message.parent_id.clone(),
                created_at: message.created_at,
                metadata: message.metadata.clone(),
            },
            session_id: message.session_id.clone(),
            timestamp: message.created_at,
        }
    }

    /// Convert SessionMessageLine back to SessionMessage.
    fn from_line(line: &SessionMessageLine) -> Result<SessionMessage> {
        let message: Message = serde_json::from_value(line.data.message.clone())
            .map_err(|e| StoreError::Serialization(format!("Failed to parse message: {}", e)))?;

        Ok(SessionMessage {
            id: line.data.id.clone(),
            session_id: line.data.session_id.clone(),
            message,
            parent_id: line.data.parent_id.clone(),
            created_at: line.data.created_at,
            metadata: line.data.metadata.clone(),
        })
    }
}

#[async_trait]
impl MessageStore for FileMessageStore {
    async fn save(&self, message: SessionMessage) -> Result<()> {
        let line = Self::to_line(&message);
        let json = serde_json::to_string(&line).map_err(|e| {
            StoreError::Serialization(format!("Failed to serialize message: {}", e))
        })?;
        self.append_line(&json).map_err(|e| StoreError::Io(e))?;
        Ok(())
    }

    async fn get_by_session(&self, _session_id: &str, limit: usize) -> Result<Vec<SessionMessage>> {
        let lines = self.read_all_lines().map_err(|e| StoreError::Io(e))?;

        let mut messages = Vec::new();
        for line in lines {
            let msg_line: SessionMessageLine = serde_json::from_str(&line).map_err(|e| {
                StoreError::Serialization(format!("Failed to parse JSONL line: {}", e))
            })?;
            messages.push(Self::from_line(&msg_line)?);
            if messages.len() >= limit {
                break;
            }
        }
        Ok(messages)
    }

    async fn get_before(
        &self,
        _session_id: &str,
        _before: i64,
        _limit: usize,
    ) -> Result<Vec<SessionMessage>> {
        // TODO: Implement timestamp-based pagination
        unimplemented!("get_before is not yet implemented")
    }

    async fn get_after(
        &self,
        _session_id: &str,
        after: i64,
        limit: usize,
    ) -> Result<Vec<SessionMessage>> {
        let lines = self.read_all_lines().map_err(|e| StoreError::Io(e))?;

        let mut messages = Vec::new();
        for line in lines {
            let msg_line: SessionMessageLine = serde_json::from_str(&line).map_err(|e| {
                StoreError::Serialization(format!("Failed to parse JSONL line: {}", e))
            })?;
            if msg_line.timestamp > after {
                messages.push(Self::from_line(&msg_line)?);
                if messages.len() >= limit {
                    break;
                }
            }
        }
        Ok(messages)
    }

    async fn delete_session(&self, _session_id: &str) -> Result<()> {
        if self.file_path.exists() {
            fs::remove_file(&self.file_path).map_err(|e| StoreError::Io(e))?;
        }
        Ok(())
    }

    async fn update(&self, _id: &str, _message: SessionMessage) -> Result<()> {
        // JSONL is append-only; updates would require rewriting the entire file
        unimplemented!("update is not supported for append-only JSONL storage")
    }

    async fn get_count(&self, _session_id: &str) -> Result<usize> {
        let lines = self.read_all_lines().map_err(|e| StoreError::Io(e))?;
        Ok(lines.len())
    }

    async fn cleanup_expired(&self, _before: i64) -> Result<()> {
        // TODO: Implement time-based cleanup
        unimplemented!("cleanup_expired is not yet implemented")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use vol_llm_core::Message;

    #[tokio::test]
    async fn test_file_message_store_save() {
        let temp_dir = tempdir().unwrap();
        let store = FileMessageStore::new(temp_dir.path(), "test-session");

        let message =
            SessionMessage::new("test-session".to_string(), Message::user("Hello, World!"));

        store.save(message).await.unwrap();

        // Verify file exists and has content
        assert!(store.file_path.exists(), "JSONL file should exist");

        let content = fs::read_to_string(&store.file_path).unwrap();
        assert!(!content.is_empty(), "JSONL file should have content");
        assert!(
            content.contains("SessionMessage"),
            "Should contain event type"
        );
        assert!(
            content.contains("Hello, World!"),
            "Should contain message content"
        );
    }
}
