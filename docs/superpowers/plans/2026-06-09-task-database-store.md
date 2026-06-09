# Task Database Store Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a single global SQLx-backed database task store selected by `[runtime.task_store] type = "database"` while preserving the existing file-store default.

**Architecture:** `vol-llm-task` gains a `DatabaseTaskStore` that implements the existing `TaskStore` trait. `vol-llm-runtime` owns task store configuration and builds exactly one global `Arc<dyn TaskStore>`, and `vol-agent-server` parses `[runtime.task_store]` and passes it through `AgentServerCoreBuilder` into `AgentRuntimeBuilder`.

**Tech Stack:** Rust, Tokio, SQLx with SQLite migrations, Serde/TOML config, existing `TaskStore` trait, existing JSON-RPC task handler.

---

## Scope and Boundaries

This plan implements the approved spec at `docs/superpowers/specs/2026-06-09-task-database-store-design.md`.

The plan deliberately does not add `.agents/task-providers`, per-agent task stores, `tool_config.task` routing, UI store selection, multi-tenant stores, or file-to-database migration.

## File Structure

### Create

- `crates/vol-llm-task/src/stores/database.rs` — SQLx-backed `DatabaseTaskStore`, SQLite URL validation, migrations, row-to-task mapping, and `TaskStore` implementation.
- `crates/vol-llm-task/migrations/sqlite/0001_create_tasks.sql` — SQLite schema for persisted tasks.

### Modify

- `Cargo.toml` — add workspace `sqlx` dependency.
- `crates/vol-llm-task/Cargo.toml` — depend on workspace `sqlx`.
- `crates/vol-llm-task/src/store.rs` — add `Database` error variant.
- `crates/vol-llm-task/src/stores/mod.rs` — export `DatabaseTaskStore`.
- `crates/vol-llm-runtime/src/lib.rs` — define runtime task store config, add builder option, construct file or database store.
- `crates/vol-llm-agent-channel/src/server_core.rs` — add builder pass-through for task store config.
- `crates/vol-agent-server/Cargo.toml` — add direct `vol-llm-runtime` dependency for config types.
- `crates/vol-agent-server/src/config.rs` — parse and validate `[runtime.task_store]`.
- `crates/vol-agent-server/src/main.rs` — pass parsed task store config to `AgentServerCoreBuilder`.
- `config.vol-agent.example.toml` — document file and database task store config.
- `docs/wiki` — ingest the implementation summary after code is complete by running the project-required `wiki-ingest` skill.

---

### Task 1: Add Runtime Task Store Config Parsing

**Files:**
- Modify: `crates/vol-llm-runtime/src/lib.rs`
- Modify: `crates/vol-agent-server/Cargo.toml`
- Modify: `crates/vol-agent-server/src/config.rs`

- [ ] **Step 1: Add a failing config parse test for database store**

Add this test to `crates/vol-agent-server/src/config.rs` inside the existing `#[cfg(test)] mod tests` block:

```rust
#[test]
fn test_parse_database_task_store_config() {
    let toml_str = r#"
[runtime]
working_dir = "/app"
store_dir = "/data"

[runtime.task_store]
type = "database"
url = "sqlite:///tmp/vol-agent/tasks.db"
"#;

    let config: ServerConfig = toml::from_str(toml_str).unwrap();
    let task_store = config.runtime.task_store.as_ref().unwrap();
    assert_eq!(task_store.store_type, vol_llm_runtime::TaskStoreType::Database);
    assert_eq!(task_store.url.as_deref(), Some("sqlite:///tmp/vol-agent/tasks.db"));
}
```

- [ ] **Step 2: Run the config test and verify it fails**

Run:

```bash
cargo test -p vol-agent-server config::tests::test_parse_database_task_store_config -- --exact
```

Expected: FAIL because `RuntimeSection` has no `task_store` field and `vol-agent-server` does not yet depend on `vol-llm-runtime`.

- [ ] **Step 3: Add SQL-independent config types in runtime**

In `crates/vol-llm-runtime/src/lib.rs`, add these public types near the builder section, before `pub struct AgentRuntimeBuilder`:

```rust
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TaskStoreType {
    File,
    Database,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct TaskStoreConfig {
    #[serde(rename = "type")]
    pub store_type: TaskStoreType,
    pub url: Option<String>,
}

impl TaskStoreConfig {
    pub fn validate(&self) -> Result<(), String> {
        match self.store_type {
            TaskStoreType::File => {
                if self.url.is_some() {
                    return Err("runtime.task_store.url is not valid when type = \"file\"".to_string());
                }
                Ok(())
            }
            TaskStoreType::Database => {
                let url = self
                    .url
                    .as_deref()
                    .ok_or_else(|| "runtime.task_store.url is required when type = \"database\"".to_string())?;
                validate_database_url_scheme(url)
            }
        }
    }

    pub fn required_url(&self) -> Result<&str, String> {
        self.url
            .as_deref()
            .ok_or_else(|| "runtime.task_store.url is required when type = \"database\"".to_string())
    }
}

pub fn validate_database_url_scheme(url: &str) -> Result<(), String> {
    let scheme = url
        .split_once(':')
        .map(|(scheme, _)| scheme)
        .unwrap_or_default();

    match scheme {
        "sqlite" | "postgres" | "postgresql" | "mysql" => Ok(()),
        "" => Err("unsupported task store database url scheme: <missing>".to_string()),
        other => Err(format!("unsupported task store database url scheme: {other}")),
    }
}
```

- [ ] **Step 4: Add runtime dependency to server crate**

In `crates/vol-agent-server/Cargo.toml`, add:

```toml
vol-llm-runtime = { path = "../vol-llm-runtime" }
```

The dependency block should include both `vol-llm-agent-channel` and `vol-llm-runtime`.

- [ ] **Step 5: Add `task_store` to server runtime config**

In `crates/vol-agent-server/src/config.rs`, change `RuntimeSection` to:

```rust
#[derive(Debug, Clone, Deserialize)]
pub struct RuntimeSection {
    #[serde(default = "default_working_dir")]
    pub working_dir: String,
    #[serde(default = "default_store_dir")]
    pub store_dir: String,
    #[serde(default)]
    pub task_store: Option<vol_llm_runtime::TaskStoreConfig>,
}
```

Update the `Default for RuntimeSection` implementation to include:

```rust
task_store: None,
```

- [ ] **Step 6: Validate task store config on load**

In `crates/vol-agent-server/src/config.rs`, add this method inside `impl ServerConfig`:

```rust
pub fn validate(&self) -> Result<(), String> {
    if let Some(task_store) = &self.runtime.task_store {
        task_store.validate()?;
    }
    Ok(())
}
```

Change `ServerConfig::load` to validate parsed config:

```rust
pub fn load(path: &std::path::Path) -> Result<Self, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read config file {:?}: {}", path, e))?;
    let config: Self = toml::from_str(&content)
        .map_err(|e| format!("Failed to parse config {:?}: {}", path, e))?;
    config.validate()?;
    Ok(config)
}
```

- [ ] **Step 7: Run the config test and verify it passes**

Run:

```bash
cargo test -p vol-agent-server config::tests::test_parse_database_task_store_config -- --exact
```

Expected: PASS.

- [ ] **Step 8: Add validation tests for rejected configs**

Add these tests to `crates/vol-agent-server/src/config.rs`:

```rust
#[test]
fn test_database_task_store_requires_url() {
    let toml_str = r#"
[runtime.task_store]
type = "database"
"#;

    let config: ServerConfig = toml::from_str(toml_str).unwrap();
    let err = config.validate().unwrap_err();
    assert_eq!(err, "runtime.task_store.url is required when type = \"database\"");
}

#[test]
fn test_file_task_store_rejects_url() {
    let toml_str = r#"
[runtime.task_store]
type = "file"
url = "sqlite:///tmp/tasks.db"
"#;

    let config: ServerConfig = toml::from_str(toml_str).unwrap();
    let err = config.validate().unwrap_err();
    assert_eq!(err, "runtime.task_store.url is not valid when type = \"file\"");
}

#[test]
fn test_database_task_store_rejects_unknown_scheme() {
    let toml_str = r#"
[runtime.task_store]
type = "database"
url = "oracle://localhost/tasks"
"#;

    let config: ServerConfig = toml::from_str(toml_str).unwrap();
    let err = config.validate().unwrap_err();
    assert_eq!(err, "unsupported task store database url scheme: oracle");
}
```

- [ ] **Step 9: Run all server config tests**

Run:

```bash
cargo test -p vol-agent-server config::tests -- --nocapture
```

Expected: PASS.

- [ ] **Step 10: Commit config parsing**

Run:

```bash
git add crates/vol-llm-runtime/src/lib.rs crates/vol-agent-server/Cargo.toml crates/vol-agent-server/src/config.rs
git commit -m "feat(task): parse runtime task store config" -m "Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 2: Wire Task Store Config Through Server Core and Runtime Builder

**Files:**
- Modify: `crates/vol-llm-runtime/src/lib.rs`
- Modify: `crates/vol-llm-agent-channel/src/server_core.rs`
- Modify: `crates/vol-agent-server/src/main.rs`

- [ ] **Step 1: Add builder storage for optional task store config**

In `crates/vol-llm-runtime/src/lib.rs`, change `AgentRuntimeBuilder` to:

```rust
pub struct AgentRuntimeBuilder {
    working_dir: PathBuf,
    store_dir: PathBuf,
    task_store_config: Option<TaskStoreConfig>,
}
```

Change `AgentRuntimeBuilder::new` to:

```rust
pub fn new(working_dir: PathBuf, store_dir: PathBuf) -> Self {
    Self { working_dir, store_dir, task_store_config: None }
}
```

Add this method inside `impl AgentRuntimeBuilder`:

```rust
pub fn with_task_store_config(mut self, config: Option<TaskStoreConfig>) -> Self {
    self.task_store_config = config;
    self
}
```

- [ ] **Step 2: Add a helper that still creates the file store**

In `crates/vol-llm-runtime/src/lib.rs`, add this private helper near `expand_tilde`:

```rust
async fn build_file_task_store(store_dir: &std::path::Path) -> Result<Arc<dyn TaskStore>, String> {
    let tasks_dir = store_dir.join("tasks");
    std::fs::create_dir_all(&tasks_dir)
        .map_err(|e| format!("failed to create tasks dir: {e}"))?;
    let store = FileTaskStore::new(&tasks_dir)
        .await
        .map_err(|e| format!("failed to create file task store: {e}"))?;
    Ok(Arc::new(store))
}
```

- [ ] **Step 3: Replace inline file-store construction with the helper**

In `AgentRuntimeBuilder::build`, replace the existing `tasks_dir` and `FileTaskStore::new` block with:

```rust
let task_store: Arc<dyn TaskStore> = match self.task_store_config.as_ref() {
    None => build_file_task_store(&store_dir).await?,
    Some(config) if config.store_type == TaskStoreType::File => build_file_task_store(&store_dir).await?,
    Some(config) if config.store_type == TaskStoreType::Database => {
        return Err(format!(
            "database task store is not implemented yet for url scheme: {}",
            config.required_url()?.split_once(':').map(|(scheme, _)| scheme).unwrap_or("<missing>")
        ));
    }
    Some(_) => return Err("unsupported task store configuration".to_string()),
};
```

This intentionally keeps database as a clear runtime error until `DatabaseTaskStore` exists in a later task.

- [ ] **Step 4: Add task store config to AgentServerCoreBuilder**

In `crates/vol-llm-agent-channel/src/server_core.rs`, update the imports:

```rust
use vol_llm_runtime::{AgentRuntime, TaskStoreConfig};
```

Change `AgentServerCoreBuilder` to:

```rust
pub struct AgentServerCoreBuilder {
    working_dir: PathBuf,
    store_dir: PathBuf,
    task_store_config: Option<TaskStoreConfig>,
    extra_handlers: Vec<Arc<dyn crate::domain::handler::DomainHandler>>,
}
```

Update `Default` and `new` initializers to set `task_store_config: None`.

Add this builder method:

```rust
pub fn with_task_store_config(mut self, config: Option<TaskStoreConfig>) -> Self {
    self.task_store_config = config;
    self
}
```

- [ ] **Step 5: Pass builder config into AgentRuntimeBuilder**

In `AgentServerCoreBuilder::build`, replace:

```rust
let runtime = AgentRuntime::builder(self.working_dir.clone(), self.store_dir.clone())
    .build()
    .await?;
```

with:

```rust
let runtime = AgentRuntime::builder(self.working_dir.clone(), self.store_dir.clone())
    .with_task_store_config(self.task_store_config.clone())
    .build()
    .await?;
```

- [ ] **Step 6: Pass parsed config from server main**

In `crates/vol-agent-server/src/main.rs`, replace:

```rust
let core = AgentServerCore::new(&config.runtime.working_dir, &config.runtime.store_dir)
    .await
    .unwrap_or_else(|e| {
        tracing::error!("Failed to build AgentServerCore: {}", e);
        std::process::exit(1);
    });
```

with:

```rust
let core = AgentServerCore::builder(&config.runtime.working_dir, &config.runtime.store_dir)
    .with_task_store_config(config.runtime.task_store.clone())
    .build()
    .await
    .unwrap_or_else(|e| {
        tracing::error!("Failed to build AgentServerCore: {}", e);
        std::process::exit(1);
    });
```

- [ ] **Step 7: Run compile checks for runtime and server core**

Run:

```bash
cargo check -p vol-llm-runtime -p vol-llm-agent-channel -p vol-agent-server
```

Expected: PASS.

- [ ] **Step 8: Commit builder wiring**

Run:

```bash
git add crates/vol-llm-runtime/src/lib.rs crates/vol-llm-agent-channel/src/server_core.rs crates/vol-agent-server/src/main.rs
git commit -m "feat(task): wire task store config into runtime" -m "Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 3: Add SQLx Dependency, Migration, and Database Store Skeleton

**Files:**
- Modify: `Cargo.toml`
- Modify: `crates/vol-llm-task/Cargo.toml`
- Modify: `crates/vol-llm-task/src/store.rs`
- Modify: `crates/vol-llm-task/src/stores/mod.rs`
- Create: `crates/vol-llm-task/src/stores/database.rs`
- Create: `crates/vol-llm-task/migrations/sqlite/0001_create_tasks.sql`

- [ ] **Step 1: Add SQLx workspace dependency**

In root `Cargo.toml` under `[workspace.dependencies]`, add:

```toml
sqlx = { version = "0.8", default-features = false, features = ["runtime-tokio-rustls", "sqlite", "migrate"] }
```

In `crates/vol-llm-task/Cargo.toml`, add:

```toml
sqlx = { workspace = true }
```

- [ ] **Step 2: Add database error conversion**

In `crates/vol-llm-task/src/store.rs`, add this variant to `StoreError`:

```rust
#[error("Database error: {0}")]
Database(String),
```

Do not use `#[from] sqlx::Error` here because callers should redact URLs and add store-specific context before converting errors.

- [ ] **Step 3: Create SQLite migration**

Create `crates/vol-llm-task/migrations/sqlite/0001_create_tasks.sql` with:

```sql
CREATE TABLE IF NOT EXISTS tasks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    status TEXT NOT NULL,
    kind TEXT NOT NULL,
    publisher TEXT NULL,
    assignee TEXT NULL,
    subject TEXT NOT NULL,
    description TEXT NOT NULL,
    active_form TEXT NULL,
    dependencies_json TEXT NOT NULL,
    blocks_json TEXT NOT NULL,
    result_json TEXT NULL,
    summary TEXT NULL,
    output_file TEXT NULL,
    created_at_secs INTEGER NOT NULL,
    started_at_secs INTEGER NULL,
    completed_at_secs INTEGER NULL
);

CREATE INDEX IF NOT EXISTS idx_tasks_status ON tasks(status);
```

- [ ] **Step 4: Write a failing URL validation test**

Create `crates/vol-llm-task/src/stores/database.rs` with this initial content:

```rust
//! SQLx-backed database task store.

use crate::store::{Result, StoreError, TaskStore};

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

pub struct DatabaseTaskStore;

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
}
```

- [ ] **Step 5: Export the database module**

In `crates/vol-llm-task/src/stores/mod.rs`, change it to:

```rust
//! TaskStore implementations.

mod database;
mod file;
mod memory;

pub use database::DatabaseTaskStore;
pub use file::FileTaskStore;
pub use memory::InMemoryTaskStore;
```

- [ ] **Step 6: Run the database URL tests**

Run:

```bash
cargo test -p vol-llm-task stores::database::tests::infer_backend -- --nocapture
```

Expected: PASS.

- [ ] **Step 7: Commit skeleton**

Run:

```bash
git add Cargo.toml crates/vol-llm-task/Cargo.toml crates/vol-llm-task/src/store.rs crates/vol-llm-task/src/stores/mod.rs crates/vol-llm-task/src/stores/database.rs crates/vol-llm-task/migrations/sqlite/0001_create_tasks.sql
git commit -m "feat(task): add database store skeleton" -m "Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 4: Implement SQLite DatabaseTaskStore CRUD

**Files:**
- Modify: `crates/vol-llm-task/src/stores/database.rs`

- [ ] **Step 1: Add failing create/get test**

Replace the test module in `crates/vol-llm-task/src/stores/database.rs` with this expanded test module, keeping the existing URL tests and adding async store tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Task, TaskKind, TaskStatus};

    async fn temp_store() -> (DatabaseTaskStore, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("tasks.db");
        let url = format!("sqlite://{}", db_path.display());
        let store = DatabaseTaskStore::connect(&url).await.unwrap();
        (store, dir)
    }

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

    #[tokio::test]
    async fn create_assigns_id_and_get_retrieves_task() {
        let (store, _dir) = temp_store().await;
        let mut task = Task::new(TaskKind::Agent, "database task".to_string(), vec![]);
        task.description = "stored in sqlite".to_string();
        task.publisher = Some("planner".to_string());
        task.assignee = Some("worker".to_string());
        task.active_form = Some("Working".to_string());

        let id = store.create(task).await.unwrap();
        assert_eq!(id.0, 1);

        let got = store.get(&id).await.unwrap().unwrap();
        assert_eq!(got.id, id);
        assert_eq!(got.subject, "database task");
        assert_eq!(got.description, "stored in sqlite");
        assert_eq!(got.publisher.as_deref(), Some("planner"));
        assert_eq!(got.assignee.as_deref(), Some("worker"));
        assert_eq!(got.active_form.as_deref(), Some("Working"));
        assert_eq!(got.status, TaskStatus::Pending);
    }
}
```

- [ ] **Step 2: Run the new test and verify it fails**

Run:

```bash
cargo test -p vol-llm-task stores::database::tests::create_assigns_id_and_get_retrieves_task -- --exact
```

Expected: FAIL because `DatabaseTaskStore::connect`, `create`, and `get` are not implemented.

- [ ] **Step 3: Implement SQLite connection, migrations, and row mapping**

Replace the non-test content of `crates/vol-llm-task/src/stores/database.rs` with:

```rust
//! SQLx-backed database task store.

use std::path::PathBuf;
use std::str::FromStr;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use sqlx::migrate::Migrator;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Row, SqlitePool};

use crate::model::{Task, TaskId, TaskKind, TaskResult, TaskStatus};
use crate::store::{Result, StoreError, TaskStore};

static SQLITE_MIGRATOR: Migrator = sqlx::migrate!("./migrations/sqlite");

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
    pool: SqlitePool,
}

impl DatabaseTaskStore {
    pub async fn connect(url: &str) -> Result<Self> {
        match infer_backend(url)? {
            DatabaseBackend::Sqlite => Self::connect_sqlite(url).await,
            DatabaseBackend::Postgres => Err(StoreError::Database(
                "database task store backend is recognized but not enabled yet: postgres".to_string(),
            )),
            DatabaseBackend::MySql => Err(StoreError::Database(
                "database task store backend is recognized but not enabled yet: mysql".to_string(),
            )),
        }
    }

    async fn connect_sqlite(url: &str) -> Result<Self> {
        create_sqlite_parent_dir(url)?;
        let options = SqliteConnectOptions::from_str(url)
            .map_err(|e| StoreError::Database(format!("invalid sqlite task store url: {e}")))?
            .create_if_missing(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(options)
            .await
            .map_err(|e| StoreError::Database(format!("failed to connect sqlite task store: {e}")))?;
        SQLITE_MIGRATOR
            .run(&pool)
            .await
            .map_err(|e| StoreError::Database(format!("failed to migrate sqlite task store: {e}")))?;
        Ok(Self { pool })
    }
}

fn create_sqlite_parent_dir(url: &str) -> Result<()> {
    let path = sqlite_file_path(url);
    if let Some(path) = path {
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
    let raw = url.strip_prefix("sqlite://").or_else(|| url.strip_prefix("sqlite:"))?;
    if raw.is_empty() || raw == ":memory:" {
        return None;
    }
    Some(PathBuf::from(raw))
}

fn status_to_db(status: TaskStatus) -> &'static str {
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
    Ok(duration.as_secs() as i64)
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

fn row_to_task(row: &sqlx::sqlite::SqliteRow) -> Result<Task> {
    let id: i64 = row.get("id");
    let status: String = row.get("status");
    let kind: String = row.get("kind");
    let dependencies_json: String = row.get("dependencies_json");
    let blocks_json: String = row.get("blocks_json");
    let result_json: Option<String> = row.get("result_json");
    let created_at_secs: i64 = row.get("created_at_secs");
    let started_at_secs: Option<i64> = row.get("started_at_secs");
    let completed_at_secs: Option<i64> = row.get("completed_at_secs");
    let output_file: Option<String> = row.get("output_file");

    Ok(Task {
        id: TaskId(id as u64),
        status: status_from_db(&status)?,
        kind: kind_from_db(&kind)?,
        publisher: row.get("publisher"),
        assignee: row.get("assignee"),
        subject: row.get("subject"),
        description: row.get("description"),
        active_form: row.get("active_form"),
        dependencies: task_ids_from_json(&dependencies_json)?,
        blocks: task_ids_from_json(&blocks_json)?,
        result: result_from_json(result_json)?,
        summary: row.get("summary"),
        output_file: path_from_db(output_file),
        created_at: secs_to_system_time(created_at_secs)?,
        started_at: option_secs_to_time(started_at_secs)?,
        completed_at: option_secs_to_time(completed_at_secs)?,
    })
}
```

- [ ] **Step 4: Implement create/get**

Append this trait implementation before the test module in `crates/vol-llm-task/src/stores/database.rs`:

```rust
#[async_trait::async_trait]
impl TaskStore for DatabaseTaskStore {
    async fn create(&self, task: Task) -> Result<TaskId> {
        let dependencies_json = task_ids_to_json(&task.dependencies)?;
        let blocks_json = task_ids_to_json(&task.blocks)?;
        let result_json = result_to_json(&task.result)?;
        let created_at_secs = system_time_to_secs(task.created_at)?;
        let started_at_secs = option_time_to_secs(task.started_at)?;
        let completed_at_secs = option_time_to_secs(task.completed_at)?;
        let output_file = path_to_db(&task.output_file);

        let result = sqlx::query(
            r#"
            INSERT INTO tasks (
                status, kind, publisher, assignee, subject, description, active_form,
                dependencies_json, blocks_json, result_json, summary, output_file,
                created_at_secs, started_at_secs, completed_at_secs
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
            "#,
        )
        .bind(status_to_db(task.status))
        .bind(kind_to_db(&task.kind))
        .bind(task.publisher)
        .bind(task.assignee)
        .bind(task.subject)
        .bind(task.description)
        .bind(task.active_form)
        .bind(dependencies_json)
        .bind(blocks_json)
        .bind(result_json)
        .bind(task.summary)
        .bind(output_file)
        .bind(created_at_secs)
        .bind(started_at_secs)
        .bind(completed_at_secs)
        .execute(&self.pool)
        .await
        .map_err(|e| StoreError::Database(format!("failed to create task: {e}")))?;

        Ok(TaskId(result.last_insert_rowid() as u64))
    }

    async fn get(&self, task_id: &TaskId) -> Result<Option<Task>> {
        let row = sqlx::query("SELECT * FROM tasks WHERE id = ?1")
            .bind(task_id.0 as i64)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| StoreError::Database(format!("failed to get task {}: {e}", task_id)))?;

        row.as_ref().map(row_to_task).transpose()
    }

    async fn update(&self, _task: Task) -> Result<()> {
        Err(StoreError::Internal("database task update is not implemented".to_string()))
    }

    async fn delete(&self, _task_id: &TaskId) -> Result<()> {
        Err(StoreError::Internal("database task delete is not implemented".to_string()))
    }

    async fn list(&self, _status: Option<TaskStatus>) -> Result<Vec<Task>> {
        Err(StoreError::Internal("database task list is not implemented".to_string()))
    }

    async fn get_ready_tasks(&self) -> Result<Vec<TaskId>> {
        Err(StoreError::Internal("database task ready query is not implemented".to_string()))
    }
}
```

- [ ] **Step 5: Run create/get test**

Run:

```bash
cargo test -p vol-llm-task stores::database::tests::create_assigns_id_and_get_retrieves_task -- --exact
```

Expected: PASS.

- [ ] **Step 6: Add update/delete/list tests**

Add these tests to the same test module:

```rust
#[tokio::test]
async fn update_persists_all_mutable_fields() {
    let (store, _dir) = temp_store().await;
    let id = store
        .create(Task::new(TaskKind::Agent, "original".to_string(), vec![]))
        .await
        .unwrap();

    let mut task = store.get(&id).await.unwrap().unwrap();
    task.status = TaskStatus::Completed;
    task.subject = "updated".to_string();
    task.description = "done".to_string();
    task.summary = Some("summary".to_string());
    task.output_file = Some(PathBuf::from("/tmp/output.txt"));
    task.result = Some(crate::model::TaskResult {
        success: true,
        output_truncated: "ok".to_string(),
        output_file: PathBuf::from("/tmp/result.txt"),
    });
    task.completed_at = Some(SystemTime::now());

    store.update(task).await.unwrap();

    let got = store.get(&id).await.unwrap().unwrap();
    assert_eq!(got.status, TaskStatus::Completed);
    assert_eq!(got.subject, "updated");
    assert_eq!(got.description, "done");
    assert_eq!(got.summary.as_deref(), Some("summary"));
    assert_eq!(got.output_file.as_deref(), Some(std::path::Path::new("/tmp/output.txt")));
    assert_eq!(got.result.as_ref().unwrap().success, true);
    assert!(got.completed_at.is_some());
}

#[tokio::test]
async fn delete_removes_task_and_missing_delete_succeeds() {
    let (store, _dir) = temp_store().await;
    let id = store
        .create(Task::new(TaskKind::Agent, "delete".to_string(), vec![]))
        .await
        .unwrap();

    store.delete(&id).await.unwrap();
    assert!(store.get(&id).await.unwrap().is_none());
    store.delete(&id).await.unwrap();
}

#[tokio::test]
async fn list_returns_tasks_in_id_order_and_filters_status() {
    let (store, _dir) = temp_store().await;
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
    store.update(second).await.unwrap();

    let all = store.list(None).await.unwrap();
    assert_eq!(all.iter().map(|task| task.id).collect::<Vec<_>>(), vec![id1, id2]);

    let completed = store.list(Some(TaskStatus::Completed)).await.unwrap();
    assert_eq!(completed.len(), 1);
    assert_eq!(completed[0].id, id2);
}
```

Add these imports to the test module:

```rust
use std::path::PathBuf;
use std::time::SystemTime;
```

- [ ] **Step 7: Run tests and verify update/delete/list fail**

Run:

```bash
cargo test -p vol-llm-task stores::database::tests -- --nocapture
```

Expected: FAIL on update/delete/list tests because those trait methods still return `Internal`.

- [ ] **Step 8: Implement update/delete/list**

Replace the placeholder `update`, `delete`, and `list` methods with:

```rust
async fn update(&self, task: Task) -> Result<()> {
    let dependencies_json = task_ids_to_json(&task.dependencies)?;
    let blocks_json = task_ids_to_json(&task.blocks)?;
    let result_json = result_to_json(&task.result)?;
    let created_at_secs = system_time_to_secs(task.created_at)?;
    let started_at_secs = option_time_to_secs(task.started_at)?;
    let completed_at_secs = option_time_to_secs(task.completed_at)?;
    let output_file = path_to_db(&task.output_file);

    let result = sqlx::query(
        r#"
        UPDATE tasks SET
            status = ?1,
            kind = ?2,
            publisher = ?3,
            assignee = ?4,
            subject = ?5,
            description = ?6,
            active_form = ?7,
            dependencies_json = ?8,
            blocks_json = ?9,
            result_json = ?10,
            summary = ?11,
            output_file = ?12,
            created_at_secs = ?13,
            started_at_secs = ?14,
            completed_at_secs = ?15
        WHERE id = ?16
        "#,
    )
    .bind(status_to_db(task.status))
    .bind(kind_to_db(&task.kind))
    .bind(task.publisher)
    .bind(task.assignee)
    .bind(task.subject)
    .bind(task.description)
    .bind(task.active_form)
    .bind(dependencies_json)
    .bind(blocks_json)
    .bind(result_json)
    .bind(task.summary)
    .bind(output_file)
    .bind(created_at_secs)
    .bind(started_at_secs)
    .bind(completed_at_secs)
    .bind(task.id.0 as i64)
    .execute(&self.pool)
    .await
    .map_err(|e| StoreError::Database(format!("failed to update task {}: {e}", task.id)))?;

    if result.rows_affected() == 0 {
        return Err(StoreError::NotFound(format!("Task {}", task.id)));
    }
    Ok(())
}

async fn delete(&self, task_id: &TaskId) -> Result<()> {
    sqlx::query("DELETE FROM tasks WHERE id = ?1")
        .bind(task_id.0 as i64)
        .execute(&self.pool)
        .await
        .map_err(|e| StoreError::Database(format!("failed to delete task {}: {e}", task_id)))?;
    Ok(())
}

async fn list(&self, status: Option<TaskStatus>) -> Result<Vec<Task>> {
    let rows = if let Some(status) = status {
        sqlx::query("SELECT * FROM tasks WHERE status = ?1 ORDER BY id ASC")
            .bind(status_to_db(status))
            .fetch_all(&self.pool)
            .await
    } else {
        sqlx::query("SELECT * FROM tasks ORDER BY id ASC")
            .fetch_all(&self.pool)
            .await
    }
    .map_err(|e| StoreError::Database(format!("failed to list tasks: {e}")))?;

    rows.iter().map(row_to_task).collect()
}
```

- [ ] **Step 9: Run database store tests**

Run:

```bash
cargo test -p vol-llm-task stores::database::tests -- --nocapture
```

Expected: PASS except `get_ready_tasks` is not covered yet.

- [ ] **Step 10: Commit CRUD implementation**

Run:

```bash
git add crates/vol-llm-task/src/stores/database.rs
git commit -m "feat(task): implement database task CRUD" -m "Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 5: Implement Ready Task Query and Persistence Tests

**Files:**
- Modify: `crates/vol-llm-task/src/stores/database.rs`

- [ ] **Step 1: Add ready-task tests**

Add these tests to `crates/vol-llm-task/src/stores/database.rs`:

```rust
#[tokio::test]
async fn get_ready_tasks_returns_pending_tasks_without_dependencies() {
    let (store, _dir) = temp_store().await;
    let id1 = store
        .create(Task::new(TaskKind::Agent, "one".to_string(), vec![]))
        .await
        .unwrap();
    let id2 = store
        .create(Task::new(TaskKind::Agent, "two".to_string(), vec![]))
        .await
        .unwrap();

    let ready = store.get_ready_tasks().await.unwrap();
    assert_eq!(ready, vec![id1, id2]);
}

#[tokio::test]
async fn get_ready_tasks_excludes_incomplete_dependencies() {
    let (store, _dir) = temp_store().await;
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
}

#[tokio::test]
async fn get_ready_tasks_includes_task_after_dependency_completes() {
    let (store, _dir) = temp_store().await;
    let dependency_id = store
        .create(Task::new(TaskKind::Agent, "dependency".to_string(), vec![]))
        .await
        .unwrap();
    let blocked_id = store
        .create(Task::new(TaskKind::Agent, "blocked".to_string(), vec![dependency_id]))
        .await
        .unwrap();

    let mut dependency = store.get(&dependency_id).await.unwrap().unwrap();
    dependency.status = TaskStatus::Completed;
    store.update(dependency).await.unwrap();

    let ready = store.get_ready_tasks().await.unwrap();
    assert_eq!(ready, vec![blocked_id]);
}
```

- [ ] **Step 2: Run tests and verify ready-task tests fail**

Run:

```bash
cargo test -p vol-llm-task stores::database::tests::get_ready_tasks -- --nocapture
```

Expected: FAIL because `get_ready_tasks` still returns `Internal`.

- [ ] **Step 3: Implement `get_ready_tasks`**

Replace the placeholder `get_ready_tasks` method with:

```rust
async fn get_ready_tasks(&self) -> Result<Vec<TaskId>> {
    let tasks = self.list(None).await?;
    let completed_ids: std::collections::HashSet<TaskId> = tasks
        .iter()
        .filter(|task| task.status == TaskStatus::Completed)
        .map(|task| task.id)
        .collect();

    let ready = tasks
        .iter()
        .filter(|task| {
            task.status == TaskStatus::Pending
                && task.dependencies.iter().all(|id| completed_ids.contains(id))
        })
        .map(|task| task.id)
        .collect();

    Ok(ready)
}
```

- [ ] **Step 4: Add persistence-across-reconnect test**

Add this test:

```rust
#[tokio::test]
async fn tasks_persist_across_reconnect() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("tasks.db");
    let url = format!("sqlite://{}", db_path.display());

    let store = DatabaseTaskStore::connect(&url).await.unwrap();
    let id = store
        .create(Task::new(TaskKind::Agent, "persisted".to_string(), vec![]))
        .await
        .unwrap();
    drop(store);

    let reopened = DatabaseTaskStore::connect(&url).await.unwrap();
    let got = reopened.get(&id).await.unwrap().unwrap();
    assert_eq!(got.subject, "persisted");
}
```

- [ ] **Step 5: Run all database store tests**

Run:

```bash
cargo test -p vol-llm-task stores::database::tests -- --nocapture
```

Expected: PASS.

- [ ] **Step 6: Run all task crate tests**

Run:

```bash
cargo test -p vol-llm-task
```

Expected: PASS.

- [ ] **Step 7: Commit ready-task behavior**

Run:

```bash
git add crates/vol-llm-task/src/stores/database.rs
git commit -m "feat(task): support ready tasks in database store" -m "Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 6: Enable Runtime Construction of DatabaseTaskStore

**Files:**
- Modify: `crates/vol-llm-runtime/src/lib.rs`

- [ ] **Step 1: Add DatabaseTaskStore import**

In `crates/vol-llm-runtime/src/lib.rs`, replace:

```rust
use vol_llm_task::FileTaskStore;
```

with:

```rust
use vol_llm_task::{DatabaseTaskStore, FileTaskStore};
```

- [ ] **Step 2: Add database task store builder helper**

Add this helper near `build_file_task_store`:

```rust
async fn build_database_task_store(url: &str) -> Result<Arc<dyn TaskStore>, String> {
    let store = DatabaseTaskStore::connect(url)
        .await
        .map_err(|e| format!("failed to create database task store: {e}"))?;
    Ok(Arc::new(store))
}
```

- [ ] **Step 3: Replace temporary database runtime error**

In `AgentRuntimeBuilder::build`, replace the temporary database branch:

```rust
Some(config) if config.store_type == TaskStoreType::Database => {
    return Err(format!(
        "database task store is not implemented yet for url scheme: {}",
        config.required_url()?.split_once(':').map(|(scheme, _)| scheme).unwrap_or("<missing>")
    ));
}
```

with:

```rust
Some(config) if config.store_type == TaskStoreType::Database => {
    build_database_task_store(config.required_url()?).await?
}
```

- [ ] **Step 4: Add a runtime builder test for database config**

Add this test module at the bottom of `crates/vol-llm-runtime/src/lib.rs` if no test module exists there:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn builder_accepts_database_task_store_config_until_provider_requirement() {
        let temp = tempfile::tempdir().unwrap();
        let db_url = format!("sqlite://{}", temp.path().join("tasks.db").display());
        let config = TaskStoreConfig {
            store_type: TaskStoreType::Database,
            url: Some(db_url),
        };

        let result = AgentRuntime::builder(".", temp.path())
            .with_task_store_config(Some(config))
            .build()
            .await;

        if let Err(err) = result {
            assert!(
                err.contains("No LLM provider configured") || err.contains("failed to create database task store"),
                "unexpected runtime build error: {err}"
            );
        }
    }
}
```

Add `tempfile = "3"` to `crates/vol-llm-runtime/Cargo.toml` under `[dev-dependencies]`:

```toml
[dev-dependencies]
tempfile = "3"
```

- [ ] **Step 5: Run runtime check and tests**

Run:

```bash
cargo check -p vol-llm-runtime
cargo test -p vol-llm-runtime tests::builder_accepts_database_task_store_config_until_provider_requirement -- --exact
```

Expected: PASS. The test accepts LLM-provider setup failure because it only verifies database task store config wiring reaches the runtime builder without the old “not implemented yet” error.

- [ ] **Step 6: Commit runtime database store construction**

Run:

```bash
git add crates/vol-llm-runtime/src/lib.rs crates/vol-llm-runtime/Cargo.toml
git commit -m "feat(task): build database task store in runtime" -m "Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 7: Update Example Config and Verify Server Wiring

**Files:**
- Modify: `config.vol-agent.example.toml`
- Modify: `crates/vol-agent-server/src/main.rs`

- [ ] **Step 1: Update example config comments**

In `config.vol-agent.example.toml`, replace the runtime comment block around `store_dir` with:

```toml
# Data store directory (sessions, file-backed tasks, logs)
# Supports ~ tilde expansion
store_dir = "~/.vol"

# Optional task store backend. If omitted, the server uses the file task store
# rooted under runtime.store_dir.
#
# Explicit file store:
# [runtime.task_store]
# type = "file"
#
# Database store. The concrete database is inferred from the URL scheme.
# Currently SQLite is implemented; postgres/postgresql and mysql schemes are
# recognized for clear errors until those backends are added.
# [runtime.task_store]
# type = "database"
# url = "sqlite://./data/tasks.db"
```

- [ ] **Step 2: Add server main logging for task store type**

In `crates/vol-agent-server/src/main.rs`, before building the core, add:

```rust
if let Some(task_store) = &config.runtime.task_store {
    tracing::info!(task_store_type = ?task_store.store_type, "Using configured task store");
} else {
    tracing::info!("Using default file task store");
}
```

- [ ] **Step 3: Run server crate tests and check**

Run:

```bash
cargo test -p vol-agent-server
cargo check -p vol-agent-server
```

Expected: PASS.

- [ ] **Step 4: Run task/runtime/channel/server checks together**

Run:

```bash
cargo check -p vol-llm-task -p vol-llm-runtime -p vol-llm-agent-channel -p vol-agent-server
```

Expected: PASS.

- [ ] **Step 5: Commit docs and logging**

Run:

```bash
git add config.vol-agent.example.toml crates/vol-agent-server/src/main.rs
git commit -m "docs(task): document database task store config" -m "Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 8: Final Verification and Project Wiki Ingest

**Files:**
- Update through skill: `docs/wiki/**`

- [ ] **Step 1: Run full focused test suite**

Run:

```bash
cargo test -p vol-llm-task
cargo test -p vol-agent-server
cargo check -p vol-llm-runtime -p vol-llm-agent-channel -p vol-agent-server
```

Expected: all commands PASS.

- [ ] **Step 2: Manually verify SQLite persistence through the store tests**

Run:

```bash
cargo test -p vol-llm-task stores::database::tests::tasks_persist_across_reconnect -- --exact --nocapture
```

Expected: PASS.

- [ ] **Step 3: Review working tree**

Run:

```bash
git status --short
```

Expected: only intentional implementation files are modified. The pre-existing `crates/vol-llm-ui/assets/tailwind.css` change should remain untouched unless the user explicitly asks to handle it.

- [ ] **Step 4: Ingest project wiki**

Invoke the `wiki-ingest` skill with this source set:

```text
Ingest the task database store implementation and docs from:
- docs/superpowers/specs/2026-06-09-task-database-store-design.md
- docs/superpowers/plans/2026-06-09-task-database-store.md
- crates/vol-llm-task/src/stores/database.rs
- crates/vol-llm-runtime/src/lib.rs
- crates/vol-agent-server/src/config.rs
- config.vol-agent.example.toml
```

Expected: `docs/wiki` is updated with the new task database store architecture.

- [ ] **Step 5: Commit wiki updates**

Run:

```bash
git add docs/wiki
git commit -m "docs(wiki): ingest task database store" -m "Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

- [ ] **Step 6: Final status**

Run:

```bash
git status --short
```

Expected: no implementation changes remain except unrelated pre-existing files such as `crates/vol-llm-ui/assets/tailwind.css` if it was still present before this work.

---

## Self-Review

- Spec coverage: covered global database store, `type = "database"`, URL scheme inference, file fallback, automatic SQLite migrations, shared runtime store, task tool/RPC compatibility by preserving `runtime.task_store`, tests, config example, and non-goals.
- Placeholder scan: no steps rely on unspecified implementation. Each code-changing step includes the concrete code to add or replace.
- Type consistency: config types are defined in `vol-llm-runtime` and reused by `vol-agent-server` and `vol-llm-agent-channel`; store implementation is named `DatabaseTaskStore`; store type enum uses `File` and `Database`; config field is `runtime.task_store`.
