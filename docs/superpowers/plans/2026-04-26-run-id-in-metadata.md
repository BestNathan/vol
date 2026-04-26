# Move run_id into Session Entry Metadata — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove the dedicated `run_id` field from session types and store it in `metadata["run_id"]` instead.

**Architecture:** Replace `run_id: Option<String>` with `metadata: HashMap<String, String>` on `SessionEntry` and `SessionEntryLine`, remove `run_id` from `SessionMessage`, and route run_id through metadata at all call sites. No backward compatibility for old JSONL.

**Tech Stack:** Rust, serde, tokio, vol-session crate

---

### Task 1: Update `SessionEntry` — remove `run_id`, add `metadata`

**Files:**
- Modify: `crates/vol-session/src/entry.rs`

- [ ] **Step 1: Rewrite `SessionEntry` struct, `SessionEntryData`, and constructors**

Replace the `run_id` field with `metadata: HashMap<String, String>`, add the `RUN_ID_KEY` constant, and update constructors. Full file content:

```rust
//! Session entry types for multi-type session persistence.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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
    /// Extensible metadata. run_id is stored under `RUN_ID_KEY` if present.
    #[serde(default)]
    pub metadata: HashMap<String, String>,
}

impl SessionEntry {
    /// Create a new message entry.
    pub fn new_message(session_id: String, run_id: Option<String>, message: vol_llm_core::Message) -> Self {
        let mut metadata = HashMap::new();
        if let Some(rid) = run_id {
            metadata.insert(RUN_ID_KEY.to_string(), rid);
        }
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            session_id,
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64,
            parent_id: None,
            r#type: SessionEntryType::Message,
            data: SessionEntryData::Message { message },
            metadata,
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
            metadata: HashMap::new(),
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
            metadata: HashMap::new(),
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

Key changes:
- `run_id: Option<String>` → `metadata: HashMap<String, String>` with `#[serde(default)]`
- `new_message()` signature: `run_id: String` → `run_id: Option<String>`, puts it in metadata
- `RUN_ID_KEY` constant exported

- [ ] **Step 2: Verify compilation**

Run: `cargo check -p vol-session`
Expected: May fail at call sites that still reference `.run_id` — that's expected, we fix them in subsequent tasks.

- [ ] **Step 3: Commit**

```bash
git add crates/vol-session/src/entry.rs
git commit -m "refactor(vol-session): replace run_id field with metadata HashMap on SessionEntry"
```

---

### Task 2: Update `SessionMessage` — remove `run_id`

**Files:**
- Modify: `crates/vol-session/src/message.rs`

- [ ] **Step 1: Rewrite `SessionMessage` struct**

Full file content:

```rust
//! Session message wrapper.
//!
//! Wraps `vol_llm_core::Message` with session-related fields.

use std::collections::HashMap;
use vol_llm_core::Message;

/// Session message wrapper
///
/// Wraps `vol_llm_core::Message` with session-related fields.
#[derive(Clone, Debug)]
pub struct SessionMessage {
    /// Message unique ID (UUID)
    pub id: String,

    /// Session ID this message belongs to
    pub session_id: String,

    /// Core message body
    pub message: Message,

    /// Parent message ID, supports tree conversation structure
    /// None means root message (conversation start)
    pub parent_id: Option<String>,

    /// Creation timestamp (Unix seconds)
    pub created_at: i64,

    /// Metadata for extensible purposes
    /// e.g., user_id, tags, etc. run_id stored under RUN_ID_KEY.
    pub metadata: HashMap<String, String>,
}

impl SessionMessage {
    /// Create a new session message.
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

    /// Set parent message ID
    pub fn with_parent_id(mut self, parent_id: String) -> Self {
        self.parent_id = Some(parent_id);
        self
    }

    /// Add metadata
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
- `with_metadata()` remains

- [ ] **Step 2: Verify compilation**

Run: `cargo check -p vol-session`
Expected: May fail at call sites still referencing `.run_id`.

- [ ] **Step 3: Commit**

```bash
git add crates/vol-session/src/message.rs
git commit -m "refactor(vol-session): remove run_id field from SessionMessage"
```

---

### Task 3: Update `FileSessionEntryStore` — `SessionEntryLine` metadata

**Files:**
- Modify: `crates/vol-session/src/file_store.rs`

- [ ] **Step 1: Update `SessionEntryLine` and serialization**

Only change the `SessionEntryLine` struct and the `to_json`/`from_json` functions. Replace:

```rust
/// JSONL line format for SessionEntry.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct SessionEntryLine {
    id: String,
    session_id: String,
    created_at: i64,
    parent_id: Option<String>,
    r#type: String,
    data: serde_json::Value,
    #[serde(default)]
    metadata: HashMap<String, String>,
}
```

Update `to_json`:
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
        metadata: entry.metadata.clone(),
    };
    serde_json::to_string(&line).map_err(|e| {
        StoreError::Serialization(format!("Failed to serialize entry: {}", e))
    })
}
```

Update `from_json`:
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
        metadata: line.metadata,
    })
}
```

Also add `use std::collections::HashMap;` at the top of the file if not present.

- [ ] **Step 2: Verify compilation**

Run: `cargo check -p vol-session`

- [ ] **Step 3: Commit**

```bash
git add crates/vol-session/src/file_store.rs
git commit -m "refactor(vol-session): replace run_id with metadata in SessionEntryLine"
```

---

### Task 4: Update `Session` — remove run_id mappings

**Files:**
- Modify: `crates/vol-session/src/session.rs`

- [ ] **Step 1: Update `add_message`**

Change:
```rust
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
        metadata: message.metadata.clone(),
    };
    self.entry_store.save(entry).await
}
```

- [ ] **Step 2: Update `get_messages`**

Replace the two `SessionMessage` construction blocks in `get_messages()` (the Message and Summary arms):

```rust
SessionEntryData::Message { message } => {
    messages.push(SessionMessage {
        id: entry.id,
        session_id: entry.session_id,
        message,
        parent_id: entry.parent_id,
        created_at: entry.created_at,
        metadata: entry.metadata,
    });
}
SessionEntryData::Summary { summary } => {
    messages.push(SessionMessage {
        id: entry.id,
        session_id: entry.session_id,
        message: Message::system(summary),
        parent_id: entry.parent_id,
        created_at: entry.created_at,
        metadata: entry.metadata,
    });
}
```

- [ ] **Step 3: Update `compress`**

In the compressed message entry construction:
```rust
let mut entry = SessionEntry {
    id: msg.id.clone(),
    session_id: self.id.clone(),
    created_at: msg.created_at.max(checkpoint_ts + 1),
    parent_id: msg.parent_id.clone(),
    r#type: SessionEntryType::Message,
    data: SessionEntryData::Message {
        message: msg.message.clone(),
    },
    metadata: msg.metadata.clone(),
};
```

- [ ] **Step 4: Add `HashMap` import if needed**

Ensure `use std::collections::HashMap;` is at the top (it already is in the current file).

- [ ] **Step 5: Verify compilation and tests**

Run: `cargo check -p vol-session && cargo test -p vol-session`
Expected: All tests pass (session tests don't assert on run_id field directly).

- [ ] **Step 6: Commit**

```bash
git add crates/vol-session/src/session.rs
git commit -m "refactor(vol-session): remove run_id mapping from Session methods"
```

---

### Task 5: Update `SessionListener` — put run_id into metadata

**Files:**
- Modify: `crates/vol-session/src/listener.rs`

- [ ] **Step 1: Update `record_event`**

Replace the `record_event` method:

```rust
async fn record_event(&self, event: &AgentStreamEvent) -> Result<(), SessionError> {
    let session_msg = match self.event_to_message(event) {
        Some(msg) => msg,
        None => return Ok(()),
    };

    let mut metadata = session_msg.metadata;
    metadata.insert(crate::entry::RUN_ID_KEY.to_string(), self.run_id.clone());

    let entry = SessionEntry {
        id: session_msg.id,
        session_id: session_msg.session_id,
        created_at: session_msg.created_at,
        parent_id: session_msg.parent_id,
        r#type: SessionEntryType::Message,
        data: SessionEntryData::Message {
            message: session_msg.message,
        },
        metadata,
    };

    self.store.save(entry).await.map_err(SessionError::StoreError)
}
```

- [ ] **Step 2: Verify compilation and tests**

Run: `cargo test -p vol-session`
Expected: All listener tests pass.

- [ ] **Step 3: Commit**

```bash
git add crates/vol-session/src/listener.rs
git commit -m "refactor(vol-session): SessionListener puts run_id into entry metadata"
```

---

### Task 6: Update integration tests and remaining references

**Files:**
- Modify: `crates/vol-session/tests/integration_test.rs`
- Modify: `crates/vol-session/src/file_store.rs` (inline tests)
- Modify: `crates/vol-session/src/memory_store.rs` (inline tests)

- [ ] **Step 1: Fix integration test — summary creation**

In `test_file_entry_store_mixed_types`, replace the summary construction:

```rust
let summary = SessionEntry {
    id: "summary-1".to_string(),
    session_id: "session-mixed".to_string(),
    created_at: 300,
    parent_id: None,
    r#type: vol_session::SessionEntryType::Summary,
    data: summary_data,
    metadata: std::collections::HashMap::new(),
};
```

- [ ] **Step 2: Fix file_store.rs inline tests**

In `crates/vol-session/src/file_store.rs`, the `entry_tests` module uses `SessionEntry::new_message()` which now takes `Option<String>` for run_id. Change all occurrences of `"run-1".to_string()` to `Some("run-1".to_string())`. For example:

```rust
let entry = SessionEntry::new_message(
    "test-session".to_string(),
    Some("run-1".to_string()),
    Message::user("Hello, World!"),
);
```

Apply this to all test functions in the `entry_tests` module.

- [ ] **Step 3: Fix memory_store.rs inline tests**

Same treatment — change all `"run-1".to_string()` / `"run-a".to_string()` / `"run-b".to_string()` in `InMemoryEntryStore` tests to `Some(...)`. For example:

```rust
let entry = SessionEntry::new_message(
    "test-session".to_string(),
    Some("run-1".to_string()),
    Message::user("Hello, World!"),
);
```

- [ ] **Step 4: Update lib.rs exports**

Add `RUN_ID_KEY` to the public exports:

```rust
pub use entry::{CheckpointReason, RUN_ID_KEY, SessionEntry, SessionEntryData, SessionEntryType};
```

- [ ] **Step 5: Full build and test**

Run: `cargo test -p vol-session --all-features`
Expected: All tests pass.

- [ ] **Step 6: Commit**

```bash
git add crates/vol-session/tests/integration_test.rs crates/vol-session/src/file_store.rs crates/vol-session/src/memory_store.rs crates/vol-session/src/lib.rs
git commit -m "refactor(vol-session): update tests for run_id in metadata"
```

---

### Task 7: Verify no remaining run_id references in vol-session

**Files:**
- Search: `crates/vol-session/`

- [ ] **Step 1: Search for remaining `.run_id` references**

Run: `grep -rn "run_id" crates/vol-session/src/ crates/vol-session/tests/ --include="*.rs"`

Expected output: Only `listener.rs` field `run_id: String` (the internal struct field), `RUN_ID_KEY` constant references, and test string literals should remain. No `.run_id` field access on `SessionEntry` or `SessionMessage`.

- [ ] **Step 2: Full workspace check**

Run: `cargo check --workspace`
Expected: No errors.

- [ ] **Step 3: Full workspace test**

Run: `cargo test --workspace`
Expected: All tests pass.

- [ ] **Step 4: Final commit if any stragglers found**

If any unexpected references remain, fix and commit.
