# Session Database Store Design

Date: 2026-06-09

## Purpose

Add a database-backed session store that follows the existing task database store pattern. The feature should let runtime sessions use either the current file-backed JSONL store or a SeaORM-backed database store configured through server/runtime configuration.

The target behavior is full parity with the task database store integration: SQLite and Postgres support, automatic compiled migrations, runtime/server configuration validation, JSON-RPC session domain compatibility, and tests that verify persistence across reconnects.

## Scope

In scope:

- Add a SeaORM-backed session store in `vol-session`.
- Add a `SessionManager` abstraction for runtime/channel code.
- Add `[runtime.session_store]` configuration beside `[runtime.task_store]`.
- Support `file` and `database` session stores.
- Support SQLite and Postgres database URLs.
- Preserve existing file-store default behavior.
- Keep existing JSON-RPC session methods working: `session.list`, `session.resume`, and `session.entries`.
- Cover behavior with unit, integration, runtime, and channel tests.

Out of scope:

- Automatic migration from existing JSONL session files into the database.
- A user-facing CLI/admin migration tool.
- Session title/search/archive features.
- Redis, S3, MySQL, or remote session stores.

## Recommended Approach

Use the existing task database store as the implementation model, but add a session-specific management layer.

`vol-session` owns the database implementation and SeaORM details. `vol-llm-runtime` owns configuration selection and resource assembly. `vol-agent-server` parses, validates, logs, and passes configuration through. `vol-llm-agent-channel` depends on `SessionManager` instead of hard-coded file store paths.

This keeps database logic in the session crate and keeps runtime/server/channel free of SeaORM-specific details.

## Naming and Compatibility Rules

Use `agent_id` as the public and persisted scope name. In this design, `agent_id` means the same value currently used by JSON-RPC session requests and the file layout under the agent directory. Do not introduce a separate `agent_type` concept for session ownership.

Session ids are treated as globally unique UUIDs. The database schema enforces `sessions.id` as the primary key. If a file-backed manager discovers the same `session_id` under more than one `agent_id` and the caller did not provide an `agent_id`, it must return an ambiguity/scope conflict error instead of choosing one silently.

JSON-RPC response compatibility is required. `session.list` must continue returning the current client-visible fields, including `id`, `agent_id`, `session_id`, `entry_count`, and `created_at`. The implementation may compute or store additional fields such as `updated_at`, but those additions must not remove or rename existing fields.

## Architecture

### `vol-session`

Add:

- `DatabaseSessionEntryStore`
- `DatabaseSessionManager`
- `FileSessionManager`
- `SessionManager` trait
- `SessionInfo` metadata type if an existing type is not sufficient
- SeaORM entity, mapping, and migration modules

`DatabaseSessionEntryStore` implements the existing `SessionEntryStore` trait. It is scoped to a concrete `agent_id` when created for runtime use and writes entries for that agent scope.

`SessionManager` is the abstraction consumed by runtime/channel code:

```rust
#[async_trait]
pub trait SessionManager: Send + Sync {
    fn entry_store_for_agent(&self, agent_id: &str) -> Arc<dyn SessionEntryStore>;

    async fn list_sessions(
        &self,
        agent_id: Option<&str>,
    ) -> Result<Vec<SessionInfo>, SessionError>;

    async fn session_exists(
        &self,
        agent_id: Option<&str>,
        session_id: &str,
    ) -> Result<bool, SessionError>;

    async fn entry_store_for_session(
        &self,
        agent_id: Option<&str>,
        session_id: &str,
    ) -> Result<Arc<dyn SessionEntryStore>, SessionError>;
}
```

The optional `agent_id` arguments preserve current JSON-RPC behavior, where callers may provide an agent scope for `session.resume` and `session.entries`. If no `agent_id` is provided, managers may resolve by globally unique `session_id`; duplicate file-backed matches are an error.

`FileSessionManager` preserves current file behavior by wrapping the existing directory layout. It creates `FileSessionEntryStore` instances for agent-specific session directories and implements listing/resolution by scanning the current JSONL files.

`DatabaseSessionManager` wraps a database connection and creates scoped `DatabaseSessionEntryStore` instances. It implements listing and session resolution using the `sessions` table.

### `vol-llm-runtime`

Add:

- `SessionStoreType { File, Database }`
- `SessionStoreConfig { type, url }`
- `AgentRuntimeBuilder::with_session_store_config(...)`
- session manager construction in `AgentRuntimeBuilder::build()`

Runtime agent registration should stop constructing `FileSessionEntryStore` directly. Instead it should ask the shared session manager for the scoped entry store:

```rust
let entry_store = session_manager.entry_store_for_agent(&agent_id);
let session = Session::new(entry_store);
```

The default remains file-backed session storage.

### `vol-agent-server`

Extend the runtime config section:

```toml
[runtime.session_store]
type = "file"
```

or:

```toml
[runtime.session_store]
type = "database"
url = "sqlite://data/sessions.db"
```

Server startup should log whether the configured session store is default/file/database, matching the task store logging style.

### `vol-llm-agent-channel`

`AgentServerCoreBuilder` should accept `SessionStoreConfig` and forward it into the runtime builder. The session domain handler should receive an `Arc<dyn SessionManager>` instead of `agents_root` plus concrete file-store knowledge.

JSON-RPC behavior remains unchanged externally:

- `session.list` calls `SessionManager::list_sessions` and maps `SessionInfo` to the existing response shape.
- `session.resume` resolves an entry store with `entry_store_for_session(agent_id, session_id)`, then calls `Session::resume`.
- `session.entries` resolves an entry store with `entry_store_for_session(agent_id, session_id)`, then reads entries.
- Missing sessions are detected by the manager before calling `Session::resume`, because the lower-level `Session::resume` API currently can resume from an empty store.

## Database Model

Use two tables: `sessions` and `session_entries`.

### `sessions`

| Column | Purpose |
| --- | --- |
| `id` | Session id, primary key. |
| `agent_id` | Agent scope that owns the session. |
| `created_at` | Session creation time. |
| `updated_at` | Last entry write time. |
| `entry_count` | Number of entries written for this session. |
| `metadata` | JSON/text metadata, initially `{}`. |

Indexes:

- `idx_sessions_agent_id_updated_at`
- `idx_sessions_updated_at`

`entry_count` is stored to keep `session.list` efficient and compatible with the current JSON-RPC response. It is incremented in the same transaction that inserts an entry.

### `session_entries`

| Column | Purpose |
| --- | --- |
| `id` | Entry UUID, primary key. |
| `session_id` | Associated session id. |
| `created_at` | Entry creation time. |
| `parent_id` | Optional parent entry id. |
| `entry_type` | `message`, `checkpoint`, or `summary`. |
| `data` | Entry payload JSON/text. |

Indexes:

- `idx_session_entries_session_id_created_at`
- `idx_session_entries_parent_id`

`session_entries.session_id` should be treated as a relationship to `sessions.id`. The implementation can use a database foreign key if SeaORM migration support and SQLite/Postgres compatibility remain straightforward. If that complicates SQLite behavior, a logical relationship plus explicit transactional delete behavior is acceptable.

## Write Semantics

`DatabaseSessionEntryStore::save_entry(entry)` should run in a transaction:

1. Check whether `sessions.id = entry.session_id` exists.
2. If missing, insert a new session row:
   - `id = entry.session_id`
   - `agent_id = current store agent_id`
   - `created_at = entry.created_at`
   - `updated_at = entry.created_at`
   - `entry_count = 0`
   - `metadata = "{}"`
3. If present, validate scope:
   - If the existing `agent_id` conflicts with the current scoped store, return a clear session scope conflict error.
   - If the existing `agent_id` is empty and the current store has a scope, fill it.
4. Insert the `session_entries` row.
5. Update the session row with `updated_at = entry.created_at` and `entry_count = entry_count + 1`.

This `get_or_create` behavior avoids requiring a separate explicit session creation lifecycle while still keeping session metadata normalized.

`delete_session(session_id)` should remove entries and the session row. The operation should be transactional for database stores. If a scoped delete API is added later, it should validate `agent_id` before deleting.

## Configuration Validation

Validation should mirror task store rules:

- `type = "file"` must not include `url`.
- `type = "database"` must include `url`.
- Database URL schemes accepted by config validation: `sqlite://`, `postgres://`, `postgresql://`, and `mysql://`.
- SQLite and Postgres are implemented.
- MySQL is recognized but not enabled, matching the task store backend parser style: configuration validation may accept the scheme, but database store construction returns `UnsupportedDatabaseBackend("mysql")`.
- Unknown schemes return a validation error before runtime startup.

## Error Handling

Keep error placement aligned with existing trait boundaries:

- `StoreError` should gain database-store errors needed by `SessionEntryStore` methods, such as `Database(String)` and scope conflict details when `save_entry`, `get_entries`, or `delete_session` fails.
- `SessionError` should wrap or translate store errors for higher-level session and manager operations, and should also represent manager-only failures such as `SessionNotFound` or ambiguous file-backed resolution.

Required error cases:

- `Database(String)` for connection, migration, query, or transaction failures.
- `UnsupportedDatabaseBackend(String)` for recognized but unsupported backends.
- `SessionAgentScopeConflict { session_id, expected, actual }` when a session is accessed from a conflicting agent scope.
- `SessionNotFound(String)` when manager-level `resume` or `entries` resolution targets a missing session.
- `AmbiguousSession { session_id }` when a file-backed lookup without `agent_id` finds duplicate session ids under multiple agents.

Error messages should include enough context for logs and JSON-RPC responses without exposing credentials from database URLs.

## Testing Plan

### `vol-session` database tests

Cover SQLite with temporary database files:

- Connection creates parent directory and runs migrations.
- `save_entry` creates the `sessions` row automatically.
- `save_entry` updates `sessions.updated_at` and increments `entry_count`.
- `get_entries` returns entries in creation order.
- Message, checkpoint, and summary entries round-trip correctly.
- `delete_session` deletes both session metadata and entries.
- Reconnecting to the same database preserves sessions and entries.
- Agent scope conflicts return the expected error.

Cover Postgres when `VOL_AGENT_POSTGRES_TEST_URL` is set:

- Run equivalent persistence and CRUD checks.
- Use locking or isolated identifiers to avoid concurrent-test contamination, following the task database store tests.

### `SessionManager` tests

For `FileSessionManager`:

- `entry_store_for_agent` maps to the existing agent session directory layout.
- `list_sessions(agent_id)` returns file-backed sessions with the current response-compatible fields.
- `entry_store_for_session(Some(agent_id), session_id)` finds the correct agent-scoped file store.
- `entry_store_for_session(None, session_id)` returns an ambiguity error if duplicate session ids are found under multiple agents.

For `DatabaseSessionManager`:

- `list_sessions(agent_id)` reads from the `sessions` table.
- `entry_store_for_session(agent_id, session_id)` validates the optional agent scope and returns a usable scoped entry store.
- Missing sessions return `SessionNotFound`.

### Runtime tests

- Default configuration still creates file-backed session behavior.
- SQLite database session configuration builds successfully.
- A runtime-created session can be persisted and resumed through the database store.
- Validation rejects missing database URL, file URL, and unknown URL schemes.
- MySQL URL construction fails with the explicit unsupported-backend error.

### Channel/server tests

- Existing JSON-RPC session tests keep passing with file store.
- Add at least one SQLite-backed end-to-end path covering `session.list`, `session.resume`, and `session.entries`.
- Server config parsing handles `[runtime.session_store]` examples.
- JSON-RPC responses retain `id`, `agent_id`, `session_id`, `entry_count`, and `created_at`.

## Migration and Compatibility

The default stays file-backed, so existing users do not need to change configuration.

This feature does not import existing JSONL session files into the database. Users who choose database-backed sessions start writing new sessions to the configured database. A future migration tool can be designed separately if needed.

Compiled SeaORM migrations are part of the crate, matching the task database store approach. Runtime startup runs migrations automatically when connecting to a database session store.

## Documentation

Update:

- `config.vol-agent.example.toml` with `[runtime.session_store]` examples.
- Relevant crate docs or module docs if present.
- `docs/wiki` through the required `wiki-ingest` flow after implementation.

## Acceptance Criteria

- Runtime and server accept `[runtime.session_store]` config.
- File-backed sessions remain the default and continue to pass existing tests.
- SQLite session database store works end to end.
- Postgres session database tests run when `VOL_AGENT_POSTGRES_TEST_URL` is configured.
- JSON-RPC session list/resume/entries behavior is unchanged from the client perspective.
- Database writes use `sessions` plus `session_entries` and create session metadata through `get_or_create` semantics.
- The implementation uses `agent_id` consistently for session scope and preserves existing JSON-RPC field names.
- No SeaORM-specific logic leaks into server/channel session handlers.
