---
type: source
source_type: code
date: 2026-09-02
ingested: 2026-09-02
tags: [task-id, session, binding, serde, zero-migration, atomicity]
---

# TaskId Representation Unification + Session-Task Binding

**Authors/Creators:** Claude Code (subagent-driven development)
**Date:** 2026-09-02
**Branch:** `feat/taskid-unification-session-binding`
**Specs:** [[taskid-representation-unification-design]], [[session-task-binding-design]]

## TL;DR

Two coordinated features landed in one branch of 22 commits: (1) `TaskId` now serializes as the decimal string `"1"` with hand-written serde + `Display` + `FromStr`, accepting legacy numeric ids and single-`t`-prefixed strings to keep the change zero-migration; (2) sessions can bind to a list of task ids via a generic `SessionEntryStore` metadata mechanism that is atomic at the store level, with a `Session::bind_task_ids` API called from `run_input` before the agent loop (warn-only on failure). The build profile was also repaired mid-run — `debug = 1` + `split-debuginfo = "unpacked"` — dropping test binaries from 463 MB to 125 MB and `vol-agent-server` cold-link from OOM-after-50-min to 5m15s.

## Key Takeaways

- **Lenient serde is the zero-migration trick.** `TaskId::Deserialize` accepts `1` (legacy), `"1"` (canonical), and `"t1"` (historical), but only writes `"1"`. Old rows in both the SQLite/Postgres task store and the file store keep loading without a migration.
- **Atomicity lives at the store, not the caller.** `SessionEntryStore::append_session_metadata_values` performs the read-modify-write inside each backend's own lock/transaction. The file backend's per-instance mutex is documented as not covering cross-instance merges via `FileSessionManager`, but the database and in-memory backends are fully atomic.
- **A `CallRecordingStore` test pins call structure, not just outcome.** Because `bind_task_ids`'s contract is "one atomic store call", a decorator that records which methods were invoked and asserts the exact vector is a deterministic regression guard — no concurrency, no timing.
- **`union_metadata_values` is a shared pure function** defined once in `store.rs` and called by all three backends. Semantics cannot drift.
- **Build profile is load-bearing on small boxes.** With `debug = true` (the default), `.debug*` sections were 360 MB of a 463 MB test binary and cargo linked `-j nproc` of those at once on 8 GB RAM, exhausting swap. `debug = 1` keeps line tables (panics still report `file:line`) while dropping the binary to 125 MB and peak RSS to 1.06 GB.
- **`sessions.metadata` was a dead column waiting to be used.** The column was created in `m0001` and always written as the literal `"{}"`. Reusing it required no migration.
- **The React frontend was already mistyped.** It declared `TaskEntry.id: number` while the wire had been sending strings for a while; the fix was to align the declared type with reality, plus add the optional `task_ids?: string[]`.

## Detailed Summary

### TaskId representation

Before: `TaskId(pub u64)` derived `Serialize`/`Deserialize` (bare number in JSON), a handwritten `Display` (`t{id}`), and `.0.to_string()` was used in many places (`"1"`). Three representations for one type, and the CLI tool printed `t42` but parsed `--id` with `value_parser!(u64)` — a model echoing back an id it had just been shown errored.

After: hand-written impls:
- `Display` → `1` (no prefix)
- `FromStr` → strips one `t`, rejects `"ttt1"`, rejects negatives
- `Serialize` → `"1"` (always canonical string)
- `Deserialize` → accepts number, canonical string, or single-`t`-prefixed string (lenient — this is what makes it zero-migration)

The CLI parser now uses `parse_task_id_arg` that routes through `TaskId::from_str`, so `1`, `"1"`, and `"t1"` all work at the input boundary.

### Session-task binding

A generic `SessionEntryStore` metadata layer was added:
- `get_session_metadata(session_id)` → `Map<String, Value>` (empty for unknown sessions)
- `merge_session_metadata(session_id, patch)` → shallow merge with upsert
- `append_session_metadata_values(session_id, key, values: &[String])` → **atomic** union into an array at `key`

Three backends:
- **In-memory:** single `tokio::sync::RwLock<HashMap>` covers both reads and writes
- **Database (`vol-session::database_store`):** reuses the `sessions.metadata TEXT NOT NULL` column created in `m0001` — zero migration. `mutate_session_metadata` does the upsert + ownership guard + conditional update in one transaction
- **File (`vol-session::file_store`):** sidecar `{session_id}.meta.json`. Writes go to `{session_id}.meta.json.{pid}.{seq}.tmp` then atomic rename; a `tokio::sync::Mutex<()>` serializes within a single store instance

**The file backend's per-instance lock is a documented limitation**, not a bug. `FileSessionManager::entry_store_for_agent` constructs a new store per call, so cross-instance merges through the manager can lose keys. Atomic rename prevents corruption. The database backend is the production answer for concurrent workloads.

### Session API

`Session::bind_task_ids(&self, ids: &[String])` delegates to the atomic `append_session_metadata_values`. Ordering is numeric on read (`task_ids()` sorts with `u64` key); the stored array preserves bind order.

A deterministic `CallRecordingStore` test (`test_bind_task_ids_makes_exactly_one_atomic_store_call`) asserts the exact call vector `["append_session_metadata_values"]` — a get-then-merge regression fails immediately with no concurrency needed.

### Agent-side plumbing

- `AgentInput.task_ids: Vec<vol_llm_task::TaskId>` (by-value passthrough means zero downstream changes to `AgentPayload::Submit`, `AgentRequest`, dispatcher, control-plane)
- `run_input` calls `bind_task_ids` before any spawned task, logs `warn!` with `run_id`, `session_id`, `task_ids`, `error` on failure, and **does not abort the run**
- `RunContext.task_ids` added as an attachment point for future context-injection / tool-scoping (nothing reads it yet)

### Read surface

`SessionInfo.metadata: Map<String, Value>` added. Database mapping stops dropping the column, degrades malformed text via `unwrap_or_default`. File backend `list_sessions` populates metadata via N+1 (one `read_to_string` per session) — documented at the call site, permitted by the spec for a dev/local backend. The data-plane handler emits `"metadata": <object>` in session JSON.

`SessionInfo` dropped its `Eq` derive (because `serde_json::Value` holds `f64`).

### Frontend

- `TaskEntry.id: string`, `dependencies: string[]`, `blocks: string[]`
- Removed the `t{}` prefix from `TasksPanel` and `TaskDepGraph`
- `agent.submit` params type corrected from `input: string` to the structured shape with `task_ids?: string[]`

### Build profile

`[profile.dev]` now has `debug = 1` and `split-debuginfo = "unpacked"`. Measured:
- `vol_agent_server` test binary: 462 MB → 125 MB (`.debug*` 360 MB → 73 MB)
- `vol-agent-server` lib tests cold: OOM after ~50 min → 5m15s, peak RSS 1.06 GB, zero swaps

CLAUDE.md was updated with corrected timings, scoping rules (`-p <crate>`, `--lib` when tests live in `src/`, never `--workspace` test builds), and three verified gate gaps.

## Known Gate Gaps (documented in CLAUDE.md, not fixed by this branch)

- `justfile:154` `cover-gate` reads `awk '{print $4}'` (llvm-cov's region cover) while the recipe's own output calls it "line coverage" — every number reported is mislabeled.
- `indexing_slicing = "deny"` has never run on test code because neither `just clippy` nor `clippy-strict` passes `--all-targets`. 58+ violations exist.
- `vol-llm-ui`'s `web` feature does not compile on main (6 pre-existing `E0063` errors). CI never sees it because the crate's default feature is `tui` and the web module is gated `not(feature = "tui")`.

## Entities Mentioned

- [[vol-session]]: gained three new trait methods + three backends + `Session::bind_task_ids` + `SessionInfo.metadata`
- [[vol-llm-task]]: `TaskId` serde/Display/FromStr rewritten
- [[vol-llm-agent]]: `AgentInput.task_ids` field, `run_input` bind call, `RunContext.task_ids` field
- [[vol-llm-agent-protocol]]: `TaskPayload::Get { task_id: TaskId }` (was `u64`); added `vol-llm-task` dependency (no cycle)
- [[vol-agent-server]]: task handler emits string ids; session handler emits `metadata`
- React frontend: `TaskEntry.id: string`, `agent.submit` type corrected
- Dioxus mirror (`vol-llm-ui`, deprecated): string ids, prefix removed

## Concepts Covered

- [[session-task-binding]]: new concept page
- [[lenient-serde-zero-migration]]: the `Deserialize` trick that keeps old rows loading

## Notes

- **`TaskId(0)` is a live sentinel** meaning "unassigned" (checked as `task.id.0 == 0` in `mapping.rs`). Serialization is `"0"`, the sentinel check compares before serialization, so it still works.
- **SQLite returns `BUSY` under genuine parallel write transactions** — `bind_task_ids` on the database backend fails loudly and retryably in this case rather than losing data silently.
- **Empty `values` is a no-op and skips the ownership check** on all three backends. The trait doc states it plainly; this means a caller that reads `Ok(())` as "this session is mine" would be wrong for the empty case.
- **The in-memory backend's `delete_session` now clears metadata** — verified by a second test (`test_delete_session_leaves_other_sessions_metadata_alone`) added because the obvious wrong fix (`.clear()`) would have passed the first test alone.
- **Final whole-branch review (opus)** triaged 20 deferred-minors as all safe-to-defer; zero blocking issues. Branch ready to merge with follow-ups.
