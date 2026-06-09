# Session Database Store Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a configurable file-or-database session store using SeaORM-backed SQLite/Postgres persistence while preserving existing JSON-RPC session behavior.

**Architecture:** `vol-session` owns session persistence, including `SessionManager`, file manager, database manager, SeaORM entities, mappings, and migrations. `vol-llm-runtime` builds one shared session manager from `[runtime.session_store]`; `vol-llm-agent-channel` consumes that manager instead of scanning files directly; `vol-agent-server` parses, validates, logs, and passes the config through.

**Tech Stack:** Rust, async-trait, SeaORM, sea-orm-migration, SQLite, Postgres, serde JSON, Cargo workspace tests, existing JSON-RPC channel protocol.

> **Important local constraint:** Do not create or use a git worktree. Work on the current branch and commit after each task as directed.

---

## File Structure

Create or modify these files:

- Modify `crates/vol-session/Cargo.toml`
  - Add SeaORM and `fd-lock` dependencies matching the task store crate.
- Modify `crates/vol-session/src/store.rs`
  - Add `StoreError::Database` and `StoreError::SessionAgentScopeConflict`.
- Create `crates/vol-session/src/manager.rs`
  - Define `SessionInfo`, `SessionManager`, `FileSessionManager`, and manager errors through existing `SessionError`/`StoreError` conversions.
- Create `crates/vol-session/src/database_store/mod.rs`
  - Implement backend inference, SQLite URL normalization, `DatabaseSessionEntryStore`, and `DatabaseSessionManager`.
- Create `crates/vol-session/src/database_store/entity.rs`
  - Define SeaORM entities for `sessions` and `session_entries`.
- Create `crates/vol-session/src/database_store/mapping.rs`
  - Convert between `SessionEntry` and DB models/active models.
- Create `crates/vol-session/src/database_store/migration.rs`
  - Add compiled SeaORM migrations for `sessions` and `session_entries`.
- Modify `crates/vol-session/src/lib.rs`
  - Export new database and manager types.
- Modify `crates/vol-llm-runtime/src/lib.rs`
  - Add `SessionStoreConfig`, construct a shared `Arc<dyn SessionManager>`, expose it on `AgentRuntime`, and use it when registering agents.
- Modify `crates/vol-llm-agent-channel/src/server_core.rs`
  - Pass session store config into runtime and register `SessionHandler` with the shared session manager.
- Modify `crates/vol-llm-agent-channel/src/domain/session.rs`
  - Replace file-system scanning with `SessionManager` calls.
- Modify `crates/vol-agent-server/src/config.rs`
  - Parse and validate `[runtime.session_store]`.
- Modify `crates/vol-agent-server/src/main.rs`
  - Log session store configuration and pass it into `AgentServerCoreBuilder`.
- Modify `config.vol-agent.example.toml`
  - Document `[runtime.session_store]` file/SQLite/Postgres examples.
- Add or modify tests in:
  - `crates/vol-session/src/manager.rs`
  - `crates/vol-session/src/database_store/mod.rs`
  - `crates/vol-llm-runtime/src/lib.rs`
  - `crates/vol-agent-server/src/config.rs`
  - `crates/vol-llm-agent-channel/tests/jsonrpc_e2e_test.rs`

---

## Task 1: Add session manager contracts and file manager

**Files:**
- Modify: `crates/vol-session/src/store.rs`
- Create: `crates/vol-session/src/manager.rs`
- Modify: `crates/vol-session/src/lib.rs`
- Test: `crates/vol-session/src/manager.rs`

- [ ] **Step 1: Add failing tests for `FileSessionManager`**

Create `crates/vol-session/src/manager.rs` with this initial content:

```rust
//! Session manager abstractions for listing and resolving session stores.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;

use crate::file_store::FileSessionEntryStore;
use crate::store::{Result as StoreResult, SessionEntryStore, StoreError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionInfo {
    pub id: String,
    pub agent_id: String,
    pub session_id: String,
    pub entry_count: usize,
    pub created_at: i64,
    pub updated_at: Option<i64>,
}

#[async_trait]
pub trait SessionManager: Send + Sync {
    fn entry_store_for_agent(&self, agent_id: &str) -> Arc<dyn SessionEntryStore>;

    async fn list_sessions(&self, agent_id: Option<&str>) -> StoreResult<Vec<SessionInfo>>;

    async fn session_exists(&self, agent_id: Option<&str>, session_id: &str) -> StoreResult<bool>;

    async fn resolve_session_agent(
        &self,
        agent_id: Option<&str>,
        session_id: &str,
    ) -> StoreResult<String>;

    async fn entry_store_for_session(
        &self,
        agent_id: Option<&str>,
        session_id: &str,
    ) -> StoreResult<Arc<dyn SessionEntryStore>>;
}

#[derive(Debug, Clone)]
pub struct FileSessionManager {
    agents_root: PathBuf,
}

impl FileSessionManager {
    pub fn new<P: AsRef<Path>>(agents_root: P) -> Self {
        Self { agents_root: agents_root.as_ref().to_path_buf() }
    }

    fn agent_sessions_dir(&self, agent_id: &str) -> PathBuf {
        self.agents_root.join(agent_id).join("sessions")
    }

    fn file_store(&self, agent_id: &str) -> FileSessionEntryStore {
        FileSessionEntryStore::new(self.agent_sessions_dir(agent_id))
    }

    fn agent_ids(&self) -> StoreResult<Vec<String>> {
        let dir = match std::fs::read_dir(&self.agents_root) {
            Ok(dir) => dir,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(StoreError::Io(e)),
        };

        let mut ids = Vec::new();
        for entry in dir {
            let entry = entry.map_err(StoreError::Io)?;
            if entry.path().is_dir() {
                if let Some(id) = entry.file_name().to_str() {
                    ids.push(id.to_string());
                }
            }
        }
        ids.sort();
        Ok(ids)
    }

    fn session_matches(&self, agent_id: &str, session_id: &str) -> StoreResult<bool> {
        let store = self.file_store(agent_id);
        let summaries = store.list_sessions().map_err(StoreError::Io)?;
        Ok(summaries.iter().any(|summary| summary.session_id == session_id))
    }

    fn resolve_agent_for_session(
        &self,
        agent_id: Option<&str>,
        session_id: &str,
    ) -> StoreResult<String> {
        if let Some(agent_id) = agent_id {
            if self.session_matches(agent_id, session_id)? {
                return Ok(agent_id.to_string());
            }
            return Err(StoreError::NotFound(format!(
                "session {session_id} for agent {agent_id}"
            )));
        }

        let mut matches = Vec::new();
        for id in self.agent_ids()? {
            if self.session_matches(&id, session_id)? {
                matches.push(id);
            }
        }

        match matches.len() {
            0 => Err(StoreError::NotFound(format!("session {session_id}"))),
            1 => Ok(matches.remove(0)),
            _ => Err(StoreError::Internal(format!(
                "ambiguous session {session_id}: found under agents {}",
                matches.join(", ")
            ))),
        }
    }
}

#[async_trait]
impl SessionManager for FileSessionManager {
    fn entry_store_for_agent(&self, agent_id: &str) -> Arc<dyn SessionEntryStore> {
        Arc::new(self.file_store(agent_id))
    }

    async fn list_sessions(&self, agent_id: Option<&str>) -> StoreResult<Vec<SessionInfo>> {
        let agent_ids = match agent_id {
            Some(agent_id) => vec![agent_id.to_string()],
            None => self.agent_ids()?,
        };

        let mut sessions = Vec::new();
        for agent_id in agent_ids {
            let store = self.file_store(&agent_id);
            for summary in store.list_sessions().map_err(StoreError::Io)? {
                sessions.push(SessionInfo {
                    id: summary.session_id.clone(),
                    agent_id: agent_id.clone(),
                    session_id: summary.session_id,
                    entry_count: summary.entry_count,
                    created_at: summary.created_at,
                    updated_at: None,
                });
            }
        }
        sessions.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(sessions)
    }

    async fn session_exists(&self, agent_id: Option<&str>, session_id: &str) -> StoreResult<bool> {
        match self.resolve_agent_for_session(agent_id, session_id) {
            Ok(_) => Ok(true),
            Err(StoreError::NotFound(_)) => Ok(false),
            Err(e) => Err(e),
        }
    }

    async fn resolve_session_agent(
        &self,
        agent_id: Option<&str>,
        session_id: &str,
    ) -> StoreResult<String> {
        self.resolve_agent_for_session(agent_id, session_id)
    }

    async fn entry_store_for_session(
        &self,
        agent_id: Option<&str>,
        session_id: &str,
    ) -> StoreResult<Arc<dyn SessionEntryStore>> {
        let resolved_agent_id = self.resolve_agent_for_session(agent_id, session_id)?;
        Ok(Arc::new(self.file_store(&resolved_agent_id)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entry::{SessionEntry, SessionEntryData, SessionEntryType};

    fn entry(session_id: &str, id: &str, created_at: i64) -> SessionEntry {
        SessionEntry {
            id: id.to_string(),
            session_id: session_id.to_string(),
            created_at,
            parent_id: None,
            r#type: SessionEntryType::Summary,
            data: SessionEntryData::Summary { summary: format!("summary-{id}") },
        }
    }

    #[tokio::test]
    async fn file_manager_lists_sessions_with_agent_id() {
        let temp = tempfile::tempdir().unwrap();
        let manager = FileSessionManager::new(temp.path().join("agents"));
        let alpha = manager.entry_store_for_agent("alpha");
        alpha.save(entry("session-a", "entry-a", 10)).await.unwrap();

        let sessions = manager.list_sessions(None).await.unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].id, "session-a");
        assert_eq!(sessions[0].agent_id, "alpha");
        assert_eq!(sessions[0].session_id, "session-a");
        assert_eq!(sessions[0].entry_count, 1);
        assert_eq!(sessions[0].created_at, 10);
    }

    #[tokio::test]
    async fn file_manager_resolves_store_by_agent_and_session() {
        let temp = tempfile::tempdir().unwrap();
        let manager = FileSessionManager::new(temp.path().join("agents"));
        let alpha = manager.entry_store_for_agent("alpha");
        alpha.save(entry("session-a", "entry-a", 10)).await.unwrap();

        let store = manager
            .entry_store_for_session(Some("alpha"), "session-a")
            .await
            .unwrap();
        let entries = store.get_entries("session-a").await.unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].id, "entry-a");
    }

    #[tokio::test]
    async fn file_manager_reports_ambiguous_unscoped_session() {
        let temp = tempfile::tempdir().unwrap();
        let manager = FileSessionManager::new(temp.path().join("agents"));
        manager
            .entry_store_for_agent("alpha")
            .save(entry("same-session", "entry-a", 10))
            .await
            .unwrap();
        manager
            .entry_store_for_agent("beta")
            .save(entry("same-session", "entry-b", 20))
            .await
            .unwrap();

        let err = manager
            .entry_store_for_session(None, "same-session")
            .await
            .unwrap_err();
        assert!(err.to_string().contains("ambiguous session same-session"));
    }
}
```

- [ ] **Step 2: Export the manager module**

Modify `crates/vol-session/src/lib.rs`:

```rust
pub mod manager;
```

Add this export near the other `pub use` lines:

```rust
pub use manager::{FileSessionManager, SessionInfo, SessionManager};
```

- [ ] **Step 3: Run the new tests**

Run:

```bash
cargo test -p vol-session manager::tests -- --nocapture
```

Expected now: tests compile and pass because `FileSessionManager` only uses existing errors.

- [ ] **Step 4: Add store error variants required by database work**

Modify `crates/vol-session/src/store.rs` so `StoreError` includes these variants after `Internal(String)`:

```rust
    #[error("Database error: {0}")]
    Database(String),

    #[error("Session agent scope conflict for {session_id}: expected {expected}, actual {actual}")]
    SessionAgentScopeConflict {
        session_id: String,
        expected: String,
        actual: String,
    },
```

- [ ] **Step 5: Run session tests**

Run:

```bash
cargo test -p vol-session
```

Expected: all `vol-session` tests pass.

- [ ] **Step 7: Commit**

```bash
git add crates/vol-session/src/store.rs crates/vol-session/src/manager.rs crates/vol-session/src/lib.rs
git commit -m "feat(session): add session manager abstraction"
```

---

## Task 2: Add SeaORM database schema and mapping helpers

**Files:**
- Modify: `crates/vol-session/Cargo.toml`
- Create: `crates/vol-session/src/database_store/entity.rs`
- Create: `crates/vol-session/src/database_store/mapping.rs`
- Create: `crates/vol-session/src/database_store/migration.rs`
- Create: `crates/vol-session/src/database_store/mod.rs`
- Modify: `crates/vol-session/src/lib.rs`
- Test: `crates/vol-session/src/database_store/mapping.rs`

- [ ] **Step 1: Add dependencies**

Modify `crates/vol-session/Cargo.toml` dependencies:

```toml
sea-orm = { workspace = true }
sea-orm-migration = { workspace = true }
```

Modify dev-dependencies:

```toml
fd-lock = { workspace = true }
```

- [ ] **Step 2: Add database module shell**

Create `crates/vol-session/src/database_store/mod.rs`:

```rust
//! SeaORM-backed database session store.

mod entity;
mod mapping;
mod migration;

use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use sea_orm::{ConnectOptions, Database, DatabaseConnection};
use sea_orm_migration::MigratorTrait;

use crate::manager::{SessionInfo, SessionManager};
use crate::store::{Result, SessionEntryStore, StoreError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DatabaseBackend {
    Sqlite,
    Postgres,
    MySql,
}

fn infer_backend(url: &str) -> Result<DatabaseBackend> {
    let scheme = url
        .split_once(':')
        .map(|(scheme, _)| scheme)
        .unwrap_or_default();

    match scheme {
        "sqlite" => Ok(DatabaseBackend::Sqlite),
        "postgres" | "postgresql" => Ok(DatabaseBackend::Postgres),
        "mysql" => Ok(DatabaseBackend::MySql),
        "" => Err(StoreError::Database(
            "unsupported session store database url scheme: <missing>".to_string(),
        )),
        other => Err(StoreError::Database(format!(
            "unsupported session store database url scheme: {other}"
        ))),
    }
}

fn normalize_sqlite_url(url: &str) -> Result<String> {
    if url == "sqlite::memory:" || url == "sqlite://:memory:" {
        return Ok(url.to_string());
    }

    if !url.starts_with("sqlite:") {
        return Err(StoreError::Database(
            "sqlite session store url must start with sqlite:".to_string(),
        ));
    }

    if let Some((_, query)) = url.split_once('?') {
        if query
            .split('&')
            .filter_map(|param| param.split_once('=').map(|(key, _)| key))
            .any(|key| key == "mode")
        {
            Ok(url.to_string())
        } else {
            Ok(format!("{url}&mode=rwc"))
        }
    } else {
        Ok(format!("{url}?mode=rwc"))
    }
}

fn create_sqlite_parent_dir(url: &str) -> Result<()> {
    if let Some(path) = sqlite_file_path(url) {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent).map_err(StoreError::Io)?;
            }
        }
    }
    Ok(())
}

fn sqlite_file_path(url: &str) -> Option<PathBuf> {
    if url == "sqlite::memory:" || url == "sqlite://:memory:" {
        return None;
    }
    let without_query = url.split_once('?').map(|(path, _)| path).unwrap_or(url);
    let raw = without_query
        .strip_prefix("sqlite://")
        .or_else(|| without_query.strip_prefix("sqlite:"))?;
    if raw.is_empty() || raw == ":memory:" {
        return None;
    }
    Some(PathBuf::from(raw))
}

impl DatabaseBackend {
    fn label(self) -> &'static str {
        match self {
            DatabaseBackend::Sqlite => "sqlite",
            DatabaseBackend::Postgres => "postgres",
            DatabaseBackend::MySql => "mysql",
        }
    }
}

#[derive(Clone)]
pub struct DatabaseSessionManager {
    db: DatabaseConnection,
    backend: DatabaseBackend,
}

pub struct DatabaseSessionEntryStore {
    db: DatabaseConnection,
    agent_id: String,
}

impl DatabaseSessionManager {
    pub async fn connect(url: &str) -> Result<Self> {
        match infer_backend(url)? {
            DatabaseBackend::Sqlite => {
                Self::connect_backend(DatabaseBackend::Sqlite, normalize_sqlite_url(url)?).await
            }
            DatabaseBackend::Postgres => {
                Self::connect_backend(DatabaseBackend::Postgres, url.to_string()).await
            }
            DatabaseBackend::MySql => Err(StoreError::Database(
                "database session store backend is recognized but not enabled yet: mysql".to_string(),
            )),
        }
    }

    async fn connect_backend(backend: DatabaseBackend, url: String) -> Result<Self> {
        if backend == DatabaseBackend::Sqlite {
            create_sqlite_parent_dir(&url)?;
        }

        let mut options = ConnectOptions::new(url);
        options.max_connections(5);
        let db = Database::connect(options).await.map_err(|e| {
            StoreError::Database(format!(
                "failed to connect {} session store: {e}",
                backend.label()
            ))
        })?;

        migration::Migrator::up(&db, None).await.map_err(|e| {
            StoreError::Database(format!(
                "failed to migrate {} session store: {e}",
                backend.label()
            ))
        })?;

        Ok(Self { db, backend })
    }

    fn scoped_store(&self, agent_id: &str) -> DatabaseSessionEntryStore {
        DatabaseSessionEntryStore { db: self.db.clone(), agent_id: agent_id.to_string() }
    }
}
```

- [ ] **Step 3: Export database types**

Modify `crates/vol-session/src/lib.rs`:

```rust
pub mod database_store;
```

Add:

```rust
pub use database_store::{DatabaseSessionEntryStore, DatabaseSessionManager};
```

- [ ] **Step 4: Add SeaORM entities**

Create `crates/vol-session/src/database_store/entity.rs`:

```rust
use sea_orm::entity::prelude::*;

pub mod sessions {
    use super::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
    #[sea_orm(table_name = "sessions")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub id: String,
        pub agent_id: String,
        pub created_at: i64,
        pub updated_at: i64,
        pub entry_count: i32,
        pub metadata: String,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {
        #[sea_orm(has_many = "super::session_entries::Entity")]
        SessionEntries,
    }

    impl Related<super::session_entries::Entity> for Entity {
        fn to() -> RelationDef {
            Relation::SessionEntries.def()
        }
    }

    impl ActiveModelBehavior for ActiveModel {}
}

pub mod session_entries {
    use super::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
    #[sea_orm(table_name = "session_entries")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub id: String,
        pub session_id: String,
        pub created_at: i64,
        pub parent_id: Option<String>,
        pub entry_type: String,
        pub data: String,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {
        #[sea_orm(
            belongs_to = "super::sessions::Entity",
            from = "Column::SessionId",
            to = "super::sessions::Column::Id"
        )]
        Sessions,
    }

    impl Related<super::sessions::Entity> for Entity {
        fn to() -> RelationDef {
            Relation::Sessions.def()
        }
    }

    impl ActiveModelBehavior for ActiveModel {}
}
```

- [ ] **Step 5: Add mapping helpers and tests**

Create `crates/vol-session/src/database_store/mapping.rs`:

```rust
use sea_orm::ActiveValue;

use crate::entry::{SessionEntry, SessionEntryData, SessionEntryType};
use crate::manager::SessionInfo;
use crate::store::{Result, StoreError};

use super::entity::{session_entries, sessions};

pub fn entry_type_to_db(entry_type: &SessionEntryType) -> &'static str {
    match entry_type {
        SessionEntryType::Message => "message",
        SessionEntryType::Checkpoint => "checkpoint",
        SessionEntryType::Summary => "summary",
    }
}

pub fn entry_type_from_db(value: &str) -> Result<SessionEntryType> {
    match value {
        "message" => Ok(SessionEntryType::Message),
        "checkpoint" => Ok(SessionEntryType::Checkpoint),
        "summary" => Ok(SessionEntryType::Summary),
        other => Err(StoreError::Serialization(format!(
            "unknown session entry type from database: {other}"
        ))),
    }
}

pub fn entry_to_active_model(entry: SessionEntry) -> Result<session_entries::ActiveModel> {
    let data = serde_json::to_string(&entry.data).map_err(|e| {
        StoreError::Serialization(format!("failed to serialize session entry data: {e}"))
    })?;

    Ok(session_entries::ActiveModel {
        id: ActiveValue::Set(entry.id),
        session_id: ActiveValue::Set(entry.session_id),
        created_at: ActiveValue::Set(entry.created_at),
        parent_id: ActiveValue::Set(entry.parent_id),
        entry_type: ActiveValue::Set(entry_type_to_db(&entry.r#type).to_string()),
        data: ActiveValue::Set(data),
    })
}

pub fn model_to_entry(model: session_entries::Model) -> Result<SessionEntry> {
    let data: SessionEntryData = serde_json::from_str(&model.data).map_err(|e| {
        StoreError::Serialization(format!("failed to deserialize session entry data: {e}"))
    })?;
    let entry_type = entry_type_from_db(&model.entry_type)?;

    Ok(SessionEntry {
        id: model.id,
        session_id: model.session_id,
        created_at: model.created_at,
        parent_id: model.parent_id,
        r#type: entry_type,
        data,
    })
}

pub fn session_model_to_info(model: sessions::Model) -> SessionInfo {
    SessionInfo {
        id: model.id.clone(),
        agent_id: model.agent_id,
        session_id: model.id,
        entry_count: model.entry_count.max(0) as usize,
        created_at: model.created_at,
        updated_at: Some(model.updated_at),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_summary_entry_round_trip() {
        let entry = SessionEntry {
            id: "entry-1".to_string(),
            session_id: "session-1".to_string(),
            created_at: 42,
            parent_id: Some("parent-1".to_string()),
            r#type: SessionEntryType::Summary,
            data: SessionEntryData::Summary { summary: "hello".to_string() },
        };

        let active = entry_to_active_model(entry.clone()).unwrap();
        let model = session_entries::Model {
            id: active.id.unwrap(),
            session_id: active.session_id.unwrap(),
            created_at: active.created_at.unwrap(),
            parent_id: active.parent_id.unwrap(),
            entry_type: active.entry_type.unwrap(),
            data: active.data.unwrap(),
        };

        let mapped = model_to_entry(model).unwrap();
        assert_eq!(mapped.id, entry.id);
        assert_eq!(mapped.session_id, entry.session_id);
        assert_eq!(mapped.created_at, entry.created_at);
        assert_eq!(mapped.parent_id, entry.parent_id);
        assert_eq!(mapped.r#type, entry.r#type);
        match mapped.data {
            SessionEntryData::Summary { summary } => assert_eq!(summary, "hello"),
            _ => panic!("expected summary data"),
        }
    }

    #[test]
    fn rejects_unknown_entry_type() {
        let err = entry_type_from_db("bogus").unwrap_err();
        assert!(err.to_string().contains("unknown session entry type"));
    }
}
```

- [ ] **Step 6: Add compiled migration**

Create `crates/vol-session/src/database_store/migration.rs`:

```rust
use sea_orm_migration::prelude::*;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![Box::new(M20260609_000001CreateSessions)]
    }
}

#[derive(DeriveMigrationName)]
pub struct M20260609_000001CreateSessions;

#[async_trait::async_trait]
impl MigrationTrait for M20260609_000001CreateSessions {
    async fn up(&self, manager: &SchemaManager) -> std::result::Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Sessions::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Sessions::Id).string().not_null().primary_key())
                    .col(ColumnDef::new(Sessions::AgentId).string().not_null())
                    .col(ColumnDef::new(Sessions::CreatedAt).big_integer().not_null())
                    .col(ColumnDef::new(Sessions::UpdatedAt).big_integer().not_null())
                    .col(ColumnDef::new(Sessions::EntryCount).integer().not_null().default(0))
                    .col(ColumnDef::new(Sessions::Metadata).text().not_null())
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(SessionEntries::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(SessionEntries::Id).string().not_null().primary_key())
                    .col(ColumnDef::new(SessionEntries::SessionId).string().not_null())
                    .col(ColumnDef::new(SessionEntries::CreatedAt).big_integer().not_null())
                    .col(ColumnDef::new(SessionEntries::ParentId).string().null())
                    .col(ColumnDef::new(SessionEntries::EntryType).string().not_null())
                    .col(ColumnDef::new(SessionEntries::Data).text().not_null())
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_sessions_agent_id_updated_at")
                    .table(Sessions::Table)
                    .col(Sessions::AgentId)
                    .col(Sessions::UpdatedAt)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_sessions_updated_at")
                    .table(Sessions::Table)
                    .col(Sessions::UpdatedAt)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_session_entries_session_id_created_at")
                    .table(SessionEntries::Table)
                    .col(SessionEntries::SessionId)
                    .col(SessionEntries::CreatedAt)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_session_entries_parent_id")
                    .table(SessionEntries::Table)
                    .col(SessionEntries::ParentId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> std::result::Result<(), DbErr> {
        manager
            .drop_index(Index::drop().name("idx_session_entries_parent_id").to_owned())
            .await?;
        manager
            .drop_index(Index::drop().name("idx_session_entries_session_id_created_at").to_owned())
            .await?;
        manager
            .drop_index(Index::drop().name("idx_sessions_updated_at").to_owned())
            .await?;
        manager
            .drop_index(Index::drop().name("idx_sessions_agent_id_updated_at").to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(SessionEntries::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Sessions::Table).to_owned())
            .await?;
        Ok(())
    }
}

#[derive(DeriveIden)]
enum Sessions {
    Table,
    Id,
    AgentId,
    CreatedAt,
    UpdatedAt,
    EntryCount,
    Metadata,
}

#[derive(DeriveIden)]
enum SessionEntries {
    Table,
    Id,
    SessionId,
    CreatedAt,
    ParentId,
    EntryType,
    Data,
}
```

- [ ] **Step 7: Run mapping tests**

Run:

```bash
cargo test -p vol-session database_store::mapping::tests -- --nocapture
```

Expected: mapping tests pass.

- [ ] **Step 8: Commit**

```bash
git add crates/vol-session/Cargo.toml crates/vol-session/src/lib.rs crates/vol-session/src/database_store
git commit -m "feat(session): add database session schema"
```

---

## Task 3: Implement `DatabaseSessionEntryStore` SQLite behavior

**Files:**
- Modify: `crates/vol-session/src/database_store/mod.rs`
- Test: `crates/vol-session/src/database_store/mod.rs`

- [ ] **Step 1: Add failing SQLite database store tests**

Append this test module to `crates/vol-session/src/database_store/mod.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::entry::{SessionEntry, SessionEntryData, SessionEntryType};

    fn test_entry(session_id: &str, id: &str, created_at: i64) -> SessionEntry {
        SessionEntry {
            id: id.to_string(),
            session_id: session_id.to_string(),
            created_at,
            parent_id: None,
            r#type: SessionEntryType::Summary,
            data: SessionEntryData::Summary { summary: format!("summary-{id}") },
        }
    }

    async fn sqlite_manager() -> (tempfile::TempDir, DatabaseSessionManager) {
        let temp = tempfile::tempdir().unwrap();
        let url = format!("sqlite://{}", temp.path().join("sessions.db").display());
        let manager = DatabaseSessionManager::connect(&url).await.unwrap();
        (temp, manager)
    }

    #[tokio::test]
    async fn sqlite_save_get_and_count_round_trip() {
        let (_temp, manager) = sqlite_manager().await;
        let store = manager.entry_store_for_agent("alpha");
        store.save(test_entry("session-a", "entry-1", 10)).await.unwrap();
        store.save(test_entry("session-a", "entry-2", 20)).await.unwrap();

        let entries = store.get_entries("session-a").await.unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].id, "entry-1");
        assert_eq!(entries[1].id, "entry-2");
        assert_eq!(store.get_count("session-a").await.unwrap(), 2);
    }

    #[tokio::test]
    async fn sqlite_list_sessions_reads_sessions_table() {
        let (_temp, manager) = sqlite_manager().await;
        let store = manager.entry_store_for_agent("alpha");
        store.save(test_entry("session-a", "entry-1", 10)).await.unwrap();
        store.save(test_entry("session-a", "entry-2", 20)).await.unwrap();

        let sessions = manager.list_sessions(Some("alpha")).await.unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].id, "session-a");
        assert_eq!(sessions[0].agent_id, "alpha");
        assert_eq!(sessions[0].entry_count, 2);
        assert_eq!(sessions[0].created_at, 10);
        assert_eq!(sessions[0].updated_at, Some(20));
    }

    #[tokio::test]
    async fn sqlite_delete_session_removes_metadata_and_entries() {
        let (_temp, manager) = sqlite_manager().await;
        let store = manager.entry_store_for_agent("alpha");
        store.save(test_entry("session-a", "entry-1", 10)).await.unwrap();

        store.delete_session("session-a").await.unwrap();

        assert_eq!(store.get_entries("session-a").await.unwrap().len(), 0);
        assert!(!manager.session_exists(Some("alpha"), "session-a").await.unwrap());
    }

    #[tokio::test]
    async fn sqlite_rejects_conflicting_agent_scope() {
        let (_temp, manager) = sqlite_manager().await;
        manager
            .entry_store_for_agent("alpha")
            .save(test_entry("session-a", "entry-1", 10))
            .await
            .unwrap();

        let err = manager
            .entry_store_for_agent("beta")
            .save(test_entry("session-a", "entry-2", 20))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("Session agent scope conflict"));
    }
}
```

- [ ] **Step 2: Run tests and confirm they fail because store methods are missing**

Run:

```bash
cargo test -p vol-session database_store::tests -- --nocapture
```

Expected: compile errors mention `SessionEntryStore` or `SessionManager` methods are not implemented for `DatabaseSessionEntryStore` / `DatabaseSessionManager`.

- [ ] **Step 3: Implement `DatabaseSessionEntryStore` and `DatabaseSessionManager`**

Append these implementations to `crates/vol-session/src/database_store/mod.rs` below `impl DatabaseSessionManager`:

```rust
impl DatabaseSessionEntryStore {
    async fn ensure_session_for_entry(
        &self,
        txn: &sea_orm::DatabaseTransaction,
        entry: &crate::entry::SessionEntry,
    ) -> Result<()> {
        use sea_orm::{ActiveModelTrait, ActiveValue, EntityTrait};

        let existing = entity::sessions::Entity::find_by_id(entry.session_id.clone())
            .one(txn)
            .await
            .map_err(|e| StoreError::Database(format!("failed to load session metadata: {e}")))?;

        match existing {
            Some(model) => {
                if model.agent_id != self.agent_id {
                    return Err(StoreError::SessionAgentScopeConflict {
                        session_id: entry.session_id.clone(),
                        expected: model.agent_id,
                        actual: self.agent_id.clone(),
                    });
                }
                Ok(())
            }
            None => {
                entity::sessions::ActiveModel {
                    id: ActiveValue::Set(entry.session_id.clone()),
                    agent_id: ActiveValue::Set(self.agent_id.clone()),
                    created_at: ActiveValue::Set(entry.created_at),
                    updated_at: ActiveValue::Set(entry.created_at),
                    entry_count: ActiveValue::Set(0),
                    metadata: ActiveValue::Set("{}".to_string()),
                }
                .insert(txn)
                .await
                .map_err(|e| StoreError::Database(format!("failed to create session metadata: {e}")))?;
                Ok(())
            }
        }
    }
}

#[async_trait]
impl SessionEntryStore for DatabaseSessionEntryStore {
    async fn save(&self, entry: crate::entry::SessionEntry) -> Result<()> {
        use sea_orm::{ActiveModelTrait, ActiveValue, EntityTrait, TransactionTrait};

        let txn = self
            .db
            .begin()
            .await
            .map_err(|e| StoreError::Database(format!("failed to begin session entry transaction: {e}")))?;

        self.ensure_session_for_entry(&txn, &entry).await?;

        mapping::entry_to_active_model(entry.clone())?
            .insert(&txn)
            .await
            .map_err(|e| StoreError::Database(format!("failed to insert session entry: {e}")))?;

        let session = entity::sessions::Entity::find_by_id(entry.session_id.clone())
            .one(&txn)
            .await
            .map_err(|e| StoreError::Database(format!("failed to reload session metadata: {e}")))?
            .ok_or_else(|| StoreError::NotFound(format!("session {}", entry.session_id)))?;

        let next_count = session.entry_count + 1;
        let mut active: entity::sessions::ActiveModel = session.into();
        active.updated_at = ActiveValue::Set(entry.created_at);
        active.entry_count = ActiveValue::Set(next_count);
        active
            .update(&txn)
            .await
            .map_err(|e| StoreError::Database(format!("failed to update session metadata: {e}")))?;

        txn.commit()
            .await
            .map_err(|e| StoreError::Database(format!("failed to commit session entry transaction: {e}")))?;
        Ok(())
    }

    async fn get_entries(&self, session_id: &str) -> Result<Vec<crate::entry::SessionEntry>> {
        use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};

        let models = entity::session_entries::Entity::find()
            .filter(entity::session_entries::Column::SessionId.eq(session_id.to_string()))
            .order_by_asc(entity::session_entries::Column::CreatedAt)
            .order_by_asc(entity::session_entries::Column::Id)
            .all(&self.db)
            .await
            .map_err(|e| StoreError::Database(format!("failed to get session entries: {e}")))?;

        models.into_iter().map(mapping::model_to_entry).collect()
    }

    async fn get_after(&self, session_id: &str, after: i64) -> Result<Vec<crate::entry::SessionEntry>> {
        use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};

        let models = entity::session_entries::Entity::find()
            .filter(entity::session_entries::Column::SessionId.eq(session_id.to_string()))
            .filter(entity::session_entries::Column::CreatedAt.gt(after))
            .order_by_asc(entity::session_entries::Column::CreatedAt)
            .order_by_asc(entity::session_entries::Column::Id)
            .all(&self.db)
            .await
            .map_err(|e| StoreError::Database(format!("failed to get session entries after {after}: {e}")))?;

        models.into_iter().map(mapping::model_to_entry).collect()
    }

    async fn find_latest_checkpoint(&self, session_id: &str) -> Result<Option<crate::entry::SessionEntry>> {
        use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};

        let model = entity::session_entries::Entity::find()
            .filter(entity::session_entries::Column::SessionId.eq(session_id.to_string()))
            .filter(entity::session_entries::Column::EntryType.eq("checkpoint"))
            .order_by_desc(entity::session_entries::Column::CreatedAt)
            .one(&self.db)
            .await
            .map_err(|e| StoreError::Database(format!("failed to find latest checkpoint: {e}")))?;

        model.map(mapping::model_to_entry).transpose()
    }

    async fn delete_session(&self, session_id: &str) -> Result<()> {
        use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, TransactionTrait};

        let txn = self
            .db
            .begin()
            .await
            .map_err(|e| StoreError::Database(format!("failed to begin delete session transaction: {e}")))?;

        entity::session_entries::Entity::delete_many()
            .filter(entity::session_entries::Column::SessionId.eq(session_id.to_string()))
            .exec(&txn)
            .await
            .map_err(|e| StoreError::Database(format!("failed to delete session entries: {e}")))?;

        entity::sessions::Entity::delete_by_id(session_id.to_string())
            .exec(&txn)
            .await
            .map_err(|e| StoreError::Database(format!("failed to delete session metadata: {e}")))?;

        txn.commit()
            .await
            .map_err(|e| StoreError::Database(format!("failed to commit delete session transaction: {e}")))?;
        Ok(())
    }

    async fn get_count(&self, session_id: &str) -> Result<usize> {
        use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

        let count = entity::session_entries::Entity::find()
            .filter(entity::session_entries::Column::SessionId.eq(session_id.to_string()))
            .count(&self.db)
            .await
            .map_err(|e| StoreError::Database(format!("failed to count session entries: {e}")))?;
        Ok(count as usize)
    }
}

#[async_trait]
impl SessionManager for DatabaseSessionManager {
    fn entry_store_for_agent(&self, agent_id: &str) -> Arc<dyn SessionEntryStore> {
        Arc::new(self.scoped_store(agent_id))
    }

    async fn list_sessions(&self, agent_id: Option<&str>) -> Result<Vec<SessionInfo>> {
        use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};

        let mut query = entity::sessions::Entity::find().order_by_desc(entity::sessions::Column::UpdatedAt);
        if let Some(agent_id) = agent_id {
            query = query.filter(entity::sessions::Column::AgentId.eq(agent_id.to_string()));
        }

        let models = query
            .all(&self.db)
            .await
            .map_err(|e| StoreError::Database(format!("failed to list sessions: {e}")))?;
        Ok(models.into_iter().map(mapping::session_model_to_info).collect())
    }

    async fn session_exists(&self, agent_id: Option<&str>, session_id: &str) -> Result<bool> {
        use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

        let mut query = entity::sessions::Entity::find()
            .filter(entity::sessions::Column::Id.eq(session_id.to_string()));
        if let Some(agent_id) = agent_id {
            query = query.filter(entity::sessions::Column::AgentId.eq(agent_id.to_string()));
        }

        let model = query
            .one(&self.db)
            .await
            .map_err(|e| StoreError::Database(format!("failed to check session existence: {e}")))?;
        Ok(model.is_some())
    }

    async fn resolve_session_agent(
        &self,
        agent_id: Option<&str>,
        session_id: &str,
    ) -> Result<String> {
        use sea_orm::EntityTrait;

        let model = entity::sessions::Entity::find_by_id(session_id.to_string())
            .one(&self.db)
            .await
            .map_err(|e| StoreError::Database(format!("failed to resolve session agent: {e}")))?
            .ok_or_else(|| StoreError::NotFound(format!("session {session_id}")))?;

        if let Some(expected_agent_id) = agent_id {
            if model.agent_id != expected_agent_id {
                return Err(StoreError::SessionAgentScopeConflict {
                    session_id: session_id.to_string(),
                    expected: expected_agent_id.to_string(),
                    actual: model.agent_id,
                });
            }
        }

        Ok(model.agent_id)
    }

    async fn entry_store_for_session(
        &self,
        agent_id: Option<&str>,
        session_id: &str,
    ) -> Result<Arc<dyn SessionEntryStore>> {
        let resolved_agent_id = self.resolve_session_agent(agent_id, session_id).await?;
        Ok(Arc::new(self.scoped_store(&resolved_agent_id)))
    }
}
```


- [ ] **Step 4: Run SQLite database store tests**

Run:

```bash
cargo test -p vol-session database_store::tests -- --nocapture
```

Expected: all SQLite database store tests pass.

- [ ] **Step 5: Run full session crate tests**

Run:

```bash
cargo test -p vol-session
```

Expected: all `vol-session` tests pass.

- [ ] **Step 7: Commit**

```bash
git add crates/vol-session/src/database_store/mod.rs
git commit -m "feat(session): implement database entry store"
```

---

## Task 4: Add Postgres coverage for the database session store

**Files:**
- Modify: `crates/vol-session/src/database_store/mod.rs`

- [ ] **Step 1: Add Postgres test helpers and tests**

Append this code inside the existing `#[cfg(test)] mod tests` in `crates/vol-session/src/database_store/mod.rs`:

```rust
    const POSTGRES_TEST_URL_ENV: &str = "VOL_AGENT_POSTGRES_TEST_URL";

    struct PostgresTestLock(std::fs::File);

    impl PostgresTestLock {
        fn acquire() -> Self {
            use fd_lock::RwLock;
            let path = std::env::temp_dir().join("vol-agent-postgres-session-store-test.lock");
            let file = std::fs::OpenOptions::new()
                .create(true)
                .read(true)
                .write(true)
                .open(path)
                .expect("postgres session test lock file should open");
            file.lock().expect("postgres session test lock should be acquired");
            Self(file)
        }
    }

    impl Drop for PostgresTestLock {
        fn drop(&mut self) {
            use fd_lock::RwLock;
            self.0.unlock().expect("postgres session test lock should release");
        }
    }

    async fn clean_postgres(manager: &DatabaseSessionManager) {
        use sea_orm::EntityTrait;
        entity::session_entries::Entity::delete_many().exec(&manager.db).await.unwrap();
        entity::sessions::Entity::delete_many().exec(&manager.db).await.unwrap();
    }

    #[tokio::test]
    async fn postgres_save_list_and_reconnect_when_configured() {
        let Ok(url) = std::env::var(POSTGRES_TEST_URL_ENV) else {
            eprintln!("skipping postgres session store test: {POSTGRES_TEST_URL_ENV} is not set");
            return;
        };

        let _lock = PostgresTestLock::acquire();
        let manager = DatabaseSessionManager::connect(&url).await.unwrap();
        clean_postgres(&manager).await;

        manager
            .entry_store_for_agent("alpha")
            .save(test_entry("pg-session-a", "pg-entry-1", 100))
            .await
            .unwrap();

        let reconnected = DatabaseSessionManager::connect(&url).await.unwrap();
        let sessions = reconnected.list_sessions(Some("alpha")).await.unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].session_id, "pg-session-a");
        assert_eq!(sessions[0].entry_count, 1);

        clean_postgres(&reconnected).await;
    }
```


- [ ] **Step 2: Run Postgres-gated tests without env**

Run:

```bash
cargo test -p vol-session postgres_save_list_and_reconnect_when_configured -- --nocapture
```

Expected when `VOL_AGENT_POSTGRES_TEST_URL` is not set: test passes and prints a skip message.

- [ ] **Step 3: Run with Postgres when available**

If a Postgres test URL is configured in the environment, run:

```bash
cargo test -p vol-session postgres_save_list_and_reconnect_when_configured -- --nocapture
```

Expected with `VOL_AGENT_POSTGRES_TEST_URL` set: test passes after writing, reconnecting, listing, and cleaning rows.

- [ ] **Step 4: Run full session tests**

Run:

```bash
cargo test -p vol-session
```

Expected: all `vol-session` tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/vol-session/src/database_store/mod.rs
git commit -m "test(session): cover postgres database store"
```

---

## Task 5: Wire session manager into `AgentRuntime`

**Files:**
- Modify: `crates/vol-llm-runtime/src/lib.rs`
- Test: `crates/vol-llm-runtime/src/lib.rs`

- [ ] **Step 1: Add failing runtime config tests**

In `crates/vol-llm-runtime/src/lib.rs`, add tests beside existing task store config tests:

```rust
    #[test]
    fn session_store_config_rejects_file_url() {
        let config = SessionStoreConfig {
            store_type: SessionStoreType::File,
            url: Some("sqlite://sessions.db".to_string()),
        };
        let err = config.validate().unwrap_err();
        assert!(err.contains("runtime.session_store.url is not valid"));
    }

    #[test]
    fn session_store_config_requires_database_url() {
        let config = SessionStoreConfig { store_type: SessionStoreType::Database, url: None };
        let err = config.validate().unwrap_err();
        assert!(err.contains("runtime.session_store.url is required"));
    }

    #[test]
    fn session_store_config_accepts_sqlite_postgres_and_mysql_schemes() {
        for url in [
            "sqlite://sessions.db",
            "postgres://user:pass@localhost/db",
            "postgresql://user:pass@localhost/db",
            "mysql://user:pass@localhost/db",
        ] {
            let config = SessionStoreConfig {
                store_type: SessionStoreType::Database,
                url: Some(url.to_string()),
            };
            config.validate().unwrap();
        }
    }

    #[tokio::test]
    async fn runtime_builds_with_sqlite_session_store() {
        let temp = tempfile::tempdir().unwrap();
        let db_url = format!("sqlite://{}", temp.path().join("sessions.db").display());
        let runtime = AgentRuntime::builder(temp.path().to_path_buf(), temp.path().join("store"))
            .with_session_store_config(Some(SessionStoreConfig {
                store_type: SessionStoreType::Database,
                url: Some(db_url),
            }))
            .build()
            .await;

        if let Err(err) = &runtime {
            if err.contains("No LLM provider configured") {
                return;
            }
        }
        assert!(runtime.is_ok());
    }
```

- [ ] **Step 2: Run tests and verify missing types/methods fail**

Run:

```bash
cargo test -p vol-llm-runtime session_store_config -- --nocapture
```

Expected: compile errors for `SessionStoreConfig`, `SessionStoreType`, and `with_session_store_config`.

- [ ] **Step 3: Add runtime imports**

Modify the imports near the top of `crates/vol-llm-runtime/src/lib.rs`:

```rust
use vol_session::{DatabaseSessionManager, FileSessionManager, SessionManager};
```


- [ ] **Step 4: Add session store config types**

Add this block after `TaskStoreConfig`:

```rust
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SessionStoreType {
    File,
    Database,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct SessionStoreConfig {
    #[serde(rename = "type")]
    pub store_type: SessionStoreType,
    pub url: Option<String>,
}

impl SessionStoreConfig {
    pub fn validate(&self) -> Result<(), String> {
        match self.store_type {
            SessionStoreType::File => {
                if self.url.is_some() {
                    return Err("runtime.session_store.url is not valid when type = \"file\"".to_string());
                }
                Ok(())
            }
            SessionStoreType::Database => {
                let url = self.url.as_deref().ok_or_else(|| {
                    "runtime.session_store.url is required when type = \"database\"".to_string()
                })?;
                validate_session_database_url_scheme(url)
            }
        }
    }

    pub fn required_url(&self) -> Result<&str, String> {
        self.url
            .as_deref()
            .ok_or_else(|| "runtime.session_store.url is required when type = \"database\"".to_string())
    }
}

pub fn validate_session_database_url_scheme(url: &str) -> Result<(), String> {
    let scheme = url
        .split_once(':')
        .map(|(scheme, _)| scheme)
        .unwrap_or_default();

    match scheme {
        "sqlite" | "postgres" | "postgresql" | "mysql" => Ok(()),
        "" => Err("unsupported session store database url scheme: <missing>".to_string()),
        other => Err(format!("unsupported session store database url scheme: {other}")),
    }
}
```

- [ ] **Step 5: Store the config in `AgentRuntimeBuilder`**

Modify `AgentRuntimeBuilder`:

```rust
pub struct AgentRuntimeBuilder {
    working_dir: PathBuf,
    store_dir: PathBuf,
    task_store_config: Option<TaskStoreConfig>,
    session_store_config: Option<SessionStoreConfig>,
}
```

Modify constructors:

```rust
pub fn new(working_dir: PathBuf, store_dir: PathBuf) -> Self {
    Self { working_dir, store_dir, task_store_config: None, session_store_config: None }
}
```

Add builder method:

```rust
pub fn with_session_store_config(mut self, config: Option<SessionStoreConfig>) -> Self {
    self.session_store_config = config;
    self
}
```

- [ ] **Step 6: Add `session_manager` to `AgentRuntime`**

Add a field to `AgentRuntime`:

```rust
pub session_manager: Arc<dyn SessionManager>,
```

In `AgentRuntimeBuilder::build()`, create it after `task_store`:

```rust
let session_manager: Arc<dyn SessionManager> = match self.session_store_config.as_ref() {
    None => build_file_session_manager(&agents_root).await?,
    Some(config) if config.store_type == SessionStoreType::File => build_file_session_manager(&agents_root).await?,
    Some(config) if config.store_type == SessionStoreType::Database => {
        build_database_session_manager(config.required_url()?).await?
    }
    Some(_) => return Err("unsupported session store configuration".to_string()),
};
```

Add the field to the `Ok(AgentRuntime { ... })` construction:

```rust
session_manager,
```

Add helper functions near `build_database_task_store`:

```rust
async fn build_file_session_manager(
    agents_root: &std::path::Path,
) -> Result<Arc<dyn SessionManager>, String> {
    std::fs::create_dir_all(agents_root)
        .map_err(|e| format!("failed to create agents dir for session store: {e}"))?;
    Ok(Arc::new(FileSessionManager::new(agents_root)))
}

async fn build_database_session_manager(url: &str) -> Result<Arc<dyn SessionManager>, String> {
    let manager = DatabaseSessionManager::connect(url)
        .await
        .map_err(|e| format!("failed to create database session store: {e}"))?;
    Ok(Arc::new(manager))
}
```

- [ ] **Step 7: Use session manager when registering agents**

Find `AgentRuntime::register_agent()` where it creates `sessions_dir` and `FileSessionEntryStore`. Replace that block with:

```rust
let session_store = self.session_manager.entry_store_for_agent(&agent_type);
let session = Session::new(session_store);
```

Use the existing local variable that represents the public agent id/scope. If the function currently names it `agent_type`, keep the local name but pass the same value used as the JSON-RPC `agent_id` and directory name.

- [ ] **Step 8: Run runtime tests**

Run:

```bash
cargo test -p vol-llm-runtime session_store_config -- --nocapture
```

Expected: session config tests pass. The runtime SQLite build test may return early if no LLM provider is configured.

Run:

```bash
cargo test -p vol-llm-runtime
```

Expected: all runtime tests pass.

- [ ] **Step 9: Commit**

```bash
git add crates/vol-llm-runtime/src/lib.rs
git commit -m "feat(runtime): configure session store"
```

---

## Task 6: Wire session manager through server core and session JSON-RPC handler

**Files:**
- Modify: `crates/vol-llm-agent-channel/src/server_core.rs`
- Modify: `crates/vol-llm-agent-channel/src/domain/session.rs`
- Modify: `crates/vol-llm-agent-channel/src/agent_server_protocol.rs`
- Test: `crates/vol-llm-agent-channel/tests/jsonrpc_e2e_test.rs`

- [ ] **Step 1: Add a SQLite-backed JSON-RPC session test**

In `crates/vol-llm-agent-channel/tests/jsonrpc_e2e_test.rs`, add a test that mirrors the existing file-backed session test but builds `AgentServerCore` with database session config:

```rust
#[tokio::test]
async fn session_domain_works_with_sqlite_session_store() {
    let temp = tempfile::tempdir().unwrap();
    let db_url = format!("sqlite://{}", temp.path().join("sessions.db").display());

    let core = vol_llm_agent_channel::server_core::AgentServerCore::builder(
        temp.path(),
        temp.path().join("store"),
    )
    .with_session_store_config(Some(vol_llm_runtime::SessionStoreConfig {
        store_type: vol_llm_runtime::SessionStoreType::Database,
        url: Some(db_url),
    }))
    .build()
    .await;

    if let Err(err) = &core {
        if err.contains("No LLM provider configured") {
            return;
        }
    }
    let core = core.unwrap();

    let manager = core.runtime().session_manager.clone();
    let store = manager.entry_store_for_agent("alpha");
    store
        .save(vol_session::SessionEntry::new_summary(
            "session-a".to_string(),
            "database summary".to_string(),
        ))
        .await
        .unwrap();

    let request = vol_llm_agent_channel::agent_server_protocol::AgentServerMessage::new_command(
        "msg-1".to_string(),
        vol_llm_agent_channel::agent_server_protocol::Operation::Session(
            vol_llm_agent_channel::agent_server_protocol::SessionOperation::List,
        ),
        vol_llm_agent_channel::agent_server_protocol::Payload::Session(
            vol_llm_agent_channel::agent_server_protocol::SessionPayload::List {
                agent_id: Some("alpha".to_string()),
            },
        ),
    );

    let responses = core.dispatch(request).await.unwrap();
    let response = responses.into_iter().next().unwrap();
    match response.payload {
        vol_llm_agent_channel::agent_server_protocol::Payload::Session(
            vol_llm_agent_channel::agent_server_protocol::SessionPayload::ListResult { sessions },
        ) => {
            assert_eq!(sessions.len(), 1);
            assert_eq!(sessions[0]["agent_id"], "alpha");
            assert_eq!(sessions[0]["session_id"], "session-a");
            assert_eq!(sessions[0]["entry_count"], 1);
        }
        other => panic!("unexpected payload: {other:?}"),
    }
}
```


- [ ] **Step 2: Run the new test and verify missing builder method failure**

Run:

```bash
cargo test -p vol-llm-agent-channel session_domain_works_with_sqlite_session_store -- --nocapture
```

Expected: compile error for missing `with_session_store_config` in `AgentServerCoreBuilder`.

- [ ] **Step 3: Add session store config to `AgentServerCoreBuilder`**

Modify `crates/vol-llm-agent-channel/src/server_core.rs` imports to include `SessionStoreConfig` from runtime:

```rust
use vol_llm_runtime::{AgentRuntime, TaskStoreConfig, SessionStoreConfig};
```

Add field:

```rust
session_store_config: Option<SessionStoreConfig>,
```

Update `Default` and `new()`:

```rust
session_store_config: None,
```

Add builder method:

```rust
pub fn with_session_store_config(mut self, config: Option<SessionStoreConfig>) -> Self {
    self.session_store_config = config;
    self
}
```

Pass it into runtime build:

```rust
let runtime = AgentRuntime::builder(self.working_dir.clone(), self.store_dir.clone())
    .with_task_store_config(self.task_store_config.clone())
    .with_session_store_config(self.session_store_config.clone())
    .build()
    .await?;
```

Extract manager after runtime build:

```rust
let session_manager = runtime.session_manager.clone();
```

Register the session handler with the manager:

```rust
.register(Arc::new(SessionHandler::new(session_manager, router.clone())))
```

- [ ] **Step 4: Add an owned protocol error variant**

Modify `crates/vol-llm-agent-channel/src/agent_server_protocol.rs`:

```rust
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ProtocolError {
    #[error("unknown method: {0}")]
    UnknownMethod(String),
    #[error("payload decode failed for {0}")]
    PayloadDecodeFailed(&'static str),
    #[error("payload decode failed: {0}")]
    PayloadDecodeFailedOwned(String),
}
```

- [ ] **Step 5: Replace `SessionHandler` file scanning with manager calls**

Modify `crates/vol-llm-agent-channel/src/domain/session.rs` imports:

```rust
use std::sync::Arc;

use async_trait::async_trait;
use vol_session::{Session, SessionEntryStore, SessionManager};
```

Replace `SessionHandler` struct and constructor:

```rust
pub struct SessionHandler {
    session_manager: Arc<dyn SessionManager>,
    router: AgentRouter,
}

impl SessionHandler {
    pub fn new(session_manager: Arc<dyn SessionManager>, router: AgentRouter) -> Self {
        Self { session_manager, router }
    }
}
```

Replace the `session.list` branch body with:

```rust
let sessions = self
    .session_manager
    .list_sessions(agent_id.as_deref())
    .await
    .map_err(|e| ProtocolError::PayloadDecodeFailedOwned(format!("session.list failed: {e}")))?;

let all_sessions: Vec<serde_json::Value> = sessions
    .into_iter()
    .map(|s| {
        serde_json::json!({
            "id": s.id,
            "agent_id": s.agent_id,
            "session_id": s.session_id,
            "entry_count": s.entry_count,
            "created_at": s.created_at,
        })
    })
    .collect();

Ok(vec![AgentServerMessage::new_result(
    message.message_id,
    Operation::Session(SessionOperation::List),
    Payload::Session(SessionPayload::ListResult { sessions: all_sessions }),
)])
```

Replace the `session.resume` store resolution with:

```rust
let resolved_agent_id = match self
    .session_manager
    .resolve_session_agent(agent_id.as_deref(), &session_id)
    .await
{
    Ok(agent_id) => agent_id,
    Err(e) => {
        return Ok(vec![AgentServerMessage::new_error(
            message.message_id,
            Operation::Session(SessionOperation::Resume),
            crate::agent_server_protocol::ErrorPayload {
                code: "session_not_found".to_string(),
                message: format!("Session not found: {e}"),
                detail: None,
                terminal: true,
            },
        )]);
    }
};

let session_store = match self
    .session_manager
    .entry_store_for_session(Some(&resolved_agent_id), &session_id)
    .await
{
    Ok(store) => store,
    Err(e) => {
        return Ok(vec![AgentServerMessage::new_error(
            message.message_id,
            Operation::Session(SessionOperation::Resume),
            crate::agent_server_protocol::ErrorPayload {
                code: "session_not_found".to_string(),
                message: format!("Session not found: {e}"),
                detail: None,
                terminal: true,
            },
        )]);
    }
};
```

Use `session_store` for `get_entries` and `Session::resume` exactly as the existing code does.

Replace the `session.entries` store resolution with:

```rust
let store = match self
    .session_manager
    .entry_store_for_session(agent_id.as_deref(), &session_id)
    .await
{
    Ok(store) => store,
    Err(e) => {
        return Ok(vec![AgentServerMessage::new_error(
            message.message_id,
            Operation::Session(SessionOperation::Entries),
            crate::agent_server_protocol::ErrorPayload {
                code: "session_not_found".to_string(),
                message: format!("Session not found: {e}"),
                detail: None,
                terminal: true,
            },
        )]);
    }
};
```

- [ ] **Step 6: Run channel tests**

Run:

```bash
cargo test -p vol-llm-agent-channel session_domain -- --nocapture
```

Expected: session-domain tests pass.

Run:

```bash
cargo test -p vol-llm-agent-channel
```

Expected: all channel tests pass.

- [ ] **Step 7: Commit**

```bash
git add crates/vol-llm-agent-channel/src/server_core.rs crates/vol-llm-agent-channel/src/domain/session.rs crates/vol-llm-agent-channel/src/agent_server_protocol.rs crates/vol-llm-agent-channel/tests/jsonrpc_e2e_test.rs
git commit -m "feat(channel): use session manager for session domain"
```

---

## Task 7: Parse and pass session store config in `vol-agent-server`

**Files:**
- Modify: `crates/vol-agent-server/src/config.rs`
- Modify: `crates/vol-agent-server/src/main.rs`
- Modify: `config.vol-agent.example.toml`
- Test: `crates/vol-agent-server/src/config.rs`

- [ ] **Step 1: Add failing config tests**

In `crates/vol-agent-server/src/config.rs`, add tests near existing task store config tests:

```rust
#[test]
fn parses_database_session_store_config() {
    let toml = r#"
[runtime]
working_dir = "."
store_dir = ".vol-test"

[runtime.session_store]
type = "database"
url = "sqlite://data/sessions.db"
"#;
    let config: ServerConfig = toml::from_str(toml).unwrap();
    let session_store = config.runtime.session_store.unwrap();
    assert_eq!(session_store.store_type, vol_llm_runtime::SessionStoreType::Database);
    assert_eq!(session_store.url.as_deref(), Some("sqlite://data/sessions.db"));
}

#[test]
fn validates_session_store_config() {
    let toml = r#"
[runtime.session_store]
type = "database"
"#;
    let config: ServerConfig = toml::from_str(toml).unwrap();
    let err = config.validate().unwrap_err();
    assert!(err.contains("runtime.session_store.url is required"));
}
```

- [ ] **Step 2: Run config tests and verify missing field failure**

Run:

```bash
cargo test -p vol-agent-server session_store_config -- --nocapture
```

Expected: compile errors or assertion failures because `RuntimeSection` lacks `session_store`.

- [ ] **Step 3: Add config field and validation**

Modify `RuntimeSection`:

```rust
#[serde(default)]
pub session_store: Option<vol_llm_runtime::SessionStoreConfig>,
```

Modify `Default for RuntimeSection`:

```rust
session_store: None,
```

Modify `ServerConfig::validate()`:

```rust
if let Some(session_store) = &self.runtime.session_store {
    session_store.validate()?;
}
```

- [ ] **Step 4: Log and pass config in server main**

Modify `crates/vol-agent-server/src/main.rs` after task store logging:

```rust
if let Some(session_store) = &config.runtime.session_store {
    tracing::info!(session_store_type = ?session_store.store_type, "Using configured session store");
} else {
    tracing::info!("Using default file session store");
}
```

Modify builder chain:

```rust
let core = AgentServerCore::builder(&config.runtime.working_dir, &config.runtime.store_dir)
    .with_task_store_config(config.runtime.task_store.clone())
    .with_session_store_config(config.runtime.session_store.clone())
    .build()
```

- [ ] **Step 5: Document config examples**

Modify `config.vol-agent.example.toml` near `[runtime.task_store]` examples:

```toml
# Session store configuration.
# Defaults to file-backed JSONL sessions under the agent store directory.
[runtime.session_store]
type = "file"

# SQLite session database example:
# [runtime.session_store]
# type = "database"
# url = "sqlite://data/sessions.db"

# Postgres session database example:
# [runtime.session_store]
# type = "database"
# url = "postgres://vol_agent:vol_agent@localhost:5432/vol_agent_sessions"
```

- [ ] **Step 6: Run server tests**

Run:

```bash
cargo test -p vol-agent-server session_store_config -- --nocapture
```

Expected: session config tests pass.

Run:

```bash
cargo test -p vol-agent-server
```

Expected: all server tests pass.

- [ ] **Step 7: Commit**

```bash
git add crates/vol-agent-server/src/config.rs crates/vol-agent-server/src/main.rs config.vol-agent.example.toml
git commit -m "feat(server): parse session store config"
```

---

## Task 8: Workspace verification, docs wiki ingest, and final review

**Files:**
- Modify through wiki tooling: `docs/wiki/**`
- No source code changes unless verification reveals a defect.

- [ ] **Step 1: Run focused test suite**

Run:

```bash
cargo test -p vol-session
cargo test -p vol-llm-runtime
cargo test -p vol-llm-agent-channel
cargo test -p vol-agent-server
```

Expected: all tests pass.

- [ ] **Step 2: Run workspace check**

Run:

```bash
cargo check --workspace
```

Expected: workspace check passes.

- [ ] **Step 3: Run formatting**

Run:

```bash
cargo fmt --all --check
```

Expected: formatting check passes. If it fails, run:

```bash
cargo fmt --all
```

Then rerun:

```bash
cargo fmt --all --check
```

Expected: formatting check passes.

- [ ] **Step 4: Run clippy for touched crates**

Run:

```bash
cargo clippy -p vol-session -p vol-llm-runtime -p vol-llm-agent-channel -p vol-agent-server --all-targets -- -D warnings
```

Expected: clippy passes with no warnings.

- [ ] **Step 5: Ingest implementation into wiki**

Use the required project skill:

```text
/wiki-ingest Session database store implementation: vol-session DatabaseSessionEntryStore and SessionManager, runtime/server config, channel JSON-RPC integration, tests, and config examples.
```

Expected: `docs/wiki` updates to describe the session database store implementation and related entities/concepts.

- [ ] **Step 6: Commit wiki updates**

```bash
git status --short
git add docs/wiki
git commit -m "docs(wiki): ingest session database store"
```

Expected: wiki changes are committed. If `wiki-ingest` reports no wiki changes, do not create an empty commit.

- [ ] **Step 7: Review final git status and commit history**

Run:

```bash
git status --short
git log --oneline -8
```

Expected: no uncommitted changes, and recent commits include the session manager, database store, runtime config, channel wiring, server config, and wiki ingest commits.

- [ ] **Step 8: Request code review**

Use the required review skill before declaring complete:

```text
/superpowers:requesting-code-review Review the session database store implementation on the current branch.
```

Expected: reviewer findings are either addressed in follow-up commits or explicitly documented as non-issues with evidence.

- [ ] **Step 9: Final verification before completion**

Use the required verification skill before final completion:

```text
/superpowers:verification-before-completion Verify the session database store implementation is complete and tests pass.
```

Expected: final response cites the exact verification commands and results.
