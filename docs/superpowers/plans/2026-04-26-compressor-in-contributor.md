# Move Compressor from Session to SessionContributor — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Move `MessageCompressor` ownership from `Session` (data layer) to `SessionContributor` (context management layer), and inline the compression logic.

**Architecture:** `SessionContributor` gains a `compressor` field (default: `PositionSampleCompressor`) and a `with_compressor()` builder method. Its `compress()` method replaces calling `session.compress()` with inline logic: get messages → compress → write checkpoint → write summary → write compressed entries. `Session` loses `compressor` and `compress()`.

**Tech Stack:** Rust, async, vol-session crate

---

### Task 1: Add compressor to SessionContributor and implement compression inline

**Files:**
- Modify: `crates/vol-session/src/session_contributor.rs` — add compressor field, implement inline compression
- Test: `crates/vol-session/src/session_contributor.rs` — update/add compression tests

- [ ] **Step 1: Add compressor field and update constructor**

Replace the current `SessionContributor` struct and impl in `crates/vol-session/src/session_contributor.rs`:

```rust
use std::sync::Arc;
use async_trait::async_trait;
use vol_llm_context::{AttentionAnchor, ContextBlock, ContextContributor};
use vol_llm_core::Message;

use crate::compressor::MessageCompressor;
use crate::compressors::PositionSampleCompressor;
use crate::entry::{CheckpointReason, SessionEntry, SessionEntryData, SessionEntryType};
use crate::{Session, SessionMessage};

/// Session contributor — retrieves historical messages from a session
/// and supports compression to manage context size.
pub struct SessionContributor {
    session: Arc<tokio::sync::Mutex<Session>>,
    max_history: usize,
    compressor: Arc<dyn MessageCompressor>,
}

impl SessionContributor {
    pub fn new(session: Arc<tokio::sync::Mutex<Session>>, max_history: usize) -> Self {
        Self {
            session,
            max_history,
            compressor: Arc::new(PositionSampleCompressor::default()),
        }
    }

    /// Set a custom compression strategy.
    pub fn with_compressor(mut self, compressor: Arc<dyn MessageCompressor>) -> Self {
        self.compressor = compressor;
        self
    }
}
```

- [ ] **Step 2: Run tests to verify compilation (before implementing compress)**

Run:
```bash
cargo check -p vol-session
```
Expected: Errors about missing `compress()` on SessionContributor or unused imports — we'll fix in Step 3.

- [ ] **Step 3: Replace the compress() method implementation**

Replace the `compress()` method in the `impl ContextContributor for SessionContributor` block. The current implementation calls `session.compress(messages)` which no longer exists. New implementation:

```rust
async fn compress(&mut self) {
    // 1. Get current messages from session
    let messages = match self.session.lock().await.get_messages().await {
        Ok(msgs) => msgs,
        Err(_) => return,
    };
    if messages.is_empty() {
        return;
    }

    // 2. Compress the messages
    let compressed = self.compressor.compress(messages).await;
    if compressed.is_empty() {
        return;
    }

    // 3. Write checkpoint (seal old messages)
    let checkpoint_ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    let mut cp_entry = SessionEntry::new_checkpoint(
        self.session.lock().await.id.clone(),
        CheckpointReason::Compression,
        None,
    );
    cp_entry.created_at = checkpoint_ts;
    if let Err(e) = self.session.lock().await.entry_store.save(cp_entry).await {
        tracing::error!("Failed to write checkpoint before compression: {}", e);
        return;
    }

    // 4. Build summary text from compressed messages
    let summary = compressed
        .iter()
        .filter_map(|m| m.message.content.as_ref())
        .map(|c| c.as_str())
        .collect::<Vec<_>>()
        .join("\n");

    // 5. Write summary entry (timestamp after checkpoint)
    let mut summary_entry = SessionEntry::new_summary(
        self.session.lock().await.id.clone(),
        summary,
    );
    summary_entry.created_at = checkpoint_ts + 1;
    if let Err(e) = self.session.lock().await.entry_store.save(summary_entry).await {
        tracing::error!("Failed to write summary during compression: {}", e);
        return;
    }

    // 6. Write compressed message entries (timestamp after checkpoint)
    for (i, msg) in compressed.iter().enumerate() {
        let mut entry = SessionEntry {
            id: msg.id.clone(),
            session_id: self.session.lock().await.id.clone(),
            created_at: checkpoint_ts + 1 + (i as i64),
            parent_id: msg.parent_id.clone(),
            r#type: SessionEntryType::Message,
            data: SessionEntryData::Message {
                message: msg.message.clone(),
            },
            run_id: msg.run_id.clone(),
        };
        if let Err(e) = self.session.lock().await.entry_store.save(entry).await {
            tracing::error!("Failed to write compressed message: {}", e);
        }
    }
}
```

- [ ] **Step 4: Update tests**

Replace the existing `test_session_contributor_compress` test and add a test for the custom compressor:

```rust
#[tokio::test]
async fn test_session_contributor_compress() {
    let entry_store = Arc::new(InMemoryEntryStore::new());
    let session = Session::new(entry_store);

    for i in 0..10 {
        let msg = SessionMessage::new(session.id.clone(), Message::user(format!("msg-{}", i)));
        session.add_message(msg).await.unwrap();
    }

    let session = Arc::new(tokio::sync::Mutex::new(session));
    let mut contributor = SessionContributor::new(session.clone(), 10);

    // Before compression
    let blocks = contributor.contribute().await.unwrap();
    assert_eq!(blocks[0].messages.len(), 10);

    // Compress
    contributor.compress().await;

    // After compression — fewer messages
    let blocks = contributor.contribute().await.unwrap();
    assert!(blocks[0].messages.len() < 10);
}

#[tokio::test]
async fn test_session_contributor_compress_empty() {
    let entry_store = Arc::new(InMemoryEntryStore::new());
    let session = Session::new(entry_store);
    let session = Arc::new(tokio::sync::Mutex::new(session));

    let mut contributor = SessionContributor::new(session.clone(), 10);

    // Compress on empty session — no-op
    contributor.compress().await;

    let blocks = contributor.contribute().await.unwrap();
    assert!(blocks.is_empty());
}
```

- [ ] **Step 5: Verify and commit**

Run:
```bash
cargo test -p vol-session -- --test-threads=1
```
Expected: All tests pass (some session tests may still fail if `session.compress()` is called — those will be fixed in Task 2).

```bash
git add crates/vol-session/src/session_contributor.rs
git commit -m "feat(vol-session): add compressor to SessionContributor with inline compression logic

SessionContributor now owns the MessageCompressor and implements
compression inline (checkpoint → compress → summary → compressed entries).
Session.compress() will be removed in the next step."
```

### Task 2: Remove compressor from Session

**Files:**
- Modify: `crates/vol-session/src/session.rs` — remove compressor field, `with_compressor()`, `compress()`
- Test: `crates/vol-session/src/session.rs` — remove session-level compression tests

- [ ] **Step 1: Remove compressor-related code from session.rs**

Remove from `Session` struct:
```rust
// REMOVE this field:
compressor: Arc<dyn MessageCompressor>,
```

Remove from `Session::new()`:
```rust
// REMOVE this line:
compressor: Arc::new(PositionSampleCompressor::default()),
```

Remove from `Session::resume()`:
```rust
// REMOVE this line:
compressor: Arc::new(PositionSampleCompressor::default()),
```

Remove the entire `with_compressor()` method:
```rust
// REMOVE this entire method:
pub fn with_compressor(mut self, compressor: Arc<dyn MessageCompressor>) -> Self {
    self.compressor = compressor;
    self
}
```

Remove the entire `compress()` method (lines 160-218 in current file).

Remove unused imports at the top of the file:
```rust
// REMOVE these imports (no longer used):
use crate::compressor::MessageCompressor;
use crate::compressors::PositionSampleCompressor;
use crate::entry::{CheckpointReason, SessionEntry, SessionEntryData, SessionEntryType};
```

- [ ] **Step 2: Remove session-level compression tests**

Remove from `crates/vol-session/src/session.rs` tests:
- `test_session_compress_flow` (around line 274)
- `test_session_compress_empty_messages` (around line 306)
- `test_session_resume_messages_includes_summary` (around line 318)
- `test_session_multiple_compressions` (around line 363)

These tests now belong to `SessionContributor` tests (added in Task 1).

- [ ] **Step 3: Verify and commit**

Run:
```bash
cargo test -p vol-session -- --test-threads=1
```
Expected: All vol-session tests pass.

```bash
git add crates/vol-session/src/session.rs
git commit -m "refactor(vol-session): remove compressor and compress() from Session

Session is now a pure data layer — it only provides add_message,
get_messages, checkpoint, and add_summary. Compression logic
lives entirely in SessionContributor."
```

### Task 3: Update callers and final verification

**Files:**
- Modify: `crates/vol-llm-agent/tests/compression_flow_test.rs` — uses SessionContributor with compressor (no change needed if default compressor is used)
- Verify: workspace build

- [ ] **Step 1: Check compression_flow_test.rs**

Read `crates/vol-llm-agent/tests/compression_flow_test.rs`. The test already uses `SessionContributor::new(session, max_history)`. Since we kept the same constructor signature with a default compressor, it should still work.

Run:
```bash
cargo test -p vol-llm-agent compression_flow -- --test-threads=1
```
Expected: Tests pass.

- [ ] **Step 2: Full workspace verification**

Run:
```bash
cargo check --workspace
cargo test --workspace -- --test-threads=1
```
Expected: No errors. All tests pass.

- [ ] **Step 3: Commit any remaining changes (if tests needed updates)**

```bash
git add crates/vol-llm-agent/tests/
git commit -m "test: update compression flow tests for SessionContributor compressor ownership"
```
