# Session Message Compression Design

> Status: draft — 2026-04-22

## Problem

When session history exceeds the LLM context budget, `SessionContributor` needs to compress messages. Currently `compress()` is a no-op — the contributor returns all messages every time, which will overflow the context window for long sessions.

## Solution

Three-layer design:

1. **`MessageCompressor` trait** — abstract compression: input messages, output fewer messages
2. **`Session::compress(messages)`** — accepts the messages that were already fetched, compresses them, stores result as "精华"
3. **`SessionContributor::compress()`** — delegates to Session using its cached messages

Key insight: **compression input is exactly what `get_messages()` already returned**. No extra storage query needed.

## Architecture

```
┌─────────────────────────────────────────────────┐
│ SessionContributor                               │
│   cached_blocks = get_messages(limit)            │
│                     ↓                            │
│ ContextBuilder over budget → calls compress()    │
│                     ↓                            │
│ Session.compress(cached_messages)                │
│   → compressor.compress(cached_messages)         │
│   → stores result as compressed_messages         │
│   → sets cursor to last compressed message ts    │
│                     ↓                            │
│ Next get_messages() returns:                     │
│   [compressed精华] + [after_cursor 最新]          │
└─────────────────────────────────────────────────┘
```

## Data Structures

### `MessageCompressor` trait (vol-session)

```rust
#[async_trait]
pub trait MessageCompressor: Send + Sync {
    /// Compress a set of messages into a smaller set.
    /// Input: the messages that SessionContributor just contributed
    ///        (i.e., what get_messages(limit) returned).
    /// Output: a smaller set of "精华" messages to keep in context.
    async fn compress(&self, messages: Vec<SessionMessage>) -> Vec<SessionMessage>;
}
```

### `Session` new fields

```rust
pub struct Session {
    // ... existing fields ...
    /// Compressed "精华" messages from history.
    compressed_messages: Vec<SessionMessage>,
    /// Timestamp cursor — only fetch messages after this point.
    /// Set when compress() is called, to the ts of the last
    /// message that was compressed.
    compressed_after_ts: Option<i64>,
    /// Compression strategy (injected, configurable).
    compressor: Arc<dyn MessageCompressor>,
}
```

### `Session::compress(messages)`

```rust
impl Session {
    /// Compress the given messages and store the result as "精华".
    /// The input `messages` is what was just returned by get_messages().
    pub async fn compress(&mut self, messages: Vec<SessionMessage>) {
        if messages.is_empty() {
            return;
        }

        // Compress to "精华"
        let compressed = self.compressor.compress(messages).await;

        // Update cursor: last message ts from the input set
        let last_ts = compressed.last()
            .map(|m| m.message.timestamp)
            .or_else(|| compressed.first().map(|m| m.message.timestamp));

        self.compressed_messages = compressed;
        if let Some(ts) = last_ts {
            self.compressed_after_ts = Some(ts);
        }
    }

    pub async fn get_messages(&self, limit: usize) -> Result<Vec<SessionMessage>> {
        let mut result = Vec::new();

        // First: compressed "精华" messages
        result.extend(self.compressed_messages.clone());

        // Then: latest messages after cursor (only if compressed)
        if let Some(after_ts) = self.compressed_after_ts {
            let latest = self.message_store
                .get_after(&self.id, after_ts, limit)
                .await
                .unwrap_or_default();
            result.extend(latest);
        } else {
            // No compression yet — return normally
            let normal = self.message_store
                .get_by_session(&self.id, limit)
                .await
                .unwrap_or_default();
            result.extend(normal);
        }

        Ok(result)
    }
}
```

## Compression Strategies (builtin implementations)

### `PositionSampleCompressor`
- Keep first N messages (always preserve session start)
- Sample every M-th message from the rest
- No external dependencies, deterministic

### `RoleFilterCompressor`
- Keep messages from selected roles (e.g., only User + final Assistant)
- Filter out intermediate tool calls
- Simple rule-based

### `LlmSummaryCompressor` (future)
- Send old messages to LLM, get back a summary
- Requires LLM client injection

## MessageStore: new method

```rust
#[async_trait]
pub trait MessageStore: Send + Sync {
    // ... existing methods ...

    /// Get messages after a timestamp (for compressed history).
    async fn get_after(
        &self,
        session_id: &str,
        after: i64,
        limit: usize,
    ) -> Result<Vec<SessionMessage>>;
}
```

## SessionContributor changes

```rust
pub struct SessionContributor {
    session: Arc<Mutex<Session>>,  // changed from Arc<Session> — compress needs &mut
    max_history: usize,
    cached_blocks: Option<Vec<ContextBlock>>,
}

impl SessionContributor {
    async fn compress(&mut self) {
        // Get the messages that are currently cached
        if let Some(ref blocks) = self.cached_blocks {
            let messages: Vec<SessionMessage> = blocks.iter()
                .flat_map(|b| b.messages.iter().map(|m| {
                    SessionMessage::new(self.session.lock().await.id.clone(), m.clone())
                }))
                .collect();

            let mut session = self.session.lock().await;
            session.compress(messages).await;
        }

        // Invalidate cache — next contribute() will get compressed result
        self.cached_blocks = None;
    }
}
```

## Flow

```
1. SessionContributor.contribute() → Session.get_messages(limit)
   → returns [Msg1..Msg20] → cached as blocks

2. ContextBuilder.build() estimates tokens → over budget
   → calls SessionContributor.compress()

3. SessionContributor passes cached messages to Session.compress()
   → compressor.compress([Msg1..Msg20]) → [Msg1*, Msg5*, Msg15*]
   → Session.compressed_messages = [Msg1*, Msg5*, Msg15*]
   → Session.compressed_after_ts = Msg15.timestamp

4. SessionContributor clears cached_blocks

5. ContextBuilder re-collects blocks → SessionContributor.contribute()
   → Session.get_messages() → [Msg1*, Msg5*, Msg15*] + [Msg21..] (after cursor)
   → smaller, within budget
```

## File Structure

| File | Action | Purpose |
|------|--------|---------|
| `crates/vol-session/src/compressor.rs` | Create | `MessageCompressor` trait |
| `crates/vol-session/src/compressors/mod.rs` | Create | Compressor module |
| `crates/vol-session/src/compressors/position_sample.rs` | Create | PositionSampleCompressor |
| `crates/vol-session/src/compressors/role_filter.rs` | Create | RoleFilterCompressor |
| `crates/vol-session/src/session.rs` | Modify | Add compress(), get_messages() rewrite, compressed fields, compressor |
| `crates/vol-session/src/store.rs` | Modify | Add `get_after` to MessageStore trait |
| `crates/vol-session/src/memory_store.rs` | Modify | Implement `get_after` for InMemoryMessageStore |
| `crates/vol-session/src/file_store.rs` | Modify | Implement `get_after` for FileMessageStore |
| `crates/vol-llm-agent/src/react/context_contributors.rs` | Modify | SessionContributor uses Arc<Mutex<Session>>, delegate compress() to Session |
