# Task Database Store Design

## Summary

Add a database-backed implementation of the existing `TaskStore` abstraction and allow the agent server runtime to select the global task store backend from `config.vol-agent.toml`.

The runtime continues to own exactly one task store. The `task` tool, scheduler, RPC task handler, and UI-facing task APIs all share `runtime.task_store`. This design does not add per-agent task stores, `.agents/task-providers`, or `tool_config.task` overrides.

## Goals

- Add a database-backed task store using SQLx.
- Configure the global task store from the agent server runtime config.
- Keep the current file task store as the default when no task store config is present.
- Use `type = "database"` for the database-backed store.
- Infer the database backend from the database URL scheme.
- Run database migrations automatically during store initialization.
- Preserve the existing `TaskStore` trait and task tool behavior.

## Non-goals

- No `.agents/task-providers` directory.
- No per-agent task store routing.
- No `tool_config.task.provider` or `tool_config.task.store` support.
- No UI selector for multiple task stores.
- No multi-tenant task store model.
- No automatic migration of existing file-backed task data into the database.

## Configuration

Add an optional nested task store config under `[runtime]`:

```toml
[runtime]
working_dir = "."
store_dir = "./data"

[runtime.task_store]
type = "database"
url = "sqlite://./data/tasks.db"
```

Explicit file store configuration is also valid:

```toml
[runtime.task_store]
type = "file"
```

If `[runtime.task_store]` is omitted, the runtime keeps the current behavior and uses the file task store based on `runtime.store_dir`.

### Store Types

Supported `type` values:

- `file`: use the existing file-backed `TaskStore`.
- `database`: use the new SQLx-backed database `TaskStore`.

For `type = "database"`, `url` is required. The database backend is inferred from the URL scheme:

- `sqlite://...` -> SQLite
- `postgres://...` or `postgresql://...` -> Postgres
- `mysql://...` -> MySQL

Implementation may enable and test database backends incrementally. If a URL scheme is recognized but the corresponding backend is not compiled or implemented, startup must fail with a clear error.

## Config Types

The server runtime config should add a task store config similar to:

```rust
#[derive(Debug, Clone, Deserialize)]
pub struct TaskStoreConfig {
    #[serde(rename = "type")]
    pub store_type: TaskStoreType,
    pub url: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TaskStoreType {
    File,
    Database,
}
```

Validation rules:

- `type = "file"` must not require `url`.
- `type = "file"` with `url` should be rejected to avoid misleading configuration.
- `type = "database"` requires `url`.
- `type = "database"` requires a supported URL scheme.
- Errors must avoid leaking database credentials from the URL.

## Runtime Assembly

`AgentRuntimeBuilder::build()` should stop hardcoding only `FileTaskStore`. It should construct the one global task store from the runtime config:

```rust
match task_store_config {
    None => Arc::new(FileTaskStore::new(...)),
    Some(config) if config.store_type == TaskStoreType::File => Arc::new(FileTaskStore::new(...)),
    Some(config) if config.store_type == TaskStoreType::Database => {
        Arc::new(DatabaseTaskStore::connect(config.required_url()?).await?)
    }
}
```

The resulting `Arc<dyn TaskStore>` remains the single source of truth and is reused by existing consumers:

- `AgentRuntime.task_store`
- `vol_llm_task::tools::register_cli(..., task_store.clone())`
- `TaskHandler::new(runtime.task_store.clone())`
- scheduler or future task consumers

No task store resolution happens inside the task tool. The tool continues to use the runtime-provided store captured during registration.

## Database Store Implementation

Add a new concrete store in `vol-llm-task`, named `DatabaseTaskStore`.

The public name should use `Database`, not `Sqlx`, because the configuration and user-facing behavior are database-oriented. SQLx is the internal implementation detail.

The existing `TaskStore` trait remains unchanged.

## Database Schema

The database schema should preserve the current `Task` model. A single `tasks` table is sufficient for the first implementation:

| Column | Type | Notes |
| --- | --- | --- |
| `id` | integer/bigint primary key | database-generated task id |
| `title` | text | required |
| `description` | text | required |
| `status` | text | serialized task status |
| `parent_id` | integer/bigint nullable | optional parent task |
| `assignee` | text nullable | optional assignee |
| `publisher` | text nullable | optional publisher |
| `created_at` | timestamp/text | existing task timestamp semantics |
| `updated_at` | timestamp/text | existing task timestamp semantics |
| `result` | text nullable | optional task result |
| `summary` | text nullable | optional task summary |
| `output_file` | text nullable | optional output file path |
| `dependencies_json` | text/json | serialized dependency task ids |
| `blocks_json` | text/json | serialized blocked task ids |

`dependencies` and `blocks` are stored as serialized JSON for the first version. This avoids introducing join tables before there is a performance or query requirement.

## ID Strategy

The database store owns task id allocation.

`TaskStore::create` inserts a task and returns the database-generated id:

- SQLite: `INTEGER PRIMARY KEY AUTOINCREMENT` or equivalent rowid behavior.
- Postgres: identity column or `BIGSERIAL`.
- MySQL: `BIGINT AUTO_INCREMENT`.

This matches the existing store semantics where the store assigns task ids.

## Migrations

`DatabaseTaskStore::connect(url)` automatically runs embedded SQLx migrations before returning the store.

Requirements:

- Users only configure `type = "database"` and `url`.
- Server startup fails if connection or migration fails.
- Migration files are compiled into the binary.
- Future schema changes are added as new migration files.
- Migration errors should include enough context to diagnose the migration version and database type without printing credentials.

## Store Method Semantics

`DatabaseTaskStore` must preserve the behavior of existing `TaskStore` implementations:

- `create`: insert the task, assign id, and return the stored task.
- `get`: return `Some(Task)` by id or `None` if missing.
- `update`: persist all task fields.
- `delete`: delete by id and match existing missing-task behavior.
- `list`: return all tasks in stable order, preferably `id ASC`.
- `get_ready_tasks`: return pending tasks whose dependencies are all completed.

For `get_ready_tasks`, the first implementation can load pending tasks and dependency records, then evaluate readiness in Rust. SQL optimization can be added later if needed.

## Error Handling

Startup should fail early for invalid configuration or unusable database state.

Expected errors:

- Missing database URL: `runtime.task_store.url is required when type = "database"`.
- Unsupported store type: serde/config error listing valid values.
- Unsupported database URL scheme: `unsupported task store database url scheme: <scheme>`.
- Database connection failure: include store type and URL scheme, but redact password and avoid printing full credentials.
- Migration failure: include migration context and SQLx error.

Runtime task operation errors should use the existing task store error path and preserve current tool-facing error behavior.

## Testing Strategy

### `vol-llm-task` Tests

Add tests for `DatabaseTaskStore` using SQLite in-memory or a temporary SQLite file:

- `create` assigns an id and `get` retrieves the task.
- `update` persists status, result, summary, and output file changes.
- `delete` removes the task.
- `list` returns tasks in stable order.
- `get_ready_tasks` returns pending tasks without dependencies.
- `get_ready_tasks` excludes tasks with incomplete dependencies.
- `get_ready_tasks` includes tasks once dependencies are completed.

### Config Tests

Add server/runtime config tests for:

- Missing `[runtime.task_store]` falls back to file.
- `type = "file"` is valid without `url`.
- `type = "file"` with `url` is rejected.
- `type = "database"` without `url` is rejected.
- `type = "database"` with a SQLite URL is valid.
- Unknown or unsupported URL scheme is rejected.

### Runtime Wiring Tests

Add runtime tests for:

- Default config still creates and registers the file task store.
- Database config creates a database task store.
- The `task` CLI tool uses the runtime-created store.
- `TaskHandler` continues to share `runtime.task_store`.

### Manual Verification

Manual verification after implementation:

1. Configure SQLite database task store.
2. Start the agent server.
3. Create, list, get, update, and delete tasks through the `task` tool.
4. Restart the server.
5. Verify persisted tasks remain in the database.

## Compatibility

Existing deployments remain compatible because omitted `[runtime.task_store]` keeps the file task store default.

Switching from file to database starts with an empty database task store unless users manually import data. File-to-database migration is intentionally out of scope for this design.
