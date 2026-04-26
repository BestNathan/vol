# Move run_id into Session Message Metadata — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove `run_id` from session types. `SessionEntryData::Message` wraps `SessionMessage` (which carries metadata). `SessionEntry::from_message()` takes `SessionMessage` by value.

**Architecture:** `SessionMessage` owns `metadata` for extensibility and persistence. `SessionEntry` has no `run_id` or `metadata` — the message data variant IS `SessionMessage`, so metadata flows through serialization naturally. No backward compatibility for old JSONL.

**Tech Stack:** Rust, serde, tokio, vol-session crate

---

### Task 1: Update `SessionEntry` — remove `run_id`, `SessionEntryData::Message` wraps `SessionMessage`

**Files:**
- Modify: `crates/vol-session/src/entry.rs`

- [ ] **Step 1: Rewrite `entry.rs`**

Full file content:

```rust
//! Session entry types for multi-type session persistence.

use serde::{Deserialize, Serialize};

use crate::message::SessionMessage;

/// Metadata key for run_id.
pub const RUN_ID_KEY: &str = "run_id";

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
        message: SessionMessage,
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
    /// Create a message entry from a SessionMessage.
    pub fn from_message(msg: SessionMessage) -> Self {
        Self {
            id: msg.id.clone(),
            session_id: msg.session_id.clone(),
            created_at: msg.created_at,
            parent_id: msg.parent_id.clone(),
            r#type: SessionEntryType::Message,
            data: SessionEntryData::Message { message: msg },
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
}
```

Key changes:
- `run_id` removed from `SessionEntry`
- `SessionEntryData::Message` wraps `SessionMessage` (not `vol_llm_core::Message`)
- `from_message()` takes `SessionMessage` by value
- `RUN_ID_KEY` constant exported
- No `into_message()` — callers match on `SessionEntryData::Message { message }` directly

- [ ] **Step 2: Verify compilation**

Run: `cargo check -p vol-session`
Expected: Fails at call sites still using `SessionEntry::new_message()` or referencing `.run_id`.

- [ ] **Step 3: Commit**

```bash
git add crates/vol-session/src/entry.rs
git commit -m "refactor(vol-session): SessionEntry.from_message() takes SessionMessage, no run_id"
```

---

### Task 2: Update `SessionMessage` — remove `run_id`

**Files:**
- Modify: `crates/vol-session/src/message.rs`

- [ ] **Step 1: Rewrite `message.rs`**

Full file content:

```rust
//! Session message wrapper.
//!
//! Wraps `vol_llm_core::Message` with session-related fields.

use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use vol_llm_core::Message;

/// Session message wrapper
///
/// Wraps `vol_llm_core::Message` with session-related fields.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SessionMessage {
    pub id: String,
    pub session_id: String,
    pub message: Message,
    pub parent_id: Option<String>,
    pub created_at: i64,
    pub metadata: HashMap<String, String>,
}

impl SessionMessage {
    pub fn new(session_id: String, message: Message) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            session_id,
            message,
            parent_id: None,
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64,
            metadata: HashMap::new(),
        }
    }

    pub fn with_parent_id(mut self, parent_id: String) -> Self {
        self.parent_id = Some(parent_id);
        self
    }

    pub fn with_metadata(mut self, key: &str, value: &str) -> Self {
        self.metadata.insert(key.to_string(), value.to_string());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_message_creation() {
        let msg = SessionMessage::new("session-123".to_string(), Message::user("Hello"));
        assert_eq!(msg.session_id, "session-123");
        assert!(msg.parent_id.is_none());
        assert!(!msg.id.is_empty());
    }

    #[test]
    fn test_session_message_with_parent() {
        let msg = SessionMessage::new("session-123".to_string(), Message::user("Reply"))
            .with_parent_id("msg-456".to_string());
        assert_eq!(msg.parent_id, Some("msg-456".to_string()));
    }

    #[test]
    fn test_session_message_metadata() {
        let msg = SessionMessage::new("session-123".to_string(), Message::user("Test"))
            .with_metadata("user_id", "user-1");
        assert_eq!(msg.metadata.get("user_id"), Some(&"user-1".to_string()));
    }
}
```

Key changes:
- Removed `run_id: Option<String>` field
- Removed `with_run_id()` builder

- [ ] **Step 2: Verify compilation**

Run: `cargo check -p vol-session`

- [ ] **Step 3: Commit**

```bash
git add crates/vol-session/src/message.rs
git commit -m "refactor(vol-session): remove run_id field from SessionMessage"
```

---

### Task 3: Update `FileSessionEntryStore` — remove `run_id` from JSONL line

**Files:**
- Modify: `crates/vol-session/src/file_store.rs`

- [ ] **Step 1: Update `SessionEntryLine` and serialization**

Replace the struct (remove `run_id`):

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
struct SessionEntryLine {
    id: String,
    session_id: String,
    created_at: i64,
    parent_id: Option<String>,
    r#type: String,
    data: serde_json::Value,
}
```

Update `to_json` — remove `run_id` field:

```rust
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
```

Update `from_json` — remove `run_id`:

```rust
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
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check -p vol-session`

- [ ] **Step 3: Commit**

```bash
git add crates/vol-session/src/file_store.rs
git commit -m "refactor(vol-session): remove run_id from SessionEntryLine JSONL format"
```

---

### Task 4: Update `Session` — use `from_message`

**Files:**
- Modify: `crates/vol-session/src/session.rs`

- [ ] **Step 1: Update `add_message`**

```rust
pub async fn add_message(&self, message: SessionMessage) -> Result<()> {
    let entry = SessionEntry::from_message(message);
    self.entry_store.save(entry).await
}
```

Note: `from_message` takes by value, so no `&` or `.clone()`.

- [ ] **Step 2: Update `get_messages`**

Message arm — just move the inner `SessionMessage` out:

```rust
SessionEntryData::Message { message } => {
    messages.push(message);
}
```

Summary arm — unchanged pattern, create SessionMessage manually:

```rust
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
```

- [ ] **Step 3: Update `compress`**

Replace the inline entry construction with:

```rust
let entry = SessionEntry::from_message(msg);
```

The for loop becomes:
```rust
for (i, msg) in compressed.into_iter().enumerate() {
    let mut entry = SessionEntry::from_message(msg);
    entry.created_at = checkpoint_ts + 1 + (i as i64);
    if let Err(e) = self.entry_store.save(entry).await {
        tracing::error!("Failed to write compressed message: {}", e);
    }
}
```

- [ ] **Step 4: Verify compilation and tests**

Run: `cargo test -p vol-session`
Expected: All tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/vol-session/src/session.rs
git commit -m "refactor(vol-session): Session uses SessionEntry::from_message"
```

---

### Task 5: Update `SessionListener` — use `with_metadata` for run_id

**Files:**
- Modify: `crates/vol-session/src/listener.rs`

- [ ] **Step 1: Update `record_event`**

Replace:

```rust
async fn record_event(&self, event: &AgentStreamEvent) -> Result<(), SessionError> {
    let session_msg = match self.event_to_message(event) {
        Some(msg) => msg.with_metadata(crate::entry::RUN_ID_KEY, &self.run_id),
        None => return Ok(()),
    };

    let entry = SessionEntry::from_message(session_msg);
    self.store.save(entry).await.map_err(SessionError::StoreError)
}
```

run_id flows through `SessionMessage::with_metadata` → `metadata` → JSONL (since `SessionEntryData::Message` wraps the full `SessionMessage`).

- [ ] **Step 2: Verify compilation and tests**

Run: `cargo test -p vol-session`
Expected: All listener tests pass.

- [ ] **Step 3: Commit**

```bash
git add crates/vol-session/src/listener.rs
git commit -m "refactor(vol-session): SessionListener uses with_metadata for run_id"
```

---

### Task 6: Update all tests

**Files:**
- Modify: `crates/vol-session/tests/integration_test.rs`
- Modify: `crates/vol-session/src/file_store.rs` (inline `entry_tests`)
- Modify: `crates/vol-session/src/memory_store.rs` (inline `entry_tests`)
- Modify: `crates/vol-session/src/lib.rs`

- [ ] **Step 1: Fix integration tests**

All `SessionEntry::new_message("session", "run-1", Message::...)` calls become:

```rust
SessionEntry::from_message(
    SessionMessage::new("session-1".to_string(), Message::user("Hello"))
)
```

For `test_file_entry_store_mixed_types`, replace the summary struct (no `run_id` field):

```rust
let summary = SessionEntry::new_summary("session-mixed".to_string(), "Session summary".to_string());
```

- [ ] **Step 2: Fix file_store.rs inline tests**

Replace `SessionEntry::new_message("session", "run-1", Message::...)` with:

```rust
SessionEntry::from_message(
    SessionMessage::new("test-session".to_string(), Message::user("hello"))
)
```

Tests needing explicit timestamps:
```rust
let mut entry = SessionEntry::from_message(
    SessionMessage::new("test-session".to_string(), Message::user("before"))
);
entry.created_at = 1000;
```

- [ ] **Step 3: Fix memory_store.rs inline tests**

Same pattern:
```rust
let entry = SessionEntry::from_message(
    SessionMessage::new("test-session".to_string(), Message::user("Hello, World!"))
);
```

Explicit timestamps on entry after construction.

- [ ] **Step 4: Update lib.rs exports**

```rust
pub use entry::{CheckpointReason, RUN_ID_KEY, SessionEntry, SessionEntryData, SessionEntryType};
```

- [ ] **Step 5: Full build and test**

Run: `cargo test -p vol-session --all-features`
Expected: All tests pass.

- [ ] **Step 6: Commit**

```bash
git add crates/vol-session/tests/integration_test.rs crates/vol-session/src/file_store.rs crates/vol-session/src/memory_store.rs crates/vol-session/src/lib.rs
git commit -m "refactor(vol-session): update tests for SessionEntry::from_message pattern"
```

---

### Task 7: Verify workspace-wide

- [ ] **Step 1: Search for remaining `.run_id` references in vol-session**

Run: `grep -rn "\.run_id\|run_id:" crates/vol-session/src/ crates/vol-session/tests/ --include="*.rs"`

Expected: Only `listener.rs` struct field `run_id: String`, `RUN_ID_KEY` constant, string literals in tests.

- [ ] **Step 2: Search for `SessionEntry::new_message` callers**

Run: `grep -rn "SessionEntry::new_message" crates/vol-session/ --include="*.rs"`

Expected: None — replaced by `SessionEntry::from_message`.

- [ ] **Step 3: Full workspace check**

Run: `cargo check --workspace`
Expected: No errors.

- [ ] **Step 4: Full workspace test**

Run: `cargo test --workspace`
Expected: All tests pass.

- [ ] **Step 5: Final commit if any stragglers**
