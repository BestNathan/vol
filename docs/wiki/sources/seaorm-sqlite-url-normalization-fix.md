---
type: source
source_type: code
date: 2026-06-09
ingested: 2026-06-09
tags: [task-store, seaorm, sqlite, url-normalization]
---

# SeaORM SQLite URL Normalization Fix

**Authors/Creators:** Nathan, Claude Code
**Date:** 2026-06-09
**Link:** `/Users/admin/Documents/learn/vol-agent/crates/vol-llm-task/src/stores/database/mod.rs`

## TL;DR
A SeaORM Task 1 review fix changed SQLite task-store URL normalization to detect an explicit `mode` query parameter by parsing query keys exactly instead of searching the whole URL for `mode=`. This prevents `journal_mode=wal` from being mistaken for an existing create mode and preserves explicit `mode=rwc` URLs unchanged.

## Key Takeaways
- `normalize_sqlite_url` now splits after `?`, splits query params by `&`, and compares the key before `=` to `mode`.
- `sqlite:///tmp/tasks.db?journal_mode=wal` now normalizes to `sqlite:///tmp/tasks.db?journal_mode=wal&mode=rwc`.
- `sqlite:///tmp/tasks.db?mode=rwc` remains unchanged.
- Verification covered the exact targeted unit test and `cargo check -p vol-llm-task`.

## Detailed Summary
The previous SeaORM skeleton implementation checked `url.contains("mode=")` when deciding whether to append `mode=rwc` to SQLite URLs. That was too broad because query keys such as `journal_mode` contain the substring `mode=` and could suppress `mode=rwc`, preventing file creation behavior from being requested.

The fix keeps the normalization local to `crates/vol-llm-task/src/stores/database/mod.rs`: after confirming a SQLite URL is not in-memory, the code checks whether a query string exists, splits it into `&`-separated parameters, extracts each key before `=`, and only treats the URL as already configured when a key is exactly `mode`.

The existing `stores::database::tests::normalize_sqlite_url_adds_create_mode` test was extended with the `journal_mode=wal` regression case while retaining the explicit `mode=rwc` unchanged case.

## Entities Mentioned
- [[vol-llm-task-crate]]: owns the SeaORM task database skeleton and SQLite URL normalization helper.

## Concepts Covered
- [[runtime-task-store-configuration]]: database-backed runtime task stores depend on correct SQLite URL handling when constructing the store.

## Notes
This change was amended into the SeaORM Task 1 skeleton commit. It intentionally did not touch the unrelated pre-existing web UI Tailwind CSS working-tree change.
