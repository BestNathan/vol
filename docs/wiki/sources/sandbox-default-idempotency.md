---
type: source
source_type: code
date: 2026-08-28
ingested: 2026-08-28
tags: [sandbox, manager, leak, idempotency, instance-identity, default-sandbox]
---

# Sandbox `default()` Idempotency and Instance Identity

**Authors/Creators:** Claude (Nathan)
**Date:** 2026-08-28
**Link:** `crates/vol-llm-sandbox/src/manager.rs`

## TL;DR
`SandboxManager::default()` branched on the *total* store record count, giving three different behaviors: with 0 records it created and registered a scratch tmp sandbox; with exactly 1 it returned whatever that single record was — possibly an unrelated SSH instance; with 2 or more it created and registered a **brand new** tmp sandbox on *every call*, an unbounded leak. Since the data-plane `SandboxHandler` calls `default()` on all six of its exec/read/write/create_dir/read_dir/metadata operations, the third regime leaked one store record plus one cached `Arc` per RPC. `default()` is now keyed on a reserved `DEFAULT_TMP_PROFILE` ("default-tmp") profile lookup and serialized by a mutex, making it idempotent regardless of what else is in the store.

## Key Takeaways
- **Root cause was a count-based heuristic.** `if records.len() == 1 { return that one }` served double duty as both the idempotency mechanism and an implicit "guess the default" rule. It only worked because the store started empty and `default()`'s own record made the count exactly 1.
- **The leak was reachable, not theoretical.** Any second record — from `create()`, `register_instance()`, or a hypothetical change making `acquire_by_name()` record instances — pushes the store to ≥2 and turns every subsequent `default()` call into a fresh allocation.
- **A proposed "fix" would have triggered it.** Making `acquire_by_name()` write store records (to populate the empty `sandbox.list`) would have put 4 records in production's store, moving it straight into the leaking regime. Measuring first prevented shipping the leak.
- **Instance identity is per-provider, not universal.** `backend_id` is a pure function of the spec for `local` (the work dir path) and `ssh` (`user@host`), but carries real per-instance state for `tmp` (a random `/tmp/<x>` path) and firecracker (a VM id). Only the latter group has an instance to record.
- **`sandbox.list` returning empty in production is semantically correct**, not a bug: every configured profile is local or ssh, neither of which has instance identity. Callers wanting "what sandboxes are configured" should use `sandbox.list_specs`.

## Detailed Summary

### The three regimes

Measured against the pre-fix code with a probe test, starting from a store holding two unrelated instances and calling `default()` four times:

```text
store size after each of 4 default() calls: [3, 4, 5, 6]
default-tmp records accumulated: 4
```

| store records | pre-fix behavior |
|---|---|
| 0 | create tmp + insert record → return it (count becomes 1) |
| 1 | return that record's sandbox — whatever kind it happens to be |
| ≥2 | create a new tmp + insert a record on **every** call |

The `/tmp/<random>` directory is computed in `TmpSandbox::new()` but only materialized on first use, so the disk leak tracks actually-used scratch sandboxes while the record and `instances` map leaks are unconditional.

### The fix

`default()` now looks up an existing record by profile name instead of counting:

```text
DEFAULT_TMP_PROFILE = "default-tmp"   (pub const, re-exported from lib.rs)

lock default_lock
store.list(filter { profile: DEFAULT_TMP_PROFILE })
  → found?  return self.get(record.id)
  → none?   provider("tmp").create(spec) → insert record → cache → return
```

Three properties this buys:
- **Idempotent** — at most one `default-tmp` record ever exists, independent of other store contents.
- **No wrong target** — an unrelated instance is never returned. Callers needing a specific sandbox must use `acquire_by_name()`.
- **Race-free** — a `default_lock: tokio::sync::Mutex<()>` serializes the check-then-create, so concurrent RPCs cannot each create their own "default".

The no-tmp-provider fallback still returns a bare `LocalSandbox` but deliberately does *not* insert a store record, consistent with local having no instance identity to track.

### Test changes

`test_default_with_existing_sandbox` was removed. Its comment — `// Default should return the existing sandbox` — asserted exactly the behavior being removed, and it would have kept passing by coincidence: it never registered a `tmp` provider, so post-fix it falls through to the `LocalSandbox` branch and still reports `kind() == "local"`. Four tests replaced it:

| Test | Asserts |
|---|---|
| `test_default_ignores_unrelated_sandbox` | with one unrelated instance present, `default()` creates its own scratch instance instead of returning it |
| `test_default_is_idempotent_with_empty_store` | 5 calls → store stays at 1 |
| `test_default_does_not_leak_when_multiple_records_exist` | with 2 unrelated records, 4 calls → `[3,3,3,3]` (pre-fix: `[3,4,5,6]`) |
| `test_default_concurrent_calls_create_one_instance` | 8 concurrent calls → store stays at 1 |

## Verification
- `vol-llm-sandbox`: 86/86 pass, including with `--features ssh`
- `vol-agent-server`: 17/17 sandbox tests pass, covering all six handler ops that route through `default()`
- `manager.rs` line coverage 98.39% → 98.67%; crate total 68.47% → 68.73% (the sub-80 gap is the pre-existing `local.rs` / `tmp.rs` package-scoped shortfall, unrelated)
- `just check` / `fmt-check` / `clippy-strict` / `no-doc-tests` / `boundaries` / `no-clippy-allow` all pass

## Notes / Open Questions
- **`preload()` residency is still unaddressed.** Eagerly instantiating every profile at startup is unnecessary (`acquire_by_name()` already creates lazily) and for SSH is net-negative: `SSHSandbox::new()` spawns a 1-second-tick background idle task that lives as long as the `Arc`, while `create()` never calls `start()` so no connection is actually warmed. Two SSH profiles means two permanent tickers per process whether or not the sandboxes are ever used.
- **`default_tmp()` reuses the string `"default-tmp"`** for its spec name but goes through `build_inline()`, which never records anything — so there is no collision with the now-reserved profile today. The name overlap is still a readability trap.
- The warn-and-skip in `preload()` / `load_dir()` that hid [[schema-drift]] remains in place.

## Entities Mentioned
- [[vol-llm-sandbox-crate]]: `default()` rewritten; `DEFAULT_TMP_PROFILE` const added and re-exported; `default_lock` field added
- [[vol-agent-server-crate]]: unchanged, but its `SandboxHandler` is the sole production caller of `default()`

## Concepts Covered
- [[sandbox-lifecycle]]: clarifies which providers have instance identity worth recording in the `SandboxStore`
- [[schema-drift]]: same class of silent-misbehavior failure — a wrong default is as invisible as a skipped config
