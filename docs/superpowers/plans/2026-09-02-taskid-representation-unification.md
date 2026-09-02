# TaskId Representation Unification Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make a task id render exactly one way — the decimal string `"1"` — across tools, CLI, wire protocol, persistence, and frontend, so a model can feed back an id it was just shown.

**Architecture:** Replace `TaskId`'s derived serde and its `t`-prefixing `Display` with hand-written impls. Serialization is asymmetric: writes always emit a string, reads accept a bare integer (legacy data), a string, or a `t`-prefixed string (historical model output). That leniency is what makes this a zero-migration change — every previously persisted row keeps loading.

**Tech Stack:** Rust, serde, clap (CLI arg parsing), SeaORM (persistence), React/TypeScript (frontend), Dioxus (deprecated mirror).

**Spec:** `docs/superpowers/specs/2026-09-02-taskid-representation-unification-design.md`

## Global Constraints

- **Use `just`, never raw `cargo`** — recipes wrap nextest, feature flags, and fallbacks. Exception: `cargo nextest run -p <crate> --no-run` to watch compile progress during diagnosis.
- **Coverage ≥ 80%**: `just cover-gate vol-llm-task 80` before claiming done.
- **Every new `pub fn` gets at least one test.**
- **No doc tests.** Use `#[cfg(test)]` unit tests or `tests/`. Doc comment code examples must be ` ```text `, never ` ```rust `. Verify with `just no-doc-tests`.
- **Canonical serialized form of a `TaskId` is a JSON string of decimal digits**, no prefix: `"1"`. Human-readable text renders bare digits: `Task 1 created`.
- **`FromStr` strips at most ONE leading `t`.** `"ttt1"` must be rejected.
- **Compilation dominates test time.** `vol-agent-server` test binaries take ~9 minutes to compile cold, then run in ~2.5 seconds. A "hanging" test run is almost always still compiling.
- `just test-*` recipes redirect stderr to `/dev/null`, swallowing nextest progress. Use `cargo nextest run` directly only when you need to watch progress.

---

### Task 1: `TaskId` core — serde, Display, FromStr

The foundation. Everything else follows from this. Changing `Display` breaks assertions across the crate, so this task fixes all of them and leaves the build green.

**Files:**
- Modify: `crates/vol-llm-task/src/model.rs:1-16` (imports, derive list, Display)
- Modify: `crates/vol-llm-task/src/model.rs:100-110` (existing `test_task_id_display`)
- Modify: `crates/vol-llm-task/src/cli/format.rs:117-148` (assertions encoding the old prefix)
- Modify: `crates/vol-llm-task/src/tools/task_claim.rs:235` (assertion encoding the old prefix)
- Modify: `crates/vol-llm-task/tests/task_cli_integration.rs:43,76` (assertions encoding the old prefix)

**Interfaces:**
- Consumes: nothing — this is the first task.
- Produces:
  - `TaskId::from_str(&str) -> Result<TaskId, ParseTaskIdError>` (via `std::str::FromStr`)
  - `pub struct ParseTaskIdError` — implements `Display` + `std::error::Error`
  - `impl Display for TaskId` → bare digits, e.g. `1`
  - `impl Serialize for TaskId` → JSON string, e.g. `"1"`
  - `impl<'de> Deserialize<'de> for TaskId` → accepts `1`, `"1"`, `"t1"`

- [ ] **Step 1: Write the failing tests**

Add to the `mod tests` block in `crates/vol-llm-task/src/model.rs`:

```rust
    #[test]
    fn test_task_id_serializes_as_string() {
        assert_eq!(serde_json::to_string(&TaskId(1)).unwrap(), "\"1\"");
        assert_eq!(serde_json::to_string(&TaskId(0)).unwrap(), "\"0\"");
        assert_eq!(
            serde_json::to_string(&TaskId(u64::MAX)).unwrap(),
            format!("\"{}\"", u64::MAX)
        );
    }

    #[test]
    fn test_task_id_deserializes_from_legacy_number() {
        // Rows written before this change hold bare integers.
        assert_eq!(serde_json::from_str::<TaskId>("1").unwrap(), TaskId(1));
        assert_eq!(
            serde_json::from_str::<Vec<TaskId>>("[1,2]").unwrap(),
            vec![TaskId(1), TaskId(2)]
        );
    }

    #[test]
    fn test_task_id_deserializes_from_canonical_string() {
        assert_eq!(serde_json::from_str::<TaskId>("\"1\"").unwrap(), TaskId(1));
        assert_eq!(
            serde_json::from_str::<Vec<TaskId>>("[\"1\",\"2\"]").unwrap(),
            vec![TaskId(1), TaskId(2)]
        );
    }

    #[test]
    fn test_task_id_deserializes_from_prefixed_string() {
        // Historical: models were shown "t1" for a long time.
        assert_eq!(serde_json::from_str::<TaskId>("\"t1\"").unwrap(), TaskId(1));
    }

    #[test]
    fn test_task_id_rejects_malformed() {
        assert!(serde_json::from_str::<TaskId>("\"ttt1\"").is_err());
        assert!(serde_json::from_str::<TaskId>("\"\"").is_err());
        assert!(serde_json::from_str::<TaskId>("\"t\"").is_err());
        assert!(serde_json::from_str::<TaskId>("\"abc\"").is_err());
        assert!(serde_json::from_str::<TaskId>("\"-1\"").is_err());
        assert!(serde_json::from_str::<TaskId>("\" 1\"").is_err());
        assert!(serde_json::from_str::<TaskId>("-1").is_err());
    }

    #[test]
    fn test_task_id_round_trip() {
        for raw in [0u64, 1, 42, u64::MAX] {
            let json = serde_json::to_string(&TaskId(raw)).unwrap();
            assert_eq!(serde_json::from_str::<TaskId>(&json).unwrap(), TaskId(raw));
        }
    }

    #[test]
    fn test_task_id_from_str() {
        use std::str::FromStr;
        assert_eq!(TaskId::from_str("1").unwrap(), TaskId(1));
        assert_eq!(TaskId::from_str("t1").unwrap(), TaskId(1));
        assert_eq!(TaskId::from_str("t42").unwrap(), TaskId(42));
        assert!(TaskId::from_str("ttt1").is_err());
        assert!(TaskId::from_str("").is_err());
        assert!(TaskId::from_str("t").is_err());
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `just test-crate vol-llm-task`

Expected: FAIL. `test_task_id_serializes_as_string` fails with left `"1"` right `1` — the derive currently passes the newtype through as a bare number. `test_task_id_from_str` fails to compile or with "no function or associated item named `from_str`".

- [ ] **Step 3: Implement the four impls**

In `crates/vol-llm-task/src/model.rs`, remove `serde::Serialize, serde::Deserialize` from the derive at lines 7-10 and replace the `Display` impl at lines 12-16:

```rust
/// Unique task identifier (newtype over u64, auto-increment).
///
/// Canonical serialized form is a decimal string: `"1"`. Deserialization also
/// accepts a bare integer (data written before the representation was
/// unified) and a single `t` prefix (what models were shown historically).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TaskId(pub u64);

/// Error returned when a string cannot be parsed as a [`TaskId`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseTaskIdError(String);

impl std::fmt::Display for ParseTaskIdError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid task id: {:?}", self.0)
    }
}

impl std::error::Error for ParseTaskIdError {}

impl std::fmt::Display for TaskId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for TaskId {
    type Err = ParseTaskIdError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Strip at most one leading 't'. The previous implementation used
        // trim_start_matches('t'), which accepted "ttt1".
        let digits = s.strip_prefix('t').unwrap_or(s);
        if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
            return Err(ParseTaskIdError(s.to_string()));
        }
        digits
            .parse::<u64>()
            .map(TaskId)
            .map_err(|_| ParseTaskIdError(s.to_string()))
    }
}

impl serde::Serialize for TaskId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.0.to_string())
    }
}

impl<'de> serde::Deserialize<'de> for TaskId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct TaskIdVisitor;

        impl serde::de::Visitor<'_> for TaskIdVisitor {
            type Value = TaskId;

            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("a task id as a decimal string or an unsigned integer")
            }

            fn visit_u64<E>(self, v: u64) -> Result<TaskId, E>
            where
                E: serde::de::Error,
            {
                Ok(TaskId(v))
            }

            fn visit_i64<E>(self, v: i64) -> Result<TaskId, E>
            where
                E: serde::de::Error,
            {
                u64::try_from(v)
                    .map(TaskId)
                    .map_err(|_| E::custom(format!("negative task id: {v}")))
            }

            fn visit_str<E>(self, v: &str) -> Result<TaskId, E>
            where
                E: serde::de::Error,
            {
                use std::str::FromStr;
                TaskId::from_str(v).map_err(E::custom)
            }
        }

        // deserialize_any is required to accept both the number and string
        // forms. This works for JSON; TaskId is never sent through a
        // non-self-describing format in this workspace.
        deserializer.deserialize_any(TaskIdVisitor)
    }
}
```

- [ ] **Step 4: Update the assertions the Display change breaks**

`crates/vol-llm-task/src/model.rs:105-108`:

```rust
    #[test]
    fn test_task_id_display() {
        let id = TaskId(42);
        assert_eq!(format!("{}", id), "42");
    }
```

`crates/vol-llm-task/src/cli/format.rs:120,124,125`:

```rust
        assert!(output.contains("Task 42"));
        assert!(output.contains("1, 2"));
        assert!(output.contains("50"));
```

`crates/vol-llm-task/src/cli/format.rs:139`:

```rust
        assert!(output.contains("42"));
```

`crates/vol-llm-task/src/cli/format.rs:146`:

```rust
        assert!(output.contains("Task 42 created"));
```

`crates/vol-llm-task/tests/task_cli_integration.rs:43`:

```rust
    assert!(r.content.contains("Task 1"));
```

`crates/vol-llm-task/tests/task_cli_integration.rs:76`:

```rust
    assert!(r.content.contains("Task 1 updated"));
```

`crates/vol-llm-task/src/tools/task_claim.rs:235` — change the expected `"taskId": "t999"` to `"taskId": "999"`.

- [ ] **Step 5: Run tests to verify they pass**

Run: `just test-crate vol-llm-task`

Expected: PASS, all tests.

- [ ] **Step 6: Verify gates**

Run: `just clippy-strict && just no-doc-tests`

Expected: clean. If clippy flags the `ParseTaskIdError` tuple field as never read, add a getter or `#[allow]` — do not delete the field, the message needs it.

- [ ] **Step 7: Commit**

```bash
git add crates/vol-llm-task/src/model.rs \
        crates/vol-llm-task/src/cli/format.rs \
        crates/vol-llm-task/src/tools/task_claim.rs \
        crates/vol-llm-task/tests/task_cli_integration.rs
git commit -m "feat(task): canonical TaskId form is the decimal string \"1\"

Hand-write Serialize/Deserialize/Display/FromStr for TaskId. Writes emit
a string; reads accept a bare integer (pre-existing data), a string, or a
single t prefix (what models were shown historically). Display drops the
t prefix so text output and parseable input agree."
```

---

### Task 2: CLI and tool input accept both forms

`TaskCliTool` is the only task tool registered in production (`vol-llm-runtime/src/lib.rs:502`). Its `--id` flag currently uses `value_parser!(u64)` and rejects anything a model might echo back from older output.

**Files:**
- Modify: `crates/vol-llm-task/src/cli/parser.rs:8` (imports), and the 11 `value_parser!(u64)` sites at lines 82, 89, 100, 121, 128, 137, 161, 171, 184, 192, 227
- Modify: `crates/vol-llm-task/src/tools/task_claim.rs:48` (parameter description), `:67-71` (greedy prefix strip)
- Test: `crates/vol-llm-task/src/cli/parser.rs` (`mod tests`)

**Interfaces:**
- Consumes: `TaskId::from_str` and `ParseTaskIdError` from Task 1.
- Produces: `fn parse_task_id_arg(&str) -> Result<u64, String>` in `cli/parser.rs`, usable as a clap value parser. Returns `u64` so `ParsedCommand` keeps its existing field types.

- [ ] **Step 1: Write the failing tests**

Add to the `mod tests` block in `crates/vol-llm-task/src/cli/parser.rs`:

```rust
    #[test]
    fn test_parse_task_id_arg_accepts_plain_and_prefixed() {
        assert_eq!(parse_task_id_arg("1").unwrap(), 1);
        assert_eq!(parse_task_id_arg("t1").unwrap(), 1);
        assert_eq!(parse_task_id_arg("42").unwrap(), 42);
        assert_eq!(parse_task_id_arg("t42").unwrap(), 42);
    }

    #[test]
    fn test_parse_task_id_arg_rejects_malformed() {
        assert!(parse_task_id_arg("ttt1").is_err());
        assert!(parse_task_id_arg("abc").is_err());
        assert!(parse_task_id_arg("").is_err());
    }

    #[test]
    fn test_get_accepts_prefixed_id_end_to_end() {
        // The loop that motivated this work: the model echoes back an id it
        // was shown. Both spellings must reach the same task.
        let plain = parse("get --id 7").expect("plain id parses");
        let prefixed = parse("get --id t7").expect("prefixed id parses");
        assert_eq!(format!("{plain:?}"), format!("{prefixed:?}"));
    }
```

If the parser entry point is not named `parse`, use whatever `mod tests` in that file already calls — match the existing convention rather than inventing one.

- [ ] **Step 2: Run tests to verify they fail**

Run: `just test-crate vol-llm-task`

Expected: FAIL with "cannot find function `parse_task_id_arg` in this scope", and `test_get_accepts_prefixed_id_end_to_end` failing on `invalid value 't7' for '--id <id>'`.

- [ ] **Step 3: Add the value parser**

In `crates/vol-llm-task/src/cli/parser.rs`, add near the top:

```rust
/// clap value parser for task id arguments.
///
/// Accepts the canonical `1` and the historical `t1`. Returns `u64` so
/// `ParsedCommand` keeps carrying plain integers.
fn parse_task_id_arg(s: &str) -> Result<u64, String> {
    use std::str::FromStr;
    crate::model::TaskId::from_str(s)
        .map(|id| id.0)
        .map_err(|e| e.to_string())
}
```

- [ ] **Step 4: Swap the 11 clap sites**

At `parser.rs` lines 82, 89, 100, 121, 128, 137, 161, 171, 184, 192, 227, replace:

```rust
                        .value_parser(value_parser!(u64))
```

with:

```rust
                        .value_parser(parse_task_id_arg)
```

Leave the two `.value_parser([...])` status enumerations at lines 106 and 147 untouched — they are not ids.

If `value_parser!` becomes unused after this, drop it from the `use clap::{...}` at line 8 to keep `clippy-strict` quiet.

- [ ] **Step 5: Fix `task_claim`'s greedy strip and stale description**

`crates/vol-llm-task/src/tools/task_claim.rs:67-71`:

```rust
        let task_id = {
            use std::str::FromStr;
            crate::model::TaskId::from_str(&params.task_id)
                .map_err(|_| ToolError::InvalidArguments(format!("Invalid task ID: {}", params.task_id)))?
        };
```

`crates/vol-llm-task/src/tools/task_claim.rs:48` — replace the description advertising `'t1'`, `'t42'`:

```rust
                        "description": "ID of the task to claim (e.g. '1', '42')",
```

- [ ] **Step 6: Run tests to verify they pass**

Run: `just test-crate vol-llm-task`

Expected: PASS.

- [ ] **Step 7: Verify gates and commit**

```bash
just clippy-strict && just no-doc-tests
git add crates/vol-llm-task/src/cli/parser.rs crates/vol-llm-task/src/tools/task_claim.rs
git commit -m "feat(task): accept both '1' and 't1' as task id input

The CLI tool printed 't42' but parsed with value_parser!(u64), so a model
echoing back the id it was shown always errored. Route every id argument
through TaskId::from_str, which strips at most one t. Replaces
task_claim's trim_start_matches('t'), which accepted 'ttt1'."
```

---

### Task 3: Lock the zero-migration claim with back-compat tests

No implementation. These tests are the guard on the assertion that persisted data written before Task 1 keeps loading. If they fail, the change needs a migration and the design is wrong.

**Files:**
- Test: `crates/vol-llm-task/src/stores/file.rs` (`mod tests`)
- Test: `crates/vol-llm-task/src/stores/database/mod.rs` (`mod tests`) — match the file the existing database store tests live in

**Interfaces:**
- Consumes: the lenient `Deserialize` from Task 1.
- Produces: nothing consumed by later tasks.

- [ ] **Step 1: Write the file-store legacy test**

Add to `mod tests` in `crates/vol-llm-task/src/stores/file.rs`:

```rust
    #[tokio::test]
    async fn test_file_store_reads_legacy_numeric_ids() {
        // Bodies written before the representation change hold bare integers
        // for `id`, `dependencies`, and `blocks`. They must still load.
        let dir = tempfile::tempdir().expect("tempdir");
        let store = FileTaskStore::new(dir.path().to_path_buf())
            .await
            .expect("store");

        let tasks_dir = dir.path().join("tasks");
        std::fs::create_dir_all(&tasks_dir).expect("tasks dir");
        std::fs::write(
            tasks_dir.join("7.json"),
            r#"{
                "id": 7,
                "status": "Pending",
                "kind": "Agent",
                "publisher": null,
                "assignee": null,
                "subject": "legacy",
                "description": "",
                "active_form": null,
                "dependencies": [1, 2],
                "blocks": [],
                "result": null,
                "summary": null,
                "output_file": null,
                "created_at": { "secs_since_epoch": 0, "nanos_since_epoch": 0 },
                "started_at": null,
                "completed_at": null
            }"#,
        )
        .expect("write legacy task");

        let loaded = store
            .get(&TaskId(7))
            .await
            .expect("get succeeds")
            .expect("task present");
        assert_eq!(loaded.id, TaskId(7));
        assert_eq!(loaded.dependencies, vec![TaskId(1), TaskId(2)]);
    }
```

Match the `created_at` shape to however `SystemTime` actually serializes in this crate — write one task through the store first and read the file back to confirm the exact JSON before hard-coding it.

- [ ] **Step 2: Run it to verify it passes**

Run: `just test-crate vol-llm-task`

Expected: PASS. This test should pass immediately — it is a guard, not a driver. **If it fails, stop.** The lenient `Deserialize` from Task 1 is not working and the zero-migration premise is broken.

- [ ] **Step 3: Write the database legacy test**

Add alongside the existing database store tests:

```rust
    #[tokio::test]
    async fn test_database_store_reads_legacy_dependencies_json() {
        // dependencies_json / blocks_json rows written before the change hold
        // "[1,2]", not "[\"1\",\"2\"]".
        let store = new_sqlite_test_store().await;
        let id = store
            .create(sample_task("legacy deps"))
            .await
            .expect("create");

        // Overwrite the column with the pre-change encoding.
        set_dependencies_json_raw(&store, id, "[1,2]").await;

        let loaded = store.get(&id).await.expect("get").expect("present");
        assert_eq!(loaded.dependencies, vec![TaskId(1), TaskId(2)]);
    }
```

`new_sqlite_test_store`, `sample_task`, and `set_dependencies_json_raw` are helpers — reuse the existing test setup helpers in that module rather than adding new ones. `set_dependencies_json_raw` is a raw SeaORM `update_many().col_expr(...)` against `Column::DependenciesJson`; write it inline if no equivalent exists.

Postgres tests read `VOL_AGENT_POSTGRES_TEST_URL` and fail loudly when unset. Use the SQLite path for this test.

- [ ] **Step 4: Run it to verify it passes**

Run: `just test-crate vol-llm-task`

Expected: PASS.

- [ ] **Step 5: Check coverage and commit**

```bash
just cover-gate vol-llm-task 80
git add crates/vol-llm-task/src/stores/
git commit -m "test(task): guard legacy numeric id deserialization

TaskId now serializes as a string, but every previously written row and
file holds bare integers. These tests are the standing proof that the
change needs no data migration."
```

---

### Task 4: Wire protocol and server handler emit canonical ids

`TaskPayload::Get` carries `u64` and the handler hand-builds JSON with `t.id.0`. Both move to `TaskId` so the wire matches everything else.

**Files:**
- Modify: `crates/vol-llm-agent-protocol/Cargo.toml` (add `vol-llm-task`)
- Modify: `crates/vol-llm-agent-protocol/src/agent_server_protocol.rs:1344` (`TaskPayload::Get`), `:854-862` (decode)
- Modify: `crates/vol-agent-server/src/data_plane/handlers/task.rs:62,70,71,85,88,96,97`
- Test: `crates/vol-llm-agent-protocol/src/agent_server_protocol.rs` (`mod tests`), `crates/vol-agent-server/src/data_plane/handlers/task.rs` (`mod tests`)

**Interfaces:**
- Consumes: `TaskId` with its serde impls from Task 1.
- Produces: `TaskPayload::Get { task_id: TaskId }` — the handler and any future caller destructure a `TaskId`, not a `u64`.

- [ ] **Step 1: Write the failing protocol test**

Add to `mod tests` in `crates/vol-llm-agent-protocol/src/agent_server_protocol.rs`:

```rust
    #[test]
    fn test_task_get_decodes_string_and_legacy_number() {
        let from_string = Payload::from_operation(
            Operation::Task(TaskOperation::Get),
            serde_json::json!({ "task_id": "7" }),
        )
        .expect("string id decodes");
        assert!(matches!(
            from_string,
            Payload::Task(TaskPayload::Get { task_id }) if task_id == vol_llm_task::TaskId(7)
        ));

        let from_number = Payload::from_operation(
            Operation::Task(TaskOperation::Get),
            serde_json::json!({ "task_id": 7 }),
        )
        .expect("legacy numeric id decodes");
        assert!(matches!(
            from_number,
            Payload::Task(TaskPayload::Get { task_id }) if task_id == vol_llm_task::TaskId(7)
        ));
    }

    #[test]
    fn test_task_get_rejects_malformed_id() {
        assert!(Payload::from_operation(
            Operation::Task(TaskOperation::Get),
            serde_json::json!({ "task_id": "abc" }),
        )
        .is_err());
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `just test-crate vol-llm-agent-protocol`

Expected: FAIL — `vol_llm_task` is not a dependency yet, so this will not compile.

- [ ] **Step 3: Add the dependency and change the wire type**

`crates/vol-llm-agent-protocol/Cargo.toml`, under `[dependencies]`:

```toml
vol-llm-task = { workspace = true }
```

If `vol-llm-task` has no `workspace = true` entry in the root `Cargo.toml` `[workspace.dependencies]`, use `{ path = "../vol-llm-task" }` to match the style already used in that file for `vol-llm-agent`.

`agent_server_protocol.rs:1344`:

```rust
    Get { task_id: vol_llm_task::TaskId },
```

`agent_server_protocol.rs:854-862`:

```rust
            Operation::Task(TaskOperation::Get) => {
                #[derive(Deserialize)]
                struct P {
                    task_id: vol_llm_task::TaskId,
                }
                let p: P = serde_json::from_value(value)
                    .map_err(|_| ProtocolError::PayloadDecodeFailed("task.get"))?;
                Ok(Payload::Task(TaskPayload::Get { task_id: p.task_id }))
            }
```

- [ ] **Step 4: Run to verify it passes, and check the boundary gates**

```bash
just test-crate vol-llm-agent-protocol
just boundaries
./scripts/check-protocol-registration.sh
```

Expected: tests PASS. `just boundaries` clean — `vol-llm-task` depends only on `vol-llm-core` and `vol-llm-tool`, so this adds no cycle to the normal dependency graph. `check-protocol-registration.sh` clean — no new `*Operation` variant was added, so no codec arm is needed.

- [ ] **Step 5: Write the failing handler test**

Add to `mod tests` in `crates/vol-agent-server/src/data_plane/handlers/task.rs`:

```rust
    #[tokio::test]
    async fn test_task_list_emits_string_ids() {
        let handler = handler_with_one_task().await;
        let result = handler.list(None, None).await.expect("list");
        let tasks = result["tasks"].as_array().expect("array");
        assert_eq!(tasks[0]["id"], serde_json::json!("1"));
        assert_eq!(tasks[0]["dependencies"], serde_json::json!([]));
    }

    #[tokio::test]
    async fn test_task_get_emits_string_ids_including_dependencies() {
        let handler = handler_with_dependent_task().await;
        let result = handler.get(TaskId(2)).await.expect("get");
        assert_eq!(result["task"]["id"], serde_json::json!("2"));
        assert_eq!(result["task"]["dependencies"], serde_json::json!(["1"]));
    }
```

`handler_with_one_task` / `handler_with_dependent_task` build a `TaskHandler` over an `InMemoryTaskStore`. Reuse whatever setup the existing tests in that module use; match their method signatures for `list` / `get` rather than the shapes sketched here.

- [ ] **Step 6: Run to verify it fails**

Run: `just test-integration` (integration tests need `--features vol-agent-server/test-utils`, which this recipe passes; `just test-crate` does not).

Expected: FAIL — ids come back as the numbers `1` and `2`.

This crate's test binaries take ~9 minutes to compile cold. Run `cargo nextest run -p vol-agent-server --no-run` first if you want to watch progress.

- [ ] **Step 7: Change the handler**

`crates/vol-agent-server/src/data_plane/handlers/task.rs` — at lines 62 and 88, replace `"id": t.id.0` with `"id": t.id` (serde now emits the string). At lines 70, 71, 96, 97, drop the `.map(|d| d.0)` so the `Vec<TaskId>` serializes directly:

```rust
                    "dependencies": t.dependencies,
                    "blocks": t.blocks,
```

At line 85, the payload already carries a `TaskId`:

```rust
        let task = self.store.get(&task_id).await.unwrap_or(None);
```

- [ ] **Step 8: Run to verify it passes**

Run: `just test-integration`

Expected: PASS.

- [ ] **Step 9: Verify gates and commit**

```bash
just clippy-strict && just boundaries && just no-doc-tests
git add crates/vol-llm-agent-protocol/ crates/vol-agent-server/src/data_plane/handlers/task.rs
git commit -m "feat(protocol): task ids are strings on the wire

TaskPayload::Get carries TaskId instead of u64, so malformed ids are
rejected at decode. The data-plane handler stops unwrapping .0 and lets
TaskId serialize itself, making handler output agree with tool output."
```

---

### Task 5: Frontend types and removal of render-time prefixes

The React app types ids as `number` and prepends `t` in three places at render time. Without this task the UI still shows `t1` after everything else says `1`.

**Files:**
- Modify: `frontend/src/types/index.ts:166-182` (`TaskEntry`)
- Modify: `frontend/src/lib/protocol.ts:163` (`task.get` params)
- Modify: `frontend/src/stores/tasks.ts:10` (`selectedTaskIdAtom`)
- Modify: `frontend/src/components/panels/TasksPanel.tsx:210`
- Modify: `frontend/src/components/dialogs/TaskDepGraph.tsx:38,179,268,300`
- Test: `frontend/tests/unit/` — add a task id rendering test

**Interfaces:**
- Consumes: the string-typed `id` / `dependencies` / `blocks` the handler now emits (Task 4).
- Produces: `TaskEntry.id: string`, `TaskEntry.dependencies: string[]`, `TaskEntry.blocks: string[]`.

- [ ] **Step 1: Write the failing test**

Create `frontend/tests/unit/task-id-rendering.test.ts`:

```ts
import { describe, expect, it } from 'vitest'
import type { TaskEntry } from '@/types'

describe('task id representation', () => {
  it('types ids as strings', () => {
    const task: TaskEntry = {
      id: '1',
      dependencies: ['2', '3'],
      blocks: [],
    } as TaskEntry
    expect(task.id).toBe('1')
    expect(task.dependencies).toEqual(['2', '3'])
  })

  it('does not prefix ids for display', () => {
    const task = { id: '42' } as TaskEntry
    expect(`${task.id}`).toBe('42')
    expect(`${task.id}`).not.toMatch(/^t/)
  })
})
```

- [ ] **Step 2: Run to verify it fails**

Run: `just fe-test-unit`

Expected: FAIL — a type error, because `TaskEntry.id` is `number` and `'1'` is not assignable.

- [ ] **Step 3: Change the types**

`frontend/src/types/index.ts:166-182`:

```ts
export interface TaskEntry {
  id: string
  // ...unchanged fields...
  dependencies: string[]
  blocks: string[]
```

`frontend/src/lib/protocol.ts:163`:

```ts
  'task.get': { params: { task_id: string }; result: { task: TaskEntry | null } }
```

`frontend/src/stores/tasks.ts:10`:

```ts
export const selectedTaskIdAtom = atom<string | null>(null)
```

`frontend/src/components/dialogs/TaskDepGraph.tsx:38,179` — both `Map<number, TaskEntry>` become `Map<string, TaskEntry>`.

- [ ] **Step 4: Remove the render-time prefixes**

`frontend/src/components/panels/TasksPanel.tsx:210` — `t{task.id}` becomes `{task.id}`.

`frontend/src/components/dialogs/TaskDepGraph.tsx:268`:

```tsx
    const label = isCenter ? `★ ${n.id}` : `${n.id}`
```

`frontend/src/components/dialogs/TaskDepGraph.tsx:300`:

```tsx
    <span className="font-mono text-primary">{selectedTask.id}</span>
```

- [ ] **Step 5: Run to verify it passes**

```bash
just fe-test-unit
just fe-test-integration
```

Expected: PASS. `TaskDepGraph` keys its edge resolution on ids — if integration tests fail there, the `Map<string, …>` change missed a lookup site still passing a number.

- [ ] **Step 6: Commit**

```bash
git add frontend/src/types/index.ts frontend/src/lib/protocol.ts \
        frontend/src/stores/tasks.ts \
        frontend/src/components/panels/TasksPanel.tsx \
        frontend/src/components/dialogs/TaskDepGraph.tsx \
        frontend/tests/unit/task-id-rendering.test.ts
git commit -m "feat(frontend): task ids are strings, drop display-time t prefix

The prefix was added at render time in three components. Leaving them
would keep the UI showing t1 while every other layer says 1."
```

---

### Task 6: Update the deprecated Dioxus mirror

`vol-llm-ui` is a workspace member and compiles in CI, but its task panel builds request JSON by hand. A string-typed wire breaks it **at runtime, not compile time** — a silent failure. `vol-llm-tui` does not reference tasks and needs nothing.

**Files:**
- Modify: `crates/vol-llm-ui/src/web/client.rs:181-195` (`TaskEntry`), `:1797,1802` (`task_get`)
- Modify: `crates/vol-llm-ui/src/web/components/tasks_panel.rs:357`
- Modify: `crates/vol-llm-ui/src/web/components/task_dep_graph.rs:286`

**Interfaces:**
- Consumes: the string-typed handler output from Task 4.
- Produces: nothing — this is a leaf.

- [ ] **Step 1: Change the client types**

`crates/vol-llm-ui/src/web/client.rs:181-195`:

```rust
pub struct TaskEntry {
    pub id: String,
    // ...unchanged fields...
    pub dependencies: Vec<String>,
    pub blocks: Vec<String>,
}
```

`crates/vol-llm-ui/src/web/client.rs:1797,1802`:

```rust
    pub async fn task_get(&self, task_id: String, /* ...existing params... */) -> /* unchanged */ {
        // ...
        "params": { "task_id": task_id },
```

Update the call sites the compiler flags.

- [ ] **Step 2: Remove the render-time prefixes**

`crates/vol-llm-ui/src/web/components/tasks_panel.rs:357` — `"t{task_id}"` becomes `"{task_id}"`.

`crates/vol-llm-ui/src/web/components/task_dep_graph.rs:286`:

```rust
    format!("★ {}", n.id)
```

- [ ] **Step 3: Verify it compiles**

Run: `just check`

Expected: clean across the workspace. Do not use `cargo build`/`cargo run` for this crate — CLAUDE.md routes web frontend work through `just` recipes.

- [ ] **Step 4: Run the full test suite**

Run: `just test`

Expected: PASS.

- [ ] **Step 5: Verify all gates and commit**

```bash
just clippy-strict && just no-doc-tests && just boundaries && just fmt-check
just cover-gate vol-llm-task 80
git add crates/vol-llm-ui/src/web/
git commit -m "fix(ui): update deprecated Dioxus task panel for string ids

This crate hand-builds request JSON, so a string-typed wire would have
broken it at runtime without a compile error."
```

---

## Self-Review

**Spec coverage:**

| Spec section | Task |
|---|---|
| §1 `TaskId` core (serde/Display/FromStr) | 1 |
| §2 Persistence — no migration | 3 (tests guarding the claim; no impl needed) |
| §3 Wire protocol | 4 |
| §4 Server handler | 4 |
| §5 Tool + CLI output | 1 (Display cascade), 2 (parser) |
| §6 Test-only tools under `tools/` | 1 (`task_claim.rs:235`), 2 (`:48`, `:67-71`) |
| §7 Frontend | 5 |
| §8 Deprecated Dioxus mirror | 6 |

Spec §6 notes `task_update.rs:167,178` silently drops unparseable dependency ids. The spec places it out of scope; no task implements it. Intentional.

**Type consistency:** `TaskId::from_str` returns `Result<TaskId, ParseTaskIdError>` (Task 1) and is consumed as such by `parse_task_id_arg` (Task 2, maps to `Result<u64, String>`), the protocol decode (Task 4, via `Deserialize`), and `task_claim` (Task 2). `TaskPayload::Get { task_id: TaskId }` (Task 4) is destructured as a `TaskId` in the handler at the same task. Frontend `TaskEntry.id: string` (Task 5) matches the handler's string output (Task 4). Dioxus `TaskEntry.id: String` (Task 6) matches the same.

**Ordering:** Task 4 must follow Task 1 (needs the serde impls). Task 5 and Task 6 must follow Task 4 (they consume its output shape). Task 2 and Task 3 depend only on Task 1 and can run in either order.
