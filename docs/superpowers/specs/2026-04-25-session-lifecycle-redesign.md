# Session Lifecycle Redesign Spec

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Simplify Session to entry-only management with checkpoint-based reads, correct compress flow, and add `Session::resume()` constructor.

**Architecture:** Session owns only `entry_store` — no `SessionStore`. `SessionStore` is for a future external `SessionManager`. Checkpoint acts as a logical boundary: reads always start from the latest checkpoint, compress writes checkpoint BEFORE compressed content.

**Tech Stack:** vol-session crate, Rust async

---

## Problems with Current Design

1. **`Session::new(id, session_store, entry_store)`** requires external ID — new sessions should self-generate
2. **`SessionStore` unused** — no one calls `session_store.create()` / `update()`. Session metadata management belongs in a future external `SessionManager`
3. **Compress order wrong** — currently `delete_all → summary → compressed → checkpoint`. Should be: `checkpoint → summary → compressed`, no delete
4. **`get_messages(limit)`** has arbitrary limit, ignores checkpoint boundary
5. **No resume constructor** — can't reconstruct from a known session ID

## Corrected Lifecycle

```
new() → self-generate UUID → append messages
    → compress triggers:
        get_messages() (all after latest CP) → compress
        → write checkpoint ("seal old messages")
        → write summary
        → write compressed messages
    → continue appending new messages

resume(id) → load from entry_store → get_messages() starts from latest CP
```

No delete. Checkpoint is a logical seal — everything before it is "history".

## Session Entry Store Contract Changes

**Core principle:** `SessionEntryStore` is a multi-session storage layer, NOT bound to a single session. Read methods accept `session_id` to scope operations. `save()` uses `entry.session_id` internally — no extra parameter needed.

### Trait signature

```rust
#[async_trait]
pub trait SessionEntryStore: Send + Sync {
    // Write: entry already carries session_id, no param needed.
    async fn save(&self, entry: SessionEntry) -> Result<()>;

    // Read: need session_id to scope to the right session's data.
    async fn get_entries(&self, session_id: &str) -> Result<Vec<SessionEntry>>;
    async fn get_after(&self, session_id: &str, after: i64) -> Result<Vec<SessionEntry>>;
    async fn find_latest_checkpoint(&self, session_id: &str) -> Result<Option<SessionEntry>>;

    // Management: session_id specifies which session to operate on.
    async fn delete_session(&self, session_id: &str) -> Result<()>;
    async fn get_count(&self, session_id: &str) -> Result<usize>;
}
```

Compare with the old signature — `get_entries(limit)`, `get_after(after, limit)`, `find_latest_checkpoint()`, `delete_session()`, `get_count()` all had NO session_id, relying on construction-time binding.

### `FileSessionEntryStore` changes

```rust
// Old: binds session_id at construction
FileSessionEntryStore::new(dir, session_id)

// New: session-agnostic, resolves file path per-method-call
FileSessionEntryStore::new(dir)
```

Internally, each method resolves `{dir}/{session_id}.jsonl` from the passed `session_id`.

### `InMemoryEntryStore` changes

Internal storage changes from `Vec<SessionEntry>` (single session) to `HashMap<String, Vec<SessionEntry>>` keyed by `session_id`, following the same pattern as `InMemoryMessageStore`.

### Legacy methods removed

- `get_entries(limit: usize)` → `get_entries(session_id: &str)` — no limit, scoped to session
- `get_after(after: i64, limit: usize)` → `get_after(session_id: &str, after: i64)` — no limit, scoped to session

## Session API Changes

### Fields (simplified)

```rust
pub struct Session {
    pub id: String,
    pub created_at: i64,
    entry_store: Arc<dyn SessionEntryStore>,
    compressor: Arc<dyn MessageCompressor>,
}
```

Removed: `session_store`, `metadata`.

### Constructors

```rust
impl Session {
    /// New session — self-generates UUID, current timestamp.
    pub fn new(
        entry_store: Arc<dyn SessionEntryStore>,
    ) -> Self;

    /// Resume from existing session — external ID provided.
    /// Loads created_at from the first entry if available.
    pub fn resume(
        id: String,
        entry_store: Arc<dyn SessionEntryStore>,
    ) -> Result<Self>;
}
```

### Read Methods

```rust
impl Session {
    /// Get all messages after the latest checkpoint.
    /// If no checkpoint exists, returns all messages.
    /// No limit parameter.
    pub async fn get_messages(&self) -> Result<Vec<SessionMessage>> {
        let entries = match self.entry_store.find_latest_checkpoint(&self.id).await? {
            Some(cp) => self.entry_store.get_after(&self.id, cp.created_at).await?,
            None => self.entry_store.get_entries(&self.id).await?,
        };
        // convert entries to SessionMessage...
    }
}
```

Resume 就是构造 Session，之后 `get_messages()` 自动返回 checkpoint 之后的内容（或全部）。不需要单独的 `resume_messages()`。

### Compress

```rust
impl Session {
    /// Compress flow:
    /// 1. Write checkpoint entry (seal)
    /// 2. Compress input messages
    /// 3. Write summary entry
    /// 4. Write compressed message entries
    pub async fn compress(&mut self, messages: Vec<SessionMessage>);
}
```

### Write (unchanged)

```rust
impl Session {
    pub async fn add_message(&self, message: SessionMessage) -> Result<()>;
    pub async fn add_summary(&self, summary: String) -> Result<()>;
    pub async fn checkpoint(&self, reason: CheckpointReason, note: Option<String>) -> Result<()>;
}
```

## Compressor Trait (unchanged)

```rust
#[async_trait]
pub trait MessageCompressor: Send + Sync {
    async fn compress(&self, messages: Vec<SessionMessage>) -> Vec<SessionMessage>;
}
```

## File Changes Summary

| File | Change |
|------|--------|
| `crates/vol-session/src/session.rs` | Rewrite: remove session_store/metadata, new/resume constructors, no-limit reads, correct compress, pass `&self.id` to store methods |
| `crates/vol-session/src/store.rs` | Add `session_id: &str` to all read/management methods on SessionEntryStore trait |
| `crates/vol-session/src/file_store.rs` | Constructor: `new(dir)` only (no session_id). Methods accept session_id to resolve `{dir}/{session_id}.jsonl` |
| `crates/vol-session/src/memory_store.rs` | InMemoryEntryStore: internal storage → `HashMap<String, Vec<SessionEntry>>`, methods accept session_id |
| `crates/vol-session/src/lib.rs` | Update re-exports if affected |
| `crates/vol-session/src/listener.rs` | Update SessionEntryStore method calls with session_id |
| `crates/vol-session/tests/` | Update tests for new API |
| `crates/vol-llm-agent/src/react/` | Update Session creation sites (new/resume) |
| `crates/vol-llm-agent/src/react/context_contributors.rs` | Update SessionContributor (no limit on get_messages) |
| `crates/vol-llm-tui/src/main.rs` | Update session creation: `FileSessionEntryStore::new(dir)`, `Session::new(entry_store)` |
| `crates/vol-llm-agents/src/coding/` | Update session creation sites |
