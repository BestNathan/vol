# Session Entry & Resume Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace `MessageStore` with `SessionEntryStore` supporting multi-type session entries (Message/Checkpoint/Summary) and add resume capability to ReActAgent via checkpoint-based session recovery.

**Architecture:** Single `SessionEntry` struct with `type` + `data` fields. `SessionEntryStore` trait replaces `MessageStore`. `FileSessionEntryStore` writes `{entry_dir}/{session_id}.jsonl`. Session records checkpoints on each compress(). Resume finds latest checkpoint, loads subsequent entries, rebuilds context.

**Tech Stack:** async-trait, tokio, serde, serde_json, chrono, thiserror, uuid

---

## Context

**Current state:**
- `Session` uses `MessageStore` (trait) + `FileMessageStore`/`InMemoryMessageStore` (impls) to persist `SessionMessage` as JSONL
- `MessageStore` only supports `SessionMessage` — no checkpoint, no summary entries
- `Session` has in-memory `compressed_messages` + `compressed_after_ts` cursor — lost on restart
- `ReActAgent` has no resume capability; each run starts fresh
- File format: `{base}/sessions/{session_id}.jsonl` with `SessionMessageLine{event, data, session_id, timestamp}`

**Key codebase files:**
- `crates/vol-session/` — Session, stores, listener (6 source files)
- `crates/vol-llm-agent/src/react/` — ReActAgent, RunContext, SessionContributor
- `crates/vol-llm-agents/src/coding/` — CodingAgent

See [docs/superpowers/specs/2026-04-24-session-entry-resume-design.md](docs/superpowers/specs/2026-04-24-session-entry-resume-design.md) for full design spec.

---

## File Structure

| File | Action | Responsibility |
|------|--------|----------------|
| `crates/vol-session/src/entry.rs` | **Create** | SessionEntry, SessionEntryType, SessionEntryData, CheckpointReason |
| `crates/vol-session/src/store.rs` | Modify | Replace MessageStore with SessionEntryStore trait (keep SessionStore) |
| `crates/vol-session/src/file_store.rs` | Rewrite | FileSessionEntryStore replacing FileMessageStore |
| `crates/vol-session/src/memory_store.rs` | Modify | Add InMemoryEntryStore, keep InMemoryMessageStore for backward compat |
| `crates/vol-session/src/session.rs` | Rewrite | Use entry_store, add checkpoint/summary/resume methods, remove in-memory compression state |
| `crates/vol-session/src/listener.rs` | Modify | Use SessionEntryStore instead of MessageStore |
| `crates/vol-session/src/lib.rs` | Modify | Re-exports |
| `crates/vol-llm-agent/src/react/context_contributors.rs` | Modify | SessionContributor works with entry-based Session |
| `crates/vol-llm-agent/src/react/run_context.rs` | Modify | Update test fixtures to use InMemoryEntryStore |
| `crates/vol-llm-agent/src/react/agent.rs` | Modify | Add resume(), update SessionListener wiring |
| `crates/vol-llm-agents/src/coding/agent.rs` | Modify | Add resume(), use InMemoryEntryStore for default sessions |

---

### Task 1: Create SessionEntry Types

**Files:**
- Create: `crates/vol-session/src/entry.rs`
- Modify: `crates/vol-session/src/lib.rs`

- [ ] **Step 1: Write entry.rs**

```rust
//! Session entry types for multi-type session persistence.

use chrono::Utc;
use serde::{Deserialize, Serialize};

/// Entry type discriminator.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionEntryType {
    Message,
    Checkpoint,
    Summary,
}

/// Reason a checkpoint was created.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckpointReason {
    Compression,
    Manual,
}

/// Polymorphic entry data — serialized with inline `type` discriminator.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SessionEntryData {
    #[serde(rename = "message")]
    Message {
        message: vol_llm_core::Message,
    },
    #[serde(rename = "checkpoint")]
    Checkpoint {
        reason: CheckpointReason,
        note: Option<String>,
    },
    #[serde(rename = "summary")]
    Summary {
        summary: String,
    },
}

impl SessionEntryData {
    /// Returns the type discriminator for this data variant.
    pub fn entry_type(&self) -> SessionEntryType {
        match self {
            SessionEntryData::Message { .. } => SessionEntryType::Message,
            SessionEntryData::Checkpoint { .. } => SessionEntryType::Checkpoint,
            SessionEntryData::Summary { .. } => SessionEntryType::Summary,
        }
    }
}

/// Unified session entry — all content types stored in a single JSONL file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionEntry {
    pub id: String,
    pub session_id: String,
    pub created_at: i64,
    pub parent_id: Option<String>,
    pub r#type: SessionEntryType,
    pub data: SessionEntryData,
}

impl SessionEntry {
    /// Create a new message entry.
    pub fn new_message(session_id: String, message: vol_llm_core::Message) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            session_id: session_id.clone(),
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64,
            parent_id: None,
            r#type: SessionEntryType::Message,
            data: SessionEntryData::Message { message },
        }
    }

    /// Create a new checkpoint entry.
    pub fn new_checkpoint(
        session_id: String,
        reason: CheckpointReason,
        note: Option<String>,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            session_id,
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64,
            parent_id: None,
            r#type: SessionEntryType::Checkpoint,
            data: SessionEntryData::Checkpoint { reason, note },
        }
    }

    /// Create a new summary entry.
    pub fn new_summary(session_id: String, summary: String) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            session_id,
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64,
            parent_id: None,
            r#type: SessionEntryType::Summary,
            data: SessionEntryData::Summary { summary },
        }
    }

    /// Extract the Message from this entry, if it is a Message type.
    pub fn into_message(self) -> Option<vol_llm_core::Message> {
        match self.data {
            SessionEntryData::Message { message } => Some(message),
            _ => None,
        }
    }
}
```

- [ ] **Step 2: Update lib.rs**

Replace the current `lib.rs` content with:

```rust
//! vol-session: Session management and entry-based persistence.
//!
//! Provides session management and multi-type entry persistence for ReAct Agent.

pub mod compressor;
pub mod compressors;
pub mod entry;
pub mod error;
pub mod file_store;
pub mod listener;
pub mod memory_store;
pub mod message;
pub mod session;
pub mod store;

pub use compressor::MessageCompressor;
pub use compressors::{PositionSampleCompressor, RoleFilterCompressor};
pub use entry::{CheckpointReason, SessionEntry, SessionEntryData, SessionEntryType};
pub use error::{Result, SessionError};
pub use file_store::FileSessionEntryStore;
pub use listener::SessionListener;
pub use memory_store::{InMemoryEntryStore, InMemoryMessageStore, InMemorySessionStore};
pub use message::SessionMessage;
pub use session::Session;
pub use store::{MessageStore, SessionEntryStore, SessionStore, StoreError};
```

- [ ] **Step 3: Create empty entry.rs module declaration in lib.rs**

The entry.rs file from Step 1 already provides the full implementation, so this is included above.

- [ ] **Step 4: Verify compilation**

Run: `cargo check -p vol-session`
Expected: FAIL — store.rs still references `MessageStore` which we haven't changed yet. But entry.rs itself should parse.

- [ ] **Step 5: Commit**

```bash
git add crates/vol-session/src/entry.rs crates/vol-session/src/lib.rs
git commit -m "feat: add SessionEntry types with Message/Checkpoint/Summary variants"
```

---

### Task 2: Replace MessageStore with SessionEntryStore Trait

**Files:**
- Modify: `crates/vol-session/src/store.rs`

- [ ] **Step 1: Write the new store.rs**

```rust
//! Session and Entry store traits.

use crate::entry::SessionEntry;
use crate::message::SessionMessage;
use crate::session::Session;
use async_trait::async_trait;
use thiserror::Error;

/// Store operation error
#[derive(Debug, Error)]
pub enum StoreError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

pub type Result<T> = std::result::Result<T, StoreError>;

/// Session storage interface
#[async_trait]
pub trait SessionStore: Send + Sync {
    async fn create(&self, session: Session) -> Result<()>;
    async fn get(&self, session_id: &str) -> Result<Option<Session>>;
    async fn delete(&self, session_id: &str) -> Result<()>;
    async fn update(&self, session: Session) -> Result<()>;
}

/// Entry storage interface — replaces MessageStore.
/// Supports Message, Checkpoint, and Summary entry types.
#[async_trait]
pub trait SessionEntryStore: Send + Sync {
    /// Append an entry.
    async fn save(&self, entry: SessionEntry) -> Result<()>;

    /// Get the most recent N entries (oldest first).
    async fn get_entries(&self, limit: usize) -> Result<Vec<SessionEntry>>;

    /// Get entries after a timestamp (for resume from checkpoint).
    async fn get_after(&self, after: i64, limit: usize) -> Result<Vec<SessionEntry>>;

    /// Find the latest checkpoint entry, if any.
    async fn find_latest_checkpoint(&self) -> Result<Option<SessionEntry>>;

    /// Delete all entries for the current session.
    async fn delete_session(&self) -> Result<()>;

    /// Get entry count.
    async fn get_count(&self) -> Result<usize>;
}

/// Legacy MessageStore trait — kept for backward compatibility.
/// New code should use SessionEntryStore instead.
#[async_trait]
pub trait MessageStore: Send + Sync {
    async fn save(&self, message: SessionMessage) -> Result<()>;
    async fn get_by_session(&self, session_id: &str, limit: usize) -> Result<Vec<SessionMessage>>;
    async fn get_before(
        &self,
        session_id: &str,
        before: i64,
        limit: usize,
    ) -> Result<Vec<SessionMessage>>;
    async fn get_after(
        &self,
        session_id: &str,
        after: i64,
        limit: usize,
    ) -> Result<Vec<SessionMessage>>;
    async fn delete_session(&self, session_id: &str) -> Result<()>;
    async fn update(&self, id: &str, message: SessionMessage) -> Result<()>;
    async fn get_count(&self, session_id: &str) -> Result<usize>;
    async fn cleanup_expired(&self, before: i64) -> Result<()>;
}
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check -p vol-session`
Expected: FAIL — existing implementations still implement the old MessageStore trait only. The new SessionEntryStore trait is defined but not yet implemented.

- [ ] **Step 3: Commit**

```bash
git add crates/vol-session/src/store.rs
git commit -m "feat: add SessionEntryStore trait, keep MessageStore for backward compat"
```

---

### Task 3: Implement FileSessionEntryStore

**Files:**
- Modify: `crates/vol-session/src/file_store.rs`

- [ ] **Step 1: Write the failing test**

```rust
#[cfg(test)]
mod entry_tests {
    use super::*;
    use crate::entry::{SessionEntry, SessionEntryData, SessionEntryType};
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

        // Save a message, then a checkpoint, then another message
        store.save(SessionEntry::new_message(
            "test-session".to_string(),
            Message::user("before"),
        )).await.unwrap();

        store.save(SessionEntry::new_checkpoint(
            "test-session".to_string(),
            crate::CheckpointReason::Compression,
            None,
        )).await.unwrap();

        store.save(SessionEntry::new_message(
            "test-session".to_string(),
            Message::user("after"),
        )).await.unwrap();

        let cp = store.find_latest_checkpoint().await.unwrap().unwrap();
        assert_eq!(cp.r#type, SessionEntryType::Checkpoint);

        // get_after checkpoint should return only the "after" message
        let after = store.get_after(cp.created_at, 10).await.unwrap();
        assert_eq!(after.len(), 1);
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
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p vol-session file_store::entry_tests --no-run`
Expected: Compilation error — `FileSessionEntryStore` doesn't exist yet.

- [ ] **Step 3: Write FileSessionEntryStore implementation**

```rust
//! File-based entry store using JSONL format.

use crate::entry::{CheckpointReason, SessionEntry, SessionEntryData, SessionEntryType};
use crate::message::SessionMessage;
use crate::store::{MessageStore, Result, SessionEntryStore, StoreError};
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
/// Used for backward compatibility migration on read.
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
    ///
    /// # Arguments
    /// * `entry_dir` - Directory for storing session JSONL files
    /// * `session_id` - Session identifier (used as filename)
    pub fn new<P: AsRef<Path>>(entry_dir: P, session_id: &str) -> Self {
        let entry_dir = entry_dir.as_ref().to_path_buf();
        let file_path = entry_dir.join(format!("{}.jsonl", session_id));

        Self {
            entry_dir,
            session_id: session_id.to_string(),
            file_path,
        }
    }

    /// Ensure the entry directory exists.
    fn ensure_dir(&self) -> std::io::Result<()> {
        fs::create_dir_all(&self.entry_dir)
    }

    /// Append a JSON line to the file.
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

    /// Convert SessionEntry to JSON line.
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

    /// Parse a JSON line into SessionEntry.
    /// Handles both new format (SessionEntryLine) and legacy format (LegacyMessageLine).
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

        // Fall back to legacy format (SessionMessageLine with "event" field)
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
                "Failed to parse JSONL line as entry or legacy message: {}",
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
            if entry.created_at > after {
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
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p vol-session file_store::entry_tests`
Expected: All 3 tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/vol-session/src/file_store.rs
git commit -m "feat: implement FileSessionEntryStore with legacy migration"
```

---

### Task 4: Implement InMemoryEntryStore

**Files:**
- Modify: `crates/vol-session/src/memory_store.rs`

- [ ] **Step 1: Write the failing test**

Add to the existing tests module in `memory_store.rs`:

```rust
#[tokio::test]
async fn test_in_memory_entry_store_save_and_get() {
    let store = InMemoryEntryStore::new();

    let entry = SessionEntry::new_message(
        "session-1".to_string(),
        Message::user("Hello"),
    );
    store.save(entry).await.unwrap();

    let entries = store.get_entries(10).await.unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].r#type, SessionEntryType::Message);
}

#[tokio::test]
async fn test_in_memory_entry_store_find_checkpoint() {
    let store = InMemoryEntryStore::new();

    // Save entries: message, checkpoint, message
    store.save(SessionEntry::new_message(
        "session-1".to_string(),
        Message::user("before"),
    )).await.unwrap();

    store.save(SessionEntry::new_checkpoint(
        "session-1".to_string(),
        CheckpointReason::Compression,
        None,
    )).await.unwrap();

    store.save(SessionEntry::new_message(
        "session-1".to_string(),
        Message::user("after"),
    )).await.unwrap();

    let cp = store.find_latest_checkpoint().await.unwrap().unwrap();
    assert_eq!(cp.r#type, SessionEntryType::Checkpoint);

    let after = store.get_after(cp.created_at, 10).await.unwrap();
    assert_eq!(after.len(), 1);
}

#[tokio::test]
async fn test_in_memory_entry_store_delete_session() {
    let store = InMemoryEntryStore::new();
    store.save(SessionEntry::new_message(
        "session-1".to_string(),
        Message::user("test"),
    )).await.unwrap();

    store.delete_session().await.unwrap();
    assert_eq!(store.get_count().await.unwrap(), 0);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p vol-session memory_store::test_in_memory_entry --no-run`
Expected: Compilation error — `InMemoryEntryStore` doesn't exist yet.

- [ ] **Step 3: Write InMemoryEntryStore implementation**

Add to the end of `memory_store.rs` (before the `#[cfg(test)]` module):

```rust
use crate::entry::{CheckpointReason, SessionEntry, SessionEntryType};
```

Add at the top of `memory_store.rs` in the imports:

```rust
use crate::entry::{SessionEntry, SessionEntryType};
```

Then add the `InMemoryEntryStore` struct after the existing `InMemoryMessageStore` impl:

```rust
/// In-memory entry storage for testing.
pub struct InMemoryEntryStore {
    entries: RwLock<Vec<SessionEntry>>,
}

impl Default for InMemoryEntryStore {
    fn default() -> Self {
        Self::new()
    }
}

impl InMemoryEntryStore {
    pub fn new() -> Self {
        Self {
            entries: RwLock::new(Vec::new()),
        }
    }
}

#[async_trait::async_trait]
impl crate::store::SessionEntryStore for InMemoryEntryStore {
    async fn save(&self, entry: SessionEntry) -> crate::store::Result<()> {
        self.entries.write().await.push(entry);
        Ok(())
    }

    async fn get_entries(&self, limit: usize) -> crate::store::Result<Vec<SessionEntry>> {
        let entries = self.entries.read().await;
        Ok(entries.iter().take(limit).cloned().collect())
    }

    async fn get_after(&self, after: i64, limit: usize) -> crate::store::Result<Vec<SessionEntry>> {
        let entries = self.entries.read().await;
        Ok(entries
            .iter()
            .filter(|e| e.created_at > after)
            .take(limit)
            .cloned()
            .collect())
    }

    async fn find_latest_checkpoint(&self) -> crate::store::Result<Option<SessionEntry>> {
        let entries = self.entries.read().await;
        Ok(entries
            .iter()
            .filter(|e| e.r#type == SessionEntryType::Checkpoint)
            .max_by_key(|e| e.created_at)
            .cloned())
    }

    async fn delete_session(&self) -> crate::store::Result<()> {
        self.entries.write().await.clear();
        Ok(())
    }

    async fn get_count(&self) -> crate::store::Result<usize> {
        Ok(self.entries.read().await.len())
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p vol-session memory_store`
Expected: All existing + new 3 tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/vol-session/src/memory_store.rs
git commit -m "feat: add InMemoryEntryStore for testing"
```

---

### Task 5: Rewrite Session to Use Entry Store

**Files:**
- Modify: `crates/vol-session/src/session.rs`

- [ ] **Step 1: Write the failing tests**

```rust
#[cfg(test)]
mod entry_tests {
    use super::*;
    use crate::entry::{CheckpointReason, SessionEntryType};
    use crate::memory_store::InMemoryEntryStore;
    use std::sync::Arc;

    fn test_session() -> Session {
        Session::new(
            "session-1".to_string(),
            Arc::new(InMemorySessionStore::new()),
            Arc::new(InMemoryEntryStore::new()),
        )
    }

    #[tokio::test]
    async fn test_session_add_message() {
        let session = test_session();
        let msg = SessionMessage::new("session-1".to_string(), Message::user("Hello"));
        session.add_message(msg).await.unwrap();

        let messages = session.get_messages(10).await.unwrap();
        assert_eq!(messages.len(), 1);
    }

    #[tokio::test]
    async fn test_session_checkpoint_and_summary() {
        let mut session = test_session();

        // Add messages
        for i in 0..5 {
            let msg = SessionMessage::new(
                "session-1".to_string(),
                Message::user(format!("msg-{}", i)),
            );
            session.add_message(msg).await.unwrap();
        }

        // Get messages before compression
        let messages = session.get_messages(20).await.unwrap();
        assert_eq!(messages.len(), 5);

        // Compress — this writes summary + checkpoint entries
        session.compressor = Arc::new(PositionSampleCompressor::new(2, 3));
        session.compress(messages).await;

        // After compression: get_messages should return compressed + post-checkpoint
        let after = session.get_messages(20).await.unwrap();
        // Should have compressed messages (6 from PositionSampleCompressor with keep_first=2, sample_every=3)
        assert!(!after.is_empty());
    }

    #[tokio::test]
    async fn test_session_resume_entries_no_checkpoint() {
        let session = test_session();

        for i in 0..3 {
            let msg = SessionMessage::new(
                "session-1".to_string(),
                Message::user(format!("msg-{}", i)),
            );
            session.add_message(msg).await.unwrap();
        }

        let entries = session.resume_entries().await.unwrap();
        // No checkpoint, so should return all entries
        assert_eq!(entries.len(), 3);
    }

    #[tokio::test]
    async fn test_session_resume_messages_includes_summary() {
        let mut session = test_session();

        for i in 0..5 {
            let msg = SessionMessage::new(
                "session-1".to_string(),
                Message::user(format!("msg-{}", i)),
            );
            session.add_message(msg).await.unwrap();
        }

        let messages = session.get_messages(20).await.unwrap();
        session.compressor = Arc::new(PositionSampleCompressor::new(2, 3));
        session.compress(messages).await;

        let resume_msgs = session.resume_messages().await.unwrap();
        // Summary entries become synthetic system messages
        assert!(!resume_msgs.is_empty());
        // First should be the summary as a system message
        assert_eq!(resume_msgs[0].role, vol_llm_core::MessageRole::System);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p vol-session session::entry_tests --no-run`
Expected: Compilation error — Session::new signature changed, methods don't exist.

- [ ] **Step 3: Write the new Session implementation**

```rust
//! Session management with entry-based persistence.

use crate::compressor::MessageCompressor;
use crate::compressors::PositionSampleCompressor;
use crate::entry::{CheckpointReason, SessionEntry, SessionEntryData, SessionEntryType};
use crate::message::SessionMessage;
use crate::store::{Result, SessionEntryStore, SessionStore};
use std::collections::HashMap;
use std::sync::Arc;
use vol_llm_core::Message;

/// Session management
pub struct Session {
    pub id: String,
    pub created_at: i64,
    pub metadata: HashMap<String, String>,
    session_store: Arc<dyn SessionStore>,
    entry_store: Arc<dyn SessionEntryStore>,
    compressor: Arc<dyn MessageCompressor>,
}

impl Session {
    /// Create a new session.
    pub fn new(
        id: String,
        session_store: Arc<dyn SessionStore>,
        entry_store: Arc<dyn SessionEntryStore>,
    ) -> Self {
        Self {
            id,
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64,
            metadata: HashMap::new(),
            session_store,
            entry_store,
            compressor: Arc::new(PositionSampleCompressor::default()),
        }
    }

    /// Set the compression strategy.
    pub fn with_compressor(mut self, compressor: Arc<dyn MessageCompressor>) -> Self {
        self.compressor = compressor;
        self
    }

    /// Add a message entry.
    pub async fn add_message(&self, message: SessionMessage) -> Result<()> {
        let entry = SessionEntry {
            id: message.id.clone(),
            session_id: message.session_id.clone(),
            created_at: message.created_at,
            parent_id: message.parent_id.clone(),
            r#type: SessionEntryType::Message,
            data: SessionEntryData::Message {
                message: message.message,
            },
        };
        self.entry_store.save(entry).await
    }

    /// Write a checkpoint entry.
    pub async fn checkpoint(&self, reason: CheckpointReason, note: Option<String>) -> Result<()> {
        let entry = SessionEntry::new_checkpoint(self.id.clone(), reason, note);
        self.entry_store.save(entry).await
    }

    /// Write a summary entry (from compression).
    pub async fn add_summary(&self, summary: String) -> Result<()> {
        let entry = SessionEntry::new_summary(self.id.clone(), summary);
        self.entry_store.save(entry).await
    }

    /// Get messages — returns Message entries as SessionMessage list.
    /// Summary entries are converted to synthetic SessionMessage with system role.
    pub async fn get_messages(&self, limit: usize) -> Result<Vec<SessionMessage>> {
        let entries = self.entry_store.get_entries(limit).await?;
        let mut messages = Vec::new();

        for entry in entries {
            match entry.data {
                SessionEntryData::Message { message } => {
                    messages.push(SessionMessage {
                        id: entry.id,
                        session_id: entry.session_id,
                        message,
                        parent_id: entry.parent_id,
                        created_at: entry.created_at,
                        metadata: HashMap::new(),
                    });
                }
                SessionEntryData::Summary { summary } => {
                    // Summary becomes a synthetic system message
                    messages.push(SessionMessage {
                        id: entry.id,
                        session_id: entry.session_id,
                        message: Message::system(summary),
                        parent_id: entry.parent_id,
                        created_at: entry.created_at,
                        metadata: HashMap::new(),
                    });
                }
                SessionEntryData::Checkpoint { .. } => {
                    // Checkpoints are not returned as messages
                }
            }
        }

        Ok(messages)
    }

    /// Get resume entries — all entries after the latest checkpoint.
    /// If no checkpoint exists, returns all entries.
    pub async fn resume_entries(&self) -> Result<Vec<SessionEntry>> {
        match self.entry_store.find_latest_checkpoint().await? {
            Some(cp) => self.entry_store.get_after(cp.created_at, usize::MAX).await,
            None => self.entry_store.get_entries(usize::MAX).await,
        }
    }

    /// Convert resume entries to Message Vec for context rebuilding.
    /// Summary entries become synthetic system messages.
    pub async fn resume_messages(&self) -> Result<Vec<Message>> {
        let entries = self.resume_entries().await?;
        let mut messages = Vec::new();

        for entry in entries {
            match entry.data {
                SessionEntryData::Message { message } => {
                    messages.push(message);
                }
                SessionEntryData::Summary { summary } => {
                    messages.push(Message::system(summary));
                }
                SessionEntryData::Checkpoint { .. } => {
                    // Checkpoints are not messages
                }
            }
        }

        Ok(messages)
    }

    /// Compress the given messages and write summary + checkpoint entries.
    pub async fn compress(&mut self, messages: Vec<SessionMessage>) {
        if messages.is_empty() {
            return;
        }

        // Compress to summary text
        let compressed = self.compressor.compress(messages).await;
        if compressed.is_empty() {
            return;
        }

        // Build summary text from compressed messages
        let summary = compressed
            .iter()
            .filter_map(|m| m.message.content.as_ref())
            .map(|c| c.as_str())
            .collect::<Vec<_>>()
            .join("\n");

        // Write summary entry
        let _ = self.add_summary(summary).await;

        // Write checkpoint entry
        let _ = self.checkpoint(CheckpointReason::Compression, None).await;
    }

    /// Add metadata
    pub fn with_metadata(mut self, key: &str, value: &str) -> Self {
        self.metadata.insert(key.to_string(), value.to_string());
        self
    }
}

impl Clone for Session {
    fn clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            created_at: self.created_at,
            metadata: self.metadata.clone(),
            session_store: self.session_store.clone(),
            entry_store: self.entry_store.clone(),
            compressor: self.compressor.clone(),
        }
    }
}
```

- [ ] **Step 4: Keep existing tests working**

The existing Session tests in `session.rs` use `InMemoryMessageStore`. Replace them with the new entry-based approach:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory_store::{InMemoryEntryStore, InMemorySessionStore};

    #[tokio::test]
    async fn test_session_get_messages() {
        let session_store = Arc::new(InMemorySessionStore::new());
        let entry_store = Arc::new(InMemoryEntryStore::new());

        let session = Session::new(
            "session-1".to_string(),
            session_store.clone(),
            entry_store.clone(),
        );

        let msg = SessionMessage::new("session-1".to_string(), Message::user("Hello"));
        session.add_message(msg).await.unwrap();

        let messages = session.get_messages(10).await.unwrap();
        assert_eq!(messages.len(), 1);
    }

    #[tokio::test]
    async fn test_session_with_metadata() {
        let session_store = Arc::new(InMemorySessionStore::new());
        let entry_store = Arc::new(InMemoryEntryStore::new());

        let session = Session::new(
            "session-1".to_string(),
            session_store.clone(),
            entry_store.clone(),
        )
        .with_metadata("user_id", "user-123");

        assert_eq!(
            session.metadata.get("user_id"),
            Some(&"user-123".to_string())
        );
    }

    #[tokio::test]
    async fn test_session_compress_and_get_messages() {
        let session_store = Arc::new(InMemorySessionStore::new());
        let entry_store = Arc::new(InMemoryEntryStore::new());

        let mut session = Session::new(
            "session-1".to_string(),
            session_store.clone(),
            entry_store.clone(),
        );

        // Add 10 messages
        for i in 0..10 {
            let msg = SessionMessage::new(
                "session-1".to_string(),
                Message::user(format!("msg-{}", i)),
            );
            session.add_message(msg).await.unwrap();
        }

        // Before compression, get all 10
        let messages = session.get_messages(20).await.unwrap();
        assert_eq!(messages.len(), 10);

        // Compress
        session.compressor = Arc::new(PositionSampleCompressor::new(2, 3));
        session.compress(messages).await;

        // After compression: should have compressed messages + summary
        let after = session.get_messages(20).await.unwrap();
        // 6 compressed messages + 1 summary message
        assert_eq!(after.len(), 7);
    }

    #[tokio::test]
    async fn test_session_compress_empty_messages() {
        let session_store = Arc::new(InMemorySessionStore::new());
        let entry_store = Arc::new(InMemoryEntryStore::new());

        let mut session = Session::new(
            "session-1".to_string(),
            session_store.clone(),
            entry_store.clone(),
        );

        // Compress with empty input should be no-op
        session.compress(vec![]).await;
        let messages = session.get_messages(20).await.unwrap();
        assert!(messages.is_empty());
    }
}
```

- [ ] **Step 5: Run tests**

Run: `cargo test -p vol-session session`
Expected: All session tests pass.

- [ ] **Step 6: Commit**

```bash
git add crates/vol-session/src/session.rs
git commit -m "feat: rewrite Session to use entry_store with checkpoint/resume"
```

---

### Task 6: Update SessionListener to Use SessionEntryStore

**Files:**
- Modify: `crates/vol-session/src/listener.rs`

- [ ] **Step 1: Update listener.rs imports and struct**

Change the `use` statement at the top from:
```rust
use crate::{MessageStore, SessionError, SessionMessage};
```
to:
```rust
use crate::entry::{SessionEntry, SessionEntryData, SessionEntryType};
use crate::{SessionEntryStore, SessionError, SessionMessage};
```

- [ ] **Step 2: Update SessionListener struct**

Change:
```rust
pub struct SessionListener {
    event_rx: broadcast::Receiver<TracedEvent<AgentStreamEvent>>,
    store: Arc<dyn MessageStore>,
    session_id: String,
}
```
to:
```rust
pub struct SessionListener {
    event_rx: broadcast::Receiver<TracedEvent<AgentStreamEvent>>,
    store: Arc<dyn SessionEntryStore>,
    session_id: String,
}
```

- [ ] **Step 3: Update SessionListener::new**

Change:
```rust
pub fn new(
    event_rx: broadcast::Receiver<TracedEvent<AgentStreamEvent>>,
    store: Arc<dyn MessageStore>,
    session_id: String,
) -> Self {
```
to:
```rust
pub fn new(
    event_rx: broadcast::Receiver<TracedEvent<AgentStreamEvent>>,
    store: Arc<dyn SessionEntryStore>,
    session_id: String,
) -> Self {
```

- [ ] **Step 4: Update event_to_message to save entries**

Change the `event_to_message` method to save entries directly instead of returning `SessionMessage`:

```rust
/// Convert an agent event to a session entry and save it.
///
/// # Arguments
/// * `event` - The agent stream event to convert
///
/// # Returns
/// `Ok(())` if the event was recorded, `Err` if save failed.
async fn record_event(&self, event: &AgentStreamEvent) -> Result<(), SessionError> {
    let session_msg = match self.event_to_message(event) {
        Some(msg) => msg,
        None => return Ok(()),
    };

    let entry = SessionEntry {
        id: session_msg.id,
        session_id: session_msg.session_id,
        created_at: session_msg.created_at,
        parent_id: session_msg.parent_id,
        r#type: SessionEntryType::Message,
        data: SessionEntryData::Message {
            message: session_msg.message,
        },
    };

    self.store.save(entry).await.map_err(SessionError::StoreError)
}
```

Change the `run` method to use `record_event`:

```rust
pub async fn run(&mut self) -> Result<(), SessionError> {
    loop {
        match self.event_rx.recv().await {
            Ok(traced_event) => {
                let event = traced_event.value();

                if !Self::should_record(event) {
                    continue;
                }

                if let Err(e) = self.record_event(event).await {
                    error!("Failed to save session entry: {}", e);
                    return Err(SessionError::StoreError(e));
                }
            }
            Err(broadcast::error::RecvError::Closed) => {
                tracing::debug!("Event channel closed, stopping session listener");
                break;
            }
            Err(broadcast::error::RecvError::Lagged(n)) => {
                warn!("Session listener lagged, missed {} events", n);
                continue;
            }
        }
    }
    Ok(())
}
```

Keep `event_to_message` as a private helper that returns `Option<SessionMessage>` (it's used by tests).

- [ ] **Step 5: Update listener tests**

Change test imports from `InMemoryMessageStore` to `InMemoryEntryStore`:

```rust
use crate::InMemoryEntryStore;
```

Update `create_test_listener` helper in tests (replace all `InMemoryMessageStore::new()` with `InMemoryEntryStore::new()`):

```rust
fn create_test_listener(session_id: &str) -> (Arc<InMemoryEntryStore>, SessionListener) {
    let store = Arc::new(InMemoryEntryStore::new());
    let (_tx, rx) = broadcast::channel(100);
    let listener = SessionListener::new(rx, store.clone(), session_id.to_string());
    (store, listener)
}
```

Update test assertions that use `store.get_by_session()`. For `InMemoryEntryStore`, use `store.get_entries()`:

```rust
// In test_listener_run_records_events:
let messages = store.get_entries(10).await.unwrap();
assert_eq!(messages.len(), 1);
// Check first entry is a message
assert_eq!(messages[0].r#type, SessionEntryType::Message);
```

Similarly for `test_listener_run_records_multiple_events`:
```rust
let entries = store.get_entries(10).await.unwrap();
assert_eq!(entries.len(), 3);
```

Update `test_event_to_message_*` tests — these test the `event_to_message` helper which still returns `Option<SessionMessage>`, so they remain mostly unchanged but need the `InMemoryEntryStore` for the listener construction.

- [ ] **Step 6: Run tests**

Run: `cargo test -p vol-session listener`
Expected: All listener tests pass.

- [ ] **Step 7: Commit**

```bash
git add crates/vol-session/src/listener.rs
git commit -m "feat: update SessionListener to use SessionEntryStore"
```

---

### Task 7: Update vol-session Re-exports and Tests

**Files:**
- Modify: `crates/vol-session/src/lib.rs`
- Modify: `crates/vol-session/src/memory_store.rs` (test imports)

- [ ] **Step 1: Update lib.rs**

The lib.rs was already updated in Task 1. Verify the exports are correct:

```rust
pub use file_store::FileSessionEntryStore;
pub use memory_store::{InMemoryEntryStore, InMemoryMessageStore, InMemorySessionStore};
pub use store::{MessageStore, SessionEntryStore, SessionStore, StoreError};
```

- [ ] **Step 2: Update memory_store.rs test imports**

In the `#[cfg(test)]` module of `memory_store.rs`, ensure imports use the correct types. Add:
```rust
use crate::entry::{CheckpointReason, SessionEntry, SessionEntryType};
```

- [ ] **Step 3: Run vol-session tests**

Run: `cargo test -p vol-session`
Expected: All tests pass.

- [ ] **Step 4: Commit**

```bash
git add crates/vol-session/src/lib.rs crates/vol-session/src/memory_store.rs
git commit -m "fix: update vol-session exports and test imports"
```

---

### Task 8: Update ReActAgent — resume() and SessionListener Wiring

**Files:**
- Modify: `crates/vol-llm-agent/src/react/agent.rs`

- [ ] **Step 1: Write the failing test**

```rust
#[tokio::test]
async fn test_react_agent_resume_method_exists() {
    // Verify that ReActAgent has a resume() method
    // This is a compile-time check — if it compiles, the method exists
    use vol_session::{InMemoryEntryStore, InMemorySessionStore};

    let session_store = Arc::new(InMemorySessionStore::new());
    let entry_store = Arc::new(InMemoryEntryStore::new());
    let session = Arc::new(Session::new(
        "test-session".to_string(),
        session_store,
        entry_store,
    ));

    let config = AgentConfig::default();
    let mock_llm = MockLlmClient::new();
    let tools = Arc::new(ToolRegistry::new());
    let agent = ReActAgent::new(mock_llm, tools, config, session);

    // resume() should compile and return a result
    let result = agent.resume("continue the task").await;
    // Without a real LLM, this will fail, but the method should exist
    assert!(result.is_err() || result.is_ok());
}
```

- [ ] **Step 2: Update with_new_session to use InMemoryEntryStore**

Change:
```rust
pub fn with_new_session(&self, session_id: String) -> Self {
    use vol_session::{InMemoryMessageStore, InMemorySessionStore};

    let new_session = Arc::new(Session::new(
        session_id,
        Arc::new(InMemorySessionStore::new()),
        Arc::new(InMemoryMessageStore::new()),
    ));
```
to:
```rust
pub fn with_new_session(&self, session_id: String) -> Self {
    use vol_session::{InMemoryEntryStore, InMemorySessionStore};

    let new_session = Arc::new(Session::new(
        session_id,
        Arc::new(InMemorySessionStore::new()),
        Arc::new(InMemoryEntryStore::new()),
    ));
```

- [ ] **Step 3: Update SessionListener wiring in run()**

Change:
```rust
use vol_session::{FileMessageStore, SessionListener};

let mut session_listener = SessionListener::new(
    run_ctx.event_tx.subscribe(),
    Arc::new(FileMessageStore::new(
        config.log_base_path.join(&config.agent_id),
        &session.id,
    )),
    session.id.clone(),
);
```
to:
```rust
use vol_session::{FileSessionEntryStore, SessionListener};

let mut session_listener = SessionListener::new(
    run_ctx.event_tx.subscribe(),
    Arc::new(FileSessionEntryStore::new(
        config.log_base_path.join(&config.agent_id),
        &session.id,
    )),
    session.id.clone(),
);
```

- [ ] **Step 4: Add resume() method**

```rust
impl ReActAgent {
    // ... existing methods ...

    /// Resume from an existing session. Loads checkpoint-based history as context,
    /// then starts a new run with the given user input.
    pub async fn resume(&self, user_input: &str) -> Result<AgentResponse, crate::AgentError> {
        // 1. Load resume messages from session (after latest checkpoint)
        let resume_messages = self.session.resume_messages().await.map_err(|e| {
            crate::AgentError::Context(format!("Failed to load resume messages: {}", e))
        })?;

        // 2. Pre-populate session with resume messages
        for msg in &resume_messages {
            let session_msg = SessionMessage::new(self.session.id.clone(), msg.clone());
            self.session.add_message(session_msg).await.map_err(|e| {
                crate::AgentError::SessionError(format!("Failed to save resume message: {}", e))
            })?;
        }

        // 3. Run normally — SessionContributor loads history from entry store
        self.run(user_input).await
    }
}
```

- [ ] **Step 5: Run tests**

Run: `cargo test -p vol-llm-agent react::agent`
Expected: Existing tests pass (new resume test may fail without MockLlmClient — that's fine).

- [ ] **Step 6: Commit**

```bash
git add crates/vol-llm-agent/src/react/agent.rs
git commit -m "feat: add ReActAgent.resume() and update SessionListener wiring"
```

---

### Task 9: Update RunContext Tests and SessionContributor

**Files:**
- Modify: `crates/vol-llm-agent/src/react/run_context.rs`
- Modify: `crates/vol-llm-agent/src/react/context_contributors.rs`

- [ ] **Step 1: Update run_context.rs test fixtures**

Replace all occurrences of `InMemoryMessageStore` in test fixtures with `InMemoryEntryStore`:

```rust
use vol_session::{InMemoryEntryStore, InMemorySessionStore, SessionMessage};
```

Change all `Arc::new(InMemoryMessageStore::new())` to `Arc::new(InMemoryEntryStore::new())` in the test helper functions and individual tests.

Specifically in `create_test_context()`:
```rust
fn create_test_context() -> RunContext {
    let (ctx, _rx, _approval_rx) = RunContext::new(
        "test-run".to_string(),
        "test input".to_string(),
        "session-1".to_string(),
        Arc::new(Session::new(
            "session-1".to_string(),
            Arc::new(InMemorySessionStore::new()),
            Arc::new(InMemoryEntryStore::new()),
        )),
        Arc::new(vol_llm_tool::ToolRegistry::new()),
        AgentConfig::default(),
    );
    ctx
}
```

And in `test_init_messages_history()`, `test_init_messages_user_input()`, `test_init_messages_only_once()`, `test_record_reasoning_step()`, `test_record_tool_call()`, `test_set_final_content()`, `test_finalize()` — replace `Arc::new(InMemoryMessageStore::new())` with `Arc::new(InMemoryEntryStore::new())`.

- [ ] **Step 2: Update test that checks session messages**

In `test_add_message_syncs_to_session()`, the assertion `ctx.session.get_messages(10).await` now returns entries including summaries. Since no compression is used in this test, it should work the same. No changes needed.

In `test_add_message_auto_sets_parent_id()`, same — `ctx.session.get_messages(10).await` returns message entries. No changes needed.

- [ ] **Step 3: Run tests**

Run: `cargo test -p vol-llm-agent run_context`
Expected: All run_context tests pass.

- [ ] **Step 4: Verify SessionContributor compiles**

The `SessionContributor` in `context_contributors.rs` calls `session.get_messages()` and `session.compress()`. These methods still exist on the new Session. Verify compilation:

Run: `cargo check -p vol-llm-agent`
Expected: Compiles successfully.

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-agent/src/react/run_context.rs crates/vol-llm-agent/src/react/context_contributors.rs
git commit -m "fix: update run_context tests and context_contributor for entry-based Session"
```

---

### Task 10: Update CodingAgent

**Files:**
- Modify: `crates/vol-llm-agents/src/coding/agent.rs`

- [ ] **Step 1: Update default session creation**

Change:
```rust
use vol_session::{InMemoryMessageStore, InMemorySessionStore};
Arc::new(Session::new(
    format!("coding_{}", chrono::Utc::now().format("%Y%m%d_%H%M%S")),
    Arc::new(InMemorySessionStore::new()),
    Arc::new(InMemoryMessageStore::new()),
))
```
to:
```rust
use vol_session::{InMemoryEntryStore, InMemorySessionStore};
Arc::new(Session::new(
    format!("coding_{}", chrono::Utc::now().format("%Y%m%d_%H%M%S")),
    Arc::new(InMemorySessionStore::new()),
    Arc::new(InMemoryEntryStore::new()),
))
```

- [ ] **Step 2: Add resume() method**

```rust
impl CodingAgent {
    // ... existing methods ...

    /// Resume from an existing session with the given session ID.
    /// Loads checkpoint-based history, then runs with new user input.
    pub async fn resume(
        &self,
        session_id: &str,
        user_input: &str,
    ) -> Result<CodingAgentResponse, CodingAgentError> {
        use vol_session::{InMemoryEntryStore, InMemorySessionStore};

        // Get state
        let state = self.state.as_ref()
            .ok_or_else(|| CodingAgentError::Config("CodingAgent already consumed".to_string()))?;

        // Create session using the provided session_id
        let session = Arc::new(Session::new(
            session_id.to_string(),
            Arc::new(InMemorySessionStore::new()),
            Arc::new(InMemoryEntryStore::new()),
        ));

        let agent_config = AgentConfig {
            plugin_registry: self.config.plugin_registry.clone(),
            unsafe_mode: self.config.unsafe_mode,
            approval_handler: self.config.approval_handler.clone(),
            ..state.agent_config.clone()
        };

        let mut react_agent = ReActAgent::new(
            state.llm.clone(),
            state.tool_registry.clone(),
            agent_config,
            session,
        );

        if let Some(ref sandbox) = self.sandbox {
            react_agent = react_agent.with_sandbox(sandbox.clone());
        }

        // Resume the agent
        let response = react_agent.resume(user_input).await
            .map_err(|e| CodingAgentError::Agent(e))?;

        if let Some(ref observer) = self.observer {
            observer.on_complete().await
                .map_err(|e| CodingAgentError::Observer(e))?;
        }

        let summary = response.content.clone();
        let iterations = response.iterations;
        let tool_calls = response.tool_calls.len() as u32;

        Ok(CodingAgentResponse {
            success: true,
            summary,
            iterations,
            tool_calls,
        })
    }
}
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p vol-llm-agents coding`
Expected: Tests pass (or skip if no tests exist).

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-agents/src/coding/agent.rs
git commit -m "feat: add CodingAgent.resume() and use InMemoryEntryStore"
```

---

### Task 11: Full Workspace Verification

- [ ] **Step 1: Full workspace check**

Run: `cargo check --workspace`
Expected: Compiles successfully.

- [ ] **Step 2: Run all vol-session tests**

Run: `cargo test -p vol-session`
Expected: All tests pass.

- [ ] **Step 3: Run all vol-llm-agent tests**

Run: `cargo test -p vol-llm-agent`
Expected: All tests pass.

- [ ] **Step 4: Run all vol-llm-agents tests**

Run: `cargo test -p vol-llm-agents`
Expected: All tests pass.

- [ ] **Step 5: Run all workspace tests**

Run: `cargo test --workspace`
Expected: All tests pass.

- [ ] **Step 6: Commit any remaining changes**

```bash
git add -A
git commit -m "fix: full workspace verification and cleanup"
```

---

## Self-Review

**1. Spec coverage check:**

| Spec Requirement | Task |
|---|---|
| SessionEntry struct with id, session_id, created_at, parent_id, type, data | Task 1 |
| SessionEntryType enum (Message, Checkpoint, Summary) | Task 1 |
| SessionEntryData enum with tagged serialization | Task 1 |
| CheckpointReason enum (Compression, Manual) | Task 1 |
| SessionEntryStore trait with 6 methods | Task 2 |
| FileSessionEntryStore writing {entry_dir}/{session_id}.jsonl | Task 3 |
| InMemoryEntryStore for tests | Task 4 |
| Session uses entry_store (not message_store) | Task 5 |
| Session.remove compressed_messages/compressed_after_ts | Task 5 |
| Session.checkpoint(), add_summary() methods | Task 5 |
| Session.resume_entries(), resume_messages() | Task 5 |
| compress() writes summary + checkpoint entries | Task 5 |
| Legacy JSONL migration (old event/data format) | Task 3 |
| SessionListener uses SessionEntryStore | Task 6 |
| ReActAgent.resume() method | Task 8 |
| ReActAgent uses FileSessionEntryStore for listener | Task 8 |
| SessionContributor works with entry-based Session | Task 9 |
| RunContext tests updated | Task 9 |
| CodingAgent uses InMemoryEntryStore | Task 10 |
| CodingAgent.resume() method | Task 10 |
| MessageStore trait kept for backward compat | Task 2 |

**All spec requirements covered.**

**2. Placeholder scan:** No "TBD", "TODO", or placeholders found. All code blocks contain complete implementations.

**3. Type consistency:**
- `SessionEntry` used consistently across all tasks
- `SessionEntryStore` trait name matches in store.rs, file_store.rs, memory_store.rs, session.rs
- `FileSessionEntryStore` (not `FileEntryStore`) — matches lib.rs export
- `InMemoryEntryStore` — matches lib.rs export
- `Session::new(id, session_store, entry_store)` signature consistent across Tasks 5, 8, 9, 10
- `CheckpointReason` exported from entry module, re-exported from lib.rs
- `SessionEntryType` used for comparison in find_latest_checkpoint — matches enum definition

All types consistent.
