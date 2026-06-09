# SeaORM Task Database Store Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the current SQLx task database store with a SeaORM + SeaORM Migration implementation that supports SQLite and Postgres while preserving the existing `TaskStore` API and global runtime store behavior.

**Architecture:** `vol-llm-task` keeps exporting `DatabaseTaskStore`, but its internals move from SQLx raw queries to private SeaORM entity, migration, and mapping modules. Runtime/server/channel wiring stays unchanged: `[runtime.task_store] type = "database"` still builds one global `runtime.task_store`, and the task tool plus JSON-RPC task handler share it.

**Tech Stack:** Rust, Tokio, SeaORM 1.x, SeaORM Migration 1.x, SQLite, mandatory Postgres test database configured by `VOL_AGENT_POSTGRES_TEST_URL` (for example `postgres://USER:PASSWORD@HOST:5432/DATABASE`), existing `TaskStore` trait.

---

## Scope and Boundaries

This plan implements the approved spec at `docs/superpowers/specs/2026-06-09-seaorm-task-database-store-design.md`.

Do not use a git worktree. Preserve the unrelated pre-existing change at `crates/vol-llm-ui/assets/tailwind.css`; do not stage or modify it.

This plan does not add `.agents/task-providers`, per-agent task stores, `tool_config.task`, MySQL support, UI store selection, multi-tenant stores, or file-store-to-database migration.

## File Structure

### Create

- `crates/vol-llm-task/src/stores/database/mod.rs` — public store implementation, backend inference, connection flow, `TaskStore` impl, shared SQLite/Postgres behavior tests.
- `crates/vol-llm-task/src/stores/database/entity.rs` — private SeaORM `tasks` entity.
- `crates/vol-llm-task/src/stores/database/migration.rs` — private SeaORM Rust migrator for `tasks` and `idx_tasks_status`.
- `crates/vol-llm-task/src/stores/database/mapping.rs` — private conversion helpers between `Task` and SeaORM models/active models.

### Delete

- `crates/vol-llm-task/src/stores/database.rs` — replaced by the `database/` module directory.
- `crates/vol-llm-task/migrations/sqlite/0001_create_tasks.sql` — replaced by SeaORM Rust migration.

### Modify

- `Cargo.toml` — replace workspace `sqlx` direct dependency with `sea-orm` and `sea-orm-migration` workspace dependencies.
- `Cargo.lock` — updated by Cargo.
- `crates/vol-llm-task/Cargo.toml` — replace direct `sqlx` dependency with `sea-orm` and `sea-orm-migration`.
- `crates/vol-llm-task/src/stores/mod.rs` — keep `mod database; pub use database::DatabaseTaskStore;` working after directory split.
- `crates/vol-llm-runtime/src/lib.rs` — keep existing SQLite runtime test and add mandatory Postgres runtime test.
- `config.vol-agent.example.toml` — update wording to say SQLite and Postgres are implemented; MySQL is recognized but not enabled.
- `docs/wiki/**` — update wiki after implementation using `wiki-ingest`.

---

### Task 1: Replace Dependencies and Create SeaORM Module Skeleton

**Files:**
- Modify: `Cargo.toml`
- Modify: `crates/vol-llm-task/Cargo.toml`
- Delete: `crates/vol-llm-task/src/stores/database.rs`
- Delete: `crates/vol-llm-task/migrations/sqlite/0001_create_tasks.sql`
- Create: `crates/vol-llm-task/src/stores/database/mod.rs`
- Create: `crates/vol-llm-task/src/stores/database/entity.rs`
- Create: `crates/vol-llm-task/src/stores/database/migration.rs`
- Create: `crates/vol-llm-task/src/stores/database/mapping.rs`

- [ ] **Step 1: Replace workspace database dependencies**

In root `Cargo.toml`, replace the current workspace `sqlx` line:

```toml
sqlx = { version = "0.8", default-features = false, features = ["runtime-tokio-rustls", "sqlite", "migrate", "macros"] }
```

with:

```toml
sea-orm = { version = "1", default-features = false, features = ["macros", "runtime-tokio-rustls", "sqlx-sqlite", "sqlx-postgres"] }
sea-orm-migration = { version = "1", default-features = false, features = ["runtime-tokio-rustls", "sqlx-sqlite", "sqlx-postgres"] }
```

In `crates/vol-llm-task/Cargo.toml`, replace:

```toml
sqlx = { workspace = true }
```

with:

```toml
sea-orm = { workspace = true }
sea-orm-migration = { workspace = true }
```

- [ ] **Step 2: Delete SQLx-specific files**

Run:

```bash
rm crates/vol-llm-task/src/stores/database.rs
rm crates/vol-llm-task/migrations/sqlite/0001_create_tasks.sql
rmdir crates/vol-llm-task/migrations/sqlite
rmdir crates/vol-llm-task/migrations
```

Expected: the old SQLx store file and migration directory are gone. If `rmdir` reports the directory is not empty, inspect it and remove only SQLx task-store migration files.

- [ ] **Step 3: Create initial SeaORM database module skeleton**

Create `crates/vol-llm-task/src/stores/database/mod.rs`:

```rust
//! SeaORM-backed database task store.

mod entity;
mod mapping;
mod migration;

use std::path::PathBuf;

use sea_orm::{ConnectOptions, Database, DatabaseConnection};
use sea_orm_migration::MigratorTrait;

use crate::store::{Result, StoreError};

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
            "unsupported task store database url scheme: <missing>".to_string(),
        )),
        other => Err(StoreError::Database(format!(
            "unsupported task store database url scheme: {other}"
        ))),
    }
}

pub struct DatabaseTaskStore {
    db: DatabaseConnection,
    backend: DatabaseBackend,
}

impl DatabaseTaskStore {
    pub async fn connect(url: &str) -> Result<Self> {
        match infer_backend(url)? {
            DatabaseBackend::Sqlite => Self::connect_backend(DatabaseBackend::Sqlite, normalize_sqlite_url(url)?).await,
            DatabaseBackend::Postgres => Self::connect_backend(DatabaseBackend::Postgres, url.to_string()).await,
            DatabaseBackend::MySql => Err(StoreError::Database(
                "database task store backend is recognized but not enabled yet: mysql".to_string(),
            )),
        }
    }

    async fn connect_backend(backend: DatabaseBackend, url: String) -> Result<Self> {
        if backend == DatabaseBackend::Sqlite {
            create_sqlite_parent_dir(&url)?;
        }

        let mut options = ConnectOptions::new(url);
        options.max_connections(5);
        let db = Database::connect(options)
            .await
            .map_err(|e| StoreError::Database(format!("failed to connect {} task store: {e}", backend.label())))?;

        migration::Migrator::up(&db, None)
            .await
            .map_err(|e| StoreError::Database(format!("failed to migrate {} task store: {e}", backend.label())))?;

        Ok(Self { db, backend })
    }
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

fn normalize_sqlite_url(url: &str) -> Result<String> {
    if url == "sqlite::memory:" || url == "sqlite://:memory:" {
        return Ok(url.to_string());
    }

    if !url.starts_with("sqlite:") {
        return Err(StoreError::Database(
            "sqlite task store url must start with sqlite:".to_string(),
        ));
    }

    if url.contains('?') {
        if url.contains("mode=") {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn infer_backend_from_sqlite_url() {
        assert_eq!(infer_backend("sqlite:///tmp/tasks.db").unwrap(), DatabaseBackend::Sqlite);
    }

    #[test]
    fn infer_backend_from_postgres_url() {
        assert_eq!(infer_backend("postgres://localhost/tasks").unwrap(), DatabaseBackend::Postgres);
        assert_eq!(infer_backend("postgresql://localhost/tasks").unwrap(), DatabaseBackend::Postgres);
    }

    #[test]
    fn infer_backend_from_mysql_url() {
        assert_eq!(infer_backend("mysql://localhost/tasks").unwrap(), DatabaseBackend::MySql);
    }

    #[test]
    fn infer_backend_rejects_unknown_url() {
        let err = infer_backend("oracle://localhost/tasks").unwrap_err();
        assert!(err.to_string().contains("unsupported task store database url scheme: oracle"));
    }

    #[test]
    fn normalize_sqlite_url_adds_create_mode() {
        assert_eq!(
            normalize_sqlite_url("sqlite:///tmp/tasks.db").unwrap(),
            "sqlite:///tmp/tasks.db?mode=rwc"
        );
        assert_eq!(
            normalize_sqlite_url("sqlite:///tmp/tasks.db?cache=shared").unwrap(),
            "sqlite:///tmp/tasks.db?cache=shared&mode=rwc"
        );
        assert_eq!(
            normalize_sqlite_url("sqlite:///tmp/tasks.db?mode=rwc").unwrap(),
            "sqlite:///tmp/tasks.db?mode=rwc"
        );
    }
}
```

- [ ] **Step 4: Create placeholder entity/migration/mapping files**

Create `crates/vol-llm-task/src/stores/database/entity.rs`:

```rust
//! SeaORM entity for persisted tasks.
```

Create `crates/vol-llm-task/src/stores/database/migration.rs`:

```rust
//! SeaORM migration for persisted tasks.

pub(super) struct Migrator;
```

Create `crates/vol-llm-task/src/stores/database/mapping.rs`:

```rust
//! Mapping between task models and SeaORM rows.
```

- [ ] **Step 5: Run skeleton tests and verify expected failure**

Run:

```bash
cargo test -p vol-llm-task stores::database::tests::infer_backend -- --nocapture
```

Expected: FAIL because `migration::Migrator` does not yet implement `MigratorTrait`, and `DatabaseTaskStore` no longer implements `TaskStore`.

- [ ] **Step 6: Temporarily implement empty migrator so skeleton tests compile**

Replace `crates/vol-llm-task/src/stores/database/migration.rs` with:

```rust
//! SeaORM migration for persisted tasks.

use sea_orm_migration::prelude::*;

pub(super) struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        Vec::new()
    }
}
```

Add a temporary placeholder `TaskStore` impl to the bottom of `database/mod.rs`, before tests:

```rust
#[async_trait::async_trait]
impl crate::store::TaskStore for DatabaseTaskStore {
    async fn create(&self, _task: crate::model::Task) -> Result<crate::model::TaskId> {
        Err(StoreError::Internal("SeaORM database task create is not implemented".to_string()))
    }

    async fn get(&self, _task_id: &crate::model::TaskId) -> Result<Option<crate::model::Task>> {
        Err(StoreError::Internal("SeaORM database task get is not implemented".to_string()))
    }

    async fn update(&self, _task: crate::model::Task) -> Result<()> {
        Err(StoreError::Internal("SeaORM database task update is not implemented".to_string()))
    }

    async fn delete(&self, _task_id: &crate::model::TaskId) -> Result<()> {
        Err(StoreError::Internal("SeaORM database task delete is not implemented".to_string()))
    }

    async fn list(&self, _status: Option<crate::model::TaskStatus>) -> Result<Vec<crate::model::Task>> {
        Err(StoreError::Internal("SeaORM database task list is not implemented".to_string()))
    }

    async fn get_ready_tasks(&self) -> Result<Vec<crate::model::TaskId>> {
        Err(StoreError::Internal("SeaORM database task ready query is not implemented".to_string()))
    }
}
```

- [ ] **Step 7: Run skeleton tests and check**

Run:

```bash
cargo test -p vol-llm-task stores::database::tests::infer_backend -- --nocapture
cargo check -p vol-llm-task
```

Expected: PASS with possible dead-code warnings for placeholder modules.

- [ ] **Step 8: Commit skeleton**

Run:

```bash
git add Cargo.toml Cargo.lock crates/vol-llm-task/Cargo.toml crates/vol-llm-task/src/stores/database crates/vol-llm-task/src/stores/database.rs crates/vol-llm-task/migrations
git commit -m "refactor(task): replace SQLx store skeleton with SeaORM" -m "Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 2: Add SeaORM Entity and Rust Migration

**Files:**
- Modify: `crates/vol-llm-task/src/stores/database/entity.rs`
- Modify: `crates/vol-llm-task/src/stores/database/migration.rs`
- Modify: `crates/vol-llm-task/src/stores/database/mod.rs`

- [ ] **Step 1: Write migration tests for SQLite and Postgres**

Add these tests to `crates/vol-llm-task/src/stores/database/mod.rs` inside the test module:

```rust
const POSTGRES_TEST_URL: &str = "postgres://USER:PASSWORD@HOST:5432/DATABASE";

async fn clear_store(store: &DatabaseTaskStore) {
    use sea_orm::{ConnectionTrait, Statement};
    let backend = match store.backend {
        DatabaseBackend::Sqlite => sea_orm::DatabaseBackend::Sqlite,
        DatabaseBackend::Postgres => sea_orm::DatabaseBackend::Postgres,
        DatabaseBackend::MySql => unreachable!("mysql is not enabled"),
    };
    store
        .db
        .execute(Statement::from_string(backend, "DELETE FROM tasks".to_string()))
        .await
        .unwrap();
}

#[tokio::test]
async fn sqlite_connect_runs_migration() {
    use sea_orm::{ConnectionTrait, Statement};
    let dir = tempfile::tempdir().unwrap();
    let url = format!("sqlite://{}", dir.path().join("tasks.db").display());
    let store = DatabaseTaskStore::connect(&url).await.unwrap();
    let rows = store
        .db
        .query_all(Statement::from_string(
            sea_orm::DatabaseBackend::Sqlite,
            "SELECT name FROM sqlite_master WHERE type = 'table' AND name = 'tasks'".to_string(),
        ))
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
}

#[tokio::test]
async fn postgres_connect_runs_migration() {
    use sea_orm::{ConnectionTrait, Statement};
    let store = DatabaseTaskStore::connect(POSTGRES_TEST_URL).await.unwrap();
    clear_store(&store).await;
    let rows = store
        .db
        .query_all(Statement::from_string(
            sea_orm::DatabaseBackend::Postgres,
            "SELECT table_name FROM information_schema.tables WHERE table_schema = 'public' AND table_name = 'tasks'".to_string(),
        ))
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
}
```

- [ ] **Step 2: Run migration tests and verify they fail**

Run:

```bash
cargo test -p vol-llm-task stores::database::tests::sqlite_connect_runs_migration stores::database::tests::postgres_connect_runs_migration -- --nocapture
```

Expected: FAIL because the migrator has no migrations and `tasks` is not created.

- [ ] **Step 3: Implement SeaORM entity**

Replace `crates/vol-llm-task/src/stores/database/entity.rs` with:

```rust
//! SeaORM entity for persisted tasks.

use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "tasks")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub status: String,
    pub kind: String,
    pub publisher: Option<String>,
    pub assignee: Option<String>,
    pub subject: String,
    pub description: String,
    pub active_form: Option<String>,
    pub dependencies_json: String,
    pub blocks_json: String,
    pub result_json: Option<String>,
    pub summary: Option<String>,
    pub output_file: Option<String>,
    pub created_at_secs: i64,
    pub started_at_secs: Option<i64>,
    pub completed_at_secs: Option<i64>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
```

- [ ] **Step 4: Implement SeaORM Rust migration**

Replace `crates/vol-llm-task/src/stores/database/migration.rs` with:

```rust
//! SeaORM migration for persisted tasks.

use sea_orm_migration::prelude::*;

pub(super) struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![Box::new(CreateTasks)]
    }
}

struct CreateTasks;

#[async_trait::async_trait]
impl MigrationTrait for CreateTasks {
    fn name(&self) -> &str {
        "m20260609_000001_create_tasks"
    }

    async fn up(&self, manager: &SchemaManager) -> std::result::Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Tasks::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Tasks::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Tasks::Status).string().not_null())
                    .col(ColumnDef::new(Tasks::Kind).string().not_null())
                    .col(ColumnDef::new(Tasks::Publisher).string())
                    .col(ColumnDef::new(Tasks::Assignee).string())
                    .col(ColumnDef::new(Tasks::Subject).string().not_null())
                    .col(ColumnDef::new(Tasks::Description).text().not_null())
                    .col(ColumnDef::new(Tasks::ActiveForm).string())
                    .col(ColumnDef::new(Tasks::DependenciesJson).text().not_null())
                    .col(ColumnDef::new(Tasks::BlocksJson).text().not_null())
                    .col(ColumnDef::new(Tasks::ResultJson).text())
                    .col(ColumnDef::new(Tasks::Summary).text())
                    .col(ColumnDef::new(Tasks::OutputFile).text())
                    .col(ColumnDef::new(Tasks::CreatedAtSecs).big_integer().not_null())
                    .col(ColumnDef::new(Tasks::StartedAtSecs).big_integer())
                    .col(ColumnDef::new(Tasks::CompletedAtSecs).big_integer())
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_tasks_status")
                    .if_not_exists()
                    .table(Tasks::Table)
                    .col(Tasks::Status)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> std::result::Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name("idx_tasks_status")
                    .table(Tasks::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await?;

        manager
            .drop_table(Table::drop().table(Tasks::Table).if_exists().to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum Tasks {
    Table,
    Id,
    Status,
    Kind,
    Publisher,
    Assignee,
    Subject,
    Description,
    ActiveForm,
    DependenciesJson,
    BlocksJson,
    ResultJson,
    Summary,
    OutputFile,
    CreatedAtSecs,
    StartedAtSecs,
    CompletedAtSecs,
}
```

- [ ] **Step 5: Run migration tests**

Run:

```bash
cargo test -p vol-llm-task stores::database::tests::sqlite_connect_runs_migration -- --exact --nocapture
cargo test -p vol-llm-task stores::database::tests::postgres_connect_runs_migration -- --exact --nocapture
```

Expected: both PASS. If Postgres fails to connect, stop and report the connectivity failure because Postgres is mandatory.

- [ ] **Step 6: Commit entity and migration**

Run:

```bash
git add crates/vol-llm-task/src/stores/database/entity.rs crates/vol-llm-task/src/stores/database/migration.rs crates/vol-llm-task/src/stores/database/mod.rs
git commit -m "feat(task): add SeaORM task entity and migration" -m "Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 3: Implement Mapping Layer With Conversion Tests

**Files:**
- Modify: `crates/vol-llm-task/src/stores/database/mapping.rs`
- Modify: `crates/vol-llm-task/src/stores/database/mod.rs`

- [ ] **Step 1: Add mapping tests**

Add these tests to `crates/vol-llm-task/src/stores/database/mod.rs` inside the test module:

```rust
#[test]
fn mapping_rejects_task_id_larger_than_i64() {
    let err = mapping::task_id_to_db(crate::model::TaskId(i64::MAX as u64 + 1)).unwrap_err();
    assert!(err.to_string().contains("task id exceeds database i64 range"));
}

#[test]
fn mapping_rejects_negative_database_id() {
    let err = mapping::task_id_from_db(-1).unwrap_err();
    assert!(err.to_string().contains("negative task id"));
}

#[test]
fn mapping_roundtrips_task_model_fields() {
    use crate::model::{Task, TaskKind, TaskResult, TaskStatus};
    use std::path::PathBuf;
    use std::time::{Duration, UNIX_EPOCH};

    let mut task = Task::new(TaskKind::Agent, "mapped".to_string(), vec![crate::model::TaskId(7)]);
    task.id = crate::model::TaskId(42);
    task.status = TaskStatus::Running;
    task.publisher = Some("publisher".to_string());
    task.assignee = Some("assignee".to_string());
    task.description = "description".to_string();
    task.active_form = Some("Mapping".to_string());
    task.blocks = vec![crate::model::TaskId(9)];
    task.result = Some(TaskResult {
        success: true,
        output_truncated: "ok".to_string(),
        output_file: PathBuf::from("/tmp/result.txt"),
    });
    task.summary = Some("summary".to_string());
    task.output_file = Some(PathBuf::from("/tmp/output.txt"));
    task.created_at = UNIX_EPOCH + Duration::from_secs(11);
    task.started_at = Some(UNIX_EPOCH + Duration::from_secs(12));
    task.completed_at = Some(UNIX_EPOCH + Duration::from_secs(13));

    let active = mapping::task_to_active_model(task.clone()).unwrap();
    let model = entity::Model {
        id: 42,
        status: active.status.unwrap(),
        kind: active.kind.unwrap(),
        publisher: active.publisher.unwrap(),
        assignee: active.assignee.unwrap(),
        subject: active.subject.unwrap(),
        description: active.description.unwrap(),
        active_form: active.active_form.unwrap(),
        dependencies_json: active.dependencies_json.unwrap(),
        blocks_json: active.blocks_json.unwrap(),
        result_json: active.result_json.unwrap(),
        summary: active.summary.unwrap(),
        output_file: active.output_file.unwrap(),
        created_at_secs: active.created_at_secs.unwrap(),
        started_at_secs: active.started_at_secs.unwrap(),
        completed_at_secs: active.completed_at_secs.unwrap(),
    };

    let roundtripped = mapping::model_to_task(model).unwrap();
    assert_eq!(roundtripped.id, task.id);
    assert_eq!(roundtripped.status, task.status);
    assert_eq!(roundtripped.subject, task.subject);
    assert_eq!(roundtripped.description, task.description);
    assert_eq!(roundtripped.dependencies, task.dependencies);
    assert_eq!(roundtripped.blocks, task.blocks);
    assert_eq!(roundtripped.result.unwrap().success, true);
    assert_eq!(roundtripped.summary, task.summary);
    assert_eq!(roundtripped.output_file, task.output_file);
    assert_eq!(roundtripped.created_at, task.created_at);
    assert_eq!(roundtripped.started_at, task.started_at);
    assert_eq!(roundtripped.completed_at, task.completed_at);
}
```

- [ ] **Step 2: Run mapping tests and verify they fail**

Run:

```bash
cargo test -p vol-llm-task stores::database::tests::mapping_ -- --nocapture
```

Expected: FAIL because mapping helpers do not exist.

- [ ] **Step 3: Implement mapping helpers**

Replace `crates/vol-llm-task/src/stores/database/mapping.rs` with:

```rust
//! Mapping between task models and SeaORM rows.

use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use sea_orm::ActiveValue::{NotSet, Set};

use crate::model::{Task, TaskId, TaskKind, TaskResult, TaskStatus};
use crate::store::{Result, StoreError};

use super::entity;

pub(super) fn task_id_to_db(id: TaskId) -> Result<i64> {
    i64::try_from(id.0).map_err(|_| {
        StoreError::Serialization(format!("task id exceeds database i64 range: {}", id.0))
    })
}

pub(super) fn task_id_from_db(id: i64) -> Result<TaskId> {
    let id = u64::try_from(id)
        .map_err(|_| StoreError::Serialization(format!("negative task id: {id}")))?;
    Ok(TaskId(id))
}

pub(super) fn status_to_db(status: TaskStatus) -> &'static str {
    match status {
        TaskStatus::Pending => "pending",
        TaskStatus::Running => "running",
        TaskStatus::Completed => "completed",
        TaskStatus::Failed => "failed",
        TaskStatus::Killed => "killed",
    }
}

fn status_from_db(value: &str) -> Result<TaskStatus> {
    match value {
        "pending" => Ok(TaskStatus::Pending),
        "running" => Ok(TaskStatus::Running),
        "completed" => Ok(TaskStatus::Completed),
        "failed" => Ok(TaskStatus::Failed),
        "killed" => Ok(TaskStatus::Killed),
        other => Err(StoreError::Serialization(format!("unknown task status: {other}"))),
    }
}

fn kind_to_db(kind: &TaskKind) -> &'static str {
    match kind {
        TaskKind::Agent => "agent",
        TaskKind::Manual => "manual",
    }
}

fn kind_from_db(value: &str) -> Result<TaskKind> {
    match value {
        "agent" => Ok(TaskKind::Agent),
        "manual" => Ok(TaskKind::Manual),
        other => Err(StoreError::Serialization(format!("unknown task kind: {other}"))),
    }
}

fn system_time_to_secs(value: SystemTime) -> Result<i64> {
    let duration = value
        .duration_since(UNIX_EPOCH)
        .map_err(|e| StoreError::Serialization(format!("task time is before unix epoch: {e}")))?;
    i64::try_from(duration.as_secs()).map_err(|_| {
        StoreError::Serialization(format!("task timestamp exceeds database i64 range: {}", duration.as_secs()))
    })
}

fn secs_to_system_time(value: i64) -> Result<SystemTime> {
    let secs = u64::try_from(value)
        .map_err(|_| StoreError::Serialization(format!("negative task timestamp: {value}")))?;
    Ok(UNIX_EPOCH + Duration::from_secs(secs))
}

fn option_time_to_secs(value: Option<SystemTime>) -> Result<Option<i64>> {
    value.map(system_time_to_secs).transpose()
}

fn option_secs_to_time(value: Option<i64>) -> Result<Option<SystemTime>> {
    value.map(secs_to_system_time).transpose()
}

fn task_ids_to_json(ids: &[TaskId]) -> Result<String> {
    serde_json::to_string(ids).map_err(|e| StoreError::Serialization(e.to_string()))
}

fn task_ids_from_json(value: &str) -> Result<Vec<TaskId>> {
    serde_json::from_str(value).map_err(|e| StoreError::Serialization(e.to_string()))
}

fn result_to_json(result: &Option<TaskResult>) -> Result<Option<String>> {
    result
        .as_ref()
        .map(serde_json::to_string)
        .transpose()
        .map_err(|e| StoreError::Serialization(e.to_string()))
}

fn result_from_json(value: Option<String>) -> Result<Option<TaskResult>> {
    value
        .map(|json| serde_json::from_str(&json))
        .transpose()
        .map_err(|e| StoreError::Serialization(e.to_string()))
}

fn path_to_db(path: &Option<PathBuf>) -> Option<String> {
    path.as_ref().map(|p| p.to_string_lossy().to_string())
}

fn path_from_db(value: Option<String>) -> Option<PathBuf> {
    value.map(PathBuf::from)
}

pub(super) fn task_to_active_model(task: Task) -> Result<entity::ActiveModel> {
    Ok(entity::ActiveModel {
        id: if task.id.0 == 0 { NotSet } else { Set(task_id_to_db(task.id)?) },
        status: Set(status_to_db(task.status).to_string()),
        kind: Set(kind_to_db(&task.kind).to_string()),
        publisher: Set(task.publisher),
        assignee: Set(task.assignee),
        subject: Set(task.subject),
        description: Set(task.description),
        active_form: Set(task.active_form),
        dependencies_json: Set(task_ids_to_json(&task.dependencies)?),
        blocks_json: Set(task_ids_to_json(&task.blocks)?),
        result_json: Set(result_to_json(&task.result)?),
        summary: Set(task.summary),
        output_file: Set(path_to_db(&task.output_file)),
        created_at_secs: Set(system_time_to_secs(task.created_at)?),
        started_at_secs: Set(option_time_to_secs(task.started_at)?),
        completed_at_secs: Set(option_time_to_secs(task.completed_at)?),
    })
}

pub(super) fn model_to_task(model: entity::Model) -> Result<Task> {
    Ok(Task {
        id: task_id_from_db(model.id)?,
        status: status_from_db(&model.status)?,
        kind: kind_from_db(&model.kind)?,
        publisher: model.publisher,
        assignee: model.assignee,
        subject: model.subject,
        description: model.description,
        active_form: model.active_form,
        dependencies: task_ids_from_json(&model.dependencies_json)?,
        blocks: task_ids_from_json(&model.blocks_json)?,
        result: result_from_json(model.result_json)?,
        summary: model.summary,
        output_file: path_from_db(model.output_file),
        created_at: secs_to_system_time(model.created_at_secs)?,
        started_at: option_secs_to_time(model.started_at_secs)?,
        completed_at: option_secs_to_time(model.completed_at_secs)?,
    })
}
```

- [ ] **Step 4: Expose mapping module to tests**

In `crates/vol-llm-task/src/stores/database/mod.rs`, keep `mod mapping;` as-is. Tests inside the same file can access private module items through `mapping::...`.

- [ ] **Step 5: Run mapping tests**

Run:

```bash
cargo test -p vol-llm-task stores::database::tests::mapping_ -- --nocapture
```

Expected: PASS.

- [ ] **Step 6: Commit mapping layer**

Run:

```bash
git add crates/vol-llm-task/src/stores/database/mapping.rs crates/vol-llm-task/src/stores/database/mod.rs
git commit -m "feat(task): add SeaORM task mapping" -m "Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 4: Implement SeaORM CRUD for SQLite and Postgres

**Files:**
- Modify: `crates/vol-llm-task/src/stores/database/mod.rs`

- [ ] **Step 1: Replace SQLite-only tests with backend matrix helpers**

In `crates/vol-llm-task/src/stores/database/mod.rs`, replace the existing store behavior tests with these helpers and tests:

```rust
async fn sqlite_store() -> (DatabaseTaskStore, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("tasks.db");
    let url = format!("sqlite://{}", db_path.display());
    let store = DatabaseTaskStore::connect(&url).await.unwrap();
    clear_store(&store).await;
    (store, dir)
}

async fn postgres_store() -> DatabaseTaskStore {
    let store = DatabaseTaskStore::connect(POSTGRES_TEST_URL).await.unwrap();
    clear_store(&store).await;
    store
}

async fn assert_create_get(store: &DatabaseTaskStore) {
    use crate::model::{Task, TaskKind, TaskStatus};
    let mut task = Task::new(TaskKind::Agent, "database task".to_string(), vec![]);
    task.description = "stored with seaorm".to_string();
    task.publisher = Some("planner".to_string());
    task.assignee = Some("worker".to_string());
    task.active_form = Some("Working".to_string());

    let id = store.create(task).await.unwrap();
    assert!(id.0 > 0);

    let got = store.get(&id).await.unwrap().unwrap();
    assert_eq!(got.id, id);
    assert_eq!(got.subject, "database task");
    assert_eq!(got.description, "stored with seaorm");
    assert_eq!(got.publisher.as_deref(), Some("planner"));
    assert_eq!(got.assignee.as_deref(), Some("worker"));
    assert_eq!(got.active_form.as_deref(), Some("Working"));
    assert_eq!(got.status, TaskStatus::Pending);
}

#[tokio::test]
async fn sqlite_create_assigns_id_and_get_retrieves_task() {
    let (store, _dir) = sqlite_store().await;
    assert_create_get(&store).await;
}

#[tokio::test]
async fn postgres_create_assigns_id_and_get_retrieves_task() {
    let store = postgres_store().await;
    assert_create_get(&store).await;
}
```

- [ ] **Step 2: Run create/get tests and verify they fail**

Run:

```bash
cargo test -p vol-llm-task create_assigns_id_and_get_retrieves_task -- --nocapture
```

Expected: FAIL because the placeholder `create`/`get` methods still return `Internal`.

- [ ] **Step 3: Implement create/get/list/update/delete using SeaORM**

Replace the placeholder `TaskStore` impl in `crates/vol-llm-task/src/stores/database/mod.rs` with:

```rust
#[async_trait::async_trait]
impl crate::store::TaskStore for DatabaseTaskStore {
    async fn create(&self, task: crate::model::Task) -> Result<crate::model::TaskId> {
        use sea_orm::ActiveModelTrait;
        let mut active = mapping::task_to_active_model(task)?;
        active.id = sea_orm::ActiveValue::NotSet;
        let inserted = active
            .insert(&self.db)
            .await
            .map_err(|e| StoreError::Database(format!("failed to create task: {e}")))?;
        mapping::task_id_from_db(inserted.id)
    }

    async fn get(&self, task_id: &crate::model::TaskId) -> Result<Option<crate::model::Task>> {
        use sea_orm::EntityTrait;
        let id = mapping::task_id_to_db(*task_id)?;
        let model = entity::Entity::find_by_id(id)
            .one(&self.db)
            .await
            .map_err(|e| StoreError::Database(format!("failed to get task {}: {e}", task_id)))?;
        model.map(mapping::model_to_task).transpose()
    }

    async fn update(&self, task: crate::model::Task) -> Result<()> {
        use sea_orm::{ActiveModelTrait, EntityTrait};
        let id = mapping::task_id_to_db(task.id)?;
        let exists = entity::Entity::find_by_id(id)
            .one(&self.db)
            .await
            .map_err(|e| StoreError::Database(format!("failed to get task {} for update: {e}", task.id)))?;
        if exists.is_none() {
            return Err(StoreError::NotFound(format!("Task {}", task.id)));
        }

        mapping::task_to_active_model(task.clone())?
            .update(&self.db)
            .await
            .map_err(|e| StoreError::Database(format!("failed to update task {}: {e}", task.id)))?;
        Ok(())
    }

    async fn delete(&self, task_id: &crate::model::TaskId) -> Result<()> {
        use sea_orm::EntityTrait;
        let id = mapping::task_id_to_db(*task_id)?;
        entity::Entity::delete_by_id(id)
            .exec(&self.db)
            .await
            .map_err(|e| StoreError::Database(format!("failed to delete task {}: {e}", task_id)))?;
        Ok(())
    }

    async fn list(&self, status: Option<crate::model::TaskStatus>) -> Result<Vec<crate::model::Task>> {
        use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
        let mut query = entity::Entity::find().order_by_asc(entity::Column::Id);
        if let Some(status) = status {
            query = query.filter(entity::Column::Status.eq(mapping::status_to_db(status)));
        }
        let models = query
            .all(&self.db)
            .await
            .map_err(|e| StoreError::Database(format!("failed to list tasks: {e}")))?;
        models.into_iter().map(mapping::model_to_task).collect()
    }

    async fn get_ready_tasks(&self) -> Result<Vec<crate::model::TaskId>> {
        Err(StoreError::Internal("SeaORM database task ready query is not implemented".to_string()))
    }
}
```

- [ ] **Step 4: Add update/delete/list matrix tests**

Add these helper tests to the test module:

```rust
async fn assert_update_delete_list(store: &DatabaseTaskStore) {
    use crate::model::{Task, TaskKind, TaskResult, TaskStatus};
    use std::path::PathBuf;
    use std::time::SystemTime;

    let id1 = store
        .create(Task::new(TaskKind::Agent, "first".to_string(), vec![]))
        .await
        .unwrap();
    let id2 = store
        .create(Task::new(TaskKind::Manual, "second".to_string(), vec![]))
        .await
        .unwrap();

    let mut second = store.get(&id2).await.unwrap().unwrap();
    second.status = TaskStatus::Completed;
    second.subject = "updated".to_string();
    second.description = "done".to_string();
    second.summary = Some("summary".to_string());
    second.output_file = Some(PathBuf::from("/tmp/output.txt"));
    second.result = Some(TaskResult {
        success: true,
        output_truncated: "ok".to_string(),
        output_file: PathBuf::from("/tmp/result.txt"),
    });
    second.completed_at = Some(SystemTime::now());
    store.update(second).await.unwrap();

    let updated = store.get(&id2).await.unwrap().unwrap();
    assert_eq!(updated.status, TaskStatus::Completed);
    assert_eq!(updated.subject, "updated");
    assert_eq!(updated.summary.as_deref(), Some("summary"));
    assert_eq!(updated.result.as_ref().unwrap().success, true);

    let all = store.list(None).await.unwrap();
    assert_eq!(all.iter().map(|task| task.id).collect::<Vec<_>>(), vec![id1, id2]);
    let completed = store.list(Some(TaskStatus::Completed)).await.unwrap();
    assert_eq!(completed.len(), 1);
    assert_eq!(completed[0].id, id2);

    store.delete(&id1).await.unwrap();
    assert!(store.get(&id1).await.unwrap().is_none());
    store.delete(&id1).await.unwrap();
}

#[tokio::test]
async fn sqlite_update_delete_list() {
    let (store, _dir) = sqlite_store().await;
    assert_update_delete_list(&store).await;
}

#[tokio::test]
async fn postgres_update_delete_list() {
    let store = postgres_store().await;
    assert_update_delete_list(&store).await;
}
```

- [ ] **Step 5: Run CRUD tests**

Run:

```bash
cargo test -p vol-llm-task create_assigns_id_and_get_retrieves_task -- --nocapture
cargo test -p vol-llm-task update_delete_list -- --nocapture
```

Expected: PASS for both SQLite and Postgres variants.

- [ ] **Step 6: Commit CRUD implementation**

Run:

```bash
git add crates/vol-llm-task/src/stores/database/mod.rs
git commit -m "feat(task): implement SeaORM task CRUD" -m "Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 5: Implement Ready Tasks and Persistence Matrix

**Files:**
- Modify: `crates/vol-llm-task/src/stores/database/mod.rs`

- [ ] **Step 1: Add ready-task and persistence matrix tests**

Add these helpers/tests to `crates/vol-llm-task/src/stores/database/mod.rs`:

```rust
async fn assert_ready_tasks(store: &DatabaseTaskStore) {
    use crate::model::{Task, TaskKind, TaskStatus};

    let dependency_id = store
        .create(Task::new(TaskKind::Agent, "dependency".to_string(), vec![]))
        .await
        .unwrap();
    let blocked_id = store
        .create(Task::new(TaskKind::Agent, "blocked".to_string(), vec![dependency_id]))
        .await
        .unwrap();

    let ready = store.get_ready_tasks().await.unwrap();
    assert_eq!(ready, vec![dependency_id]);
    assert!(!ready.contains(&blocked_id));

    let mut dependency = store.get(&dependency_id).await.unwrap().unwrap();
    dependency.status = TaskStatus::Completed;
    store.update(dependency).await.unwrap();

    let ready = store.get_ready_tasks().await.unwrap();
    assert_eq!(ready, vec![blocked_id]);
}

#[tokio::test]
async fn sqlite_ready_tasks() {
    let (store, _dir) = sqlite_store().await;
    assert_ready_tasks(&store).await;
}

#[tokio::test]
async fn postgres_ready_tasks() {
    let store = postgres_store().await;
    assert_ready_tasks(&store).await;
}

#[tokio::test]
async fn sqlite_tasks_persist_across_reconnect() {
    use crate::model::{Task, TaskKind};
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("tasks.db");
    let url = format!("sqlite://{}", db_path.display());
    let store = DatabaseTaskStore::connect(&url).await.unwrap();
    clear_store(&store).await;
    let id = store.create(Task::new(TaskKind::Agent, "persisted".to_string(), vec![])).await.unwrap();
    drop(store);

    let reopened = DatabaseTaskStore::connect(&url).await.unwrap();
    let got = reopened.get(&id).await.unwrap().unwrap();
    assert_eq!(got.subject, "persisted");
}

#[tokio::test]
async fn postgres_tasks_persist_across_reconnect() {
    use crate::model::{Task, TaskKind};
    let store = DatabaseTaskStore::connect(POSTGRES_TEST_URL).await.unwrap();
    clear_store(&store).await;
    let id = store.create(Task::new(TaskKind::Agent, "persisted pg".to_string(), vec![])).await.unwrap();
    drop(store);

    let reopened = DatabaseTaskStore::connect(POSTGRES_TEST_URL).await.unwrap();
    let got = reopened.get(&id).await.unwrap().unwrap();
    assert_eq!(got.subject, "persisted pg");
    clear_store(&reopened).await;
}
```

- [ ] **Step 2: Run ready-task tests and verify they fail**

Run:

```bash
cargo test -p vol-llm-task ready_tasks -- --nocapture
```

Expected: FAIL because `get_ready_tasks` still returns `Internal`.

- [ ] **Step 3: Implement ready-task behavior**

Replace `get_ready_tasks` in the `TaskStore` impl with:

```rust
async fn get_ready_tasks(&self) -> Result<Vec<crate::model::TaskId>> {
    let tasks = self.list(None).await?;
    let completed_ids: std::collections::HashSet<crate::model::TaskId> = tasks
        .iter()
        .filter(|task| task.status == crate::model::TaskStatus::Completed)
        .map(|task| task.id)
        .collect();

    let ready = tasks
        .iter()
        .filter(|task| {
            task.status == crate::model::TaskStatus::Pending
                && task.dependencies.iter().all(|id| completed_ids.contains(id))
        })
        .map(|task| task.id)
        .collect();

    Ok(ready)
}
```

- [ ] **Step 4: Run database store tests**

Run:

```bash
cargo test -p vol-llm-task stores::database -- --nocapture
```

Expected: PASS for SQLite and Postgres tests.

- [ ] **Step 5: Run full task crate tests**

Run:

```bash
cargo test -p vol-llm-task
```

Expected: PASS.

- [ ] **Step 6: Commit ready/persistence behavior**

Run:

```bash
git add crates/vol-llm-task/src/stores/database/mod.rs
git commit -m "feat(task): support SeaORM ready tasks and persistence" -m "Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 6: Add Postgres Runtime Test and Update Config Example

**Files:**
- Modify: `crates/vol-llm-runtime/src/lib.rs`
- Modify: `config.vol-agent.example.toml`

- [ ] **Step 1: Add Postgres runtime builder test**

In `crates/vol-llm-runtime/src/lib.rs`, add this test inside the existing `#[cfg(test)] mod tests`:

```rust
#[tokio::test]
async fn builder_accepts_postgres_database_task_store_config() {
    let temp = tempfile::tempdir().unwrap();
    let providers_dir = temp.path().join(".agents/providers");
    std::fs::create_dir_all(&providers_dir).unwrap();
    std::fs::write(
        providers_dir.join("test.toml"),
        r#"
provider = "anthropic"
model = "claude-test"
api_key = "sk-test"
base_url = "https://api.test.com"
"#,
    )
    .unwrap();

    let config = TaskStoreConfig {
        store_type: TaskStoreType::Database,
        url: Some("postgres://USER:PASSWORD@HOST:5432/DATABASE".to_string()),
    };

    let runtime = AgentRuntime::builder(temp.path(), temp.path())
        .with_task_store_config(Some(config.clone()))
        .build()
        .await
        .expect("runtime should build with postgres database task store config");

    let mut task = vol_llm_task::Task::new(
        vol_llm_task::TaskKind::Manual,
        "runtime postgres database task store test".to_string(),
        Vec::new(),
    );
    task.description = "created through AgentRuntime::task_store using postgres".to_string();
    let task_id = runtime
        .task_store
        .create(task)
        .await
        .expect("postgres database task store should create tasks");
    drop(runtime);

    let runtime = AgentRuntime::builder(temp.path(), temp.path())
        .with_task_store_config(Some(config))
        .build()
        .await
        .expect("runtime should reconnect to postgres database task store");
    let persisted = runtime
        .task_store
        .get(&task_id)
        .await
        .expect("postgres database task store should get tasks")
        .expect("created task should persist across runtime rebuilds");

    assert_eq!(persisted.id, task_id);
    assert_eq!(persisted.subject, "runtime postgres database task store test");
}
```

- [ ] **Step 2: Run Postgres runtime test**

Run:

```bash
cargo test -p vol-llm-runtime tests::builder_accepts_postgres_database_task_store_config -- --exact --nocapture
```

Expected: PASS. If Postgres is unreachable, stop and report the connectivity failure.

- [ ] **Step 3: Update example config wording**

In `config.vol-agent.example.toml`, replace:

```toml
# Database store. The concrete database is inferred from the URL scheme.
# Currently SQLite is implemented; postgres/postgresql and mysql schemes are
# recognized for clear errors until those backends are added.
# [runtime.task_store]
# type = "database"
# url = "sqlite://./data/tasks.db"
```

with:

```toml
# Database store. The concrete database is inferred from the URL scheme.
# SQLite and Postgres are implemented; mysql is recognized for a clear
# not-enabled error until that backend is added.
# [runtime.task_store]
# type = "database"
# url = "sqlite://./data/tasks.db"
#
# Postgres example:
# [runtime.task_store]
# type = "database"
# url = "postgres://USER:PASSWORD@HOST:5432/DATABASE"
```

- [ ] **Step 4: Run runtime and server checks**

Run:

```bash
cargo test -p vol-llm-runtime tests::builder_accepts_database_task_store_config_until_provider_requirement -- --exact --nocapture
cargo test -p vol-llm-runtime tests::builder_accepts_postgres_database_task_store_config -- --exact --nocapture
cargo check -p vol-llm-runtime -p vol-llm-agent-channel -p vol-agent-server
```

Expected: PASS.

- [ ] **Step 5: Commit runtime/config updates**

Run:

```bash
git add crates/vol-llm-runtime/src/lib.rs config.vol-agent.example.toml
git commit -m "test(task): verify SeaORM task store in runtime" -m "Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 7: Final Verification and Wiki Ingest

**Files:**
- Modify through skill: `docs/wiki/**`

- [ ] **Step 1: Run final focused test suite**

Run:

```bash
cargo test -p vol-llm-task
cargo test -p vol-agent-server
cargo test -p vol-llm-runtime tests::builder_accepts_database_task_store_config_until_provider_requirement -- --exact --nocapture
cargo test -p vol-llm-runtime tests::builder_accepts_postgres_database_task_store_config -- --exact --nocapture
cargo check -p vol-llm-task -p vol-llm-runtime -p vol-llm-agent-channel -p vol-agent-server
```

Expected: all commands PASS. Postgres tests must not be skipped.

- [ ] **Step 2: Confirm no direct SQLx use remains in vol-llm-task**

Run:

```bash
rg "sqlx|SqlitePool|SqliteConnectOptions|sqlx::" crates/vol-llm-task Cargo.toml crates/vol-llm-task/Cargo.toml
```

Expected: no direct `vol-llm-task` SQLx usage. It is acceptable if `Cargo.lock` still contains SQLx transitively through SeaORM.

- [ ] **Step 3: Review working tree**

Run:

```bash
git status --short
```

Expected: only intentional SeaORM/database-store files are modified, plus the unrelated pre-existing `crates/vol-llm-ui/assets/tailwind.css` which must remain untouched.

- [ ] **Step 4: Ingest wiki updates**

Invoke the `wiki-ingest` skill with this source set:

```text
Ingest the SeaORM task database store replacement from:
- docs/superpowers/specs/2026-06-09-seaorm-task-database-store-design.md
- docs/superpowers/plans/2026-06-09-seaorm-task-database-store.md
- crates/vol-llm-task/src/stores/database/mod.rs
- crates/vol-llm-task/src/stores/database/entity.rs
- crates/vol-llm-task/src/stores/database/migration.rs
- crates/vol-llm-task/src/stores/database/mapping.rs
- crates/vol-llm-runtime/src/lib.rs
- config.vol-agent.example.toml

Context: SQLx direct task-store implementation was replaced with SeaORM + SeaORM Migration. SQLite and Postgres are implemented. Postgres tests use the mandatory `VOL_AGENT_POSTGRES_TEST_URL` environment variable (for example `postgres://USER:PASSWORD@HOST:5432/DATABASE`). MySQL remains recognized but not enabled. Single global runtime.task_store semantics are unchanged.
```

- [ ] **Step 5: Commit wiki updates**

Run:

```bash
git add docs/wiki
git commit -m "docs(wiki): ingest SeaORM task database store" -m "Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

- [ ] **Step 6: Final status**

Run:

```bash
git status --short
```

Expected: no implementation changes remain except unrelated pre-existing `crates/vol-llm-ui/assets/tailwind.css` if it was still present before this work.

---

## Self-Review

- Spec coverage: covered dependency replacement, SeaORM module split, entity, Rust migration, SQLite/Postgres connection behavior, MySQL not-enabled behavior, mapping semantics, CRUD, ready-task behavior, mandatory Postgres tests, runtime Postgres test, config docs, wiki updates, and final direct-SQLx search.
- Placeholder scan: no TBD/TODO/fill-in steps remain. Every code-changing step includes concrete code.
- Type consistency: `DatabaseTaskStore`, `DatabaseBackend`, `TaskStoreConfig`, `TaskStoreType`, `entity::Model`, and mapping helper names are consistent across tasks.
