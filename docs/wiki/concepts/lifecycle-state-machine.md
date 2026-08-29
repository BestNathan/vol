---
type: concept
category: pattern
tags: [state-machine, lifecycle, validation, sandbox]
created: 2026-08-27
updated: 2026-08-28
source_count: 2
---

# Lifecycle State Machine

The transition-validation mechanism inside `SandboxManager`. Full lifecycle treatment, including which parts are actually reachable, is in [[sandbox-lifecycle]].

> **Corrected 2026-08-28.** This page previously presented the declared state machine — `Creating → Created → Running`, `pause`/`resume`, capability-gated operations — as the operative behavior. Probing showed otherwise: only `Running` and `Stopped` are ever assigned, `pause`/`resume` have no `SandboxManager` method, and `destroy()` performs no validation at all.

## The mechanism

`validate_transition(from, to) -> SandboxResult<()>` is a `matches!` over allowed `(from, to)` pairs, returning `SandboxError::InvalidTransition { from, to }` on anything else.

```text
(Created,    Starting)  | (Created,  Running)
(Starting,   Running)
(Running,    Pausing)   | (Running,  Stopping)  | (Running, Stopped)
(Pausing,    Paused)
(Paused,     Starting)  | (Paused,   Running)
(Paused,     Stopping)  | (Paused,   Stopped)
(Stopping,   Stopped)
(Stopped,    Starting)  | (Stopped,  Running)
(Stopped,    Destroying)| (Stopped,  Destroyed)
(Destroying, Destroyed)
(_,          Failed)
```

## What the manager actually asks it

Only two call sites exist, and each passes a fixed `to`:

| Call site | `to` | Therefore succeeds from |
|---|---|---|
| `start(id)` | `Running` | `Created`, `Starting`, `Paused`, `Stopped` |
| `stop(id)` | `Stopped` | `Running`, `Paused`, `Stopping` |

`destroy(id)` does **not** call it. No other operation does either.

Because `create()` writes records directly at `Running`, and nothing ever assigns `Created` / `Starting` / `Pausing` / `Paused` / `Stopping` / `Destroying` / `Destroyed` / `Creating` / `Failed`, the reachable subset collapses to `Running ⇄ Stopped`:

| Call | From | Result |
|---|---|---|
| `start()` | `Running` | `InvalidTransition: Running -> Running` |
| `start()` | `Stopped` | ok |
| `stop()` | `Running` | ok |
| `stop()` | `Stopped` | `InvalidTransition: Stopped -> Stopped` |
| `destroy()` | anything | ok, unvalidated |

So the practical effect of the validator is narrow but real: it makes `start()` and `stop()` idempotency errors rather than silent no-ops.

## Ordering

Validation happens *before* the provider call, and the status update *after* it:

```text
load record
validate_transition(record.status, target)?   // reject early
provider.stop(backend_id).await?              // may fail
store.update_status(id, target).await?        // only on success
```

A failing provider call therefore leaves the recorded status unchanged — no phantom `Stopped` for a sandbox that refused to stop.

## Caveat: status is not authority

For `local` and `ssh`, `stop()` is a no-op at the provider level and `get()` keeps returning a working handle afterwards. A `Stopped` record does not mean execution is prevented. Status is bookkeeping; see [[sandbox-lifecycle]].

## Error handling

```text
match manager.stop(&id).await {
    Ok(()) => ...,
    Err(SandboxError::InvalidTransition { from, to }) => ...,  // already stopped
    Err(e) => ...,
}
```

## Related Concepts
- [[sandbox-lifecycle]] — the full lifecycle, declared vs. reachable
- [[sandbox-architecture]] — layers and resolution paths
- [[provider-pattern]] — backend adapter pattern
- [[vol-llm-sandbox-crate]] — API reference
