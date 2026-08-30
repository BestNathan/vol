---
type: concept
category: architecture
tags: [sandbox, lifecycle, manager, state-machine, instance-identity, status]
created: 2026-08-11
updated: 2026-08-28
source_count: 4
---

# Sandbox Lifecycle Management

For the layer diagram, provider matrix, config schema, and resolution paths, see [[sandbox-architecture]]. This page covers **lifecycle and state transitions** specifically.

## The short version

The declared lifecycle is an 11-state machine with `pause`/`resume`, capability gating, and validated transitions. **The reachable lifecycle is two states.** Understanding the gap is the point of this page, because the code, the type definitions, and the older wiki pages all describe the declared shape.

| | Declared | Actually reachable |
|---|---|---|
| States | 11 (`Creating`…`Failed`) | **`Running`, `Stopped`** |
| Manager verbs | `create` / `start` / `stop` / `destroy` / (`pause`/`resume` in the transition table) | `create` / `start` / `stop` / `destroy` — **no `pause`/`resume` method exists** |
| Transition validation | on every operation | on `start` and `stop` only — **`destroy` skips it entirely** |
| Capability enforcement | backends declare limits, orchestration respects them | **never enforced** — read once for reporting in `list()` |
| RPC lifecycle ops | — | **none on the wire** |

## Instance identity: what is worth tracking

This decides whether a lifecycle even applies.

A store record only carries information when the instance holds state the spec does not. `backend_id` is a **pure function of the spec** for `local` (the work dir path) and `ssh` (`user@host`) — build the same spec twice and you get two interchangeable objects, so there is nothing to track. `tmp` (a random `/tmp/<x>` path) and `firecracker` (a VM id) do carry per-instance state.

Consequences:

- `sandbox.list` (reads the store) is **legitimately empty** in a deployment whose profiles are all local/ssh. `sandbox.list_specs` (reads the spec map) answers "what is configured".
- Manufacturing store records for local/ssh just to populate `list` would invent identity that does not exist — and would trip the leak described in [[sandbox-default-idempotency]].
- `SSHSandboxProvider::get(backend_id)` always returns `NotFound`: an SSH instance cannot be reconstructed from its `backend_id`, which is the same fact from the other direction.

## States

`SandboxStatus` declares eleven variants: `Creating`, `Created`, `Starting`, `Running`, `Pausing`, `Paused`, `Stopping`, `Stopped`, `Destroying`, `Destroyed`, `Failed`.

**Only `Running` and `Stopped` are ever written by any code in `src/`.** The nine transitional and terminal states — including `Failed` — are declared but never assigned. `create()` inserts a record already at `Running`; it never passes through `Creating`/`Created`.

## Transitions

`validate_transition(from, to)` encodes 17 pairs plus `(_, Failed)`. But the manager only ever calls it with `to = Running` (from `start()`) or `to = Stopped` (from `stop()`), so most pairs are unreachable. What actually happens:

```text
            create()
               │
               ▼
         ┌──────────┐   stop()    ┌──────────┐
         │ Running  │────────────►│ Stopped  │
         │          │◄────────────│          │
         └────┬─────┘   start()   └────┬─────┘
              │                        │
              │      destroy()         │
              └───────────┬────────────┘
                          ▼
                 record deleted, handle
                 evicted, name unmapped
                 (no state validation)
```

| Call | From | Result |
|---|---|---|
| `start()` | `Running` | **fails** — `InvalidTransition: Running -> Running` |
| `start()` | `Stopped` | ok → `Running` |
| `stop()` | `Running` | ok → `Stopped` |
| `stop()` | `Stopped` | **fails** — `InvalidTransition: Stopped -> Stopped` |
| `destroy()` | any state | ok — no validation performed |

Two consequences worth internalizing:

**`start()` after `create()` always fails.** `create()` lands the record at `Running`, and `(Running, Running)` is not a valid pair. The declared `Created → Starting → Running` path is unreachable because `Created` is never assigned. A freshly created sandbox is already usable; calling `start()` on it is an error.

**`destroy()` ignores the state machine and the capability flags.** It never calls `validate_transition`, so it succeeds from `Running` even though the table only permits `Stopped → Destroying`. It also does not consult `capabilities().destroyable`, which is `false` for both `local` and `ssh` — those providers' `destroy()` is simply a no-op that returns `Ok(())`, and the manager then deletes the record regardless.

### `stop()` on local/ssh is bookkeeping only

`LocalSandboxProvider::stop()` returns `Ok(())` without doing anything, and `get()` on a stopped instance still hands back a **fully working** handle. So `stop()` flips a status field and nothing else — it does not prevent execution. This is consistent with `stoppable: false` in those providers' capabilities, but nothing enforces it: the status change is accepted and the sandbox keeps working.

## Capabilities are advisory

`SandboxCapabilities { persistent, pausable, stoppable, destroyable }` is intended for a UI to decide which lifecycle actions to offer.

| Provider | persistent | pausable | stoppable | destroyable |
|---|---|---|---|---|
| `local` | ✓ | ✗ | ✗ | ✗ |
| `tmp` | ✗ | ✗ | ✗ | ✓ |
| `ssh` | ✓ | ✗ | ✗ | ✗ |

`firecracker` and `wasm` declare no capabilities because they have no `SandboxProvider` impl at all.

The manager reads `capabilities()` in exactly one place — building `SandboxInfo` for `list()` — and never to gate an operation. `stop()` and `destroy()` both proceed on providers that declare they cannot be stopped or destroyed. Treat the flags as documentation of *intent*, not as a guarantee.

## Operation walkthroughs

### `create(profile) -> SandboxId`

1. look up the spec by profile name (`NotFound` if absent)
2. look up the provider by `spec.provider()` (`UnknownType` if unregistered)
3. `provider.create(spec)` → `BackendSandboxRef { backend_id, sandbox }`
4. insert a `SandboxRecord` at status `Running` with a fresh ULID `SandboxId`
5. cache the handle in `instances[backend_id]` and index `name_to_backend[profile]`

Note: **no production code calls this.** Real resolution goes through `acquire_by_name()`, which performs steps 1–3 and 5 but *not* step 4 — hence the empty `sandbox.list`.

### `get(id) -> Arc<dyn Sandbox>`

1. load the record (`NotFound` if absent)
2. return `instances[record.backend_id]` on a cache hit
3. otherwise `provider.get(backend_id)` and cache the result

Step 3 is where the identity distinction bites: it works for `local` and `tmp` (both reconstruct from a path) but always fails for `ssh`.

### `stop(id)` / `start(id)`

Load record → `validate_transition` → delegate to the provider → `store.update_status`. If the provider call fails the status is not updated, so a failed stop leaves the record at `Running`.

### `destroy(id)`

Load record → `provider.destroy(backend_id)` → evict `instances[backend_id]` → drop every `name_to_backend` entry pointing at that `backend_id` → `store.delete(id)`. No state validation, no capability check.

### `default()`

Returns the implicit scratch sandbox, keyed on the reserved `DEFAULT_TMP_PROFILE` (`"default-tmp"`) profile and serialized by a mutex, so it is idempotent: at most one such instance exists regardless of what else is in the store, and an unrelated instance is never returned. This is the only lifecycle path reachable over the RPC surface, since `SandboxHandler` calls it for all six of its I/O operations. Its previous count-based form leaked a record per call — see [[sandbox-default-idempotency]].

## Durability

`InMemorySandboxStore` is the only `SandboxStore` implementation, so **all instance records are lost on restart**. Profiles are re-read from disk by `load_profiles()` and re-instantiated on first use, so recovery is automatic for named profiles — but any `SandboxId` handed out before a restart becomes permanently unresolvable. A persistent store (SQLite/Postgres) is a natural extension; the trait boundary already exists for it.

### Dropping an instance releases it

`SSHSandbox` spawns a background idle-timeout task in `new()`. Since 2026-08-30 a `Drop` impl aborts it: dropping a `JoinHandle` does not stop a tokio task, so previously the loop kept its own `Arc<SshSession>` clone alive for the rest of the process and an evicted or destroyed SSH sandbox never released its connection. `destroy()` evicting the handle from `instances` is therefore now sufficient to actually free the backend. See [[sandbox-ssh-idle-task-lifecycle]].

## Known gaps

- `pause`/`resume` exist on `SandboxProvider` and in the transition table but have **no `SandboxManager` method**, so `Pausing`/`Paused` are unreachable.
- `Failed` is never assigned, so a failed instance is indistinguishable from a healthy one in the store.
- `destroy()` bypasses both transition validation and capability checks.
- Capability flags are never enforced.
- No lifecycle operation is exposed over the `sandbox.*` RPC surface, so none of this is drivable from the frontend.

## Related
- [[sandbox-architecture]] — layers, providers, config schema, resolution paths
- [[vol-llm-sandbox-crate]] — API reference and crate layout
- [[provider-pattern]] — backend adapter pattern
- [[capability-discovery]] — capability reporting
- [[sandbox-default-idempotency]] — the `default()` fix and instance identity
- [[sandbox-registry-manager-unification]] — how `SandboxManager` became the sole resolution path
- [[schema-drift]] — why silent profile-loading failures are dangerous
- [[vol-agent-server-crate]] — hosts the `sandbox.*` handler
