# Design: Move run_id into Session Message Metadata

## Problem

`run_id` is an agent execution concept, not a session-level concept. A session can have many runs, so attaching `run_id` as a dedicated field on session types conflates two independent lifecycles. `SessionMessage` already has `metadata: HashMap<String, String>` — that's where run_id belongs.

## Decision

Remove `run_id` from `SessionEntry` (both the struct and the data variant). `SessionEntryData::Message` wraps `SessionMessage` instead of `vol_llm_core::Message`, so metadata (including run_id) flows through serialization naturally. `SessionEntry` itself has no `run_id` or `metadata` field — it delegates entirely to the inner `SessionMessage`.

Callers set run_id via `SessionMessage::with_metadata(RUN_ID_KEY, ...)`.

**No backward compatibility** — old JSONL files with top-level `run_id` will have that field silently ignored on deserialization.

## Changes

### 1. `crates/vol-session/src/entry.rs`

- Remove `run_id: Option<String>` from `SessionEntry` — no metadata field either
- `SessionEntryData::Message { message: vol_llm_core::Message }` → `SessionEntryData::Message { message: SessionMessage }`
- `from_message(msg: SessionMessage) -> Self`: wraps SessionMessage into entry, copies non-nested fields from msg
- `new_checkpoint()` / `new_summary()`: unchanged
- Export constant `RUN_ID_KEY: &str = "run_id"` for callers

### 2. `crates/vol-session/src/message.rs`

- Remove `run_id: Option<String>` from `SessionMessage`
- Remove `with_run_id()` builder
- `with_metadata()` already exists — callers use it: `msg.with_metadata(RUN_ID_KEY, run_id)`
- `metadata: HashMap<String, String>` remains

### 3. `crates/vol-session/src/file_store.rs`

- `SessionEntryLine`: remove `run_id` field — no metadata field (metadata is nested inside SessionMessage in data)
- `to_json()` / `from_json()`: map SessionEntry as-is, `SessionMessage` serializes/deserializes via serde automatically

### 4. `crates/vol-session/src/session.rs`

- `add_message()`: `SessionEntry::from_message(message)`
- `get_messages()`: extract `SessionMessage` from `SessionEntryData::Message { message }` directly
- `compress()`: `SessionEntry::from_message()` for compressed entries

### 5. `crates/vol-session/src/listener.rs`

- `SessionListener` still carries `run_id` (needed at construction)
- `record_event()`: create `SessionMessage` with `.with_metadata(RUN_ID_KEY, &self.run_id)`, pass to `SessionEntry::from_message()`

### 6. `crates/vol-session/tests/`

- Update tests: `SessionEntry::new_message(session_id, run_id, message)` → `SessionEntry::from_message(SessionMessage::new(session_id, message))`
- Tests that need explicit timestamps set `entry.created_at = ...` after construction
- Summary entries use `SessionEntry::new_summary()` directly

## Non-changes

- `vol-llm-agent` structs (`RunContext.run_id`, `Response.run_id`, `HITLRequest.run_id`, plugin fields) — agent-level concepts, not session-level. No change needed.
