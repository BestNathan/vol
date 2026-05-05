---
type: concept
category: framework
tags: [session, contributor, context]
created: 2026-05-04
updated: 2026-05-04
source_count: 1
---

# Session Contributor

**Category:** Context contributor pattern
**Related:** [[session-as-ssot]], [[context-builder]], [[session-compression]]

## Definition

`SessionContributor` is a `ContextContributor` that reads conversation history from the Session and returns it as context blocks for prompt construction. It is the bridge between persistent session storage and the agent's prompt context.

## Key Points
- Reads messages from Session on every `contribute()` call — no caching [[session-ssot-redesign]]
- Applies `max_history` limit (takes last N messages) [[session-ssot-redesign]]
- Deleted `cached_blocks` field — Session itself is the cache [[session-ssot-redesign]]
- `compress()` mutates Session in place; next `contribute()` sees the compressed result [[session-ssot-redesign]]
- `estimate_size()` returns 0 (best-effort without full Session read) [[session-ssot-redesign]]
- Returns `Result<Vec<ContextBlock>, ContextError>` — session read failures abort context build [[session-ssot-redesign]]

## How It Works

```rust
impl SessionContributor {
    async fn contribute(&self) -> Result<Vec<ContextBlock>, ContextError> {
        let history = self.session.lock().await.get_messages().await?;
        if history.is_empty() {
            return Ok(vec![]);
        }
        let trimmed: Vec<Message> = history
            .into_iter()
            .map(|sm| sm.message)
            .rev()
            .take(self.max_history)
            .rev()
            .collect();
        let block = ContextBlock::new(trimmed, AttentionAnchor::Middle(0));
        Ok(vec![block])
    }
}
```

The contributor takes the last `max_history` messages from the session, wraps them in a `ContextBlock` with an `AttentionAnchor::Middle(0)` (no special attention weighting), and returns the block. If the session is empty, returns an empty vector.

The `compress()` method calls `session.compress(messages)` to reduce message size via summarization. Since compression mutates the Session, the next `contribute()` call automatically reads the compressed messages.

## Examples / Applications

- **Normal operation**: Returns last N messages from session as context
- **After compression**: Returns compressed summary messages, reducing token usage
- **Session failure**: Returns `ContextError::Session(...)`, aborting context build

## Related Concepts
- [[session-as-ssot]]: SessionContributor reads from Session as SSOT
- [[context-builder]]: SessionContributor is a context contributor
- [[session-compression]]: SessionContributor.compress() triggers compression
- [[context-error]]: Session errors propagated through contributor
