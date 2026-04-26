# Move Compressor from Session to SessionContributor

## Context

`MessageCompressor` is currently stored on `Session`, but `Session` is the data layer — it should only write/read entries. Compression is a context management concern driven by `SessionContributor` (the `ContextContributor` that provides session history to LLM context). Moving compressor to `SessionContributor` makes the ownership clear: the contributor decides when and how to compress its own data source.

## Current State

```
Session {
    compressor: Arc<dyn MessageCompressor>,  // ← lives here but shouldn't
    compress(messages) → writes checkpoint + summary + compressed entries
}

SessionContributor {
    compress() → gets messages from session, calls session.compress(messages)
}
```

The compressor is owned by Session but controlled by SessionContributor — split responsibility.

## After

```
Session {
    // No compressor field
    // No compress() method
    // Just: add_message, get_messages, checkpoint, add_summary
}

SessionContributor {
    compressor: Arc<dyn MessageCompressor>,
    compress() → gets messages, calls compressor, writes checkpoint/summary/compressed to session
}
```

## Changes

### 1. `crates/vol-session/src/session.rs`

- Remove `compressor: Arc<dyn MessageCompressor>` field from `Session` struct
- Remove `with_compressor()` method
- Remove `compress()` method
- Keep: `add_message`, `checkpoint`, `add_summary`, `get_messages`, `resume_messages`, `new`, `resume`

### 2. `crates/vol-session/src/session_contributor.rs`

- Add `compressor: Arc<dyn MessageCompressor>` field to `SessionContributor`
- Keep `new(session, max_history)` signature with default `PositionSampleCompressor::default()`
- Add `with_compressor(compressor)` builder method for custom compressors
- Replace current `compress()` implementation: instead of calling `session.compress(messages)`, implement the compression logic inline:
  1. Get messages from session via `get_messages()`
  2. Call `compressor.compress(messages)`
  3. Write checkpoint entry to session
  4. Build summary text from compressed messages
  5. Write summary entry to session
  6. Write compressed message entries to session

### 3. `crates/vol-session/src/lib.rs`

- Export `MessageCompressor` and `PositionSampleCompressor` so callers can configure `SessionContributor`

### 4. Update callers

- `crates/vol-llm-agent/src/react/agent.rs` — `SessionContributor::new()` call site in `get_context()` may need updating (currently uses `SessionContributor::new(session, max_history)`)
- `crates/vol-session/src/session.rs` tests — remove `test_session_compress_flow`, `test_session_compress_empty_messages`, `test_session_resume_messages_includes_summary` tests that depend on `session.compress()`. Move equivalent tests to `session_contributor.rs`.
- `crates/vol-session/src/integration_test.rs` (if any) — update
- `crates/vol-llm-agent/tests/compression_flow_test.rs` — update to use `SessionContributor` with compressor

### 5. `crates/vol-session/src/compressor.rs`

No changes — the trait definition stays as is.

## Verification

```bash
cargo test -p vol-session -- --test-threads=1
cargo test -p vol-llm-agent -- --test-threads=1
cargo check --workspace
```
