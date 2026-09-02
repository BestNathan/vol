# Session ↔ Task Binding via Session Metadata

**Date:** 2026-09-02
**Status:** Design — approved, pending implementation plan
**Depends on:** [[2026-09-02-taskid-representation-unification-design]]
**Related:** [[vol-session]], [[vol-llm-task-crate]], [[run-context]], [[session-as-ssot]]

## Problem

A conversation often works on several tasks, but nothing records which. There
is no association between a session and a task in either direction.

The session layer has exactly one scoping dimension today: `agent_id` (a DB
column, a filesystem directory level, and a parameter on every `SessionManager`
method). No user, project, environment, or tenant concept exists in production
code — a workspace grep for `user_id|tenant|project_id|workspace_id` returns
three hits, all test-body string literals.

So this is not extending an identity model. It is introducing the first one,
and it should leave room for `project_id` / `environment_id` / `user_id` to
follow without another schema change.

## Decision

Bind through a **generic session-level metadata map**, with `task_ids` as the
first key. No new column, no new table, no typed `SessionBinding` struct.
Future dimensions are new keys in the same map.

**Scope: pure data maintenance.** The binding is recorded and readable. It does
*not* change agent behaviour — bound tasks are not injected into the prompt and
do not scope the `task` tool. Both are plausible follow-ups and are explicitly
out of scope here.

Both storage backends must support it.

## Design

### 1. The storage hook that already exists

`sessions.metadata TEXT NOT NULL` was created by
`m0001_create_sessions.rs:29` and is **written as the literal `"{}"` and never
read** (`database_store/mod.rs:206`). `mapping.rs:59-72`
(`session_model_to_info`) drops it. It is a fully-migrated column waiting for
a reader.

The file backend has no equivalent. `FileSessionEntryStore` is append-only
JSONL at `{entry_dir}/{agent_type}/{session_id}.jsonl` — no row, no header,
nowhere to hang a value. Session listing is a directory scan.

`Session::with_metadata` (`session.rs:143-146`) is a no-op stub that discards
both arguments; its only caller is the test at `session.rs:186-194` asserting
it does nothing.

### 2. `SessionEntryStore` gains two methods

`crates/vol-session/src/store.rs:59-77`:

```text
async fn get_session_metadata(&self, session_id: &str)
    -> Result<serde_json::Map<String, Value>>;

async fn merge_session_metadata(&self, session_id: &str,
    patch: serde_json::Map<String, Value>) -> Result<()>;
```

`merge` is a **shallow merge with upsert**. Upsert matters: a run may bind task
ids before the first message is written, so the session row need not exist yet.

Shallow, not deep: `task_ids` is replaced wholesale by the caller, which
computes the union first. Deep-merge semantics for arrays are ambiguous and not
needed.

### 3. Backend implementations

**`DatabaseSessionEntryStore`** — read/write `sessions.metadata`. No migration.

Both methods **must** route through `load_owned_session`
(`database_store/mod.rs:222-248`), the guard that raises
`StoreError::SessionAgentScopeConflict` when `session.agent_id != store.agent_id`.
Metadata must not become a way around it.

`merge` is load → parse → merge → update, inside one transaction. The existing
`ensure_session_for_entry` (`database_store/mod.rs:193-220`) uses
`OnConflict…do_nothing`, so it will not clobber metadata written earlier.

**`FileSessionEntryStore`** — sidecar `{entry_dir}/{agent_type}/{session_id}.meta.json`.

Three constraints, each a real failure mode:

1. **Path construction must reuse the existing validated logic** at
   `file_store.rs:58-66`, not rebuild it. `FileSessionManager` hardened
   `agent_id` against path traversal, quarantining bad values under
   `agents_root/.invalid-agent-id/<hex>/sessions`. A second, naive path builder
   would reopen that hole.
2. **`list_sessions` must keep ignoring sidecars.** The directory scan at
   `file_store.rs:200-264` produces one `SessionSummary` per file it
   recognizes. Verified during planning: `file_store.rs:225-227` already
   filters on the `jsonl` extension, so a `.meta.json` sidecar is skipped and
   the phantom-session bug does not exist. Keep a regression test so it stays
   that way.
3. **`delete_session` must remove the sidecar.** Otherwise metadata resurrects
   onto a later session that reuses the id.

Writes are read-modify-write to a temp file plus atomic rename, serialized by a
per-store `tokio::Mutex`. Cross-process concurrency is not handled; the task
store's flock pattern (`stores/file.rs:31-55`) is available if it turns out to
be needed, but sessions are single-writer in practice.

**`InMemoryEntryStore`** — a `DashMap<String, Map<String, Value>>`.

### 4. `Session` API — `crates/vol-session/src/session.rs`

```text
pub const TASK_IDS_KEY: &str = "task_ids";   // beside the existing RUN_ID_KEY

impl Session {
    pub async fn metadata(&self) -> Result<Map<String, Value>>;
    pub async fn merge_metadata(&self, patch: Map<String, Value>) -> Result<()>;
    pub async fn task_ids(&self) -> Result<Vec<String>>;
    pub async fn bind_task_ids(&self, ids: &[String]) -> Result<()>;
}
```

`bind_task_ids` reads the current array, unions, dedups, sorts, writes back.
**Union semantics: the association only grows.** A run carrying task ids adds
to the set; it never replaces it. Unbinding is not supported.

Stored shape, given the id unification spec, is an array of canonical id
strings:

```text
{ "task_ids": ["1", "3", "7"] }
```

**Delete the `with_metadata` no-op** (`session.rs:143-146`) and its test. It is
a `self`-consuming synchronous builder and cannot perform async I/O, so it
cannot be made honest. Its only caller is its own test.

**`vol-session` takes no new dependency — ids are stored as plain `String`.**

Its entire dependency list today is `vol-llm-core` and `vol-llm-context`. It is
a persistence crate and has no business knowing what a task is. Worse, adding
`vol-llm-task` would close a cycle: `vol-llm-task` already dev-depends on
`vol-llm-agent`, and `vol-llm-agent` depends on `vol-session`, giving
`vol-llm-task →(dev) vol-llm-agent → vol-session → vol-llm-task`. Cargo
tolerates dev-dependency cycles, but this one is avoidable and buys nothing:
after the id-unification spec, a `TaskId`'s canonical serialized form *is* the
string being stored.

Typing lives one layer up — `AgentInput` carries `Vec<TaskId>` and converts at
the call site (§6). The boundary check happens where `vol-llm-task` is already
in scope.

**No validation that the tasks exist.** `bind_task_ids` records whatever it is
given; binding an unknown id succeeds silently. Validation would require the
session layer to reach a `TaskStore`, which it has no handle to and should not
acquire for a metadata write.

### 5. Carrying task ids into a run

Add one field to `AgentInput` (`crates/vol-llm-agent/src/react/input.rs:31-38`):

```text
#[serde(default, skip_serializing_if = "Vec::is_empty")]
pub task_ids: Vec<TaskId>,
```

Three edits, all in `input.rs`: the struct (`:31`), the
`AgentInputWire::Structured` variant (`:44-51`), and the `Deserialize` impl
(`:61-69`). Plus the `AgentInput::new` constructor at `:71`.

**Everything downstream passes `AgentInput` by value and needs zero changes** —
`AgentPayload::Submit` (`agent_server_protocol.rs:1041`), the decode
`struct P` (`:491`), `AgentRequest` (`request.rs:9`), `AgentHandler`
(`handlers/agent.rs:77`), the dispatcher (`dispatcher.rs:147-152`), and the
control-plane re-wrap (`control_plane/handlers/client.rs:136`), which forwards
`input` verbatim.

This is why the field goes inside `AgentInput` rather than beside it in the
payload — the alternative touches seven files across three crates and forces a
signature change on `run_input`.

`vol-llm-agent` gains a `vol-llm-task` dependency for `TaskId`. This does close
a dev-dependency cycle (`vol-llm-task` dev-depends on `vol-llm-agent`), which
Cargo permits — it builds the `vol-llm-task` lib, then `vol-llm-agent`, then
`vol-llm-task`'s tests. The cost is compile coupling: `vol-llm-agent` now
rebuilds when `vol-llm-task` changes, which matters given how long test
compilation already takes in this workspace. The benefit is that malformed ids
are rejected at deserialization instead of reaching storage.

If that trade-off is judged wrong during implementation, the fallback is
`Vec<String>` here as well — wire-identical, zero new edges, no validation.

Client side, `frontend/src/components/inputs/InputArea.tsx:96-104` gains
`task_ids` in the `agent.submit` params. Note
`frontend/src/lib/protocol.ts:83-85` still declares
`params: { input: string; target?: string }` — already stale relative to the
`{parts, metadata}` shape actually sent, and worth correcting while here.

### 6. Where the write happens

In `ReActAgent::run_input` (`agent.rs:447`), before the agent loop:

```text
if !input.task_ids.is_empty() {
    let ids: Vec<String> = input.task_ids.iter().map(ToString::to_string).collect();
    if let Err(e) = config.session.bind_task_ids(&ids).await {
        warn!(...);   // do not abort the run
    }
}
```

`TaskId → String` conversion happens here, at the one place where both
`vol-llm-task` and `Session` are in scope. Given the id-unification spec,
`ToString` yields bare canonical digits (`1`), matching what
`TaskId`'s `Serialize` emits and what `FromStr` accepts on the way back.

`run_input`, not `AgentHandler`, because it covers the dispatcher path
(`dispatcher.rs:147`) and any direct caller uniformly, and because
`config.session` is already in hand — `RunContext` derives `session_id` from
`config.session.id` (`run_context.rs:133`), never from the request.

**A failed binding logs and continues.** The binding is metadata; losing it
should not kill a user's run. The cost is that the association can be silently
absent, which the `warn!` is there to surface.

Also add `pub task_ids: Vec<TaskId>` as an immutable field on `RunContext`
(`run_context.rs:112-137`), alongside `run_id` / `session_id` / `model`. Nothing
reads it yet; it is the attachment point for the follow-up features.

Note for those follow-ups: `run_ctx.data` — where `input.metadata` currently
lands (`agent.rs:503-505`) — is **write-only today**, and `ToolContext`
(`vol-llm-tool/src/tool.rs:46-50`) carries no run-scoped fields. Reaching a
tool from here is unsolved and deliberately not solved now.

### 7. Read surface

- `SessionInfo` (`manager.rs:12-22`) gains `metadata: Map<String, Value>`
- `mapping.rs:59-72` (`session_model_to_info`) stops dropping the column
- `handlers/session.rs:89-97`, which hand-builds the session JSON, includes it

**Reverse lookup — "which sessions touched task 7" — is not supported.** It
needs an indexed join table, which is exactly the schema extension this design
rules out. A caller wanting it today must scan sessions.

## Testing

Per CLAUDE.md: `#[cfg(test)]` or `tests/`, **no doc tests**, every new `pub fn`
tested, `just cover-gate vol-session 80`.

**Per backend** (all three — database, file, in-memory):

- round-trip: merge then get
- upsert: merge against a session id with no prior row or entries
- shallow merge: writing key `b` leaves key `a` intact
- unknown session: `get_session_metadata` returns empty, not an error
- concurrent merges do not lose a key

**Database-specific:**

- metadata read *and* write from a store scoped to a different `agent_id`
  raise `SessionAgentScopeConflict`
- a session row created by `ensure_session_for_entry` after a metadata write
  does not reset it to `"{}"`

**File-specific:**

- `list_sessions` does not report a phantom session for a `.meta.json` sidecar
- `delete_session` removes the sidecar
- a traversal-shaped `agent_id` lands in the quarantine directory, matching the
  `.jsonl` behaviour
- a truncated or malformed sidecar degrades to empty metadata rather than
  failing the read

**`bind_task_ids`:**

- union across two calls; no duplicates; order stable
- binding the same id twice is idempotent
- binding an id for a task that does not exist succeeds
- `task_ids()` on a session that was never bound returns empty

**`AgentInput` deserialization — back-compat is the point:**

- a bare JSON string still deserializes (the `AgentInputWire::Text` arm)
- `{parts, metadata}` with no `task_ids` deserializes, field defaults empty
- `{parts, task_ids: ["1","2"]}` deserializes
- serialization omits the field when empty

**Integration:** submit through the dispatcher with `task_ids` set, then read
the session's metadata back and assert the ids are there. Second submit with a
different id set — assert union, not replacement. A submit whose binding write
fails must still complete the run.

**Frontend:** `just fe-test`. Fix the stale `protocol.ts` `agent.submit` type.

**Gates:** `just clippy-strict`, `just no-doc-tests`, `just boundaries`,
`just cover-gate vol-session 80`.

## Out of scope

- Injecting bound tasks into the prompt or context (follow-up)
- Scoping the `task` tool to bound ids (follow-up; needs `RunContext` →
  `ToolContext` plumbing that does not exist)
- Reverse lookup task → sessions, and any index supporting it
- Unbinding, or replace-instead-of-union semantics
- Validating that bound task ids exist
- `project_id` / `environment_id` / `user_id` — the map accommodates them; no
  code here anticipates them
