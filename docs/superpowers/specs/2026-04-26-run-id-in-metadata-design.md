# Design: Move run_id into Session Entry Metadata

## Problem

`run_id` is an agent execution concept, not a session-level concept. A session can have many runs, so attaching `run_id` as a dedicated field on session messages conflates two independent lifecycles. The `SessionMessage` type already has a `metadata: HashMap<String, String>` field that serves exactly this purpose.

## Decision

Remove the dedicated `run_id` field from `SessionEntry`, `SessionMessage`, and `SessionEntryLine`. Store run_id in `metadata["run_id"]` instead.

**No backward compatibility** — old JSONL files with top-level `run_id` will have that field silently ignored on deserialization.

## Changes

### 1. `crates/vol-session/src/entry.rs`

- Remove `run_id: Option<String>` from `SessionEntry`
- Add `metadata: HashMap<String, String>` with `#[serde(default)]`
- `new_message()`: accept `run_id: Option<String>`, put into metadata if present
- `new_checkpoint()` / `new_summary()`: metadata starts empty
- Export constant `RUN_ID_KEY: &str = "run_id"`

### 2. `crates/vol-session/src/message.rs`

- Remove `run_id: Option<String>` from `SessionMessage`
- Remove `with_run_id()` builder
- Add `RUN_ID_KEY` constant (re-exported from `entry`)
- `with_metadata()` already exists — callers use it for run_id

### 3. `crates/vol-session/src/file_store.rs`

- `SessionEntryLine`: remove `run_id`, add `metadata: HashMap<String, String>` with `#[serde(default)]`
- `to_json()` / `from_json()`: map metadata directly

### 4. `crates/vol-session/src/session.rs`

- `add_message()`: no longer map run_id field
- `get_messages()`: remove `run_id` assignment in SessionMessage construction
- `compress()`: remove `run_id` copy from compressed entries

### 5. `crates/vol-session/src/listener.rs`

- `SessionListener` still carries `run_id` (needed at construction)
- `record_event()`: put run_id into entry metadata instead of top-level field

### 6. `crates/vol-session/tests/`

- Update tests that assert on `run_id` field — use metadata instead

## Non-changes

- `vol-llm-agent` structs (`RunContext.run_id`, `Response.run_id`, `HITLRequest.run_id`, plugin fields) — these are agent-level concepts, not session-level. No change needed.
