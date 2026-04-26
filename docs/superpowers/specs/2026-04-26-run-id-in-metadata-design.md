# Design: Move run_id into Session Entry Metadata

## Problem

`run_id` is an agent execution concept, not a session-level concept. A session can have many runs, so attaching `run_id` as a dedicated field on session messages conflates two independent lifecycles. The `SessionMessage` type already has a `metadata: HashMap<String, String>` field that serves exactly this purpose.

## Decision

Remove the dedicated `run_id` field from `SessionEntry` and `SessionMessage`. Add `metadata: HashMap<String, String>` only to `SessionMessage` (it already has this field). `SessionEntry` remains a pure persistence wrapper — no metadata. `SessionEntry::new_message()` takes `SessionMessage` instead of scattered parameters.

Run_id is set by callers via `SessionMessage::with_metadata(RUN_ID_KEY, ...)`. It is **not persisted** to JSONL — run_id is a runtime correlation concept, not a persisted field.

**No backward compatibility** — old JSONL files with top-level `run_id` will have that field silently ignored on deserialization.

## Changes

### 1. `crates/vol-session/src/entry.rs`

- Remove `run_id: Option<String>` from `SessionEntry` — no metadata field either
- `new_message()`: takes `SessionMessage` instead of `(session_id, run_id, message)`
- Extract what it needs from `SessionMessage` (id, session_id, created_at, parent_id, message body)
- `new_checkpoint()` / `new_summary()`: unchanged
- Export constant `RUN_ID_KEY: &str = "run_id"` for callers

### 2. `crates/vol-session/src/message.rs`

- Remove `run_id: Option<String>` from `SessionMessage`
- Remove `with_run_id()` builder
- `with_metadata()` already exists — callers use it for run_id: `msg.with_metadata(RUN_ID_KEY, run_id)`
- `metadata: HashMap<String, String>` remains (runtime-only, not persisted to JSONL)

### 3. `crates/vol-session/src/file_store.rs`

- `SessionEntryLine`: remove `run_id` field — no metadata field added
- `to_json()` / `from_json()`: map SessionEntry as-is (no metadata)

### 4. `crates/vol-session/src/session.rs`

- `add_message()`: pass `SessionMessage` to `SessionEntry::new_message(msg)`
- `get_messages()`: construct `SessionMessage` from entry, metadata starts empty
- `compress()`: `SessionEntry::new_message()` for compressed entries

### 5. `crates/vol-session/src/listener.rs`

- `SessionListener` still carries `run_id` (needed at construction)
- `record_event()`: create `SessionMessage` with `.with_metadata(RUN_ID_KEY, &self.run_id)`, pass to `SessionEntry::new_message()`

### 6. `crates/vol-session/tests/`

- Update tests: `SessionEntry::new_message(session_id, run_id, message)` → `SessionEntry::from_message(&SessionMessage::new(session_id, message))`
- Tests that need explicit timestamps set `entry.created_at = ...` after construction
- Summary entries use `SessionEntry::new_summary()` directly

## Non-changes

- `vol-llm-agent` structs (`RunContext.run_id`, `Response.run_id`, `HITLRequest.run_id`, plugin fields) — these are agent-level concepts, not session-level. No change needed.
