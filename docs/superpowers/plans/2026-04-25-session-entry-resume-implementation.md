# Session Entry & Resume Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Decouple SessionEntryStore from single-session binding, rewrite Session with simplified lifecycle (self-generate ID, resume constructor, checkpoint-based reads, correct compress order), and update all call sites.

**Architecture:** Three-layer change: (1) `SessionEntryStore` trait gains `session_id` on read methods, becoming multi-session. (2) `FileSessionEntryStore` and `InMemoryEntryStore` implementations updated to match. (3) `Session` struct simplified — no `session_store`/`metadata`, new/resume constructors, `get_messages()` auto-scopes from latest checkpoint, compress writes checkpoint→summary→compressed (no delete).

**Tech Stack:** Rust async, async-trait, vol-session crate

---

## File Structure

| File | Responsibility |
|------|---------------|
| `crates/vol-session/src/store.rs` | Trait definition — add `session_id` params to SessionEntryStore |
| `crates/vol-session/src/memory_store.rs` | InMemoryEntryStore — internal HashMap, accept session_id |
| `crates/vol-session/src/file_store.rs` | FileSessionEntryStore — `new(dir)` only, methods accept session_id |
| `crates/vol-session/src/session.rs` | Session struct — new API, constructors, compress |
| `crates/vol-session/src/listener.rs` | SessionListener — update store method calls with session_id |
| `crates/vol-session/src/lib.rs` | Re-exports (likely unchanged) |
| `crates/vol-session/tests/integration_test.rs` | Integration tests |
| `crates/vol-llm-agent/src/react/context_contributors.rs` | SessionContributor — remove limit from get_messages |
| `crates/vol-llm-agent/src/react/builder.rs` | Builder — update Session::new call |
| `crates/vol-llm-agent/src/react/agent.rs` | Agent — update Session::new and FileSessionEntryStore::new |
| `crates/vol-llm-agent/src/react/run_context.rs` | Tests — update Session::new |
| `crates/vol-llm-agent/src/react/tests.rs` | Tests — update Session::new |
| `crates/vol-llm-agent/src/plugins/` | Plugin tests — update Session::new |
| `crates/vol-llm-agent/src/observability/plugin.rs` | Test — update Session::new |
| `crates/vol-llm-agent/tests/` | Integration tests — update Session::new, FileSessionEntryStore::new |
| `crates/vol-llm-agents/src/coding/agent.rs` | CodingAgent — update Session::new, FileSessionEntryStore::new |
| `crates/vol-llm-agents/tests/observer_plugin_unit.rs` | Test — update Session::new |
| `crates/vol-llm-agents/src/coding/tests.rs` | Test — update Session::new |
| `crates/vol-llm-tui/src/main.rs` | TUI — update session creation |
| `crates/vol-llm-agent/examples/session_example.rs` | Example — update Session::new |

---

### Task 1: Update SessionEntryStore Trait

**Files:**
- Modify: `crates/vol-session/src/store.rs`

- [ ] **Step 1: Update SessionEntryStore trait methods**

Replace the trait in `store.rs` with this new signature:

```rust
#[async_trait]
pub trait SessionEntryStore: Send + Sync {
    /// Append an entry (entry already carries session_id).
    async fn save(&self, entry: SessionEntry) -> Result<()>;

    /// Get all entries for a session.
    async fn get_entries(&self, session_id: &str) -> Result<Vec<SessionEntry>>;

    /// Get entries after a timestamp for a session.
    async fn get_after(&self, session_id: &str, after: i64) -> Result<Vec<SessionEntry>>;

    /// Find the latest checkpoint entry for a session.
    async fn find_latest_checkpoint(&self, session_id: &str) -> Result<Option<SessionEntry>>;

    /// Delete all entries for a session.
    async fn delete_session(&self, session_id: &str) -> Result<()>;

    /// Get entry count for a session.
    async fn get_count(&self, session_id: &str) -> Result<usize>;
}
```

The key changes:
- `get_entries(limit: usize)` → `get_entries(session_id: &str)` — no limit
- `get_after(after: i64, limit: usize)` → `get_after(session_id: &str, after: i64)` — no limit
- `find_latest_checkpoint()` → `find_latest_checkpoint(session_id: &str)`
- `delete_session()` → `delete_session(session_id: &str)`
- `get_count()` → `get_count(session_id: &str)`

- [ ] **Step 2: Compile and observe errors**

Run: `cargo check -p vol-session`
Expected: Errors in InMemoryEntryStore, FileSessionEntryStore, and Session implementations (they implement the old trait). This confirms the trait change propagated.

- [ ] **Step 3: Commit**

```bash
git add crates/vol-session/src/store.rs
git commit -m "refactor: add session_id params to SessionEntryStore trait methods"
```

---

### Task 2: Update InMemoryEntryStore Implementation

**Files:**
- Modify: `crates/vol-session/src/memory_store.rs` (InMemoryEntryStore impl + tests)

- [ ] **Step 1: Change internal storage from Vec to HashMap**

Replace the InMemoryEntryStore struct:

```rust
/// In-memory entry store for testing.
pub struct InMemoryEntryStore {
    entries: tokio::sync::RwLock<HashMap<String, Vec<crate::entry::SessionEntry>>>,
}

impl Default for InMemoryEntryStore {
    fn default() -> Self {
        Self::new()
    }
}

impl InMemoryEntryStore {
    /// Create a new empty entry store.
    pub fn new() -> Self {
        Self {
            entries: tokio::sync::RwLock::new(HashMap::new()),
        }
    }
}
```

- [ ] **Step 2: Update trait implementation**

```rust
#[async_trait]
impl crate::store::SessionEntryStore for InMemoryEntryStore {
    async fn save(&self, entry: crate::entry::SessionEntry) -> crate::store::Result<()> {
        self.entries
            .write()
            .await
            .entry(entry.session_id.clone())
            .or_default()
            .push(entry);
        Ok(())
    }

    async fn get_entries(&self, session_id: &str) -> crate::store::Result<Vec<crate::entry::SessionEntry>> {
        let entries = self.entries.read().await;
        Ok(entries.get(session_id).cloned().unwrap_or_default())
    }

    async fn get_after(&self, session_id: &str, after: i64) -> crate::store::Result<Vec<crate::entry::SessionEntry>> {
        let entries = self.entries.read().await;
        Ok(entries
            .get(session_id)
            .map(|msgs| {
                msgs.iter()
                    .filter(|e| e.created_at >= after)
                    .cloned()
                    .collect()
            })
            .unwrap_or_default())
    }

    async fn find_latest_checkpoint(&self, session_id: &str) -> crate::store::Result<Option<crate::entry::SessionEntry>> {
        let entries = self.entries.read().await;
        Ok(entries
            .get(session_id)
            .and_then(|msgs| {
                msgs.iter()
                    .filter(|e| e.r#type == crate::entry::SessionEntryType::Checkpoint)
                    .max_by_key(|e| e.created_at)
                    .cloned()
            }))
    }

    async fn delete_session(&self, session_id: &str) -> crate::store::Result<()> {
        self.entries.write().await.remove(session_id);
        Ok(())
    }

    async fn get_count(&self, session_id: &str) -> crate::store::Result<usize> {
        let entries = self.entries.read().await;
        Ok(entries.get(session_id).map(|msgs| msgs.len()).unwrap_or(0))
    }
}
```

- [ ] **Step 3: Update InMemoryEntryStore tests**

Replace the existing `entry_tests` module:

```rust
#[cfg(test)]
mod entry_tests {
    use super::*;
    use crate::entry::{SessionEntry, SessionEntryType};
    use crate::store::SessionEntryStore;
    use crate::CheckpointReason;
    use vol_llm_core::Message;

    #[tokio::test]
    async fn test_in_memory_entry_store_save_and_get() {
        let store = InMemoryEntryStore::new();

        let entry = SessionEntry::new_message(
            "test-session".to_string(),
            Message::user("Hello, World!"),
        );

        store.save(entry.clone()).await.unwrap();

        let entries = store.get_entries("test-session").await.unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].r#type, SessionEntryType::Message);
    }

    #[tokio::test]
    async fn test_in_memory_entry_store_find_checkpoint() {
        let store = InMemoryEntryStore::new();

        let mut msg1 = SessionEntry::new_message(
            "test-session".to_string(),
            Message::user("before"),
        );
        msg1.created_at = 100;

        let mut cp = SessionEntry::new_checkpoint(
            "test-session".to_string(),
            CheckpointReason::Compression,
            None,
        );
        cp.created_at = 200;

        let mut msg2 = SessionEntry::new_message(
            "test-session".to_string(),
            Message::user("after"),
        );
        msg2.created_at = 300;

        store.save(msg1).await.unwrap();
        store.save(cp).await.unwrap();
        store.save(msg2).await.unwrap();

        let cp = store.find_latest_checkpoint("test-session").await.unwrap().unwrap();
        assert_eq!(cp.r#type, SessionEntryType::Checkpoint);

        let after = store.get_after("test-session", cp.created_at).await.unwrap();
        assert_eq!(after.len(), 2); // checkpoint + after message with >=
    }

    #[tokio::test]
    async fn test_in_memory_entry_store_delete_session() {
        let store = InMemoryEntryStore::new();

        store.save(SessionEntry::new_message(
            "test-session".to_string(),
            Message::user("test"),
        )).await.unwrap();

        store.delete_session("test-session").await.unwrap();
        let count = store.get_count("test-session").await.unwrap();
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn test_in_memory_entry_store_multiple_sessions() {
        let store = InMemoryEntryStore::new();

        store.save(SessionEntry::new_message(
            "session-a".to_string(),
            Message::user("from A"),
        )).await.unwrap();

        store.save(SessionEntry::new_message(
            "session-b".to_string(),
            Message::user("from B"),
        )).await.unwrap();

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
}
```

- [ ] **Step 4: Commit**

```bash
git add crates/vol-session/src/memory_store.rs
git commit -m "feat: InMemoryEntryStore uses HashMap keyed by session_id"
```

---

### Task 3: Update FileSessionEntryStore Implementation

**Files:**
- Modify: `crates/vol-session/src/file_store.rs`

- [ ] **Step 1: Change constructor to accept dir only**

Update the struct and constructor:

```rust
/// File-based entry store using JSONL format.
///
/// Stores all entry types in `{entry_dir}/{session_id}.jsonl`.
/// Session ID is passed per-method-call, not bound at construction.
pub struct FileSessionEntryStore {
    entry_dir: PathBuf,
}

impl FileSessionEntryStore {
    /// Create a new file entry store.
    pub fn new<P: AsRef<Path>>(entry_dir: P) -> Self {
        Self {
            entry_dir: entry_dir.as_ref().to_path_buf(),
        }
    }

    /// Resolve file path for a session.
    fn file_path(&self, session_id: &str) -> PathBuf {
        self.entry_dir.join(format!("{}.jsonl", session_id))
    }

    fn ensure_dir(&self) -> std::io::Result<()> {
        fs::create_dir_all(&self.entry_dir)
    }

    fn append_line(&self, session_id: &str, line: &str) -> std::io::Result<()> {
        self.ensure_dir()?;
        let file_path = self.file_path(session_id);
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&file_path)?;
        writeln!(file, "{}", line)?;
        Ok(())
    }
    // ... keep to_json, from_json, read_from_head, read_from_tail as-is ...
}
```

- [ ] **Step 2: Update `to_json` — remove `SessionEntryLine.session_id` from write path**

`to_json` stays the same (it writes `entry.session_id` from the entry itself). No change needed.

- [ ] **Step 3: Update `read_from_head` to use session_id**

```rust
fn read_from_head(&self, session_id: &str, max_parsed: usize) -> std::io::Result<Vec<SessionEntry>> {
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
```

- [ ] **Step 4: Update `read_from_tail` to use session_id**

```rust
fn read_from_tail(&self, session_id: &str, buf_size: u64) -> std::io::Result<Vec<SessionEntry>> {
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
```

- [ ] **Step 5: Update trait implementation**

```rust
#[async_trait]
impl SessionEntryStore for FileSessionEntryStore {
    async fn save(&self, entry: SessionEntry) -> Result<()> {
        let json = Self::to_json(&entry)?;
        self.append_line(&entry.session_id, &json).map_err(StoreError::Io)
    }

    async fn get_entries(&self, session_id: &str) -> Result<Vec<SessionEntry>> {
        self.read_from_head(session_id, usize::MAX).map_err(StoreError::Io)
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
        let tail_entries = self.read_from_tail(session_id, 64 * 1024).map_err(StoreError::Io)?;
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

        if latest.is_none() {
            let all = self.read_from_head(session_id, usize::MAX).map_err(StoreError::Io)?;
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
}
```

- [ ] **Step 6: Update file_store tests**

Replace `entry_tests` module — update all `new(dir, session_id)` → `new(dir)` and method calls to include session_id:

```rust
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
        let store = FileSessionEntryStore::new(temp_dir.path());

        let entry = SessionEntry::new_message(
            "test-session".to_string(),
            Message::user("Hello, World!"),
        );

        store.save(entry.clone()).await.unwrap();

        let entries = store.get_entries("test-session").await.unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].r#type, SessionEntryType::Message);
    }

    #[tokio::test]
    async fn test_file_entry_store_find_checkpoint() {
        let temp_dir = tempdir().unwrap();
        let store = FileSessionEntryStore::new(temp_dir.path());

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

        let cp = store.find_latest_checkpoint("test-session").await.unwrap().unwrap();
        assert_eq!(cp.r#type, SessionEntryType::Checkpoint);

        let entries = store.get_after("test-session", cp.created_at).await.unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].r#type, SessionEntryType::Checkpoint);
        assert_eq!(entries[1].r#type, SessionEntryType::Message);
    }

    #[tokio::test]
    async fn test_file_entry_store_delete_session() {
        let temp_dir = tempdir().unwrap();
        let store = FileSessionEntryStore::new(temp_dir.path());

        store.save(SessionEntry::new_message(
            "test-session".to_string(),
            Message::user("test"),
        )).await.unwrap();

        store.delete_session("test-session").await.unwrap();
        let count = store.get_count("test-session").await.unwrap();
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn test_file_entry_store_skips_bad_lines() {
        let temp_dir = tempdir().unwrap();
        let store = FileSessionEntryStore::new(temp_dir.path());

        let entry1 = SessionEntry::new_message(
            "test-session".to_string(),
            Message::user("hello"),
        );
        store.save(entry1).await.unwrap();

        std::fs::OpenOptions::new()
            .append(true)
            .open(&store.file_path("test-session"))
            .unwrap()
            .write_all(b"this is not valid json\n")
            .unwrap();

        let entry2 = SessionEntry::new_message(
            "test-session".to_string(),
            Message::user("world"),
        );
        store.save(entry2).await.unwrap();

        let entries = store.get_entries("test-session").await.unwrap();
        assert_eq!(entries.len(), 2);
    }

    #[tokio::test]
    async fn test_file_entry_store_read_from_tail() {
        let temp_dir = tempdir().unwrap();
        let store = FileSessionEntryStore::new(temp_dir.path());

        for i in 0..5 {
            let entry = SessionEntry::new_message(
                "test-session".to_string(),
                Message::user(format!("msg-{i}")),
            );
            store.save(entry).await.unwrap();
        }

        let tail = store.read_from_tail("test-session", 256).unwrap();
        assert!(!tail.is_empty());
        assert_eq!(tail.last().unwrap().data.entry_type(), SessionEntryType::Message);
    }

    #[tokio::test]
    async fn test_file_entry_store_multiple_sessions() {
        let temp_dir = tempdir().unwrap();
        let store = FileSessionEntryStore::new(temp_dir.path());

        store.save(SessionEntry::new_message(
            "session-a".to_string(),
            Message::user("from A"),
        )).await.unwrap();

        store.save(SessionEntry::new_message(
            "session-b".to_string(),
            Message::user("from B"),
        )).await.unwrap();

        let entries_a = store.get_entries("session-a").await.unwrap();
        assert_eq!(entries_a.len(), 1);

        let entries_b = store.get_entries("session-b").await.unwrap();
        assert_eq!(entries_b.len(), 1);

        store.delete_session("session-a").await.unwrap();
        assert_eq!(store.get_count("session-b").await.unwrap(), 1);
    }
}
```

- [ ] **Step 7: Commit**

```bash
git add crates/vol-session/src/file_store.rs
git commit -m "feat: FileSessionEntryStore decouples from session_id, resolves per-method"
```

---

### Task 4: Rewrite Session Struct

**Files:**
- Modify: `crates/vol-session/src/session.rs` (full rewrite)

- [ ] **Step 1: Write the new session.rs**

Replace the entire file content:

```rust
//! Session management with entry-based persistence.

use crate::compressor::MessageCompressor;
use crate::compressors::PositionSampleCompressor;
use crate::entry::{CheckpointReason, SessionEntry, SessionEntryData, SessionEntryType};
use crate::message::SessionMessage;
use crate::store::{Result, SessionEntryStore};
use std::collections::HashMap;
use std::sync::Arc;
use vol_llm_core::Message;

/// Session management
pub struct Session {
    pub id: String,
    pub created_at: i64,
    entry_store: Arc<dyn SessionEntryStore>,
    compressor: Arc<dyn MessageCompressor>,
}

impl Session {
    /// Create a new session — self-generates UUID, current timestamp.
    pub fn new(entry_store: Arc<dyn SessionEntryStore>) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64,
            entry_store,
            compressor: Arc::new(PositionSampleCompressor::default()),
        }
    }

    /// Resume from existing session — external ID provided.
    /// Loads created_at from the first entry if available.
    pub fn resume(id: String, entry_store: Arc<dyn SessionEntryStore>) -> Result<Self> {
        let created_at = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                // Try to find the first entry to get created_at
                let entries = entry_store.get_entries(&id).await?;
                let ts = entries.first().map(|e| e.created_at);
                Ok::<_, SessionStoreError>(ts)
            })
        })?;

        Ok(Self {
            id,
            created_at: created_at.unwrap_or(0),
            entry_store,
            compressor: Arc::new(PositionSampleCompressor::default()),
        })
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

    /// Get all messages after the latest checkpoint.
    /// If no checkpoint exists, returns all messages.
    /// Summary entries are converted to synthetic SessionMessage with system role.
    pub async fn get_messages(&self) -> Result<Vec<SessionMessage>> {
        let entries = match self.entry_store.find_latest_checkpoint(&self.id).await? {
            Some(cp) => self.entry_store.get_after(&self.id, cp.created_at).await?,
            None => self.entry_store.get_entries(&self.id).await?,
        };

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

    /// Compress the given messages and write checkpoint + summary + compressed entries.
    pub async fn compress(&mut self, messages: Vec<SessionMessage>) {
        if messages.is_empty() {
            return;
        }

        // 1. Write checkpoint (seal old messages)
        if let Err(e) = self.checkpoint(CheckpointReason::Compression, None).await {
            tracing::error!("Failed to write checkpoint before compression: {}", e);
            return;
        }

        // 2. Compress input messages
        let compressed = self.compressor.compress(messages).await;
        if compressed.is_empty() {
            return;
        }

        // 3. Build summary text from compressed messages
        let summary = compressed
            .iter()
            .filter_map(|m| m.message.content.as_ref())
            .map(|c| c.as_str())
            .collect::<Vec<_>>()
            .join("\n");

        // 4. Write summary entry
        if let Err(e) = self.add_summary(summary).await {
            tracing::error!("Failed to write summary during compression: {}", e);
            return;
        }

        // 5. Write compressed message entries
        for msg in &compressed {
            if let Err(e) = self.add_message(msg.clone()).await {
                tracing::error!("Failed to write compressed message: {}", e);
            }
        }
    }

    /// Add metadata (no-op, kept for backward compatibility during transition).
    pub fn with_metadata(mut self, _key: &str, _value: &str) -> Self {
        self
    }
}

impl Clone for Session {
    fn clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            created_at: self.created_at,
            entry_store: self.entry_store.clone(),
            compressor: self.compressor.clone(),
        }
    }
}
```

- [ ] **Step 2: Write the new session.rs tests**

Replace the `#[cfg(test)] mod tests` section:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory_store::{InMemoryEntryStore, InMemorySessionStore};

    #[tokio::test]
    async fn test_session_new_self_generates_id() {
        let entry_store = Arc::new(InMemoryEntryStore::new());
        let session = Session::new(entry_store);

        assert!(!session.id.is_empty());
        assert!(session.created_at > 0);
    }

    #[tokio::test]
    async fn test_session_get_messages() {
        let entry_store = Arc::new(InMemoryEntryStore::new());

        let session = Session::new(entry_store.clone());

        let msg = SessionMessage::new(session.id.clone(), Message::user("Hello"));
        session.add_message(msg).await.unwrap();

        let messages = session.get_messages().await.unwrap();
        assert_eq!(messages.len(), 1);
    }

    #[tokio::test]
    async fn test_session_with_metadata_noop() {
        let entry_store = Arc::new(InMemoryEntryStore::new());
        let session = Session::new(entry_store).with_metadata("user_id", "user-123");

        // with_metadata is a no-op now, session should still work
        let messages = session.get_messages().await.unwrap();
        assert!(messages.is_empty());
    }

    #[tokio::test]
    async fn test_session_compress_flow() {
        let entry_store = Arc::new(InMemoryEntryStore::new());

        let mut session = Session::new(entry_store.clone());

        // Add 10 messages
        for i in 0..10 {
            let msg = SessionMessage::new(
                session.id.clone(),
                Message::user(format!("msg-{}", i)),
            );
            session.add_message(msg).await.unwrap();
        }

        // Before compression: no checkpoint, get all 10
        let messages = session.get_messages().await.unwrap();
        assert_eq!(messages.len(), 10);

        // Compress
        session.compressor = Arc::new(PositionSampleCompressor::new(2, 3));
        session.compress(messages).await;

        // After compression:
        // 1 checkpoint (not returned as message) + 1 summary + 6 compressed = 7 messages returned
        let after = session.get_messages().await.unwrap();
        assert_eq!(after.len(), 7);

        // First should be the summary as a system message
        assert_eq!(after[0].message.role, vol_llm_core::MessageRole::System);
    }

    #[tokio::test]
    async fn test_session_compress_empty_messages() {
        let entry_store = Arc::new(InMemoryEntryStore::new());

        let mut session = Session::new(entry_store);

        // Compress with empty input should be no-op
        session.compress(vec![]).await;
        let messages = session.get_messages().await.unwrap();
        assert!(messages.is_empty());
    }

    #[tokio::test]
    async fn test_session_resume_entries_no_checkpoint() {
        let entry_store = Arc::new(InMemoryEntryStore::new());

        let session = Session::new(entry_store.clone());

        for i in 0..3 {
            let msg = SessionMessage::new(
                session.id.clone(),
                Message::user(format!("msg-{}", i)),
            );
            session.add_message(msg).await.unwrap();
        }

        // get_messages with no checkpoint returns all
        let messages = session.get_messages().await.unwrap();
        assert_eq!(messages.len(), 3);
    }

    #[tokio::test]
    async fn test_session_resume_messages_includes_summary() {
        let entry_store = Arc::new(InMemoryEntryStore::new());

        let mut session = Session::new(entry_store.clone());

        for i in 0..5 {
            let msg = SessionMessage::new(
                session.id.clone(),
                Message::user(format!("msg-{}", i)),
            );
            session.add_message(msg).await.unwrap();
        }

        let messages = session.get_messages().await.unwrap();
        session.compressor = Arc::new(PositionSampleCompressor::new(2, 3));
        session.compress(messages).await;

        // get_messages after compress: returns messages after checkpoint (summary + compressed)
        let msgs = session.get_messages().await.unwrap();
        assert!(!msgs.is_empty());
        // First is summary as system message
        assert_eq!(msgs[0].message.role, vol_llm_core::MessageRole::System);
    }

    #[tokio::test]
    async fn test_session_resume_constructor() {
        let entry_store = Arc::new(InMemoryEntryStore::new());

        // Create and populate a session
        let session = Session::new(entry_store.clone());
        let session_id = session.id.clone();

        let msg = SessionMessage::new(session_id.clone(), Message::user("Hello"));
        session.add_message(msg).await.unwrap();

        // Resume from the same entry_store
        let resumed = Session::resume(session_id, entry_store.clone()).unwrap();
        assert_eq!(resumed.id, session_id);

        // get_messages should return the messages after checkpoint (all, since no checkpoint)
        let messages = resumed.get_messages().await.unwrap();
        assert_eq!(messages.len(), 1);
    }

    #[tokio::test]
    async fn test_session_multiple_compressions() {
        let entry_store = Arc::new(InMemoryEntryStore::new());

        let mut session = Session::new(entry_store.clone());

        // First batch: 6 messages
        for i in 0..6 {
            let msg = SessionMessage::new(
                session.id.clone(),
                Message::user(format!("batch1-{}", i)),
            );
            session.add_message(msg).await.unwrap();
        }
        let messages = session.get_messages().await.unwrap();
        assert_eq!(messages.len(), 6);
        session.compressor = Arc::new(PositionSampleCompressor::new(2, 3));
        session.compress(messages).await;

        // After first compress: summary + 3 compressed = 4 messages
        let after1 = session.get_messages().await.unwrap();
        assert_eq!(after1.len(), 4);
        assert_eq!(after1[0].message.role, vol_llm_core::MessageRole::System);

        // Add 3 more messages
        for i in 0..3 {
            let msg = SessionMessage::new(
                session.id.clone(),
                Message::user(format!("batch2-{}", i)),
            );
            session.add_message(msg).await.unwrap();
        }

        // Now: summary + 3 compressed + 3 new = 7 messages
        let messages2 = session.get_messages().await.unwrap();
        assert_eq!(messages2.len(), 7);

        // Compress again
        session.compress(messages2).await;

        // After second compress: new checkpoint + summary + compressed
        let after2 = session.get_messages().await.unwrap();
        // The previous summary+compressed (4) + new messages (3) = 7 total
        // Compressor keeps 3 → summary + 3 compressed
        assert!(!after2.is_empty());
    }
}
```

- [ ] **Step 3: Remove `SessionStore` import if unused**

The old session.rs imported `SessionStore` (Arc<dyn SessionStore>). Remove this import since the new Session no longer uses it. Also remove `SessionStore` from `lib.rs` re-exports if it was only exported for Session use (it's not — SessionStore is still needed for future SessionManager, keep it).

- [ ] **Step 4: Commit**

```bash
git add crates/vol-session/src/session.rs
git commit -m "feat: rewrite Session — new/resume constructors, checkpoint-based reads, correct compress"
```

---

### Task 5: Update SessionListener

**Files:**
- Modify: `crates/vol-session/src/listener.rs`

- [ ] **Step 1: Update test method calls to include session_id**

In the listener tests, update all `store.get_entries(N)` calls to `store.get_entries("session-1")`:

In `test_listener_run_records_events`:
```rust
let entries = store.get_entries("session-1").await.unwrap();
```

In `test_listener_run_records_multiple_events`:
```rust
let entries = store.get_entries("session-1").await.unwrap();
```

- [ ] **Step 2: Commit**

```bash
git add crates/vol-session/src/listener.rs
git commit -m "fix: update SessionListener tests with session_id params"
```

---

### Task 6: Update vol-session Integration Tests

**Files:**
- Modify: `crates/vol-session/tests/integration_test.rs`

- [ ] **Step 1: Read current file and update all call sites**

Read the file first, then replace:
- `FileSessionEntryStore::new(dir, session_id)` → `FileSessionEntryStore::new(dir)`
- `store.get_entries(N)` → `store.get_entries("session-1")` (or appropriate session_id)
- `store.get_after(ts, N)` → `store.get_after("session-1", ts)`
- `store.find_latest_checkpoint()` → `store.find_latest_checkpoint("session-1")`
- `store.delete_session()` → `store.delete_session("session-1")`
- `store.get_count()` → `store.get_count("session-1")`

The integration test file has these patterns:

```rust
// Line ~15: let store = Arc::new(FileSessionEntryStore::new(tmp_dir.path(), "session-1"));
// → let store = Arc::new(FileSessionEntryStore::new(tmp_dir.path()));

// All store method calls need session_id added.
```

After all replacements, verify with `cargo check -p vol-session`.

- [ ] **Step 2: Commit**

```bash
git add crates/vol-session/tests/integration_test.rs
git commit -m "fix: update integration tests for decoupled SessionEntryStore"
```

---

### Task 7: Update SessionContributor (vol-llm-agent)

**Files:**
- Modify: `crates/vol-llm-agent/src/react/context_contributors.rs`

- [ ] **Step 1: Remove limit from get_messages call**

In the `contribute` method, change:

```rust
let history = self
    .session
    .lock()
    .await
    .get_messages(self.max_history)  // OLD
    .await
    .unwrap_or_default();
```

To:

```rust
let history = self
    .session
    .lock()
    .await
    .get_messages()  // NEW — no limit
    .await
    .unwrap_or_default();
```

- [ ] **Step 2: Commit**

```bash
git add crates/vol-llm-agent/src/react/context_contributors.rs
git commit -m "fix: remove limit from SessionContributor get_messages call"
```

---

### Task 8: Update vol-llm-agent Internal Session Creation Sites

**Files:**
- Modify: `crates/vol-llm-agent/src/react/builder.rs`
- Modify: `crates/vol-llm-agent/src/react/agent.rs`

- [ ] **Step 1: Update builder.rs**

In `builder.rs`, replace:

```rust
let session = self.session.unwrap_or_else(|| {
    Arc::new(Session::new(
        uuid::Uuid::new_v4().to_string(),
        Arc::new(InMemorySessionStore::new()),
        Arc::new(InMemoryEntryStore::new()),
    ))
});
```

With:

```rust
let session = self.session.unwrap_or_else(|| {
    let entry_store = Arc::new(InMemoryEntryStore::new());
    Arc::new(Session::new(entry_store))
});
```

Remove `use vol_session::InMemorySessionStore;` import if no longer needed.

- [ ] **Step 2: Update agent.rs**

In `agent.rs`, update `with_new_session`:

```rust
pub fn with_new_session(&self, session_id: String) -> Self {
    use vol_session::InMemoryEntryStore;

    let entry_store = Arc::new(InMemoryEntryStore::new());
    let new_session = Arc::new(Session::new(entry_store));
    // Note: session.id is now self-generated, session_id param is ignored for the ID.
    // If the caller needs a specific ID, use Session::resume instead.
    Self {
        session: new_session,
        llm: self.llm.clone(),
        tools: self.tools.clone(),
        config: self.config.clone(),
        sandbox: self.sandbox.clone(),
    }
}
```

Update the FileSessionEntryStore usage in agent.rs (around line 194):

```rust
// Old:
let entry_store = Arc::new(FileSessionEntryStore::new(
    &session_dir, &session_id,
));
// New:
let entry_store = Arc::new(FileSessionEntryStore::new(&session_dir));
```

And the Session creation:

```rust
// Old:
let session = Arc::new(Session::new(
    session_id,
    session_store,
    entry_store,
));
// New:
let session = Arc::new(Session::new(entry_store));
```

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-agent/src/react/builder.rs crates/vol-llm-agent/src/react/agent.rs
git commit -m "fix: update vol-llm-agent Session creation sites for new API"
```

---

### Task 9: Update vol-llm-agent Tests

**Files:**
- Modify: `crates/vol-llm-agent/src/react/run_context.rs` (tests)
- Modify: `crates/vol-llm-agent/src/react/tests.rs`
- Modify: `crates/vol-llm-agent/src/plugins/retry.rs` (test)
- Modify: `crates/vol-llm-agent/src/plugins/rate_limiter.rs` (test)
- Modify: `crates/vol-llm-agent/src/plugins/caching.rs` (test)
- Modify: `crates/vol-llm-agent/src/observability/plugin.rs` (test)
- Modify: `crates/vol-llm-agent/tests/compression_flow_test.rs`
- Modify: `crates/vol-llm-agent/tests/plugin_flow_test.rs`
- Modify: `crates/vol-llm-agent/tests/session_recording_test.rs`
- Modify: `crates/vol-llm-agent/tests/session_history_test.rs`
- Modify: `crates/vol-llm-agent/tests/observability_integration.rs`
- Modify: `crates/vol-llm-agent/tests/plugin_test.rs`
- Modify: `crates/vol-llm-agent/tests/react_agent_integration.rs`

- [ ] **Step 1: Update all Session::new patterns**

In every test file, replace the old 3-argument `Session::new` pattern:

```rust
// OLD:
Session::new(
    "session-id".to_string(),
    Arc::new(InMemorySessionStore::new()),
    Arc::new(InMemoryEntryStore::new()),
)

// NEW:
Session::new(Arc::new(InMemoryEntryStore::new()))
```

Where `InMemorySessionStore` import is no longer needed, remove it.

- [ ] **Step 2: Update FileSessionEntryStore::new in tests**

In `session_recording_test.rs` and `compression_flow_test.rs`:

```rust
// OLD:
FileSessionEntryStore::new(&session_dir, &session_id)

// NEW:
FileSessionEntryStore::new(&session_dir)
```

And add session_id to store method calls where needed.

- [ ] **Step 3: Update compression_flow_test.rs specifically**

In `compression_flow_test.rs`, the Session is created for testing — update to:

```rust
let mut session = Session::new(Arc::new(InMemoryEntryStore::new()));
```

Note that `compress` now takes `&mut self`, so the session variable needs to be `mut`.

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-agent/
git commit -m "fix: update all vol-llm-agent tests for new Session API"
```

---

### Task 10: Update vol-llm-agents and vol-llm-tui

**Files:**
- Modify: `crates/vol-llm-agents/src/coding/agent.rs`
- Modify: `crates/vol-llm-agents/src/coding/tests.rs`
- Modify: `crates/vol-llm-agents/tests/observer_plugin_unit.rs`
- Modify: `crates/vol-llm-tui/src/main.rs`

- [ ] **Step 1: Update coding/agent.rs**

Around lines 216 and 282, update Session::new and FileSessionEntryStore::new:

```rust
// OLD (line ~216):
let session = Arc::new(Session::new(
    session_id,
    session_store,
    entry_store,
));

// NEW:
let entry_store = Arc::new(FileSessionEntryStore::new(&session_dir));
let session = Arc::new(Session::new(entry_store));
```

- [ ] **Step 2: Update coding/tests.rs**

```rust
// OLD:
let session = Arc::new(Session::new(
    "test-session".to_string(),
    Arc::new(InMemorySessionStore::new()),
    Arc::new(InMemoryEntryStore::new()),
));

// NEW:
let session = Arc::new(Session::new(
    Arc::new(InMemoryEntryStore::new()),
));
```

- [ ] **Step 3: Update observer_plugin_unit.rs**

Same pattern — remove `InMemorySessionStore` and update `Session::new`.

- [ ] **Step 4: Update TUI main.rs**

Replace `create_session()` function:

```rust
fn create_session() -> Result<Arc<Session>, Box<dyn std::error::Error>> {
    let session_dir = std::env::current_dir()
        .unwrap_or_default()
        .join(".vol-sessions");

    if let Err(e) = std::fs::create_dir_all(&session_dir) {
        eprintln!("Warning: cannot create session dir: {}", e);
        eprintln!("Using in-memory session (no history persistence)");
        let entry_store = Arc::new(vol_session::InMemoryEntryStore::new());
        return Ok(Arc::new(Session::new(entry_store)));
    }

    let entry_store = Arc::new(FileSessionEntryStore::new(&session_dir));
    Ok(Arc::new(Session::new(entry_store)))
}
```

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-agents/src/coding/agent.rs crates/vol-llm-agents/src/coding/tests.rs crates/vol-llm-agents/tests/observer_plugin_unit.rs crates/vol-llm-tui/src/main.rs
git commit -m "fix: update vol-llm-agents and vol-llm-tui Session creation for new API"
```

---

### Task 11: Update Session Listener Tests (session_id on store calls)

**Files:**
- Modify: `crates/vol-session/src/memory_store.rs` (the InMemorySessionStore test at the bottom)

- [ ] **Step 1: Update the InMemorySessionStore test**

In `memory_store.rs`, the `test_memory_session_store_crud` test creates a Session with the old API:

```rust
// OLD:
let session = Session::new("session-1".to_string(), store.clone(), entry_store);

// NEW:
let session = Session::new(entry_store.clone());
// SessionStore operations (create/update/get/delete) are tested directly on store
store.create(session.clone()).await.unwrap();
```

- [ ] **Step 2: Commit**

```bash
git add crates/vol-session/src/memory_store.rs
git commit -m "fix: update session store test for new Session API"
```

---

### Task 12: Update Example

**Files:**
- Modify: `crates/vol-llm-agent/examples/session_example.rs`

- [ ] **Step 1: Update session example**

```rust
// OLD:
Session::new(
    "session-id".to_string(),
    Arc::new(InMemorySessionStore::new()),
    Arc::new(InMemoryEntryStore::new()),
)

// NEW:
Session::new(Arc::new(InMemoryEntryStore::new()))
```

- [ ] **Step 2: Commit**

```bash
git add crates/vol-llm-agent/examples/session_example.rs
git commit -m "fix: update session example for new Session API"
```

---

### Task 13: Full Workspace Verification

- [ ] **Step 1: Compile check**

```bash
cargo check --workspace
```

Expected: No errors.

- [ ] **Step 2: Run vol-session tests**

```bash
cargo test -p vol-session
```

Expected: All tests pass.

- [ ] **Step 3: Run vol-llm-agent tests**

```bash
cargo test -p vol-llm-agent
```

Expected: All tests pass.

- [ ] **Step 4: Run vol-llm-agents tests**

```bash
cargo test -p vol-llm-agents
```

Expected: All tests pass.

- [ ] **Step 5: Commit any remaining fixes**

---

## Summary of Changes

| Task | Crate | Files | Purpose |
|------|-------|-------|---------|
| 1 | vol-session | `store.rs` | Trait: add session_id params |
| 2 | vol-session | `memory_store.rs` | InMemoryEntryStore: HashMap by session_id |
| 3 | vol-session | `file_store.rs` | FileSessionEntryStore: `new(dir)`, session_id per-method |
| 4 | vol-session | `session.rs` | Rewrite: new/resume, get_messages() from checkpoint, compress checkpoint→summary→compressed |
| 5 | vol-session | `listener.rs` | Tests: session_id on store calls |
| 6 | vol-session | `tests/integration_test.rs` | Integration tests update |
| 7 | vol-llm-agent | `context_contributors.rs` | Remove limit from get_messages |
| 8 | vol-llm-agent | `builder.rs`, `agent.rs` | Session creation sites |
| 9 | vol-llm-agent | 10+ test files | Session creation + FileSessionEntryStore update |
| 10 | vol-llm-agents + tui | `agent.rs`, `tests.rs`, `main.rs` | Session creation sites |
| 11 | vol-session | `memory_store.rs` | Session store test update |
| 12 | vol-llm-agent | `session_example.rs` | Example update |
| 13 | workspace | — | Full verification |
