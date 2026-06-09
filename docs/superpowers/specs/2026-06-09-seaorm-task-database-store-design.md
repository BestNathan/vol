# SeaORM Task Database Store Design

## Summary

Replace the current SQLx-based `DatabaseTaskStore` implementation with a SeaORM-based implementation that supports SQLite and Postgres while preserving the existing runtime and tool-facing behavior.

The runtime continues to own exactly one global task store. The `task` tool, scheduler, JSON-RPC task handler, and UI-facing task APIs all share `runtime.task_store`. This design does not add per-agent task stores, `.agents/task-providers`, or `tool_config.task` overrides.

## Goals

- Replace direct SQLx usage in `vol-llm-task` with SeaORM and SeaORM Migration.
- Keep `TaskStore`, `DatabaseTaskStore::connect(url)`, and `[runtime.task_store]` configuration stable.
- Support SQLite and Postgres database task stores.
- Keep MySQL recognized but not enabled.
- Run SeaORM Rust migrations automatically during `DatabaseTaskStore::connect`.
- Preserve current task schema semantics: scalar columns plus JSON text for dependencies, blocks, and result.
- Keep the existing single global runtime task store model.
- Require Postgres tests to read the mandatory URL from `VOL_AGENT_POSTGRES_TEST_URL` (for example `postgres://USER:PASSWORD@HOST:5432/DATABASE`).

## Non-goals

- No `.agents/task-providers` directory.
- No per-agent task store routing.
- No `tool_config.task.provider` or `tool_config.task.store` support.
- No UI selector for multiple task stores.
- No multi-tenant task store model.
- No automatic file-store-to-database migration.
- No MySQL implementation in this change.

## Configuration Contract

The external server/runtime config remains unchanged:

```toml
[runtime.task_store]
type = "database"
url = "sqlite://./data/tasks.db"
```

Postgres uses the same contract:

```toml
[runtime.task_store]
type = "database"
url = "postgres://USER:PASSWORD@HOST:5432/DATABASE"
```

Omitting `[runtime.task_store]` still selects the file task store. Explicit file store configuration remains valid:

```toml
[runtime.task_store]
type = "file"
```

The URL scheme still selects the database backend:

- `sqlite://...` -> SQLite
- `postgres://...` or `postgresql://...` -> Postgres
- `mysql://...` -> recognized but not enabled yet

## Dependency Changes

Remove direct SQLx usage from `vol-llm-task` and replace it with SeaORM dependencies.

Workspace dependencies should include:

```toml
sea-orm = { version = "1", default-features = false, features = [
  "macros",
  "runtime-tokio-rustls",
  "sqlx-sqlite",
  "sqlx-postgres",
] }

sea-orm-migration = { version = "1", default-features = false, features = [
  "runtime-tokio-rustls",
  "sqlx-sqlite",
  "sqlx-postgres",
] }
```

`crates/vol-llm-task/Cargo.toml` should depend on:

```toml
sea-orm = { workspace = true }
sea-orm-migration = { workspace = true }
```

SeaORM may pull SQLx transitively as its driver implementation. `vol-llm-task` should no longer call SQLx APIs directly.

## Module Structure

Replace the current monolithic SQLx implementation with a focused private database module:

```text
crates/vol-llm-task/src/stores/database/
  mod.rs
  entity.rs
  migration.rs
  mapping.rs
```

Public API remains unchanged through `stores/mod.rs` and the crate root:

```rust
pub use database::DatabaseTaskStore;
```

Responsibilities:

- `mod.rs`: `DatabaseTaskStore`, backend inference, connection flow, `TaskStore` implementation.
- `entity.rs`: SeaORM `tasks` entity.
- `migration.rs`: SeaORM Rust migrator for the `tasks` table and status index.
- `mapping.rs`: conversions between `Task` and SeaORM `Model`/`ActiveModel` fields.

If implementation remains small enough, these can be private submodules under the database directory. Entity and migration types should not become part of the public API.

## Store Structure

`DatabaseTaskStore` should store a SeaORM connection and inferred backend:

```rust
pub struct DatabaseTaskStore {
    db: sea_orm::DatabaseConnection,
    backend: DatabaseBackend,
}
```

`backend` is used for backend-specific behavior such as SQLite parent directory creation and Postgres cleanup in tests. Runtime callers should not depend on it.

## Connection Flow

`DatabaseTaskStore::connect(url)` should:

1. Infer backend from URL scheme.
2. For SQLite:
   - Preserve current support for file SQLite URLs and in-memory URLs.
   - Create the parent directory for file-backed SQLite URLs before connecting.
   - Connect through SeaORM.
   - Run SeaORM migrations automatically.
3. For Postgres:
   - Connect through SeaORM to the existing database.
   - Do not create the database itself.
   - Run SeaORM migrations automatically.
4. For MySQL:
   - Return `StoreError::Database("database task store backend is recognized but not enabled yet: mysql")`.
5. Return `DatabaseTaskStore` after successful migration.

Connection and migration errors must include operation/backend context but must not print full URLs with credentials.

## Entity Design

The SeaORM `tasks` entity should match the current schema semantics:

```rust
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
```

Use `i64` for database ids because SQLite and Postgres auto-incrementing integer ids are signed 64-bit values. Convert to and from `TaskId(pub u64)` in the mapping layer with explicit checks.

## Migration Design

Use SeaORM Rust migration inside `vol-llm-task`.

The migration should create `tasks` if it does not exist:

- `id`: big integer, auto-increment, primary key
- `status`: string/text, required
- `kind`: string/text, required
- `publisher`: nullable string/text
- `assignee`: nullable string/text
- `subject`: string/text, required
- `description`: text, required
- `active_form`: nullable string/text
- `dependencies_json`: text, required
- `blocks_json`: text, required
- `result_json`: nullable text
- `summary`: nullable text
- `output_file`: nullable text
- `created_at_secs`: big integer, required
- `started_at_secs`: nullable big integer
- `completed_at_secs`: nullable big integer

Create `idx_tasks_status` on `status` if it does not exist.

Use SeaORM/SeaQuery abstractions rather than backend-specific SQL strings. `auto_increment()` should generate the appropriate SQLite/Postgres SQL.

Delete the old SQLx migration file:

```text
crates/vol-llm-task/migrations/sqlite/0001_create_tasks.sql
```

SeaORM Rust migration replaces it and is compiled into the crate as Rust code.

## Data Mapping

Preserve current mapping semantics:

- `TaskStatus` ↔ lowercase string:
  - `pending`
  - `running`
  - `completed`
  - `failed`
  - `killed`
- `TaskKind` ↔ lowercase string:
  - `agent`
  - `manual`
- `Vec<TaskId>` ↔ JSON string for `dependencies_json` and `blocks_json`.
- `TaskResult` ↔ JSON string for `result_json`.
- `PathBuf` ↔ string for output paths.
- `SystemTime` ↔ epoch seconds stored as `i64`.

Add explicit id conversion checks:

- When converting `TaskId` to DB id, reject values greater than `i64::MAX`.
- When converting DB id to `TaskId`, reject negative values.

## CRUD Semantics

`DatabaseTaskStore` must preserve the current `TaskStore` behavior.

### create

- Build an `ActiveModel` with `id` unset.
- Insert through SeaORM.
- Return `TaskId(inserted_model.id as u64)` after checked conversion.

### get

- Convert `TaskId` to DB id.
- Use `Entity::find_by_id(id).one(&db).await`.
- Return `None` if not found.

### update

- Convert `TaskId` to DB id.
- Check that the row exists.
- If missing, return `StoreError::NotFound(format!("Task {}", task.id))`.
- Replace all mutable persisted fields with values from the provided `Task`.

### delete

- Delete by id.
- Missing rows return `Ok(())`, matching current file/database behavior.

### list

- Query all tasks ordered by id ascending.
- If a status filter is provided, filter by `Column::Status.eq(status_to_db(status))`.

### get_ready_tasks

- Call `list(None)`.
- Collect completed task ids.
- Return pending task ids whose dependencies are all completed.
- Keep this in Rust for parity with the existing stores.

## Postgres Support

Postgres is a first-class supported backend in this change.

- Supported schemes: `postgres://` and `postgresql://`.
- Test URL comes from `VOL_AGENT_POSTGRES_TEST_URL` (for example `postgres://USER:PASSWORD@HOST:5432/DATABASE`).
- The database must already exist.
- Migration runs automatically on connect.
- JSON-like fields remain text columns for SQLite/Postgres compatibility.
- Runtime errors must not print the full URL because it contains credentials.

## Test Strategy

### Store Tests

Every database-store behavior test must run for both SQLite and Postgres.

SQLite setup:

- Use `tempfile::TempDir`.
- Use `sqlite://<tempdir>/tasks.db`.

Postgres setup:

- Read the mandatory URL from `VOL_AGENT_POSTGRES_TEST_URL` (for example `postgres://USER:PASSWORD@HOST:5432/DATABASE`).
- Postgres is mandatory. Tests must fail if the database is unreachable.
- Clean the `tasks` table before each Postgres test after running migrations. A simple `DELETE FROM tasks` is sufficient because this table is only for task-store tests.

Required behavior coverage for both backends:

- infer backend from URL schemes.
- create assigns id and get retrieves task.
- update persists status, result, summary, output file, and timestamps.
- delete removes task and missing delete succeeds.
- list returns tasks in id order and filters status.
- get_ready_tasks returns pending tasks without dependencies.
- get_ready_tasks excludes incomplete dependencies.
- get_ready_tasks includes a task after dependencies complete.
- tasks persist across reconnect.

### Runtime Tests

Keep the existing runtime builder SQLite persistence test.

Add a Postgres runtime builder test using `VOL_AGENT_POSTGRES_TEST_URL` (for example `postgres://USER:PASSWORD@HOST:5432/DATABASE`) that:

1. Creates valid fake provider config.
2. Builds runtime with Postgres database task store config.
3. Creates a task through `runtime.task_store`.
4. Drops runtime.
5. Rebuilds runtime with the same Postgres URL.
6. Reads the same task back.

### Config Tests

Existing server config tests remain valid. They do not need to know SeaORM internals.

## Error Handling

Keep `StoreError::Database(String)`.

Do not add `#[from] sea_orm::DbErr`; each operation should add context:

```rust
.map_err(|e| StoreError::Database(format!("failed to create task: {e}")))?
```

Connection errors should include backend context but redact credentials. Avoid formatting the full URL in error messages.

Examples:

- `failed to connect sqlite task store: ...`
- `failed to connect postgres task store: ...`
- `failed to migrate postgres task store: ...`
- `database task store backend is recognized but not enabled yet: mysql`

## Documentation Updates

Update docs/wiki pages that currently describe SQLx:

- `docs/wiki/entities/vol-llm-task-crate.md`
- `docs/wiki/sources/task-database-store-implementation.md`
- `docs/wiki/sources/task-store-sqlite-embedded-migrations.md`
- `docs/wiki/concepts/runtime-task-store-configuration.md`
- `docs/wiki/index.md`
- `docs/wiki/log.md`

The new docs should say SeaORM manages SQLite/Postgres stores and SeaORM Rust migrations are compiled into `vol-llm-task`.

Update `config.vol-agent.example.toml` only if wording still says SQLite is the only implemented backend. It should say SQLite and Postgres are implemented, MySQL is recognized but not enabled.

## Compatibility

Existing runtime configuration remains compatible.

Existing SQLite database files created by the SQLx implementation can continue to work because the table shape is preserved and SeaORM migration uses `if_not_exists` for the table and index.

No automatic data conversion is required.
