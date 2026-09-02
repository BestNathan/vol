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
/// Session ID is passed per-method-call, not bound at construction.
pub struct FileSessionEntryStore {
    entry_dir: PathBuf,
    agent_type: Option<String>,
    /// Serializes read-modify-write cycles on the metadata sidecar.
    meta_write_lock: tokio::sync::Mutex<()>,
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
    /// Create a new file entry store.
    pub fn new<P: AsRef<Path>>(entry_dir: P) -> Self {
        Self {
            entry_dir: entry_dir.as_ref().to_path_buf(),
            agent_type: None,
            meta_write_lock: tokio::sync::Mutex::new(()),
        }
    }

    /// Create a new file entry store, optionally scoped to an agent type subdirectory.
    ///
    /// When `agent_type` is `Some`, entries are stored in `{entry_dir}/{agent_type}/{session_id}.jsonl`.
    /// When `None`, entries use the original path `{entry_dir}/{session_id}.jsonl`.
    pub fn with_agent_type<P: AsRef<Path>>(entry_dir: P, agent_type: Option<String>) -> Self {
        Self {
            entry_dir: entry_dir.as_ref().to_path_buf(),
            agent_type,
            meta_write_lock: tokio::sync::Mutex::new(()),
        }
    }

    /// Return the base entry directory.
    pub fn entry_dir(&self) -> &Path {
        &self.entry_dir
    }

    /// Resolve the directory holding this store's session files.
    /// Includes the agent_type subdirectory when configured.
    fn session_dir(&self) -> PathBuf {
        match &self.agent_type {
            Some(agent) => self.entry_dir.join(agent),
            None => self.entry_dir.clone(),
        }
    }

    /// Resolve file path for a session.
    /// Includes agent_type subdirectory when configured.
    fn file_path(&self, session_id: &str) -> PathBuf {
        self.session_dir().join(format!("{session_id}.jsonl"))
    }

    /// Resolve the metadata sidecar path for a session.
    ///
    /// Shares [`Self::session_dir`] with [`Self::file_path`] so the
    /// path-traversal hardening applied to `agent_id` covers sidecars too.
    /// Do not rebuild this path independently.
    ///
    /// The file name is built by concatenation rather than
    /// `Path::with_extension`, which would eat the trailing segment of a
    /// `session_id` containing a dot (`a.b` → `a.meta.json`).
    fn meta_path(&self, session_id: &str) -> PathBuf {
        self.session_dir().join(format!("{session_id}.meta.json"))
    }

    /// Temp path for the atomic sidecar replacement, alongside the sidecar so
    /// the rename never crosses a filesystem boundary.
    fn meta_tmp_path(&self, session_id: &str) -> PathBuf {
        self.session_dir()
            .join(format!("{session_id}.meta.json.tmp"))
    }

    fn ensure_dir(&self) -> std::io::Result<()> {
        fs::create_dir_all(self.session_dir())
    }

    fn append_line(&self, session_id: &str, line: &str) -> std::io::Result<()> {
        self.ensure_dir()?;
        let file_path = self.file_path(session_id);
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&file_path)?;
        writeln!(file, "{line}")?;
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
                StoreError::Serialization(format!("Failed to serialize entry data: {e}"))
            })?,
        };
        serde_json::to_string(&line)
            .map_err(|e| StoreError::Serialization(format!("Failed to serialize entry: {e}")))
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
    fn read_from_head(
        &self,
        session_id: &str,
        max_parsed: usize,
    ) -> std::io::Result<Vec<SessionEntry>> {
        let file_path = self.file_path(session_id);
        let mut entries = Vec::new();
        if !file_path.exists() {
            return Ok(entries);
        }
        let file = File::open(&file_path)?;
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
    fn read_from_tail(
        &self,
        session_id: &str,
        buf_size: u64,
    ) -> std::io::Result<Vec<SessionEntry>> {
        let file_path = self.file_path(session_id);
        if !file_path.exists() {
            return Ok(Vec::new());
        }
        let file = File::open(&file_path)?;
        let file_len = file.metadata()?.len();
        if file_len == 0 {
            return Ok(Vec::new());
        }

        let read_from = file_len.saturating_sub(buf_size);
        let mut buf = Vec::new();
        let mut reader = BufReader::new(file);
        reader.seek(SeekFrom::Start(read_from))?;
        reader.read_to_end(&mut buf)?;

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

/// Summary of a session file for listing purposes.
pub struct SessionSummary {
    pub session_id: String,
    pub created_at: i64,
    pub entry_count: usize,
}

impl FileSessionEntryStore {
    /// Scan `{entry_dir}/{agent_type}/*.jsonl` and return session summaries.
    pub fn list_sessions(&self) -> std::io::Result<Vec<SessionSummary>> {
        let mut summaries = Vec::new();
        let scan_dir = self.session_dir();
        let dir = match std::fs::read_dir(&scan_dir) {
            Ok(d) => d,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(summaries),
            Err(e) => return Err(e),
        };

        for entry in dir {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
                continue;
            }
            if !path.is_file() {
                continue;
            }

            let session_id = match path.file_stem().and_then(|s| s.to_str()) {
                Some(id) => id.to_string(),
                None => continue,
            };

            let file = std::fs::File::open(&path)?;
            let reader = std::io::BufReader::new(file);
            let mut count = 0;
            let mut created_at: Option<i64> = None;

            for line in reader.lines() {
                let line = line?;
                if line.trim().is_empty() {
                    continue;
                }
                if created_at.is_none() {
                    if let Some(parsed) = Self::from_json(&line) {
                        created_at = Some(parsed.created_at);
                    }
                }
                count += 1;
            }

            if let Some(ts) = created_at {
                summaries.push(SessionSummary {
                    session_id,
                    created_at: ts,
                    entry_count: count,
                });
            }
        }

        summaries.sort_by_key(|a| std::cmp::Reverse(a.created_at));
        Ok(summaries)
    }
}

#[async_trait]
impl SessionEntryStore for FileSessionEntryStore {
    async fn save(&self, entry: SessionEntry) -> Result<()> {
        let json = Self::to_json(&entry)?;
        self.append_line(&entry.session_id, &json)
            .map_err(StoreError::Io)
    }

    async fn get_entries(&self, session_id: &str) -> Result<Vec<SessionEntry>> {
        self.read_from_head(session_id, usize::MAX)
            .map_err(StoreError::Io)
    }

    async fn get_after(&self, session_id: &str, after: i64) -> Result<Vec<SessionEntry>> {
        let mut entries = Vec::new();
        let file_path = self.file_path(session_id);
        if !file_path.exists() {
            return Ok(entries);
        }
        let file = File::open(&file_path).map_err(StoreError::Io)?;
        let reader = BufReader::new(file);
        for line in reader.lines() {
            let line = line.map_err(StoreError::Io)?;
            if line.trim().is_empty() {
                continue;
            }
            if let Some(entry) = Self::from_json(&line) {
                if entry.created_at >= after {
                    entries.push(entry);
                }
            }
        }
        Ok(entries)
    }

    async fn find_latest_checkpoint(&self, session_id: &str) -> Result<Option<SessionEntry>> {
        // Try reading from the tail first (last 64KB) — most likely to contain the latest checkpoint.
        let tail_entries = self
            .read_from_tail(session_id, 64 * 1024)
            .map_err(StoreError::Io)?;
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
            let all = self
                .read_from_head(session_id, usize::MAX)
                .map_err(StoreError::Io)?;
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

    async fn delete_session(&self, session_id: &str) -> Result<()> {
        let file_path = self.file_path(session_id);
        if file_path.exists() {
            fs::remove_file(&file_path).map_err(StoreError::Io)?;
        }
        // Drop the sidecar too, or metadata resurrects onto a later session
        // that reuses this id.
        match fs::remove_file(self.meta_path(session_id)) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(StoreError::Io(e)),
        }
        Ok(())
    }

    async fn get_count(&self, session_id: &str) -> Result<usize> {
        let file_path = self.file_path(session_id);
        if !file_path.exists() {
            return Ok(0);
        }
        let file = File::open(&file_path).map_err(StoreError::Io)?;
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

    async fn get_session_metadata(
        &self,
        session_id: &str,
    ) -> Result<serde_json::Map<String, serde_json::Value>> {
        let path = self.meta_path(session_id);
        match std::fs::read_to_string(&path) {
            // A malformed sidecar degrades to empty rather than making the
            // session unreadable.
            Ok(raw) => Ok(serde_json::from_str(&raw).unwrap_or_default()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(serde_json::Map::new()),
            Err(e) => Err(StoreError::Io(e)),
        }
    }

    async fn merge_session_metadata(
        &self,
        session_id: &str,
        patch: serde_json::Map<String, serde_json::Value>,
    ) -> Result<()> {
        // The mutex covers in-process concurrency only; sessions are
        // single-writer in practice.
        let _guard = self.meta_write_lock.lock().await;

        let mut merged = self.get_session_metadata(session_id).await?;
        for (k, v) in patch {
            merged.insert(k, v);
        }

        self.ensure_dir()?;
        let path = self.meta_path(session_id);
        let tmp = self.meta_tmp_path(session_id);
        let encoded =
            serde_json::to_string(&merged).map_err(|e| StoreError::Serialization(e.to_string()))?;

        // Temp file plus rename: a crash mid-write cannot leave a half-written
        // sidecar behind.
        std::fs::write(&tmp, encoded)?;
        std::fs::rename(&tmp, &path)?;
        Ok(())
    }
}

#[cfg(test)]
mod entry_tests {
    use super::*;
    use crate::entry::{SessionEntry, SessionEntryType};
    use crate::message::SessionMessage;
    use crate::CheckpointReason;
    use tempfile::tempdir;
    use vol_llm_core::Message;

    #[tokio::test]
    async fn test_file_entry_store_save_and_get() {
        let temp_dir = tempdir().unwrap();
        let store = FileSessionEntryStore::new(temp_dir.path());

        let entry = SessionEntry::from_message(SessionMessage::new(
            "test-session".to_string(),
            Message::user("Hello, World!"),
        ));

        store.save(entry.clone()).await.unwrap();

        let entries = store.get_entries("test-session").await.unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].r#type, SessionEntryType::Message);
    }

    #[tokio::test]
    async fn test_file_entry_store_find_checkpoint() {
        let temp_dir = tempdir().unwrap();
        let store = FileSessionEntryStore::new(temp_dir.path());

        let mut before = SessionEntry::from_message(SessionMessage::new(
            "test-session".to_string(),
            Message::user("before"),
        ));
        before.created_at = 1000;

        let mut checkpoint = SessionEntry::new_checkpoint(
            "test-session".to_string(),
            CheckpointReason::Compression,
            None,
        );
        checkpoint.created_at = 1001;

        let mut after = SessionEntry::from_message(SessionMessage::new(
            "test-session".to_string(),
            Message::user("after"),
        ));
        after.created_at = 1002;

        store.save(before).await.unwrap();
        store.save(checkpoint).await.unwrap();
        store.save(after).await.unwrap();

        let cp = store
            .find_latest_checkpoint("test-session")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(cp.r#type, SessionEntryType::Checkpoint);

        let entries = store
            .get_after("test-session", cp.created_at)
            .await
            .unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].r#type, SessionEntryType::Checkpoint);
        assert_eq!(entries[1].r#type, SessionEntryType::Message);
    }

    #[tokio::test]
    async fn test_file_entry_store_delete_session() {
        let temp_dir = tempdir().unwrap();
        let store = FileSessionEntryStore::new(temp_dir.path());

        store
            .save(SessionEntry::from_message(SessionMessage::new(
                "test-session".to_string(),
                Message::user("test"),
            )))
            .await
            .unwrap();

        store.delete_session("test-session").await.unwrap();
        let count = store.get_count("test-session").await.unwrap();
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn test_file_entry_store_skips_bad_lines() {
        let temp_dir = tempdir().unwrap();
        let store = FileSessionEntryStore::new(temp_dir.path());

        let entry1 = SessionEntry::from_message(SessionMessage::new(
            "test-session".to_string(),
            Message::user("hello"),
        ));
        store.save(entry1).await.unwrap();

        std::fs::OpenOptions::new()
            .append(true)
            .open(&store.file_path("test-session"))
            .unwrap()
            .write_all(b"this is not valid json\n")
            .unwrap();

        let entry2 = SessionEntry::from_message(SessionMessage::new(
            "test-session".to_string(),
            Message::user("world"),
        ));
        store.save(entry2).await.unwrap();

        let entries = store.get_entries("test-session").await.unwrap();
        assert_eq!(entries.len(), 2);
    }

    #[tokio::test]
    async fn test_file_entry_store_read_from_tail() {
        let temp_dir = tempdir().unwrap();
        let store = FileSessionEntryStore::new(temp_dir.path());

        for i in 0..5 {
            let entry = SessionEntry::from_message(SessionMessage::new(
                "test-session".to_string(),
                Message::user(format!("msg-{i}")),
            ));
            store.save(entry).await.unwrap();
        }

        let tail = store.read_from_tail("test-session", 1024).unwrap();
        assert!(!tail.is_empty());
        assert_eq!(
            tail.last().unwrap().data.entry_type(),
            SessionEntryType::Message
        );
    }

    #[tokio::test]
    async fn test_file_entry_store_multiple_sessions() {
        let temp_dir = tempdir().unwrap();
        let store = FileSessionEntryStore::new(temp_dir.path());

        store
            .save(SessionEntry::from_message(SessionMessage::new(
                "session-a".to_string(),
                Message::user("from A"),
            )))
            .await
            .unwrap();

        store
            .save(SessionEntry::from_message(SessionMessage::new(
                "session-b".to_string(),
                Message::user("from B"),
            )))
            .await
            .unwrap();

        let entries_a = store.get_entries("session-a").await.unwrap();
        assert_eq!(entries_a.len(), 1);
        assert_eq!(entries_a[0].session_id, "session-a");

        let entries_b = store.get_entries("session-b").await.unwrap();
        assert_eq!(entries_b.len(), 1);
        assert_eq!(entries_b[0].session_id, "session-b");

        // Deleting A should not affect B
        store.delete_session("session-a").await.unwrap();
        assert_eq!(store.get_count("session-b").await.unwrap(), 1);
    }

    #[tokio::test]
    async fn test_file_entry_store_list_sessions() {
        let temp_dir = tempdir().unwrap();
        let store = FileSessionEntryStore::new(temp_dir.path());

        let entry_a = SessionEntry::from_message(SessionMessage::new(
            "session-a".to_string(),
            Message::user("hello"),
        ));
        store.save(entry_a).await.unwrap();

        let entry_b = SessionEntry::from_message(SessionMessage::new(
            "session-b".to_string(),
            Message::user("world"),
        ));
        store.save(entry_b).await.unwrap();

        let summaries = store.list_sessions().unwrap();
        assert_eq!(summaries.len(), 2);
        assert_eq!(summaries[0].entry_count, 1);
        assert_eq!(summaries[1].entry_count, 1);
    }

    #[tokio::test]
    async fn test_file_entry_store_list_sessions_empty_dir() {
        let temp_dir = tempdir().unwrap();
        let store = FileSessionEntryStore::new(temp_dir.path());
        let summaries = store.list_sessions().unwrap();
        assert!(summaries.is_empty());
    }

    #[tokio::test]
    async fn test_file_entry_store_with_agent_type() {
        let temp_dir = tempdir().unwrap();
        let store = FileSessionEntryStore::with_agent_type(temp_dir.path(), Some("qa".to_string()));

        let entry = SessionEntry::from_message(SessionMessage::new(
            "test-session".to_string(),
            Message::user("Hello from QA"),
        ));
        store.save(entry).await.unwrap();

        let expected_path = temp_dir.path().join("qa").join("test-session.jsonl");
        assert!(
            expected_path.exists(),
            "File should exist at {}/qa/test-session.jsonl",
            temp_dir.path().display()
        );

        let entries = store.get_entries("test-session").await.unwrap();
        assert_eq!(entries.len(), 1);
    }

    #[tokio::test]
    async fn test_file_entry_store_agent_type_isolation() {
        let temp_dir = tempdir().unwrap();
        let qa_store =
            FileSessionEntryStore::with_agent_type(temp_dir.path(), Some("qa".to_string()));
        let coding_store =
            FileSessionEntryStore::with_agent_type(temp_dir.path(), Some("coding".to_string()));

        qa_store
            .save(SessionEntry::from_message(SessionMessage::new(
                "shared-session".to_string(),
                Message::user("from QA"),
            )))
            .await
            .unwrap();

        coding_store
            .save(SessionEntry::from_message(SessionMessage::new(
                "shared-session".to_string(),
                Message::user("from Coding"),
            )))
            .await
            .unwrap();

        // QA store should only see QA entries
        let qa_entries = qa_store.get_entries("shared-session").await.unwrap();
        assert_eq!(qa_entries.len(), 1);
        assert_eq!(qa_entries[0].data.entry_type(), SessionEntryType::Message);

        // Coding store should only see Coding entries
        let coding_entries = coding_store.get_entries("shared-session").await.unwrap();
        assert_eq!(coding_entries.len(), 1);
        assert_eq!(
            coding_entries[0].data.entry_type(),
            SessionEntryType::Message
        );

        // Files should be in different directories
        assert!(temp_dir
            .path()
            .join("qa")
            .join("shared-session.jsonl")
            .exists());
        assert!(temp_dir
            .path()
            .join("coding")
            .join("shared-session.jsonl")
            .exists());
    }

    #[tokio::test]
    async fn test_file_entry_store_no_agent_type_backward_compat() {
        let temp_dir = tempdir().unwrap();
        let store = FileSessionEntryStore::new(temp_dir.path());

        let entry = SessionEntry::from_message(SessionMessage::new(
            "test-session".to_string(),
            Message::user("backward compat"),
        ));
        store.save(entry).await.unwrap();

        // File should be directly in entry_dir, not in a subdirectory
        let expected_path = temp_dir.path().join("test-session.jsonl");
        assert!(
            expected_path.exists(),
            "File should exist at original path {}",
            expected_path.display()
        );

        let entries = store.get_entries("test-session").await.unwrap();
        assert_eq!(entries.len(), 1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::SessionMessage;
    use vol_llm_core::Message;

    fn sample_entry(session_id: &str) -> SessionEntry {
        SessionEntry::from_message(SessionMessage::new(
            session_id.to_string(),
            Message::user("hello"),
        ))
    }

    #[test]
    fn test_meta_path_shape() {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = FileSessionEntryStore::with_agent_type(
            dir.path().to_path_buf(),
            Some("agent-a".into()),
        );
        assert_eq!(
            store.meta_path("s1"),
            dir.path().join("agent-a").join("s1.meta.json")
        );
        assert_eq!(
            store.meta_path("a.b"),
            dir.path().join("agent-a").join("a.b.meta.json")
        );
    }

    #[tokio::test]
    async fn test_file_metadata_round_trip() {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = FileSessionEntryStore::with_agent_type(
            dir.path().to_path_buf(),
            Some("agent-a".into()),
        );

        let mut patch = serde_json::Map::new();
        patch.insert("task_ids".into(), serde_json::json!(["1", "2"]));
        store
            .merge_session_metadata("s1", patch)
            .await
            .expect("merge");

        let meta = store.get_session_metadata("s1").await.expect("get");
        assert_eq!(meta["task_ids"], serde_json::json!(["1", "2"]));
    }

    #[tokio::test]
    async fn test_file_metadata_merge_is_shallow() {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = FileSessionEntryStore::with_agent_type(
            dir.path().to_path_buf(),
            Some("agent-a".into()),
        );

        let mut first = serde_json::Map::new();
        first.insert("a".into(), serde_json::json!(1));
        first.insert("task_ids".into(), serde_json::json!(["1"]));
        store
            .merge_session_metadata("s1", first)
            .await
            .expect("first");

        let mut second = serde_json::Map::new();
        second.insert("task_ids".into(), serde_json::json!(["1", "2"]));
        store
            .merge_session_metadata("s1", second)
            .await
            .expect("second");

        let meta = store.get_session_metadata("s1").await.expect("get");
        assert_eq!(meta["a"], serde_json::json!(1));
        assert_eq!(meta["task_ids"], serde_json::json!(["1", "2"]));
    }

    #[tokio::test]
    async fn test_file_metadata_upserts_with_no_jsonl_present() {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = FileSessionEntryStore::with_agent_type(
            dir.path().to_path_buf(),
            Some("agent-a".into()),
        );

        let mut patch = serde_json::Map::new();
        patch.insert("k".into(), serde_json::json!("v"));
        store
            .merge_session_metadata("fresh", patch)
            .await
            .expect("merge");

        assert_eq!(
            store.get_session_metadata("fresh").await.expect("get")["k"],
            serde_json::json!("v")
        );
    }

    #[tokio::test]
    async fn test_file_metadata_unknown_session_is_empty() {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = FileSessionEntryStore::with_agent_type(
            dir.path().to_path_buf(),
            Some("agent-a".into()),
        );
        assert!(store
            .get_session_metadata("nope")
            .await
            .expect("get")
            .is_empty());
    }

    #[tokio::test]
    async fn test_file_metadata_malformed_sidecar_degrades_to_empty() {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = FileSessionEntryStore::with_agent_type(
            dir.path().to_path_buf(),
            Some("agent-a".into()),
        );
        let agent_dir = dir.path().join("agent-a");
        std::fs::create_dir_all(&agent_dir).expect("mkdir");
        std::fs::write(agent_dir.join("s1.meta.json"), "{ truncated").expect("write");

        assert!(store
            .get_session_metadata("s1")
            .await
            .expect("get")
            .is_empty());
    }

    #[tokio::test]
    async fn test_file_metadata_without_agent_type() {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = FileSessionEntryStore::new(dir.path());

        let mut patch = serde_json::Map::new();
        patch.insert("k".into(), serde_json::json!("v"));
        store
            .merge_session_metadata("s1", patch)
            .await
            .expect("merge");

        assert!(dir.path().join("s1.meta.json").exists());
        assert_eq!(
            store.get_session_metadata("s1").await.expect("get")["k"],
            serde_json::json!("v")
        );
    }

    #[tokio::test]
    async fn test_list_sessions_ignores_sidecar_files() {
        // Regression guard: list_sessions filters on the "jsonl" extension
        // (file_store.rs:225-227). A sidecar must never appear as a session
        // named "s1.meta".
        let dir = tempfile::tempdir().expect("tempdir");
        let store = FileSessionEntryStore::with_agent_type(
            dir.path().to_path_buf(),
            Some("agent-a".into()),
        );

        store.save(sample_entry("s1")).await.expect("save");
        let mut patch = serde_json::Map::new();
        patch.insert("k".into(), serde_json::json!("v"));
        store
            .merge_session_metadata("s1", patch)
            .await
            .expect("merge");

        let sessions = store.list_sessions().expect("list");
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].session_id, "s1");
    }

    #[tokio::test]
    async fn test_delete_session_removes_sidecar() {
        // Otherwise metadata resurrects onto a later session reusing the id.
        let dir = tempfile::tempdir().expect("tempdir");
        let store = FileSessionEntryStore::with_agent_type(
            dir.path().to_path_buf(),
            Some("agent-a".into()),
        );

        store.save(sample_entry("s1")).await.expect("save");
        let mut patch = serde_json::Map::new();
        patch.insert("k".into(), serde_json::json!("v"));
        store
            .merge_session_metadata("s1", patch)
            .await
            .expect("merge");

        store.delete_session("s1").await.expect("delete");

        assert!(store
            .get_session_metadata("s1")
            .await
            .expect("get")
            .is_empty());
        assert!(!dir.path().join("agent-a").join("s1.meta.json").exists());
    }

    #[tokio::test]
    async fn test_delete_session_without_sidecar_is_ok() {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = FileSessionEntryStore::with_agent_type(
            dir.path().to_path_buf(),
            Some("agent-a".into()),
        );

        store.save(sample_entry("s1")).await.expect("save");
        store.delete_session("s1").await.expect("delete");
        store.delete_session("never-existed").await.expect("delete");
    }
}
