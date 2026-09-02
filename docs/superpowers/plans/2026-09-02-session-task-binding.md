# Session ↔ Task Binding Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Record which tasks a session is working on, so a ReAct run carrying task ids leaves a durable association on the session.

**Architecture:** Add generic session-level metadata to `SessionEntryStore` — a shallow-merged JSON map with `task_ids` as its first key. The database backend uses the already-migrated but never-read `sessions.metadata` column; the file backend gets a sidecar JSON file. `AgentInput` grows a `task_ids` field, which every downstream layer already passes by value, so the write happens in `ReActAgent::run_input` with no protocol plumbing.

**Tech Stack:** Rust, serde_json, SeaORM (SQLite/Postgres), tokio, React/TypeScript.

**Spec:** `docs/superpowers/specs/2026-09-02-session-task-binding-design.md`

**Depends on:** `docs/superpowers/plans/2026-09-02-taskid-representation-unification.md` must ship first. Task 5 here uses `TaskId`'s string serialization and `Display`.

## Global Constraints

- **Use `just`, never raw `cargo`** — recipes wrap nextest, feature flags, and fallbacks. Exception: `cargo nextest run -p <crate> --no-run` to watch compile progress during diagnosis.
- **Coverage ≥ 80%**: `just cover-gate vol-session 80` before claiming done.
- **Every new `pub fn` gets at least one test.**
- **No doc tests.** Use `#[cfg(test)]` unit tests or `tests/`. Doc comment code examples must be ` ```text `, never ` ```rust `. Verify with `just no-doc-tests`.
- **`vol-session` must not gain a `vol-llm-task` dependency.** Its dependency list is `vol-llm-core` and `vol-llm-context`. Task ids are stored as plain `String`. Adding `vol-llm-task` would close the cycle `vol-llm-task →(dev) vol-llm-agent → vol-session → vol-llm-task`.
- **Scope is data maintenance only.** The binding is recorded and readable. It does not change agent behaviour — no prompt injection, no tool scoping.
- **Binding semantics are union-only.** A run's task ids are added to the set; they never replace it. There is no unbind.
- **A failed binding write logs `warn!` and does not abort the run.**
- **No validation that a bound task exists.**
- **Compilation dominates test time.** `vol-agent-server` test binaries take ~9 minutes to compile cold, then run in ~2.5 seconds.

---

### Task 1: `SessionEntryStore` metadata methods + in-memory backend

Two new trait methods and the simplest backend. The file and database backends get explicit "not implemented" stubs so the workspace stays green; Tasks 2 and 3 replace them. There are exactly three implementors, all inside `vol-session` — verified with `grep -rn "SessionEntryStore for" crates/`.

**Files:**
- Modify: `crates/vol-session/src/store.rs:58-77` (trait)
- Modify: `crates/vol-session/src/memory_store.rs:197` (real impl)
- Modify: `crates/vol-session/src/file_store.rs:268` (stub)
- Modify: `crates/vol-session/src/database_store/mod.rs:252` (stub)
- Test: `crates/vol-session/src/memory_store.rs` (`mod tests`)

**Interfaces:**
- Consumes: nothing — first task.
- Produces, on `SessionEntryStore`:
  - `async fn get_session_metadata(&self, session_id: &str) -> Result<serde_json::Map<String, serde_json::Value>>` — empty map for an unknown session, never an error
  - `async fn merge_session_metadata(&self, session_id: &str, patch: serde_json::Map<String, serde_json::Value>) -> Result<()>` — shallow merge, upserts the session record

- [ ] **Step 1: Write the failing tests**

Add to `mod tests` in `crates/vol-session/src/memory_store.rs`:

```rust
    #[tokio::test]
    async fn test_metadata_round_trip() {
        let store = InMemoryEntryStore::new();
        let mut patch = serde_json::Map::new();
        patch.insert("task_ids".into(), serde_json::json!(["1", "2"]));

        store
            .merge_session_metadata("s1", patch)
            .await
            .expect("merge succeeds");

        let meta = store.get_session_metadata("s1").await.expect("get");
        assert_eq!(meta["task_ids"], serde_json::json!(["1", "2"]));
    }

    #[tokio::test]
    async fn test_metadata_unknown_session_is_empty_not_error() {
        let store = InMemoryEntryStore::new();
        let meta = store.get_session_metadata("nope").await.expect("get");
        assert!(meta.is_empty());
    }

    #[tokio::test]
    async fn test_metadata_merge_is_shallow_and_preserves_other_keys() {
        let store = InMemoryEntryStore::new();

        let mut first = serde_json::Map::new();
        first.insert("a".into(), serde_json::json!(1));
        store.merge_session_metadata("s1", first).await.expect("first");

        let mut second = serde_json::Map::new();
        second.insert("b".into(), serde_json::json!(2));
        store.merge_session_metadata("s1", second).await.expect("second");

        let meta = store.get_session_metadata("s1").await.expect("get");
        assert_eq!(meta["a"], serde_json::json!(1));
        assert_eq!(meta["b"], serde_json::json!(2));
    }

    #[tokio::test]
    async fn test_metadata_merge_upserts_without_any_entries() {
        // Binding can happen before the first message is written.
        let store = InMemoryEntryStore::new();
        let mut patch = serde_json::Map::new();
        patch.insert("k".into(), serde_json::json!("v"));

        store.merge_session_metadata("brand-new", patch).await.expect("merge");

        assert_eq!(
            store.get_session_metadata("brand-new").await.expect("get")["k"],
            serde_json::json!("v")
        );
    }

    #[tokio::test]
    async fn test_metadata_same_key_overwrites() {
        let store = InMemoryEntryStore::new();

        let mut first = serde_json::Map::new();
        first.insert("task_ids".into(), serde_json::json!(["1"]));
        store.merge_session_metadata("s1", first).await.expect("first");

        let mut second = serde_json::Map::new();
        second.insert("task_ids".into(), serde_json::json!(["1", "2"]));
        store.merge_session_metadata("s1", second).await.expect("second");

        let meta = store.get_session_metadata("s1").await.expect("get");
        assert_eq!(meta["task_ids"], serde_json::json!(["1", "2"]));
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `just test-crate vol-session`

Expected: FAIL to compile — "no method named `merge_session_metadata`".

- [ ] **Step 3: Add the trait methods**

In `crates/vol-session/src/store.rs`, inside `pub trait SessionEntryStore`, after `get_count`:

```rust
    /// Read session-level metadata.
    ///
    /// Returns an empty map for a session that does not exist — absence of
    /// metadata is not an error.
    async fn get_session_metadata(
        &self,
        session_id: &str,
    ) -> Result<serde_json::Map<String, serde_json::Value>>;

    /// Shallow-merge a patch into session-level metadata.
    ///
    /// Keys in `patch` replace existing keys wholesale; keys absent from
    /// `patch` are left alone. Creates the session record if it does not yet
    /// exist — a binding can be written before the first entry.
    async fn merge_session_metadata(
        &self,
        session_id: &str,
        patch: serde_json::Map<String, serde_json::Value>,
    ) -> Result<()>;
```

No default implementations. A backend that silently no-ops is worse than one that fails to compile.

- [ ] **Step 4: Implement the in-memory backend**

In `crates/vol-session/src/memory_store.rs`, add a field to `InMemoryEntryStore`:

```rust
    session_metadata: dashmap::DashMap<String, serde_json::Map<String, serde_json::Value>>,
```

If the struct uses `Arc<RwLock<HashMap<..>>>` rather than `DashMap` for its existing storage, follow that pattern instead — match the file, do not introduce a second concurrency primitive. Initialize it in the constructor alongside the existing fields.

In the `impl crate::store::SessionEntryStore for InMemoryEntryStore` block at line 197:

```rust
    async fn get_session_metadata(
        &self,
        session_id: &str,
    ) -> Result<serde_json::Map<String, serde_json::Value>> {
        Ok(self
            .session_metadata
            .get(session_id)
            .map(|m| m.clone())
            .unwrap_or_default())
    }

    async fn merge_session_metadata(
        &self,
        session_id: &str,
        patch: serde_json::Map<String, serde_json::Value>,
    ) -> Result<()> {
        let mut entry = self.session_metadata.entry(session_id.to_string()).or_default();
        for (k, v) in patch {
            entry.insert(k, v);
        }
        Ok(())
    }
```

- [ ] **Step 5: Add explicit stubs to the other two backends**

`crates/vol-session/src/file_store.rs`, in the `impl SessionEntryStore for FileSessionEntryStore` block:

```rust
    async fn get_session_metadata(
        &self,
        _session_id: &str,
    ) -> Result<serde_json::Map<String, serde_json::Value>> {
        Err(StoreError::Internal(
            "session metadata not yet implemented for the file backend".to_string(),
        ))
    }

    async fn merge_session_metadata(
        &self,
        _session_id: &str,
        _patch: serde_json::Map<String, serde_json::Value>,
    ) -> Result<()> {
        Err(StoreError::Internal(
            "session metadata not yet implemented for the file backend".to_string(),
        ))
    }
```

Add the identical pair to `crates/vol-session/src/database_store/mod.rs` with `"database backend"` in the message.

- [ ] **Step 6: Pin the stubs with tests so they are deliberate, not forgotten**

Add one test per stubbed backend, in that backend's `mod tests`:

```rust
    #[tokio::test]
    async fn test_metadata_not_yet_implemented() {
        // Replaced with real behaviour in a later task. This test exists so
        // an unimplemented backend fails loudly rather than silently.
        let store = /* construct the backend as the other tests in this file do */;
        assert!(store.get_session_metadata("s1").await.is_err());
    }
```

- [ ] **Step 7: Run to verify everything passes**

Run: `just test-crate vol-session`

Expected: PASS.

- [ ] **Step 8: Verify gates and commit**

```bash
just clippy-strict && just no-doc-tests && just boundaries
git add crates/vol-session/src/store.rs crates/vol-session/src/memory_store.rs \
        crates/vol-session/src/file_store.rs crates/vol-session/src/database_store/mod.rs
git commit -m "feat(session): add session-level metadata to SessionEntryStore

Shallow-merge map with upsert, implemented for the in-memory backend.
File and database backends stub explicitly rather than defaulting to a
silent no-op; both are pinned by a test asserting the error."
```

---

### Task 2: Database backend — use the already-migrated `metadata` column

`sessions.metadata TEXT NOT NULL` was created by `m0001_create_sessions.rs:29` and is written as the literal `"{}"` at `database_store/mod.rs:206` and never read. No migration is needed.

**Files:**
- Modify: `crates/vol-session/src/database_store/mod.rs:252` (replace the stub)
- Test: `crates/vol-session/src/database_store/mod.rs` (`mod tests`)

**Interfaces:**
- Consumes: the trait methods from Task 1.
- Produces: nothing new — fills in the contract.

- [ ] **Step 1: Write the failing tests**

Replace the `test_metadata_not_yet_implemented` stub test with:

```rust
    #[tokio::test]
    async fn test_database_metadata_round_trip() {
        let store = new_sqlite_test_store("agent-a").await;
        let mut patch = serde_json::Map::new();
        patch.insert("task_ids".into(), serde_json::json!(["1", "2"]));

        store.merge_session_metadata("s1", patch).await.expect("merge");

        let meta = store.get_session_metadata("s1").await.expect("get");
        assert_eq!(meta["task_ids"], serde_json::json!(["1", "2"]));
    }

    #[tokio::test]
    async fn test_database_metadata_upserts_before_first_entry() {
        let store = new_sqlite_test_store("agent-a").await;
        let mut patch = serde_json::Map::new();
        patch.insert("k".into(), serde_json::json!("v"));

        // No entry has ever been saved for this session.
        store.merge_session_metadata("fresh", patch).await.expect("merge");

        assert_eq!(
            store.get_session_metadata("fresh").await.expect("get")["k"],
            serde_json::json!("v")
        );
    }

    #[tokio::test]
    async fn test_database_metadata_survives_later_entry_save() {
        // ensure_session_for_entry writes metadata: "{}" with
        // OnConflict::do_nothing. It must not clobber an earlier write.
        let store = new_sqlite_test_store("agent-a").await;
        let mut patch = serde_json::Map::new();
        patch.insert("task_ids".into(), serde_json::json!(["1"]));
        store.merge_session_metadata("s1", patch).await.expect("merge");

        store.save(sample_entry("s1")).await.expect("save entry");

        let meta = store.get_session_metadata("s1").await.expect("get");
        assert_eq!(meta["task_ids"], serde_json::json!(["1"]));
    }

    #[tokio::test]
    async fn test_database_metadata_respects_agent_scope() {
        // Metadata must not become a way around load_owned_session.
        let db = shared_sqlite_connection().await;
        let owner = store_for(&db, "agent-a");
        let intruder = store_for(&db, "agent-b");

        owner.save(sample_entry("s1")).await.expect("owner creates session");

        let mut patch = serde_json::Map::new();
        patch.insert("k".into(), serde_json::json!("v"));

        assert!(matches!(
            intruder.merge_session_metadata("s1", patch).await,
            Err(StoreError::SessionAgentScopeConflict { .. })
        ));
        assert!(matches!(
            intruder.get_session_metadata("s1").await,
            Err(StoreError::SessionAgentScopeConflict { .. })
        ));
    }

    #[tokio::test]
    async fn test_database_metadata_unknown_session_is_empty() {
        let store = new_sqlite_test_store("agent-a").await;
        assert!(store
            .get_session_metadata("never-existed")
            .await
            .expect("get")
            .is_empty());
    }
```

`new_sqlite_test_store`, `shared_sqlite_connection`, `store_for`, and `sample_entry` are helpers — reuse the setup helpers the existing tests in this module already use. Use the SQLite path; Postgres tests read `VOL_AGENT_POSTGRES_TEST_URL` and fail loudly when unset.

- [ ] **Step 2: Run to verify it fails**

Run: `just test-crate vol-session`

Expected: FAIL — the stub returns `StoreError::Internal`, not the values or the scope conflict.

- [ ] **Step 3: Implement against the existing column**

Replace the two stubs in `crates/vol-session/src/database_store/mod.rs`:

```rust
    async fn get_session_metadata(
        &self,
        session_id: &str,
    ) -> Result<serde_json::Map<String, serde_json::Value>> {
        match self.load_owned_session(&*self.db, session_id).await {
            Ok(session) => Ok(serde_json::from_str(&session.metadata).unwrap_or_default()),
            // An unknown session has no metadata; that is not an error. A
            // scope conflict is, and must propagate.
            Err(StoreError::NotFound(_)) => Ok(serde_json::Map::new()),
            Err(e) => Err(e),
        }
    }

    async fn merge_session_metadata(
        &self,
        session_id: &str,
        patch: serde_json::Map<String, serde_json::Value>,
    ) -> Result<()> {
        use sea_orm::{
            sea_query::OnConflict, ActiveValue, ColumnTrait, EntityTrait, QueryFilter,
            TransactionTrait,
        };

        let txn = self.db.begin().await.map_err(|e| {
            StoreError::Database(format!("failed to begin session metadata transaction: {e}"))
        })?;

        let now = current_timestamp();

        // Upsert: the session row may not exist yet.
        entity::sessions::Entity::insert(entity::sessions::ActiveModel {
            id: ActiveValue::Set(session_id.to_string()),
            agent_id: ActiveValue::Set(self.agent_id.clone()),
            created_at: ActiveValue::Set(now),
            updated_at: ActiveValue::Set(now),
            entry_count: ActiveValue::Set(0),
            metadata: ActiveValue::Set("{}".to_string()),
        })
        .on_conflict(
            OnConflict::column(entity::sessions::Column::Id)
                .do_nothing()
                .to_owned(),
        )
        .exec_without_returning(&txn)
        .await
        .map_err(|e| StoreError::Database(format!("failed to ensure session row: {e}")))?;

        // Ownership guard — inside the transaction, after the row exists.
        let session = self.load_owned_session(&txn, session_id).await?;

        let mut merged: serde_json::Map<String, serde_json::Value> =
            serde_json::from_str(&session.metadata).unwrap_or_default();
        for (k, v) in patch {
            merged.insert(k, v);
        }
        let encoded = serde_json::to_string(&merged)
            .map_err(|e| StoreError::Serialization(e.to_string()))?;

        entity::sessions::Entity::update_many()
            .col_expr(
                entity::sessions::Column::Metadata,
                sea_orm::sea_query::Expr::value(encoded),
            )
            .filter(entity::sessions::Column::Id.eq(session_id.to_string()))
            .filter(entity::sessions::Column::AgentId.eq(self.agent_id.clone()))
            .exec(&txn)
            .await
            .map_err(|e| StoreError::Database(format!("failed to write session metadata: {e}")))?;

        txn.commit().await.map_err(|e| {
            StoreError::Database(format!("failed to commit session metadata transaction: {e}"))
        })?;
        Ok(())
    }
```

`current_timestamp()` — reuse whatever this module already uses to produce epoch seconds; `Session::with_id` at `session.rs:26-33` shows the pattern if there is no helper.

A malformed `metadata` column degrades to an empty map (`unwrap_or_default`) rather than failing the read. Losing unreadable metadata beats making a session unopenable.

- [ ] **Step 4: Run to verify it passes**

Run: `just test-crate vol-session`

Expected: PASS.

- [ ] **Step 5: Verify gates and commit**

```bash
just clippy-strict && just no-doc-tests
git add crates/vol-session/src/database_store/mod.rs
git commit -m "feat(session): implement metadata on the database backend

Uses the sessions.metadata column that m0001 already created and nothing
ever read, so no migration is needed. Both reads and writes route through
load_owned_session so metadata cannot bypass the agent scope guard."
```

---

### Task 3: File backend — sidecar manifest

`FileSessionEntryStore` is append-only JSONL with no session-level record. Metadata goes in `{entry_dir}/{agent_type}/{session_id}.meta.json`.

**Files:**
- Modify: `crates/vol-session/src/file_store.rs:15-29` (struct — add the write mutex), `:55-66` (path helper), `:268` (replace the stubs, extend `delete_session`)
- Test: `crates/vol-session/src/file_store.rs` (`mod tests`)

**Interfaces:**
- Consumes: the trait methods from Task 1.
- Produces: `fn meta_path(&self, session_id: &str) -> PathBuf` (private) — built by delegating to the same `agent_type` branch as `file_path`.

- [ ] **Step 1: Write the failing tests**

Replace the `test_metadata_not_yet_implemented` stub test with:

```rust
    #[tokio::test]
    async fn test_file_metadata_round_trip() {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = FileSessionEntryStore::new(dir.path().to_path_buf(), Some("agent-a".into()));

        let mut patch = serde_json::Map::new();
        patch.insert("task_ids".into(), serde_json::json!(["1", "2"]));
        store.merge_session_metadata("s1", patch).await.expect("merge");

        let meta = store.get_session_metadata("s1").await.expect("get");
        assert_eq!(meta["task_ids"], serde_json::json!(["1", "2"]));
    }

    #[tokio::test]
    async fn test_file_metadata_upserts_with_no_jsonl_present() {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = FileSessionEntryStore::new(dir.path().to_path_buf(), Some("agent-a".into()));

        let mut patch = serde_json::Map::new();
        patch.insert("k".into(), serde_json::json!("v"));
        store.merge_session_metadata("fresh", patch).await.expect("merge");

        assert_eq!(
            store.get_session_metadata("fresh").await.expect("get")["k"],
            serde_json::json!("v")
        );
    }

    #[tokio::test]
    async fn test_file_metadata_unknown_session_is_empty() {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = FileSessionEntryStore::new(dir.path().to_path_buf(), Some("agent-a".into()));
        assert!(store.get_session_metadata("nope").await.expect("get").is_empty());
    }

    #[tokio::test]
    async fn test_file_metadata_malformed_sidecar_degrades_to_empty() {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = FileSessionEntryStore::new(dir.path().to_path_buf(), Some("agent-a".into()));
        let agent_dir = dir.path().join("agent-a");
        std::fs::create_dir_all(&agent_dir).expect("mkdir");
        std::fs::write(agent_dir.join("s1.meta.json"), "{ truncated").expect("write");

        assert!(store.get_session_metadata("s1").await.expect("get").is_empty());
    }

    #[tokio::test]
    async fn test_list_sessions_ignores_sidecar_files() {
        // Regression guard: list_sessions filters on the "jsonl" extension
        // (file_store.rs:225-227). A sidecar must never appear as a session
        // named "s1.meta".
        let dir = tempfile::tempdir().expect("tempdir");
        let store = FileSessionEntryStore::new(dir.path().to_path_buf(), Some("agent-a".into()));

        store.save(sample_entry("s1")).await.expect("save");
        let mut patch = serde_json::Map::new();
        patch.insert("k".into(), serde_json::json!("v"));
        store.merge_session_metadata("s1", patch).await.expect("merge");

        let sessions = store.list_sessions().expect("list");
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].session_id, "s1");
    }

    #[tokio::test]
    async fn test_delete_session_removes_sidecar() {
        // Otherwise metadata resurrects onto a later session reusing the id.
        let dir = tempfile::tempdir().expect("tempdir");
        let store = FileSessionEntryStore::new(dir.path().to_path_buf(), Some("agent-a".into()));

        store.save(sample_entry("s1")).await.expect("save");
        let mut patch = serde_json::Map::new();
        patch.insert("k".into(), serde_json::json!("v"));
        store.merge_session_metadata("s1", patch).await.expect("merge");

        store.delete_session("s1").await.expect("delete");

        assert!(store.get_session_metadata("s1").await.expect("get").is_empty());
        assert!(!dir.path().join("agent-a").join("s1.meta.json").exists());
    }
```

Match `FileSessionEntryStore::new`'s real signature and reuse the `sample_entry` helper the existing tests in this file use.

- [ ] **Step 2: Run to verify it fails**

Run: `just test-crate vol-session`

Expected: FAIL — the stub returns `StoreError::Internal`.

- [ ] **Step 3: Add the path helper and the write mutex**

In `crates/vol-session/src/file_store.rs`, add to the struct at lines 15-29:

```rust
    /// Serializes read-modify-write cycles on the metadata sidecar.
    meta_write_lock: tokio::sync::Mutex<()>,
```

Initialize it in every constructor. If the struct derives `Clone`, `tokio::sync::Mutex<()>` is not `Clone` — wrap it as `Arc<tokio::sync::Mutex<()>>` and clone the `Arc`.

Add next to `file_path` at lines 55-66:

```rust
    /// Resolve the metadata sidecar path for a session.
    ///
    /// Delegates to the same `agent_type` branch as [`Self::file_path`] so the
    /// path-traversal hardening applied to `agent_id` covers sidecars too.
    /// Do not rebuild this path independently.
    fn meta_path(&self, session_id: &str) -> PathBuf {
        self.file_path(session_id)
            .with_extension("")
            .with_extension("meta.json")
    }
```

Verify by test that `meta_path("s1")` is `.../agent-a/s1.meta.json`. `Path::with_extension` on `s1.jsonl` yields `s1`, then `.meta.json` appends correctly — but a `session_id` containing a dot would behave differently, so assert it:

```rust
    #[test]
    fn test_meta_path_shape() {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = FileSessionEntryStore::new(dir.path().to_path_buf(), Some("agent-a".into()));
        assert_eq!(
            store.meta_path("s1"),
            dir.path().join("agent-a").join("s1.meta.json")
        );
        assert_eq!(
            store.meta_path("a.b"),
            dir.path().join("agent-a").join("a.b.meta.json")
        );
    }
```

If the dotted case does not hold, build the filename by string concatenation instead: `format!("{session_id}.meta.json")` inside the same `match &self.agent_type` shape `file_path` uses.

- [ ] **Step 4: Implement the two methods**

Replace the stubs in the `impl SessionEntryStore for FileSessionEntryStore` block:

```rust
    async fn get_session_metadata(
        &self,
        session_id: &str,
    ) -> Result<serde_json::Map<String, serde_json::Value>> {
        let path = self.meta_path(session_id);
        match std::fs::read_to_string(&path) {
            // A malformed sidecar degrades to empty rather than making the
            // session unreadable.
            Ok(raw) => Ok(serde_json::from_str(&raw).unwrap_or_default()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(serde_json::Map::new()),
            Err(e) => Err(StoreError::Io(e)),
        }
    }

    async fn merge_session_metadata(
        &self,
        session_id: &str,
        patch: serde_json::Map<String, serde_json::Value>,
    ) -> Result<()> {
        let _guard = self.meta_write_lock.lock().await;

        let mut merged = self.get_session_metadata(session_id).await?;
        for (k, v) in patch {
            merged.insert(k, v);
        }

        self.ensure_dir()?;
        let path = self.meta_path(session_id);
        let tmp = path.with_extension("json.tmp");
        let encoded = serde_json::to_string(&merged)
            .map_err(|e| StoreError::Serialization(e.to_string()))?;

        std::fs::write(&tmp, encoded)?;
        std::fs::rename(&tmp, &path)?;
        Ok(())
    }
```

Writing to a temp file and renaming makes the replacement atomic, so a crash mid-write cannot leave a half-written sidecar. The mutex covers in-process concurrency only; sessions are single-writer in practice.

- [ ] **Step 5: Extend `delete_session`**

In the same impl block, after the existing `.jsonl` removal, add:

```rust
        match std::fs::remove_file(self.meta_path(session_id)) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(StoreError::Io(e)),
        }
```

- [ ] **Step 6: Run to verify it passes**

Run: `just test-crate vol-session`

Expected: PASS, including `test_list_sessions_ignores_sidecar_files`.

- [ ] **Step 7: Verify gates and commit**

```bash
just clippy-strict && just no-doc-tests
just cover-gate vol-session 80
git add crates/vol-session/src/file_store.rs
git commit -m "feat(session): implement metadata on the file backend

Sidecar {session_id}.meta.json written via temp file plus atomic rename.
meta_path delegates to file_path so the agent_id path-traversal hardening
covers sidecars. delete_session removes it; list_sessions already filters
on the jsonl extension, now pinned by a regression test."
```

---

### Task 4: `Session` binding API

**Files:**
- Modify: `crates/vol-session/src/session.rs:143-146` (delete the no-op), add the four methods
- Modify: `crates/vol-session/src/session.rs:186-194` (delete the no-op's test)
- Modify: `crates/vol-session/src/lib.rs` (re-export `TASK_IDS_KEY`, following the existing `RUN_ID_KEY` export)
- Test: `crates/vol-session/src/session.rs` (`mod tests`)

**Interfaces:**
- Consumes: `get_session_metadata` / `merge_session_metadata` from Tasks 1-3.
- Produces:
  - `pub const TASK_IDS_KEY: &str = "task_ids"`
  - `Session::metadata() -> Result<serde_json::Map<String, serde_json::Value>>`
  - `Session::merge_metadata(patch) -> Result<()>`
  - `Session::task_ids() -> Result<Vec<String>>`
  - `Session::bind_task_ids(&[String]) -> Result<()>`

- [ ] **Step 1: Write the failing tests**

Add to `mod tests` in `crates/vol-session/src/session.rs`:

```rust
    #[tokio::test]
    async fn test_bind_task_ids_then_read_back() {
        let session = Session::new(Arc::new(InMemoryEntryStore::new()));
        session
            .bind_task_ids(&["1".to_string(), "2".to_string()])
            .await
            .expect("bind");
        assert_eq!(session.task_ids().await.expect("read"), vec!["1", "2"]);
    }

    #[tokio::test]
    async fn test_bind_task_ids_unions_across_calls() {
        let session = Session::new(Arc::new(InMemoryEntryStore::new()));
        session.bind_task_ids(&["1".into(), "2".into()]).await.expect("first");
        session.bind_task_ids(&["2".into(), "3".into()]).await.expect("second");

        // Union, not replacement; no duplicates.
        assert_eq!(session.task_ids().await.expect("read"), vec!["1", "2", "3"]);
    }

    #[tokio::test]
    async fn test_bind_task_ids_is_idempotent() {
        let session = Session::new(Arc::new(InMemoryEntryStore::new()));
        session.bind_task_ids(&["7".into()]).await.expect("first");
        session.bind_task_ids(&["7".into()]).await.expect("second");
        assert_eq!(session.task_ids().await.expect("read"), vec!["7"]);
    }

    #[tokio::test]
    async fn test_bind_task_ids_sorts_numerically_not_lexicographically() {
        let session = Session::new(Arc::new(InMemoryEntryStore::new()));
        session
            .bind_task_ids(&["10".into(), "2".into(), "1".into()])
            .await
            .expect("bind");
        assert_eq!(session.task_ids().await.expect("read"), vec!["1", "2", "10"]);
    }

    #[tokio::test]
    async fn test_task_ids_empty_when_never_bound() {
        let session = Session::new(Arc::new(InMemoryEntryStore::new()));
        assert!(session.task_ids().await.expect("read").is_empty());
    }

    #[tokio::test]
    async fn test_bind_nonexistent_task_id_succeeds() {
        // No validation: the session layer has no TaskStore and should not
        // acquire one for a metadata write.
        let session = Session::new(Arc::new(InMemoryEntryStore::new()));
        session.bind_task_ids(&["999999".into()]).await.expect("bind");
        assert_eq!(session.task_ids().await.expect("read"), vec!["999999"]);
    }

    #[tokio::test]
    async fn test_bind_empty_slice_is_a_noop() {
        let session = Session::new(Arc::new(InMemoryEntryStore::new()));
        session.bind_task_ids(&[]).await.expect("bind");
        assert!(session.task_ids().await.expect("read").is_empty());
    }

    #[tokio::test]
    async fn test_merge_metadata_leaves_task_ids_alone() {
        let session = Session::new(Arc::new(InMemoryEntryStore::new()));
        session.bind_task_ids(&["1".into()]).await.expect("bind");

        let mut patch = serde_json::Map::new();
        patch.insert("project_id".into(), serde_json::json!("p1"));
        session.merge_metadata(patch).await.expect("merge");

        assert_eq!(session.task_ids().await.expect("read"), vec!["1"]);
        assert_eq!(
            session.metadata().await.expect("meta")["project_id"],
            serde_json::json!("p1")
        );
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `just test-crate vol-session`

Expected: FAIL — "no method named `bind_task_ids`".

- [ ] **Step 3: Delete the no-op stub and its test**

Remove `Session::with_metadata` at `crates/vol-session/src/session.rs:143-146` and `test_session_with_metadata_noop` at `:186-194`. It is a `self`-consuming synchronous builder that discards both arguments and cannot perform async I/O, so it cannot be made honest. Its only caller is its own test — confirm with `grep -rn "with_metadata" crates/` that nothing outside `SessionMessage::with_metadata` (a different, working method in `message.rs:46`) refers to it.

- [ ] **Step 4: Implement the four methods**

In `crates/vol-session/src/session.rs`, above `impl Session`:

```rust
/// Session metadata key holding the bound task ids, as an array of canonical
/// id strings. Companion to `RUN_ID_KEY` in `entry.rs`, which is per-message.
pub const TASK_IDS_KEY: &str = "task_ids";
```

Inside `impl Session`:

```rust
    /// Read all session-level metadata.
    pub async fn metadata(&self) -> Result<serde_json::Map<String, serde_json::Value>> {
        self.entry_store.get_session_metadata(&self.id).await
    }

    /// Shallow-merge a patch into session-level metadata.
    pub async fn merge_metadata(
        &self,
        patch: serde_json::Map<String, serde_json::Value>,
    ) -> Result<()> {
        self.entry_store.merge_session_metadata(&self.id, patch).await
    }

    /// Task ids bound to this session, ascending.
    pub async fn task_ids(&self) -> Result<Vec<String>> {
        Ok(self
            .metadata()
            .await?
            .get(TASK_IDS_KEY)
            .and_then(serde_json::Value::as_array)
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default())
    }

    /// Add task ids to this session's binding.
    ///
    /// Union semantics: the set only grows. There is no unbind. Ids are not
    /// validated against any task store.
    pub async fn bind_task_ids(&self, ids: &[String]) -> Result<()> {
        if ids.is_empty() {
            return Ok(());
        }

        let mut merged = self.task_ids().await?;
        merged.extend(ids.iter().cloned());
        // Numeric ordering, so "10" sorts after "2". Non-numeric ids sort
        // last, then lexicographically, keeping the result deterministic.
        merged.sort_by(|a, b| {
            let ka = a.parse::<u64>().unwrap_or(u64::MAX);
            let kb = b.parse::<u64>().unwrap_or(u64::MAX);
            ka.cmp(&kb).then_with(|| a.cmp(b))
        });
        merged.dedup();

        let mut patch = serde_json::Map::new();
        patch.insert(TASK_IDS_KEY.to_string(), serde_json::json!(merged));
        self.merge_metadata(patch).await
    }
```

- [ ] **Step 5: Re-export the key**

In `crates/vol-session/src/lib.rs`, add `TASK_IDS_KEY` to the `session` re-export, matching how `RUN_ID_KEY` is exported from `entry`.

- [ ] **Step 6: Run to verify it passes**

Run: `just test-crate vol-session`

Expected: PASS.

- [ ] **Step 7: Verify gates and commit**

```bash
just clippy-strict && just no-doc-tests && just boundaries
just cover-gate vol-session 80
git add crates/vol-session/src/session.rs crates/vol-session/src/lib.rs
git commit -m "feat(session): add task binding API on Session

bind_task_ids unions into metadata[task_ids] with numeric ordering. Ids
are plain Strings so vol-session takes no vol-llm-task dependency, which
would close a cycle through vol-llm-agent. Deletes the with_metadata
no-op stub, which discarded both arguments and had no caller but its
own test."
```

---

### Task 5: `AgentInput` carries task ids

Everything downstream — `AgentPayload::Submit`, `AgentRequest`, the dispatcher, the control-plane re-wrap — passes `AgentInput` by value, so this single struct change reaches the agent loop with no protocol plumbing.

**Files:**
- Modify: `crates/vol-llm-agent/Cargo.toml` (add `vol-llm-task`)
- Modify: `crates/vol-llm-agent/src/react/input.rs:31-38` (struct), `:44-51` (wire variant), `:54-72` (Deserialize), `:71+` (`new`)
- Test: `crates/vol-llm-agent/src/react/input.rs` (`mod tests`)

**Interfaces:**
- Consumes: `TaskId` from the id-unification plan (serializes as `"1"`).
- Produces: `AgentInput.task_ids: Vec<vol_llm_task::TaskId>` — read by Task 6.

- [ ] **Step 1: Write the failing tests**

Add to `mod tests` in `crates/vol-llm-agent/src/react/input.rs`:

```rust
    #[test]
    fn test_bare_string_still_deserializes() {
        // Back-compat: the untagged Text arm must keep working.
        let input: AgentInput = serde_json::from_str("\"hello\"").expect("bare string");
        assert!(input.task_ids.is_empty());
        assert_eq!(input.parts.len(), 1);
    }

    #[test]
    fn test_structured_without_task_ids_defaults_empty() {
        let input: AgentInput = serde_json::from_value(serde_json::json!({
            "parts": [{ "type": "text", "text": "hi" }],
            "metadata": { "session_id": "s1" }
        }))
        .expect("structured");
        assert!(input.task_ids.is_empty());
    }

    #[test]
    fn test_structured_with_task_ids() {
        let input: AgentInput = serde_json::from_value(serde_json::json!({
            "parts": [{ "type": "text", "text": "hi" }],
            "task_ids": ["1", "2"]
        }))
        .expect("structured with task_ids");
        assert_eq!(
            input.task_ids,
            vec![vol_llm_task::TaskId(1), vol_llm_task::TaskId(2)]
        );
    }

    #[test]
    fn test_task_ids_omitted_from_serialization_when_empty() {
        let input = AgentInput::text("hi");
        let json = serde_json::to_value(&input).expect("serialize");
        assert!(json.get("task_ids").is_none());
    }

    #[test]
    fn test_task_ids_serialize_as_strings() {
        let mut input = AgentInput::text("hi");
        input.task_ids = vec![vol_llm_task::TaskId(1)];
        let json = serde_json::to_value(&input).expect("serialize");
        assert_eq!(json["task_ids"], serde_json::json!(["1"]));
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `just test-crate vol-llm-agent`

Expected: FAIL to compile — no `task_ids` field, and `vol_llm_task` is not a dependency.

- [ ] **Step 3: Add the dependency**

`crates/vol-llm-agent/Cargo.toml`, under `[dependencies]`, matching the `path = "../..."` style already used in that file:

```toml
vol-llm-task = { path = "../vol-llm-task" }
```

This closes a dev-dependency cycle — `vol-llm-task` dev-depends on `vol-llm-agent`. Cargo permits this: it builds the `vol-llm-task` lib, then `vol-llm-agent`, then `vol-llm-task`'s tests. If `just check` reports a genuine cycle error rather than building, fall back to `task_ids: Vec<String>` throughout this task; the wire format is identical and only the decode-time validation is lost.

- [ ] **Step 4: Add the field in all three places**

`crates/vol-llm-agent/src/react/input.rs:31-38`:

```rust
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AgentInput {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run_id: Option<String>,
    pub parts: Vec<InputPart>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, serde_json::Value>,
    /// Task ids to bind to this run's session. Union semantics; never unbinds.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub task_ids: Vec<vol_llm_task::TaskId>,
}
```

`:44-51`, the wire variant:

```rust
    Structured {
        #[serde(default)]
        run_id: Option<String>,
        #[serde(default)]
        parts: Vec<InputPart>,
        #[serde(default)]
        metadata: HashMap<String, serde_json::Value>,
        #[serde(default)]
        task_ids: Vec<vol_llm_task::TaskId>,
    },
```

`:54-72`, the `Deserialize` impl:

```rust
            AgentInputWire::Structured {
                run_id,
                parts,
                metadata,
                task_ids,
            } => Ok(Self {
                run_id,
                parts,
                metadata,
                task_ids,
            }),
```

And `AgentInput::new` at `:71+`, plus any other struct literal the compiler flags:

```rust
            task_ids: Vec::new(),
```

- [ ] **Step 5: Run to verify it passes**

```bash
just test-crate vol-llm-agent
just check
```

Expected: PASS, and the workspace compiles.

- [ ] **Step 6: Verify gates and commit**

```bash
just clippy-strict && just boundaries && just no-doc-tests
git add crates/vol-llm-agent/Cargo.toml crates/vol-llm-agent/src/react/input.rs
git commit -m "feat(agent): AgentInput carries task_ids

Every downstream layer passes AgentInput by value, so this one struct
change reaches the agent loop without touching AgentPayload, AgentRequest,
the dispatcher, or the control-plane forward."
```

---

### Task 6: Bind on run

**Files:**
- Modify: `crates/vol-llm-agent/src/react/run_context.rs` (add the `task_ids` field and a builder)
- Modify: `crates/vol-llm-agent/src/react/agent.rs:447+` (`run_input`)
- Test: `crates/vol-llm-agent/src/react/agent.rs` (`mod tests`)

**Interfaces:**
- Consumes: `AgentInput.task_ids` (Task 5), `Session::bind_task_ids` (Task 4).
- Produces: `RunContext.task_ids: Vec<vol_llm_task::TaskId>` — the attachment point for the follow-up features, read by nothing yet.

- [ ] **Step 1: Write the failing tests**

Add to `mod tests` in `crates/vol-llm-agent/src/react/agent.rs`:

```rust
    #[tokio::test]
    async fn test_run_binds_task_ids_to_session() {
        let (agent, session) = agent_with_stub_llm().await;

        let mut input = AgentInput::text("do the thing");
        input.task_ids = vec![vol_llm_task::TaskId(1), vol_llm_task::TaskId(2)];
        agent.run_input(input).await.expect("run");

        assert_eq!(session.task_ids().await.expect("read"), vec!["1", "2"]);
    }

    #[tokio::test]
    async fn test_second_run_unions_task_ids() {
        let (agent, session) = agent_with_stub_llm().await;

        let mut first = AgentInput::text("one");
        first.task_ids = vec![vol_llm_task::TaskId(1)];
        agent.run_input(first).await.expect("first run");

        let mut second = AgentInput::text("two");
        second.task_ids = vec![vol_llm_task::TaskId(2)];
        agent.run_input(second).await.expect("second run");

        assert_eq!(session.task_ids().await.expect("read"), vec!["1", "2"]);
    }

    #[tokio::test]
    async fn test_run_without_task_ids_writes_nothing() {
        let (agent, session) = agent_with_stub_llm().await;
        agent.run_input(AgentInput::text("no ids")).await.expect("run");
        assert!(session.task_ids().await.expect("read").is_empty());
    }
```

`agent_with_stub_llm` builds a `ReActAgent` over a stub provider and returns the agent plus a handle to its `Session`. Reuse the existing test harness in that module; `vol-llm-core` exposes test utilities behind the `test-utils` feature, already a dev-dependency of this crate.

- [ ] **Step 2: Run to verify it fails**

Run: `just test-crate vol-llm-agent`

Expected: FAIL — `session.task_ids()` returns empty; nothing writes the binding yet.

- [ ] **Step 3: Add the `RunContext` field**

In `crates/vol-llm-agent/src/react/run_context.rs`, add to the struct alongside `run_id` / `session_id` / `model`:

```rust
    /// Task ids bound to this run. Populated from `AgentInput.task_ids`.
    /// Nothing reads this yet — it is the attachment point for context
    /// injection and tool scoping.
    pub task_ids: Vec<vol_llm_task::TaskId>,
```

Initialize it as `Vec::new()` inside `RunContext::new` (line ~117) so no existing caller changes, and add a builder next to it:

```rust
    /// Attach task ids to this context.
    #[must_use]
    pub fn with_task_ids(mut self, task_ids: Vec<vol_llm_task::TaskId>) -> Self {
        self.task_ids = task_ids;
        self
    }
```

`RunContext::new` returns `(Self, mpsc::Receiver<PluginRequest>)`, so apply the builder to the tuple's first element.

- [ ] **Step 4: Bind in `run_input`**

In `crates/vol-llm-agent/src/react/agent.rs`, after the `RunContext` is constructed and before the agent loop starts:

```rust
        if !input.task_ids.is_empty() {
            let ids: Vec<String> = input.task_ids.iter().map(ToString::to_string).collect();
            if let Err(e) = run_ctx.session.bind_task_ids(&ids).await {
                // Binding is metadata. Losing it must not kill the run.
                tracing::warn!(
                    run_id = %run_ctx.run_id,
                    session_id = %run_ctx.session_id,
                    error = %e,
                    "failed to bind task ids to session"
                );
            }
        }
```

`TaskId → String` happens here, the one place both `vol-llm-task` and `Session` are in scope. After the id-unification plan, `ToString` yields bare canonical digits (`1`), matching what `TaskId`'s `Serialize` emits and what `FromStr` accepts back.

`run_ctx.session` is the `Arc<Session>` that `RunContext::new` cloned out of `config.session` (`run_context.rs:126-131`), so this writes to the same session the run records into.

Apply `.with_task_ids(input.task_ids.clone())` where `run_ctx` is built, before it is cloned into any spawned task.

- [ ] **Step 5: Run to verify it passes**

Run: `just test-crate vol-llm-agent`

Expected: PASS.

- [ ] **Step 6: Verify gates and commit**

```bash
just clippy-strict && just no-doc-tests
git add crates/vol-llm-agent/src/react/agent.rs crates/vol-llm-agent/src/react/run_context.rs
git commit -m "feat(agent): bind task ids to the session on run

Written in run_input rather than the server handler so the dispatcher
path and any direct caller behave the same. A failed write warns and
continues -- binding is metadata and must not kill a run."
```

---

### Task 7: Expose metadata on the read surface

**Files:**
- Modify: `crates/vol-session/src/manager.rs:12-22` (`SessionInfo`)
- Modify: `crates/vol-session/src/database_store/mapping.rs:59-72` (`session_model_to_info`)
- Modify: `crates/vol-session/src/database_store/mod.rs` and `crates/vol-session/src/manager.rs` (`FileSessionManager::list_sessions`) — populate the new field
- Modify: `crates/vol-agent-server/src/data_plane/handlers/session.rs:89-97`
- Test: `crates/vol-session/src/database_store/mapping.rs`, `crates/vol-agent-server/src/data_plane/handlers/session.rs`

**Interfaces:**
- Consumes: `get_session_metadata` (Tasks 1-3).
- Produces: `SessionInfo.metadata: serde_json::Map<String, serde_json::Value>`.

- [ ] **Step 1: Write the failing tests**

In `crates/vol-session/src/database_store/mapping.rs` `mod tests`:

```rust
    #[test]
    fn test_session_model_to_info_carries_metadata() {
        let model = entity::sessions::Model {
            id: "s1".into(),
            agent_id: "agent-a".into(),
            created_at: 0,
            updated_at: 1,
            entry_count: 3,
            metadata: r#"{"task_ids":["1","2"]}"#.into(),
        };
        let info = session_model_to_info(model);
        assert_eq!(info.metadata["task_ids"], serde_json::json!(["1", "2"]));
    }

    #[test]
    fn test_session_model_to_info_tolerates_malformed_metadata() {
        let model = entity::sessions::Model {
            id: "s1".into(),
            agent_id: "agent-a".into(),
            created_at: 0,
            updated_at: 1,
            entry_count: 0,
            metadata: "{ truncated".into(),
        };
        assert!(session_model_to_info(model).metadata.is_empty());
    }
```

In `crates/vol-agent-server/src/data_plane/handlers/session.rs` `mod tests`:

```rust
    #[tokio::test]
    async fn test_session_list_includes_metadata() {
        let handler = handler_with_bound_session().await;
        let result = handler.list(None).await.expect("list");
        let sessions = result["sessions"].as_array().expect("array");
        assert_eq!(
            sessions[0]["metadata"]["task_ids"],
            serde_json::json!(["1"])
        );
    }
```

`handler_with_bound_session` builds a `SessionHandler` over a manager whose session has `task_ids` bound. Reuse the existing setup helpers in that module and match its real `list` signature.

- [ ] **Step 2: Run to verify it fails**

```bash
just test-crate vol-session
just test-integration
```

Expected: FAIL — `SessionInfo` has no `metadata` field.

- [ ] **Step 3: Add the field**

`crates/vol-session/src/manager.rs:12-22`:

```rust
pub struct SessionInfo {
    pub id: String,
    pub agent_id: String,
    pub session_id: String,
    pub entry_count: usize,
    pub created_at: i64,
    pub updated_at: Option<i64>,
    /// Session-level metadata, including `task_ids`. Empty when the backend
    /// has none recorded.
    pub metadata: serde_json::Map<String, serde_json::Value>,
}
```

`SessionInfo` derives `PartialEq, Eq` — `serde_json::Map` implements both, so the derive still holds. If `Eq` fails to derive, drop `Eq` and keep `PartialEq`; `serde_json::Value` is not `Eq` because of floats.

- [ ] **Step 4: Populate it in both managers**

`crates/vol-session/src/database_store/mapping.rs:59-72` — stop dropping the column:

```rust
        metadata: serde_json::from_str(&model.metadata).unwrap_or_default(),
```

For `FileSessionManager::list_sessions`, read each session's sidecar via `get_session_metadata`. If that makes listing too chatty for large directories, populate an empty map there and note the asymmetry in the doc comment — the field is best-effort and callers must not assume it is populated by `list_sessions` on the file backend.

Fix every other `SessionInfo` construction the compiler flags with `metadata: serde_json::Map::new()`.

- [ ] **Step 5: Include it in the handler JSON**

`crates/vol-agent-server/src/data_plane/handlers/session.rs:89-97` — add to the hand-built object:

```rust
                    "metadata": info.metadata,
```

- [ ] **Step 6: Run to verify it passes**

```bash
just test-crate vol-session
just test-integration
```

Expected: PASS.

- [ ] **Step 7: Verify gates and commit**

```bash
just clippy-strict && just no-doc-tests && just boundaries
just cover-gate vol-session 80
git add crates/vol-session/src/manager.rs crates/vol-session/src/database_store/ \
        crates/vol-agent-server/src/data_plane/handlers/session.rs
git commit -m "feat(session): expose session metadata on the read surface

SessionInfo carries the metadata map and session_model_to_info stops
dropping the column it has always loaded."
```

---

### Task 8: Correct the frontend protocol type

The React client's declared type for `agent.submit` is already stale — it says `input: string` while the code sends `{parts, metadata}`. Correct it and add `task_ids`.

**No task-selection UI is built.** The spec scopes this work to data maintenance; choosing which tasks a session is bound to has no designed interaction yet, so `task_ids` is plumbed as optional and nothing populates it. Task bindings are set by programmatic callers until a UI is designed.

**Files:**
- Modify: `frontend/src/lib/protocol.ts:83-85`
- Test: `frontend/tests/unit/`

**Interfaces:**
- Consumes: the `AgentInput` wire shape from Task 5.
- Produces: nothing — leaf.

- [ ] **Step 1: Write the failing test**

Create `frontend/tests/unit/agent-submit-params.test.ts`:

```ts
import { describe, expect, it } from 'vitest'
import type { ProtocolMethods } from '@/lib/protocol'

describe('agent.submit params', () => {
  it('accepts the structured input shape actually sent', () => {
    const params: ProtocolMethods['agent.submit']['params'] = {
      input: {
        parts: [{ type: 'text', text: 'hi' }],
        metadata: { session_id: 's1' },
        task_ids: ['1', '2'],
      },
      target: 'agent-a',
    }
    expect(params.input.task_ids).toEqual(['1', '2'])
  })

  it('allows task_ids to be omitted', () => {
    const params: ProtocolMethods['agent.submit']['params'] = {
      input: { parts: [{ type: 'text', text: 'hi' }] },
    }
    expect(params.input.task_ids).toBeUndefined()
  })
})
```

Use whatever the protocol method map is actually named in `protocol.ts` rather than `ProtocolMethods` if it differs.

- [ ] **Step 2: Run to verify it fails**

Run: `just fe-test-unit`

Expected: FAIL — a type error, because `input` is declared as `string`.

- [ ] **Step 3: Correct the type**

`frontend/src/lib/protocol.ts:83-85`:

```ts
  'agent.submit': {
    params: {
      input: {
        run_id?: string
        parts: Array<{ type: 'text'; text: string } | { type: 'image_url'; url: string; detail?: string }>
        metadata?: Record<string, unknown>
        task_ids?: string[]
      }
      target?: string
    }
    result: { run_id: string }
  }
```

Match the `InputPart` union to whatever `buildInputParts` in `frontend/src/components/inputs/InputArea.tsx` actually produces; if a shared type already exists for it, reference that instead of inlining the union.

- [ ] **Step 4: Run to verify it passes**

```bash
just fe-test-unit
just fe-test-integration
```

Expected: PASS. Fixing the stale type may surface pre-existing type errors at the `InputArea.tsx:96-104` call site — correct them to match what is genuinely being sent.

- [ ] **Step 5: Run the full suite and commit**

```bash
just test
just fe-test
git add frontend/src/lib/protocol.ts frontend/tests/unit/agent-submit-params.test.ts
git commit -m "fix(frontend): correct stale agent.submit param type, add task_ids

The declared type said input was a string while the client has been
sending {parts, metadata}. No task-selection UI is added; task_ids is
optional and unpopulated."
```

---

## Self-Review

**Spec coverage:**

| Spec section | Task |
|---|---|
| §1 The storage hook that already exists | 2 (uses the column), 4 (deletes the `with_metadata` no-op) |
| §2 `SessionEntryStore` gains two methods | 1 |
| §3 Database backend | 2 |
| §3 File backend — path reuse, listing filter, delete | 3 |
| §3 In-memory backend | 1 |
| §4 `Session` API + `TASK_IDS_KEY` + union semantics + no validation | 4 |
| §5 `AgentInput.task_ids`, frontend client | 5, 8 |
| §6 Write point, warn-not-abort, `RunContext.task_ids` | 6 |
| §7 Read surface — `SessionInfo`, mapping, handler | 7 |
| §7 Reverse lookup not supported | no task — explicitly out of scope |

**Deviation from the spec, recorded:** spec §3 flags `list_sessions` treating sidecars as phantom sessions as "the single most likely bug in this change." Verified during planning that `file_store.rs:225-227` already filters on the `jsonl` extension, so the bug does not exist. Task 3 keeps a regression test to prevent it from being introduced.

**Type consistency:** `get_session_metadata` / `merge_session_metadata` use `serde_json::Map<String, serde_json::Value>` in the trait (Task 1), all three backends (Tasks 1-3), `Session::metadata` / `merge_metadata` (Task 4), and `SessionInfo.metadata` (Task 7). `Session::bind_task_ids(&[String])` / `task_ids() -> Vec<String>` (Task 4) are called with `Vec<String>` converted from `Vec<TaskId>` at the single conversion point in `run_input` (Task 6). `AgentInput.task_ids: Vec<vol_llm_task::TaskId>` (Task 5) matches `RunContext.task_ids: Vec<vol_llm_task::TaskId>` (Task 6).

**Ordering:** Tasks 1 → 2, 1 → 3 (both replace stubs from Task 1). Task 4 needs at least Task 1 to compile and Tasks 2-3 for the backends to work. Task 5 → Task 6. Task 7 needs Tasks 1-3. Task 8 needs Task 5. Tasks 2 and 3 are independent of each other; Task 5 is independent of Tasks 1-4 and can run in parallel.
