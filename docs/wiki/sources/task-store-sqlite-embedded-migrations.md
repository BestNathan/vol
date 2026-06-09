---
type: source
source_type: code
date: 2026-06-09
ingested: 2026-06-09
tags: [task-store, sqlite, sqlx, migrations]
---

# Task Store SQLite Embedded Migrations

**Authors/Creators:** Nathan + Claude
**Date:** 2026-06-09
**Link:** `/Users/admin/Documents/learn/vol-agent/crates/vol-llm-task/src/stores/database.rs`, `/Users/admin/Documents/learn/vol-agent/Cargo.toml`

## TL;DR
SQLite task-store migrations are now embedded into the `vol-llm-task` binary with `sqlx::migrate!("./migrations/sqlite")` instead of being loaded from a source-tree path at runtime. Release binaries and containers no longer need the `migrations/sqlite` directory present on disk to initialize the SQLite task store.

## Key Takeaways
- [[vol-llm-task-crate]] now defines a static embedded `SQLITE_MIGRATOR` in `stores::database`.
- Runtime `Migrator::new(Path::new(...))` loading was removed, along with the `CARGO_MANIFEST_DIR`-derived migrations directory constant.
- The workspace `sqlx` dependency enables the `macros` feature so the compile-time migration macro is available.
- Verified with focused database-store tests and the full `vol-llm-task` test suite.

## Detailed Summary
The database task store previously used `env!("CARGO_MANIFEST_DIR")` to construct a path to `migrations/sqlite`, then called `Migrator::new(...)` during SQLite connection setup. That worked in a source checkout but would fail for deployed release binaries or containers that did not include the migration files at the same runtime path.

The fix replaces runtime directory loading with a compile-time static migrator:

```rust
static SQLITE_MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations/sqlite");
```

`connect_sqlite` now runs this embedded migrator against the opened pool. Because `sqlx::migrate!` requires SQLx macros, the workspace `sqlx` dependency now includes the `macros` feature alongside `runtime-tokio-rustls`, `sqlite`, and `migrate`.

Verification:
- `cargo test -p vol-llm-task stores::database::tests -- --nocapture`
- `cargo test -p vol-llm-task`

## Entities Mentioned
- [[vol-llm-task-crate]]: owns the SQLx-backed `DatabaseTaskStore` and embedded SQLite migrations.

## Concepts Covered
- [[runtime-task-store-configuration]]: this database backend is selected by the runtime task-store configuration contract.

## Notes
- Amended commit: `c2e065449635068b052d341307be12847dfd948a`.
- The unrelated pre-existing Tailwind CSS working-tree modification was not touched by this fix.
