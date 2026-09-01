# TaskId Representation Unification

**Date:** 2026-09-02
**Status:** Design — approved, pending implementation plan
**Related:** [[2026-09-02-session-task-binding-design]], [[vol-llm-task-crate]]

## Problem

`TaskId(pub u64)` renders three different ways depending on which code path
touches it. The split runs *inside* single functions.

| Mechanism | Output | Where |
|---|---|---|
| `impl Display` (`model.rs:12-16`) | `t1` | CLI text, `StoreError::NotFound`, 2 JSON fields |
| `id.0.to_string()` | `"1"` | Most tool `data` JSON |
| derive `Serialize` (newtype passthrough) | `1` | Wire protocol, persistence, `--json` |

Same file, ninety lines apart:

```rust
executor.rs:146  (update) "taskId": task_id.to_string(),   // -> "t5"
executor.rs:232  (stop)   "taskId": id.to_string(),        // -> "5"
```

### Why this reaches the model

Production registers exactly one task tool — `TaskCliTool`, via
`vol_llm_task::tools::register_cli` at `vol-llm-runtime/src/lib.rs:502`. The
seven individual tools under `tools/` are registered only by
`tests/tool_integration.rs:14`.

`TaskCliTool` prints `Task t42 created` (Display) but parses its `--id`
argument with clap `value_parser!(u64)`. A model that echoes back the id it
was just shown gets `Parse error: invalid value 't42' for '--id <id>'`.

The one prefix-tolerant parser in the workspace —
`task_claim.rs:67`, `trim_start_matches('t')`, whose parameter description
literally advertises `"e.g. 't1', 't42'"` — belongs to a tool that is not
registered in production. The most forgiving path is unreachable; the
strictest one is the default.

No `impl FromStr for TaskId` exists anywhere in the workspace.

## Decision

**Canonical form is a JSON string of decimal digits: `"1"`. No prefix, at any
layer.** Human-readable text renders bare digits: `Task 1 created`.

Serde is **asymmetric**: strict on write, lenient on read.

## Design

### 1. `TaskId` core — `crates/vol-llm-task/src/model.rs`

Drop `serde::Serialize, serde::Deserialize` from the derive list on
`model.rs:7-10`; keep `Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord,
Hash`. Hand-write four impls:

```text
Serialize    -> serialize_str(&self.0.to_string())      always "1"
Deserialize  -> visitor accepting u64 | i64 | &str      1 | "1" | "t1"
Display      -> write!(f, "{}", self.0)                 1
FromStr      -> strip ONE optional leading 't', parse   1 | "1" | "t1"
```

`FromStr` strips a single optional `t`, not `trim_start_matches('t')` — the
current greedy version accepts `"ttt1"`. `Deserialize` delegates to `FromStr`
for the string case so the two never drift.

New error type `ParseTaskIdError` for the `FromStr::Err` associated type.

**The lenient `Deserialize` is what makes this a zero-migration change.**
Persisted data written before this change is bare numbers; it keeps loading.

### 2. Persistence — no migration

| Site | Before | After | Migration |
|---|---|---|---|
| `sea_orm` `tasks.id` column (`entity.rs:8-9`) | `i64` | `i64` — unchanged | none |
| `dependencies_json` / `blocks_json` (`mapping.rs:91-97`) | `[1,2]` | `["1","2"]` | none — lenient read |
| `FileTaskStore` filename (`file.rs:80`) | `1.json` | `1.json` — unchanged | none |
| `FileTaskStore` body (`file.rs:59`) | `"id": 1` | `"id": "1"` | none — lenient read |
| `.lock` id counter (`file.rs:31-55`) | bare decimal | unchanged | none |

`mapping.rs:13-22` (`task_id_to_db` / `task_id_from_db`) operates on `.0` and
is unaffected. The `TaskId(0)` "unassigned" sentinel check at `mapping.rs:121`
(`if task.id.0 == 0`) compares the inner `u64` before serialization, so it
also needs no change.

Explicitly **do not** touch `file.rs:80` (`task_path`) or `file.rs:83-96`
(`scan_task_ids`) — both must stay on `.0`. A mechanical
`{task_id.0}` → `{task_id}` refactor would rename files to `t1.json` under the
old Display and silently orphan them under `scan_task_ids`'s
`parse::<u64>()`.

### 3. Wire protocol — `vol-llm-agent-protocol`

```text
agent_server_protocol.rs:1344   Get { task_id: u64 }  ->  Get { task_id: TaskId }
agent_server_protocol.rs:856    decode struct P { task_id: u64 }  ->  TaskId
```

This adds a `vol-llm-task` dependency to `vol-llm-agent-protocol`, whose
current dependency list is `vol-llm-agent` and `vol-llm-sandbox`. Verified: no
cycle in the normal dependency graph — `vol-llm-task` depends only on
`vol-llm-core` and `vol-llm-tool`. It does dev-depend on `vol-llm-agent`, which
Cargo permits. Run `./scripts/check-agent-boundaries.sh` to confirm nothing
else objects.

CLAUDE.md makes `vol-llm-agent-protocol` the owner of wire types, so typing the
id properly here — and rejecting malformed ids at decode time — is the right
place for it. If the dependency is refused, the fallback is
`task_id: String` on the wire with conversion at the handler.

Per CLAUDE.md, no new `*Operation` variant is introduced, so
`operation_codec.rs` and `method_name()` need no new arms. Run
`./scripts/check-protocol-registration.sh` regardless.

### 4. Server handler — `vol-agent-server/src/data_plane/handlers/task.rs`

```text
:62, :88        "id": t.id.0                              -> "id": t.id
:70,:71,:96,:97 dependencies/blocks .map(|d| d.0)          -> serialize TaskId directly
:85             self.store.get(&TaskId(task_id))           -> self.store.get(&task_id)
```

Control-plane (`control_plane/handlers/client.rs:156-175`,
`control_plane/endpoint.rs:34-35`) is pass-through routing and needs no change.

### 5. Tool + CLI output

Once `Display` emits bare digits, most sites correct themselves. Remaining
work is normalizing the `.0.to_string()` family to emit the canonical form and
removing the numeric/string inconsistency.

- `cli/format.rs` — 14, 40, 53, 85
- `cli/executor.rs` — 81, 139, 146, 159, 214, 226, 232, 252, 263, 267, 284,
  292, 299, 307, 312, 314, 327, 344, 363
- `scheduler.rs` — 41, 57, 72 (`StoreError::NotFound(format!("Task {task_id}"))`)
- CLI parser: the `value_parser!(u64)` sites at `cli/parser.rs:82, 89, 100,
  121, 128, 137, 161, 171, 184, 192, 227` become a `TaskId` value parser built
  on `FromStr`

### 6. Test-only tools under `tools/`

These self-align once `Display` changes. Explicit fixes needed:

- `task_claim.rs:48` — parameter description advertising `'t1'`, `'t42'`
- `task_claim.rs:67-71` — delete `trim_start_matches('t')`, use `FromStr`
- `task_get.rs`, `task_list.rs`, `task_update.rs`, `task_stop.rs`,
  `task_output.rs` — `.0.to_string()` sites emit `"1"`, already canonical;
  confirm rather than rewrite
- `task_update.rs:167,178` — deps/blocks parse failures are **silently
  dropped** (`if let Ok(...)`). Out of scope to fix, but note it: a bad id in
  `addDependencies` currently vanishes without error.

Whether these seven tools should exist at all is a separate question — they
are registered only in tests. Not decided here.

### 7. Frontend

`frontend/src/types/index.ts:166-182`:

```text
TaskEntry.id:            number    -> string
TaskEntry.dependencies:  number[]  -> string[]
TaskEntry.blocks:        number[]  -> string[]
```

- `frontend/src/lib/protocol.ts:163` — `task.get` params `task_id: number` → `string`
- `frontend/src/stores/tasks.ts:10` — `selectedTaskIdAtom` → `atom<string | null>(null)`
- `frontend/src/components/dialogs/TaskDepGraph.tsx:38,179` — `Map<number, TaskEntry>` → `Map<string, TaskEntry>`

**Remove the render-time `t` prefixes.** These are the reason the UI would
still show `t1` after a Rust-only change:

- `TasksPanel.tsx:210` — `t{task.id}`
- `TaskDepGraph.tsx:268` — `` `★ t${n.id}` `` / `` `t${n.id}` ``
- `TaskDepGraph.tsx:300` — `<span className="font-mono text-primary">t{selectedTask.id}</span>`

### 8. Deprecated Dioxus mirror — `vol-llm-ui`

`vol-llm-ui` is a workspace member and compiles in CI. Its task panel builds
request JSON by hand, so a string-typed wire breaks it **at runtime, not
compile time** — a silent failure.

- `web/client.rs:181-195` — `TaskEntry { id: u64, dependencies: Vec<u64> }`
- `web/client.rs:1797,1802` — `task_get(&self, task_id: u64)`, `"params": { "task_id": task_id }`
- `web/components/tasks_panel.rs:357` — `"t{task_id}"`
- `web/components/task_dep_graph.rs:286` — `format!("★ t{}", n.id)`

Roughly five lines. Update rather than let it rot silently. `vol-llm-tui` does
not reference tasks and is unaffected.

## Testing

Per CLAUDE.md: `#[cfg(test)]` unit tests or `tests/`, **no doc tests**, every
new `pub fn` gets at least one test, `just cover-gate vol-llm-task 80`.

**`TaskId` serde matrix** — the core of this change:

| Input | Expect |
|---|---|
| `serde_json::to_string(&TaskId(1))` | `"\"1\""` |
| `from_str::<TaskId>("1")` | `TaskId(1)` — legacy numeric |
| `from_str::<TaskId>("\"1\"")` | `TaskId(1)` — canonical |
| `from_str::<TaskId>("\"t1\"")` | `TaskId(1)` — historical |
| `from_str::<TaskId>("\"ttt1\"")` | error — greedy strip is gone |
| `from_str::<TaskId>("\"\"")` / `"\"abc\""` | error |
| round-trip `Vec<TaskId>` | `["1","2"]` |
| `from_str::<Vec<TaskId>>("[1,2]")` | legacy dependencies_json loads |
| `TaskId(u64::MAX)` | round-trips without precision loss |
| `TaskId(0)` | `"0"`; sentinel logic at `mapping.rs:121` still fires |

**Backward-compatibility tests** (guard the zero-migration claim):

- `DatabaseTaskStore`: insert a row whose `dependencies_json` is literally
  `[1,2]`, then `get()` — must succeed
- `FileTaskStore`: write a `{id}.json` containing `"id": 1, "dependencies": [2]`,
  then `get()` — must succeed

**Existing assertions that must be updated** (they encode the old prefix):
`model.rs:107`, `cli/format.rs:120,124,125,139,146`,
`tests/task_cli_integration.rs:43,76`, `tools/task_claim.rs:235`.

**End-to-end**: the loop that motivated this work — `task create` → read the
id out of the tool response → feed it back to `task get --id <that>` — must
succeed. Assert on both the text and the `data` payload.

**Frontend**: `just fe-test`. `TaskDepGraph` keys on ids; confirm the
`Map<string, …>` change did not break edge resolution.

**Gates**: `just clippy-strict`, `just no-doc-tests`, `just boundaries`,
`./scripts/check-protocol-registration.sh`.

## Out of scope

- Whether the seven test-only tools under `tools/` should be deleted
- `task_update.rs:167,178` silently dropping unparseable dependency ids
- Any change to `tasks.id` column type or the id allocation scheme
- Task scoping (session/user/project) — see the session binding spec
