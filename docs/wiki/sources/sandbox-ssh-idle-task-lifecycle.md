---
type: source
source_type: code
date: 2026-08-30
ingested: 2026-08-30
tags: [sandbox, ssh, tokio, task-leak, drop, idle-timeout, lazy-loading]
---

# SSH Idle-Task Lifecycle and Lazy Profile Loading

**Authors/Creators:** Claude (Nathan)
**Date:** 2026-08-30
**Link:** `crates/vol-llm-sandbox/src/ssh/mod.rs`, `crates/vol-llm-sandbox/src/manager.rs`

## TL;DR
`SSHSandbox::new()` spawns a background idle-timeout task and stored its `JoinHandle` in a `_idle_task` field. Dropping a `JoinHandle` does **not** stop a tokio task, and there was no `Drop` impl — so the loop ran for the remaining process lifetime holding its own `Arc<SshSession>` clone. An SSH sandbox evicted from the manager's cache or passed to `destroy()` therefore never released its connection. Fixed with a `Drop` impl that calls `abort()`. Separately, the loop polled every second regardless of the configured window; it now sleeps to the deadline (~2 wakeups per 300s window instead of 300). And `SandboxManager::preload()` — which eagerly instantiated every profile, spawning one such task per SSH profile whether used or not — was deleted in favour of `load_profiles()`.

## Key Takeaways
- **`JoinHandle` is not a guard.** Storing a handle in a field named `_idle_task` reads like ownership-based cleanup, but tokio tasks are detached: dropping the handle abandons the task, it does not cancel it. The underscore prefix actively encouraged the misreading.
- **The leak was worse than the waste.** The original motivation was "eager preload spawns tickers nobody needs". Investigation found the tickers were never reclaimable *at all*, which makes it a correctness issue in `destroy()` rather than a startup inefficiency.
- **Lazy loading buys less than expected.** `cli_tool::load_dir()` calls `acquire_by_name()` for every `sandbox_ref` in order to decide whether to register that tool, and it runs in both `vol-agent-server` and `cli-tools-mcp`. Referenced profiles are therefore still instantiated during startup — just by the cli-tool loader. Only *unreferenced* profiles (here, `ssh-dev`) actually stay uninstantiated.
- **Regression tests were validated by reverting the fix.** Both leak tests were confirmed to fail with the `abort()` call commented out, then pass with it restored.

## Detailed Summary

### The leak

```text
SSHSandbox::new():
    session      = Arc::new(SshSession::new(config))
    session_clone = Arc::clone(&session)          // moved into the task
    _idle_task    = tokio::spawn(async move { loop { ... session_clone ... } })
```

With no `Drop` impl, `drop(sandbox)` released the sandbox's own `Arc` but left the spawned task holding `session_clone` forever. Consequences:

- `SandboxManager::destroy()` evicts the handle from `instances` and deletes the store record, but the SSH session stays open and the task stays scheduled.
- Repeated create/drop cycles accumulate one live task and one live session each.

Fix:

```rust
impl Drop for SSHSandbox {
    fn drop(&mut self) {
        self.idle_task.abort();
    }
}
```

The field was renamed `_idle_task` → `idle_task`, since it is now read.

### Polling → sleep-to-deadline

The loop slept a fixed 1s per iteration and compared elapsed time against the window. It now computes `idle_dur.checked_sub(elapsed)` and sleeps the remainder, re-checking on wake (activity during the sleep simply pushes the deadline out). At the default `idle_timeout_secs = 300` this drops from 300 wakeups per window to roughly 2.

A 1-second floor is applied to the post-disconnect sleep so a configured `idle_timeout_secs = 0` cannot spin. That matches the previous behavior, which also slept 1s per iteration in that case.

### `preload()` deleted

```text
preload(dir)  =  load_profiles(dir) + acquire_by_name() for every profile
```

The eager half was redundant: `acquire_by_name()` already creates on the slow path. It also never warmed anything for SSH — `SSHSandboxProvider::create()` does not call `start()`, so no connection is established — while spawning one idle task per SSH profile.

`load_profiles()` absorbed the warn-and-skip documentation that had lived on `preload()`. Callers updated:

| Call site | Was | Now |
|---|---|---|
| `vol-llm-runtime/src/lib.rs` | `preload(&sandboxes_dir)` | `load_profiles(&sandboxes_dir)` |
| `vol-mcp-servers/src/bin/cli_tools_mcp.rs` | `preload(&cli.sandboxes_dir)` | `load_profiles(&cli.sandboxes_dir)` |

### Tests

Three inline tests added in `ssh/mod.rs` (feature-gated on `ssh`, so they run under `just test-sandbox-ssh` and **not** under a plain `just test-crate`). They can live inline because they need access to the private `session` / `idle_task` fields, and they need no reachable host because `SSHSandbox::new()` does not connect.

| Test | Asserts |
|---|---|
| `dropping_sandbox_aborts_idle_task_and_releases_session` | a `Weak` to the session cannot upgrade after `drop(sandbox)` |
| `idle_task_is_aborted_not_merely_detached` | the task's `AbortHandle` reports finished after drop |
| `idle_task_does_not_fire_before_deadline` | a 60s-window sandbox is still waiting after 50ms |

In `manager_tests.rs`, the three `preload_*` tests were replaced. A `CountingProvider` with an `AtomicUsize` create counter makes laziness directly assertable rather than inferred:

| Test | Asserts |
|---|---|
| `load_profiles_registers_specs_without_instantiating` | 2 specs registered, **0** creates |
| `acquire_by_name_instantiates_on_demand_exactly_once` | 1 create after first acquire, still 1 after a second, sibling profile untouched |
| `load_profiles_succeeds_even_when_a_provider_would_fail` | loading no longer touches the provider; failure surfaces at acquire |
| `load_profiles_missing_dir_is_ok` | missing directory is not an error |

## Verification
- `vol-llm-sandbox`: 87/87 default features, **90/90 with `--features ssh`**
- Both leak tests confirmed to **fail** with `abort()` disabled, and pass with it restored — they are genuine regression tests, not tautologies
- `vol-llm-cli-tool` / `vol-llm-runtime` / `vol-mcp-servers`: 101/101
- `just check`, `just clippy-strict`, `just fmt-check` clean

## Notes / Open Questions
- `destroy()` still performs no transition validation and no capability check — unchanged here, see [[sandbox-lifecycle]].
- The warn-and-skip that hid [[schema-drift]] remains, now in `load_profiles()` / `load_dir()`.
- `default_tmp()` still names its inline spec `"default-tmp"`, colliding textually with the reserved `DEFAULT_TMP_PROFILE`. No functional conflict — `build_inline()` records nothing — but still a readability trap.
- Nothing else in the crate spawns a task, so this class of leak is contained to `ssh/mod.rs`. `FirecrackerSandbox` already had a `Drop` impl (returning its VM to the pool).

## Entities Mentioned
- [[vol-llm-sandbox-crate]]: `Drop for SSHSandbox`; `preload()` removed; `load_profiles()` documented as the sole loader
- [[vol-llm-runtime-crate]]: builder calls `load_profiles()`
- [[vol-mcp-servers-crate]]: `cli-tools-mcp` startup calls `load_profiles()`

## Concepts Covered
- [[sandbox-lifecycle]]: dropping an instance now actually releases the backend
- [[sandbox-architecture]]: `load_profiles` resolves nothing; the resolution table lost its `preload` row
