//! File-based entry store using JSONL format.

use crate::entry::{SessionEntry, SessionEntryData, SessionEntryType};
use crate::store::{Result, SessionEntryStore, StoreError};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use vol_llm_core::Message;

/// File-based entry store using JSONL format.
///
/// Stores all entry types in `{entry_dir}/{session_id}.jsonl`.
pub struct FileSessionEntryStore {
    entry_dir: PathBuf,
    #[allow(dead_code)]
    session_id: String,
    file_path: PathBuf,
}

/// New JSONL line format for SessionEntry.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct SessionEntryLine {
    id: String,
    session_id: String,
    created_at: i64,
    parent_id: Option<String>,
    r#type: String,
    data: serde_json::Value,
}

/// Legacy JSONL line format (from old MessageStore era).
#[derive(Clone, Debug, Serialize, Deserialize)]
struct LegacyMessageLine {
    event: String,
    data: LegacyMessageData,
    session_id: String,
    timestamp: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct LegacyMessageData {
    id: String,
    session_id: String,
    message: serde_json::Value,
    parent_id: Option<String>,
    created_at: i64,
    metadata: HashMap<String, String>,
}

impl FileSessionEntryStore {
    /// Create a new file entry store for a session.
    pub fn new<P: AsRef<Path>>(entry_dir: P, session_id: &str) -> Self {
        let entry_dir = entry_dir.as_ref().to_path_buf();
        let file_path = entry_dir.join(format!("{}.jsonl", session_id));
        Self {
            entry_dir,
            session_id: session_id.to_string(),
            file_path,
        }
    }

    fn ensure_dir(&self) -> std::io::Result<()> {
        fs::create_dir_all(&self.entry_dir)
    }

    fn append_line(&self, line: &str) -> std::io::Result<()> {
        self.ensure_dir()?;
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.file_path)?;
        writeln!(file, "{}", line)?;
        Ok(())
    }

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

    fn to_json(entry: &SessionEntry) -> Result<String> {
        let line = SessionEntryLine {
            id: entry.id.clone(),
            session_id: entry.session_id.clone(),
            created_at: entry.created_at,
            parent_id: entry.parent_id.clone(),
            r#type: match entry.r#type {
                SessionEntryType::Message => "message".to_string(),
                SessionEntryType::Checkpoint => "checkpoint".to_string(),
                SessionEntryType::Summary => "summary".to_string(),
            },
            data: serde_json::to_value(&entry.data).map_err(|e| {
                StoreError::Serialization(format!("Failed to serialize entry data: {}", e))
            })?,
        };
        serde_json::to_string(&line).map_err(|e| {
            StoreError::Serialization(format!("Failed to serialize entry: {}", e))
        })
    }

    fn from_json(json: &str) -> Result<SessionEntry> {
        // Try new format first
        if let Ok(line) = serde_json::from_str::<SessionEntryLine>(json) {
            let data: SessionEntryData = serde_json::from_value(line.data).map_err(|e| {
                StoreError::Serialization(format!("Failed to parse entry data: {}", e))
            })?;
            let entry_type = match line.r#type.as_str() {
                "message" => SessionEntryType::Message,
                "checkpoint" => SessionEntryType::Checkpoint,
                "summary" => SessionEntryType::Summary,
                _ => return Err(StoreError::Serialization(format!("Unknown entry type: {}", line.r#type))),
            };
            return Ok(SessionEntry {
                id: line.id,
                session_id: line.session_id,
                created_at: line.created_at,
                parent_id: line.parent_id,
                r#type: entry_type,
                data,
            });
        }

        // Fall back to legacy format
        if let Ok(legacy) = serde_json::from_str::<LegacyMessageLine>(json) {
            let message: Message = serde_json::from_value(legacy.data.message).map_err(|e| {
                StoreError::Serialization(format!("Failed to parse legacy message: {}", e))
            })?;
            Ok(SessionEntry {
                id: legacy.data.id,
                session_id: legacy.data.session_id,
                created_at: legacy.data.created_at,
                parent_id: legacy.data.parent_id,
                r#type: SessionEntryType::Message,
                data: SessionEntryData::Message { message },
            })
        } else {
            Err(StoreError::Serialization(format!(
                "Failed to parse JSONL line: {}",
                json
            )))
        }
    }
}

#[async_trait]
impl SessionEntryStore for FileSessionEntryStore {
    async fn save(&self, entry: SessionEntry) -> Result<()> {
        let json = Self::to_json(&entry)?;
        self.append_line(&json).map_err(StoreError::Io)
    }

    async fn get_entries(&self, limit: usize) -> Result<Vec<SessionEntry>> {
        let lines = self.read_all_lines().map_err(StoreError::Io)?;
        let mut entries = Vec::new();
        for line in lines {
            if entries.len() >= limit {
                break;
            }
            entries.push(Self::from_json(&line)?);
        }
        Ok(entries)
    }

    async fn get_after(&self, after: i64, limit: usize) -> Result<Vec<SessionEntry>> {
        let lines = self.read_all_lines().map_err(StoreError::Io)?;
        let mut entries = Vec::new();
        for line in lines {
            let entry = Self::from_json(&line)?;
            if entry.created_at >= after {
                entries.push(entry);
                if entries.len() >= limit {
                    break;
                }
            }
        }
        Ok(entries)
    }

    async fn find_latest_checkpoint(&self) -> Result<Option<SessionEntry>> {
        let lines = self.read_all_lines().map_err(StoreError::Io)?;
        let mut latest: Option<SessionEntry> = None;
        for line in lines {
            if let Ok(entry) = Self::from_json(&line) {
                if entry.r#type == SessionEntryType::Checkpoint {
                    match &latest {
                        Some(current) if entry.created_at > current.created_at => {
                            latest = Some(entry);
                        }
                        None => {
                            latest = Some(entry);
                        }
                        _ => {}
                    }
                }
            }
        }
        Ok(latest)
    }

    async fn delete_session(&self) -> Result<()> {
        if self.file_path.exists() {
            fs::remove_file(&self.file_path).map_err(StoreError::Io)?;
        }
        Ok(())
    }

    async fn get_count(&self) -> Result<usize> {
        let lines = self.read_all_lines().map_err(StoreError::Io)?;
        Ok(lines.len())
    }
}

#[cfg(test)]
mod entry_tests {
    use super::*;
    use crate::entry::{SessionEntry, SessionEntryType};
    use crate::CheckpointReason;
    use tempfile::tempdir;
    use vol_llm_core::Message;

    #[tokio::test]
    async fn test_file_entry_store_save_and_get() {
        let temp_dir = tempdir().unwrap();
        let store = FileSessionEntryStore::new(temp_dir.path(), "test-session");

        let entry = SessionEntry::new_message(
            "test-session".to_string(),
            Message::user("Hello, World!"),
        );

        store.save(entry.clone()).await.unwrap();

        let entries = store.get_entries(10).await.unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].r#type, SessionEntryType::Message);
    }

    #[tokio::test]
    async fn test_file_entry_store_find_checkpoint() {
        let temp_dir = tempdir().unwrap();
        let store = FileSessionEntryStore::new(temp_dir.path(), "test-session");

        let mut before = SessionEntry::new_message(
            "test-session".to_string(),
            Message::user("before"),
        );
        before.created_at = 1000;

        let mut checkpoint = SessionEntry::new_checkpoint(
            "test-session".to_string(),
            CheckpointReason::Compression,
            None,
        );
        checkpoint.created_at = 1001;

        let mut after = SessionEntry::new_message(
            "test-session".to_string(),
            Message::user("after"),
        );
        after.created_at = 1002;

        store.save(before).await.unwrap();
        store.save(checkpoint).await.unwrap();
        store.save(after).await.unwrap();

        let cp = store.find_latest_checkpoint().await.unwrap().unwrap();
        assert_eq!(cp.r#type, SessionEntryType::Checkpoint);

        let entries = store.get_after(cp.created_at, 10).await.unwrap();
        assert_eq!(entries.len(), 2);
        // First entry is the checkpoint itself, second is the after message
        assert_eq!(entries[0].r#type, SessionEntryType::Checkpoint);
        assert_eq!(entries[1].r#type, SessionEntryType::Message);
    }

    #[tokio::test]
    async fn test_file_entry_store_delete_session() {
        let temp_dir = tempdir().unwrap();
        let store = FileSessionEntryStore::new(temp_dir.path(), "test-session");

        store.save(SessionEntry::new_message(
            "test-session".to_string(),
            Message::user("test"),
        )).await.unwrap();

        store.delete_session().await.unwrap();
        let count = store.get_count().await.unwrap();
        assert_eq!(count, 0);
    }
}
