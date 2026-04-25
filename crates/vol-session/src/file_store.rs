//! File-based entry store using JSONL format.

use crate::entry::{SessionEntry, SessionEntryData, SessionEntryType};
use crate::store::{Result, SessionEntryStore, StoreError};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

/// File-based entry store using JSONL format.
///
/// Stores all entry types in `{entry_dir}/{session_id}.jsonl`.
pub struct FileSessionEntryStore {
    entry_dir: PathBuf,
    #[allow(dead_code)]
    session_id: String,
    file_path: PathBuf,
}

/// JSONL line format for SessionEntry.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct SessionEntryLine {
    id: String,
    session_id: String,
    created_at: i64,
    parent_id: Option<String>,
    r#type: String,
    data: serde_json::Value,
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

    /// Parse a single JSONL line. Returns `None` for unparseable or non-matching lines.
    fn from_json(json: &str) -> Option<SessionEntry> {
        let line = serde_json::from_str::<SessionEntryLine>(json).ok()?;
        let data: SessionEntryData = serde_json::from_value(line.data).ok()?;
        let entry_type = match line.r#type.as_str() {
            "message" => SessionEntryType::Message,
            "checkpoint" => SessionEntryType::Checkpoint,
            "summary" => SessionEntryType::Summary,
            _ => return None,
        };
        Some(SessionEntry {
            id: line.id,
            session_id: line.session_id,
            created_at: line.created_at,
            parent_id: line.parent_id,
            r#type: entry_type,
            data,
        })
    }

    /// Read entries from the head of the file, parsing up to `max_parsed` lines.
    /// Skips unparseable lines silently.
    fn read_from_head(&self, max_parsed: usize) -> std::io::Result<Vec<SessionEntry>> {
        let mut entries = Vec::new();
        if !self.file_path.exists() {
            return Ok(entries);
        }
        let file = File::open(&self.file_path)?;
        let reader = BufReader::new(file);
        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            if let Some(entry) = Self::from_json(&line) {
                entries.push(entry);
                if entries.len() >= max_parsed {
                    break;
                }
            }
        }
        Ok(entries)
    }

    /// Read entries from the tail of the file backwards.
    /// Reads up to `buf_size` bytes from the end, parses complete lines found,
    /// and returns them in file order (oldest first).
    fn read_from_tail(&self, buf_size: u64) -> std::io::Result<Vec<SessionEntry>> {
        if !self.file_path.exists() {
            return Ok(Vec::new());
        }
        let file = File::open(&self.file_path)?;
        let file_len = file.metadata()?.len();
        if file_len == 0 {
            return Ok(Vec::new());
        }

        let read_from = file_len.saturating_sub(buf_size);
        let mut buf = Vec::new();
        let mut reader = BufReader::new(file);
        reader.seek(SeekFrom::Start(read_from))?;
        reader.read_to_end(&mut buf)?;

        // If we didn't start at position 0, the first chunk may be a partial line.
        // Skip to the first newline to find a line boundary.
        let text = String::from_utf8_lossy(&buf);
        let start = if read_from > 0 {
            text.find('\n').map(|p| p + 1).unwrap_or(text.len())
        } else {
            0
        };

        let mut entries = Vec::new();
        for line in text[start..].lines() {
            if line.trim().is_empty() {
                continue;
            }
            if let Some(entry) = Self::from_json(line) {
                entries.push(entry);
            }
        }
        Ok(entries)
    }
}

#[async_trait]
impl SessionEntryStore for FileSessionEntryStore {
    async fn save(&self, entry: SessionEntry) -> Result<()> {
        let json = Self::to_json(&entry)?;
        self.append_line(&json).map_err(StoreError::Io)
    }

    async fn get_entries(&self, limit: usize) -> Result<Vec<SessionEntry>> {
        self.read_from_head(limit).map_err(StoreError::Io)
    }

    async fn get_after(&self, after: i64, limit: usize) -> Result<Vec<SessionEntry>> {
        let mut entries = Vec::new();
        if !self.file_path.exists() {
            return Ok(entries);
        }
        let file = File::open(&self.file_path).map_err(StoreError::Io)?;
        let reader = BufReader::new(file);
        for line in reader.lines() {
            let line = line.map_err(StoreError::Io)?;
            if line.trim().is_empty() {
                continue;
            }
            if let Some(entry) = Self::from_json(&line) {
                if entry.created_at >= after {
                    entries.push(entry);
                    if entries.len() >= limit {
                        break;
                    }
                }
            }
        }
        Ok(entries)
    }

    async fn find_latest_checkpoint(&self) -> Result<Option<SessionEntry>> {
        // Try reading from the tail first (last 64KB) — most likely to contain the latest checkpoint.
        let tail_entries = self.read_from_tail(64 * 1024).map_err(StoreError::Io)?;
        let mut latest: Option<SessionEntry> = None;
        for entry in &tail_entries {
            if entry.r#type == SessionEntryType::Checkpoint {
                match &latest {
                    Some(current) if entry.created_at > current.created_at => {
                        latest = Some(entry.clone());
                    }
                    None => {
                        latest = Some(entry.clone());
                    }
                    _ => {}
                }
            }
        }

        // If no checkpoint found in tail, fall back to full scan.
        if latest.is_none() {
            let all = self.read_from_head(usize::MAX).map_err(StoreError::Io)?;
            for entry in all {
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
        if !self.file_path.exists() {
            return Ok(0);
        }
        let file = File::open(&self.file_path).map_err(StoreError::Io)?;
        let reader = BufReader::new(file);
        let mut count = 0;
        for line in reader.lines() {
            let line = line.map_err(StoreError::Io)?;
            if line.trim().is_empty() {
                continue;
            }
            if Self::from_json(&line).is_some() {
                count += 1;
            }
        }
        Ok(count)
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

    #[tokio::test]
    async fn test_file_entry_store_skips_bad_lines() {
        let temp_dir = tempdir().unwrap();
        let store = FileSessionEntryStore::new(temp_dir.path(), "test-session");

        // Write a valid entry, a bad line, then another valid entry.
        let entry1 = SessionEntry::new_message(
            "test-session".to_string(),
            Message::user("hello"),
        );
        store.save(entry1).await.unwrap();

        // Append a malformed line directly.
        std::fs::OpenOptions::new()
            .append(true)
            .open(&store.file_path)
            .unwrap()
            .write_all(b"this is not valid json\n")
            .unwrap();

        let entry2 = SessionEntry::new_message(
            "test-session".to_string(),
            Message::user("world"),
        );
        store.save(entry2).await.unwrap();

        let entries = store.get_entries(10).await.unwrap();
        assert_eq!(entries.len(), 2);
    }

    #[tokio::test]
    async fn test_file_entry_store_read_from_tail() {
        let temp_dir = tempdir().unwrap();
        let store = FileSessionEntryStore::new(temp_dir.path(), "test-session");

        // Save 5 entries.
        for i in 0..5 {
            let entry = SessionEntry::new_message(
                "test-session".to_string(),
                Message::user(format!("msg-{i}")),
            );
            store.save(entry).await.unwrap();
        }

        // read_from_tail with a small buffer should get the last few entries.
        let tail = store.read_from_tail(256).unwrap();
        assert!(!tail.is_empty());
        // The last entry should be msg-4.
        assert_eq!(tail.last().unwrap().data.entry_type(), SessionEntryType::Message);
    }
}       