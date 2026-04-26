# Session Entry & Resume Design

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace `MessageStore` with `SessionEntryStore` supporting multi-type session entries (Message/Checkpoint/Summary), and add resume capability to ReActAgent via checkpoint-based session recovery.

**Architecture:** Single `SessionEntry` struct with `type` + `data` fields. `SessionEntryStore` trait replaces `MessageStore`. `FileSessionEntryStore` writes `{entry_dir}/{session_id}.jsonl`. Session records checkpoints on each compress(). Resume finds latest checkpoint, loads subsequent entries, rebuilds context.

**Tech Stack:** async-trait, tokio, serde, serde_json, chrono

---

## Context

**Current state:**
- `Session` uses `MessageStore` (trait) + `FileMessageStore` (impl) to persist `SessionMessage` as JSONL
- `MessageStore` only supports `SessionMessage` — no checkpoint, no summary entries
- `Session` has in-memory `compressed_messages` + `compressed_after_ts` cursor — lost on restart
- `ReActAgent` has no resume capability; each run starts fresh
- File format: `{base}/sessions/{session_id}.jsonl` with `SessionMessageLine{event, data, session_id, timestamp}`

**Problem:** No persistent checkpoint/resume. Compressed messages are in-memory only. Cannot resume after process restart.

**Solution:** Introduce `SessionEntry` as the unified entry type. `SessionEntryStore` replaces `MessageStore`. Compressed summaries and checkpoints become persistent entries in the same JSONL file.

---

## Data Model

### SessionEntry

```rust
/// Unified session entry — all content types stored in a single JSONL file.
pub struct SessionEntry {
    pub id: String,
    pub session_id: String,
    pub created_at: i64,
    pub parent_id: Option<String>,
    pub r#type: SessionEntryType,
    pub data: SessionEntryData,
}
```

### SessionEntryType

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionEntryType {
    Message,
    Checkpoint,
    Summary,
}
```

### SessionEntryData

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[serde(tag = "type", content = "data")]
pub enum SessionEntryData {
    Message {
        message: Message,
    },
    Checkpoint {
        reason: CheckpointReason,
        note: Option<String>,
    },
    Summary {
        summary: String,
    },
}
```

### CheckpointReason

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckpointReason {
    Compression,
    Manual,
}
```

### JSONL file format

```jsonl
{"id":"a1","session_id":"s1","created_at":100,"parent_id":null,"type":"message","data":{"message":{...}}}
{"id":"a2","session_id":"s1","created_at":200,"parent_id":null,"type":"checkpoint","data":{"reason":"compression","note":null}}
{"id":"a3","session_id":"s1","created_at":300,"parent_id":null,"type":"summary","data":{"summary":"compressed content..."}}
{"id":"a4","session_id":"s1","created_at":400,"parent_id":"a1","type":"message","data":{"message":{...}}}
```

File path: `{entry_dir}/{session_id}.jsonl`

---

## Interfaces

### SessionEntryStore (trait)

Replaces `MessageStore`. The single storage trait for all entry types.

```rust
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
```

### Implementations

| Struct | Scope |
|--------|-------|
| `FileSessionEntryStore` | Persist to `{entry_dir}/{session_id}.jsonl` |
| `InMemoryEntryStore` | In-memory `Vec<SessionEntry>` for tests |

### FileSessionEntryStore

```rust
pub struct FileSessionEntryStore {
    entry_dir: PathBuf,
    session_id: String,
    file_path: PathBuf,  // {entry_dir}/{session_id}.jsonl
}
```

- `entry_dir` is configurable (passed in `new()`)
- File name is `{session_id}.jsonl` (not nested in `sessions/` subdirectory)
- Append-only writes

---

## Session Changes

### Before

```rust
pub struct Session {
    id: String,
    created_at: i64,
    metadata: HashMap<String, String>,
    session_store: Arc<dyn SessionStore>,
    message_store: Arc<dyn MessageStore>,     // ← replaced
    compressed_messages: Vec<SessionMessage>, // ← in-memory, lost on restart
    compressed_after_ts: Option<i64>,         // ← in-memory, lost on restart
    compressor: Arc<dyn MessageCompressor>,
}
```

### After

```rust
pub struct Session {
    id: String,
    created_at: i64,
    metadata: HashMap<String, String>,
    session_store: Arc<dyn SessionStore>,
    entry_store: Arc<dyn SessionEntryStore>,  // ← new
    compressor: Arc<dyn MessageCompressor>,
}
```

### New Session methods

```rust
impl Session {
    /// Add a message entry (backwards compatible with existing callers).
    pub async fn add_message(&self, message: SessionMessage) -> Result<()> {
        let entry = SessionEntry::new_message(self.id.clone(), message);
        self.entry_store.save(entry).await
    }

    /// Get messages — filters Message entries from the store.
    /// Respects compression: Summary entries are included as synthetic messages.
    pub async fn get_messages(&self, limit: usize) -> Result<Vec<SessionMessage>> { ... }

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

    /// Get resume entries — all entries after the latest checkpoint.
    /// If no checkpoint exists, returns all entries.
    pub async fn resume_entries(&self) -> Result<Vec<SessionEntry>> {
        match self.entry_store.find_latest_checkpoint().await? {
            Some(cp) => self.entry_store.get_after(cp.created_at, usize::MAX).await,
            None => self.entry_store.get_entries(usize::MAX).await,
        }
    }

    /// Convert resume_entries to Message Vec for context rebuilding.
    /// Summary entries become synthetic system messages.
    pub async fn resume_messages(&self) -> Result<Vec<Message>> { ... }
}
```

### Compress integration

```rust
// In Session::compress():
// 1. Write summary entry
self.add_summary(compressed_text).await?;
// 2. Write checkpoint
self.checkpoint(CheckpointReason::Compression, None).await?;
```

---

## ReActAgent Resume

### New method

```rust
impl ReActAgent {
    /// Resume from an existing session. Loads checkpoint-based history as context,
    /// then starts a new run with the given user input.
    pub async fn resume(&self, user_input: &str) -> Result<AgentResponse, AgentError> {
        // 1. Load resume entries from session
        let entries = self.session.resume_entries().await?;

        // 2. Convert to messages (Summary → synthetic system messages)
        let resume_messages = self.session.resume_messages().await?;

        // 3. Pre-populate session messages so init_messages picks them up
        //    SessionContributor will load these as Middle-zone context
        for msg in &resume_messages {
            self.session.add_message(SessionMessage::new(
                self.session.id.clone(),
                msg.clone(),
            )).await?;
        }

        // 4. Run normally — SessionContributor loads history
        self.run(user_input).await
    }
}
```

Note: Resume always requires new user input. The previous history is loaded as context, not replayed.

### CodingAgent integration

`CodingAgent` should expose a `resume()` method that delegates to the underlying `ReActAgent.resume()`.

---

## Breaking Changes

1. `MessageStore` trait is **removed**. All implementations replaced by `SessionEntryStore`.
2. `FileMessageStore` is **removed**. Replaced by `FileSessionEntryStore`.
3. `InMemoryMessageStore` is **removed**. Replaced by `InMemoryEntryStore`.
4. `Session::new()` signature changes — takes `entry_store` instead of `message_store`.
5. Existing JSONL files remain readable (migration reads old `SessionMessageLine` format).

---

## File Changes

| File | Action | Purpose |
|------|--------|---------|
| `crates/vol-session/src/lib.rs` | Modify | Re-exports |
| `crates/vol-session/src/entry.rs` | **Create** | SessionEntry, SessionEntryType, SessionEntryData, CheckpointReason |
| `crates/vol-session/src/message.rs` | Modify | Keep as convenience type, or remove in favor of SessionEntry |
| `crates/vol-session/src/store.rs` | Modify | Replace MessageStore with SessionEntryStore trait |
| `crates/vol-session/src/file_store.rs` | Modify | FileSessionEntryStore replacing FileMessageStore |
| `crates/vol-session/src/memory_store.rs` | Modify | InMemoryEntryStore replacing InMemoryMessageStore |
| `crates/vol-session/src/session.rs` | Modify | Use entry_store, add checkpoint/resume methods |
| `crates/vol-session/src/listener.rs` | Modify | SessionListener writes SessionEntry instead of SessionMessage |
| `crates/vol-session/tests/` | Modify | Update tests for new types |
| `crates/vol-llm-agent/src/react/agent.rs` | Modify | Add resume() method |
| `crates/vol-llm-agent/src/react/run_context.rs` | Modify | SessionContributor works with entry-based Session |
| `crates/vol-llm-agent/src/react/context_contributors.rs` | Modify | SessionContributor loads resume entries |
| `crates/vol-llm-agents/src/coding/agent.rs` | Modify | Expose resume() to callers |
| All callers of MessageStore | Modify | Update to SessionEntryStore |

---

## Migration Strategy

1. Keep `SessionMessage` as a convenience wrapper that internally creates `SessionEntry::Message`
2. Old JSONL files: `FileSessionEntryStore` detects old `event` field format and converts on read
3. New writes always use the new format

---
