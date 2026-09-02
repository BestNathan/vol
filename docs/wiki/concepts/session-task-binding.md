---
type: concept
category: architecture
tags: [session, task, binding, atomicity, metadata]
created: 2026-09-02
updated: 2026-09-02
source_count: 1
---

# Session-Task Binding

**Category:** Agent session ↔ task association
**Related:** [[session-as-ssot]], [[run-context]], [[vol-session]], [[vol-llm-task]]

## Definition

A session can bind to a list of task ids, recorded as a generic session-level metadata array at the well-known key `task_ids`. The binding is **append-only** (union semantics, no unbind) and **atomic at the store level** — a single store call performs the read-modify-write inside the backend's own lock or transaction.

## Key Facts

- The binding lives in `SessionEntryStore`'s generic metadata map, not a dedicated column. `Session::TASK_IDS_KEY = "task_ids"` is the well-known key.
- `Session::bind_task_ids(&self, ids: &[String])` delegates to `SessionEntryStore::append_session_metadata_values`, which performs the union inside each backend's own critical section.
- Ordering is numeric on read (the `task_ids()` method sorts with a `u64` key); the stored array preserves bind order so consumers reading the raw metadata get unsorted ids — only `task_ids()` should be used for presentation.
- No validation that bound task ids exist. `vol-session` has no `TaskStore` and must not acquire one.
- The field is `Vec<String>` at the session layer (not `Vec<TaskId>`) so `vol-session` does not depend on `vol-llm-task` — that would close a dev-dependency cycle. The `TaskId → String` conversion happens at the one place both are in scope: `run_input` in `vol-llm-agent`.

## How It Works

### Atomic append

The trait method is `append_session_metadata_values(session_id, key, values: &[String])`. Each backend performs the union inside its own critical section:

- **In-memory:** single `tokio::sync::RwLock<HashMap>` — read, union, write all under one write lock
- **Database:** one transaction: upsert with `OnConflict::do_nothing` → ownership guard (`load_owned_session`) → read `metadata` column → apply union → conditional update_many
- **File:** `tokio::sync::Mutex<()>` held across read-sidecar → union → write-temp → atomic rename

The union semantics are defined once by the pure function `union_metadata_values(&mut Map, &str, &[String]) -> Result<bool>` in `store.rs` so the three backends cannot drift.

### File backend limitation (documented)

`FileSessionManager::entry_store_for_agent` constructs a **new** store per call, so `meta_write_lock` — which is per-instance — serializes nothing across manager-mediated callers. Atomic rename prevents file corruption; a concurrent merge through separate store instances can still lose keys. The database backend is the production answer for concurrent workloads.

### Deterministic regression guard

A test-only `CallRecordingStore` decorator implements `SessionEntryStore` and records which methods were invoked. The test `test_bind_task_ids_makes_exactly_one_atomic_store_call` asserts the call vector is exactly `["append_session_metadata_values"]` and that `get_session_metadata` / `merge_session_metadata` are never called. This is a call-structure assertion, not a behavioral one — because the contract is inherently about call structure. The test is deterministic (no concurrency) and would catch a get-then-merge regression immediately.

### Failure semantics

`run_input` calls `bind_task_ids` before any spawned task. A failure logs `warn!` carrying `run_id`, `session_id`, `task_ids`, `error` and **does not abort the run**. Binding is metadata; losing it should not kill a user's run.

## SQLite BUSY

SQLite returns `database is locked` (mapped to `StoreError::Database`) under genuine parallel write transactions. The test `test_database_append_values_succeeds_under_concurrent_binds` retries only on that specific message and returns all other errors immediately; it still asserts `len == WRITERS` and per-value presence. Net: the database backend fails loudly and retryably instead of losing data silently.

## Empty values skip ownership check

`append_session_metadata_values` returns `Ok(())` without entering the transaction when `values` is empty. This is a documented optimization, but a caller that reads `Ok(())` as "this session is mine" would be wrong for the empty case.

## Examples / Applications

```rust
// In run_input, before any spawned task:
if !input.task_ids.is_empty() {
    let ids: Vec<String> = input.task_ids.iter().map(ToString::to_string).collect();
    if let Err(e) = run_ctx.session.bind_task_ids(&ids).await {
        tracing::warn!(
            run_id = %run_ctx.run_id,
            session_id = %run_ctx.session_id,
            task_ids = ?ids,
            error = %e,
            "failed to bind task ids to session"
        );
    }
}
```

```rust
// Test: call-structure assertion, deterministic
let store = CallRecordingStore::new(Arc::new(InMemoryEntryStore::new()));
let session = Session::with_id("s1".into(), Arc::new(store.clone()));
session.bind_task_ids(&["1".into(), "2".into()]).await.unwrap();
assert_eq!(store.calls(), vec!["append_session_metadata_values"]);
```

## Related Concepts

- [[session-as-ssot]]: Session is the single source of truth for messages; binding extends it to carry task association
- [[run-context]]: `RunContext.task_ids` is the attachment point for future context-injection and tool-scoping
- [[lenient-serde-zero-migration]]: the pattern that kept old task rows loading
- [[vol-session]]: owns the `SessionEntryStore` trait and all three backend implementations
- [[vol-llm-agent]]: `run_input` is the write point; `AgentInput.task_ids` is the wire entry point
