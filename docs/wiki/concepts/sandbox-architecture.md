---
type: concept
category: architecture
tags: [sandbox, architecture, provider, manager, store, overview, execution]
created: 2026-08-28
updated: 2026-08-28
source_count: 3
---

# Sandbox Architecture

## Overview

Every command and file operation an agent performs goes through a **sandbox** — an abstraction over *where* execution happens. The same tool code runs against the local filesystem, a temp directory, or a remote host over SSH, without knowing which.

The subsystem is four layers. Read them as: *what can I do* → *how is this backend built* → *what is recorded* → *who ties it together*.

```text
                        callers (ReAct agent, cli-tool, sandbox.* RPC)
                                          │
                                          ▼
   ┌───────────────────────────────────────────────────────────────────┐
   │  SandboxManager          orchestration: resolve a name/id to a    │
   │                          usable handle; own the lifecycle         │
   │  ┌─────────────┬──────────────┬──────────────┬────────────────┐   │
   │  │ specs       │ instances    │ store        │ name_to_backend│   │
   │  │ name→spec   │ backend_id   │ SandboxId    │ name→backend_id│   │
   │  │ (templates) │ →live handle │ →record      │ (index)        │   │
   │  └─────────────┴──────────────┴──────────────┴────────────────┘   │
   └───────────────────────────────────┬───────────────────────────────┘
                                       │ routes by spec.provider()
                    ┌──────────────────┼──────────────────┐
                    ▼                  ▼                  ▼
            SandboxProvider    SandboxProvider    SandboxProvider
              "local"              "tmp"              "ssh"
                    │                  │                  │
                    ▼                  ▼                  ▼
             LocalSandbox         TmpSandbox         SSHSandbox
                    └──────────────────┴──────────────────┘
                                       │  all implement
                                       ▼
                          ┌──────────────────────────┐
                          │  trait Sandbox           │
                          │  execute / read_file /   │
                          │  write_file / read_dir / │
                          │  create_dir_all /        │
                          │  metadata / resolve_path │
                          └──────────────────────────┘
```

## The four layers

### 1. `trait Sandbox` — the execution surface

What callers actually use. Backend-agnostic:

| Method | Purpose |
|---|---|
| `execute(CommandRequest)` | run a command, get `CommandOutput` |
| `read_file` / `write_file` | file I/O |
| `create_dir_all` / `read_dir` / `metadata` | directory and stat operations |
| `resolve_path(rel)` | resolve a relative path against the sandbox root |
| `id()` / `kind()` / `status()` / `root_path()` | identity and introspection |

`root_path()` returns `Option<&Path>` — not every backend has a meaningful local path.

### 2. `trait SandboxProvider` — the backend adapter

Builds and manages instances of one backend kind. Keyed by `kind()` (`"local"`, `"tmp"`, `"ssh"`).

`create(spec) -> BackendSandboxRef { backend_id, sandbox }`, plus `get(backend_id)`, `list()`, and the lifecycle verbs `start` / `pause` / `resume` / `stop` / `destroy`.

**`backend_id` is the key concept.** It identifies the instance *to the provider*, and whether it carries real information differs sharply by backend:

| Provider | `backend_id` | Derived from | Instance identity? |
|---|---|---|---|
| `local` | the work dir path | spec only | **No** — same spec ⇒ interchangeable objects |
| `ssh` | `user@host` | spec only | **No** — same spec ⇒ interchangeable objects |
| `tmp` | random `/tmp/<x>` path | fresh per create | **Yes** |
| `firecracker` | VM id | fresh per create | **Yes** |

This distinction drives everything about what is worth recording — see [[sandbox-lifecycle]].

### 3. `trait SandboxStore` — instance metadata

Persists `SandboxRecord { id: SandboxId, profile, provider_kind, backend_id, status, created_at, updated_at, metadata }`, queryable via `SandboxFilter { profile, provider_kind, status }`. `InMemorySandboxStore` is the only implementation — **records do not survive a restart**.

### 4. `SandboxManager` — orchestration

Holds four maps, which is the thing to internalize:

| Map | Key → value | Written by |
|---|---|---|
| `specs` | profile name → `SandboxSpec` | `load_profiles()`, `register_profile()` |
| `instances` | `backend_id` → live `Arc<dyn Sandbox>` | `create()`, `acquire_by_name()`, `get()`, `default()`, `register_instance()` |
| `store` | `SandboxId` → `SandboxRecord` | `create()`, `default()`, `register_instance()` |
| `name_to_backend` | profile name → `backend_id` | `create()`, `acquire_by_name()`, `register_instance()`; pruned by `destroy()` |

**`specs` and `store` are independent.** A profile can be loaded and live without any store record, and that is the normal production state. `sandbox.list` reads `store`; `sandbox.list_specs` reads `specs`. They legitimately disagree.

## Declared vs. actually wired

The crate declares more than it wires up. This matters when reading the code or the older wiki pages, which describe the declared shape as though it were all reachable.

| Thing | Declared | Reachable in production |
|---|---|---|
| Provider kinds in `SandboxProviderConfig` | `local`, `tmp`, `ssh`, `firecracker`, `wasm` | **`local`, `tmp`, `ssh` only** |
| `SandboxProvider` impls | `local`, `tmp`, `ssh` | same |
| `FirecrackerSandbox` / `WasmSandbox` | implement `Sandbox` | **no `SandboxProvider` impl** — unreachable via the manager |
| `SandboxStatus` variants | 11 | **`Running`, `Stopped`** — the other 9 are never assigned |
| Lifecycle verbs on `SandboxManager` | `create` / `start` / `stop` / `destroy` | present, but `create()` has **zero production callers** |
| `pause` / `resume` | on `SandboxProvider`, in `SandboxCapabilities`, in the transition table | **no `SandboxManager` method at all** |
| `SandboxCapabilities` | `persistent` / `pausable` / `stoppable` / `destroyable` | **advisory only** — read once for reporting in `list()`, never enforced |
| `sandbox.*` RPC operations | `list`, `list_specs`, `exec`, `read_file`, `write_file`, `create_dir`, `read_dir`, `metadata` | same — **no lifecycle ops on the wire** |

A `provider = "firecracker"` profile *parses* (the spec variant exists) but fails at instantiation with `UnknownType("firecracker")`, and because profile loading warn-and-skips, that failure is silent. See [[schema-drift]].

## How a sandbox gets resolved

Five entry points. Only some are used in production:

| Entry point | Records in `store`? | Caches handle? | Production callers |
|---|---|---|---|
| `acquire_by_name(name)` | **No** | Yes | ReAct tool loop, cli-tool loader, `preload()` |
| `preload(dir)` | No | Yes | `AgentRuntimeBuilder`, `cli-tools-mcp` startup |
| `build_inline(spec)` | No | No | cli-tool inline `[sandbox]` blocks |
| `default()` | Yes | Yes | `SandboxHandler` — all six `sandbox.*` I/O ops |
| `default_tmp()` | No | No | ReAct fallback when a named lookup misses |
| `create(profile)` | Yes | Yes | **none** — tests only |

Because every real resolution path goes through `acquire_by_name()`, which does not write a store record, `sandbox.list` is empty in production while `sandbox.list_specs` returns every configured profile. That is correct behavior, not a defect — see [[sandbox-lifecycle]].

### Startup wiring

`AgentRuntimeBuilder::build()` is the authoritative assembly point:

1. register the `local`, `tmp`, `ssh` providers
2. register a `"local"` profile pointing at `working_dir`, so `sandbox = "local"` reaches project files
3. `preload(.agents/sandboxes/)` — parse every `*.toml` into a spec, then eagerly instantiate each

`DataPlaneServerCore` reuses `runtime.sandbox_manager` rather than building its own, so control-plane `sandbox.*` calls and data-plane tool execution share instances.

### Selecting a sandbox as a caller

ReAct tool loop precedence:

1. per-tool override — `tool_config.get_sandbox(tool_name)`
2. agent default — `config.default_sandbox`
3. literal `"local"`

then `acquire_by_name(name)`, falling back to `default_tmp()` if the name does not resolve.

cli-tool configs pick exactly one of `sandbox_ref = "<profile-name>"` (resolved by `acquire_by_name`) or an inline `[sandbox]` table (built by `build_inline`); setting both, or neither, is a config error.

## Configuration

One TOML file per profile in `.agents/sandboxes/`, parsed as `SandboxSpec`. `spec.rs` is the **single schema source**. The `provider` field is a serde tag selecting the variant; for `ssh` the fields are flattened into the same table.

```toml
# local — run directly on the host filesystem
name = "local-for-cli"
provider = "local"
work_dir = "/tmp"          # optional
```

```toml
# ssh — run on a remote host
name = "ansible-prod"
provider = "ssh"
work_dir = "/opt/ansible"  # remote working directory
host = "192.168.2.106"
user = "root"
port = 22                  # default 22
key_path = "/app/.ssh/id_ed25519"
host_key = "SHA256:..."    # pin the host key
# also: passphrase, known_hosts_file
# idle_timeout_secs   default 300
# connect_timeout_secs default 10
```

```toml
# tmp — ephemeral scratch directory
name = "scratch"
provider = "tmp"
sub_dir = "my-run"         # optional; otherwise randomized
```

`identity_file` is accepted as a backward-compatible alias for `key_path`; `as_ssh()` resolves `key_path.or(identity_file)`. Prefer `key_path` in new configs.

The `firecracker` and `wasm` variants take a nested table (`[firecracker]`, `[wasm]` with a `wasm.modules` array-of-tables), but as noted above have no provider and cannot currently be instantiated.

> Secrets: SSH keys are mounted paths, not inline material. Never put a passphrase or key body in a config that lands in git or a ConfigMap.

## Related

- [[sandbox-lifecycle]] — lifecycle management, state transitions, and instance identity
- [[vol-llm-sandbox-crate]] — API reference and crate layout
- [[provider-pattern]] — the backend adapter pattern in general
- [[capability-discovery]] — runtime capability reporting
- [[cli-style-tool-pattern]] — declarative cli-tools, the heaviest sandbox consumer
- [[agent-server-control-data-plane]] — where the manager is instantiated and shared
- [[schema-drift]] — how silent profile-loading failures hide misconfiguration
