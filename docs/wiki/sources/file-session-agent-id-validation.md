---
type: source
source_type: code
date: 2026-06-09
ingested: 2026-06-09
tags: [session, persistence, security, path-validation]
---

# File Session Agent ID Validation

**Authors/Creators:** Nathan + Claude Code
**Date:** 2026-06-09
**Link:** crates/vol-session/src/manager.rs; crates/vol-session/src/store.rs

## TL;DR

`FileSessionManager` now validates `agent_id` values before using them as filesystem path components. Result-returning APIs reject invalid IDs with `StoreError::InvalidInput`, while the infallible `entry_store_for_agent` path quarantines invalid IDs under a deterministic encoded directory below `agents_root` so path traversal strings cannot escape the configured root.

## Key Takeaways

- `FileSessionManager::validate_agent_id` accepts only a non-empty single `Component::Normal` path component.
- Empty strings, `.`, `..`, absolute paths, separator-containing values such as `../evil`, and non-normal components are rejected.
- `list_sessions`, `session_exists`, `resolve_session_agent`, and `entry_store_for_session` surface invalid scoped agent IDs as `StoreError::InvalidInput`.
- `entry_store_for_agent` keeps its fixed infallible trait signature by routing invalid IDs to `agents_root/.invalid-agent-id/<hex-agent-id>/sessions`.
- Regression coverage verifies `../evil` rejection for fallible APIs and confirms saving through `entry_store_for_agent("../evil")` does not create files outside `agents_root`.

## Detailed Summary

The Task 1 quality finding identified direct use of `agent_id` in `agents_root.join(agent_id).join("sessions")`. That allowed agent IDs containing separators, parent directory components, or absolute paths to resolve outside the configured session-agent root.

The fix adds a small validation helper in `crates/vol-session/src/manager.rs`. The helper parses the ID with `Path::components()` and accepts exactly one `Component::Normal`, with an explicit empty-string rejection. Fallible manager operations call this helper before creating a `FileSessionEntryStore` for a user-supplied scoped agent.

Because `SessionManager::entry_store_for_agent` returns `Arc<dyn SessionEntryStore>` rather than a `Result`, invalid IDs cannot be reported directly there. Instead, invalid IDs are deterministically hex-encoded and rooted under `agents_root/.invalid-agent-id/<hex>/sessions`, preserving safety and determinism without escaping `agents_root`.

`crates/vol-session/src/store.rs` adds `StoreError::InvalidInput(String)` to distinguish caller input validation from `NotFound`, `Internal`, and I/O failures.

Verification: `cargo test -p vol-session` passed with 66 tests.

## Entities Mentioned

- [[vol-session]]: owns `FileSessionManager`, `SessionManager`, `SessionEntryStore`, and `StoreError`.

## Concepts Covered

- [[session-as-ssot]]: session persistence remains the durable source of agent conversation state.

## Notes

The quarantine directory name starts with `.invalid-agent-id`, which is itself rejected by the normal-agent validator, and the original invalid ID is encoded as lowercase hex to avoid introducing separators back into the path.
