---
type: entity
category: service
tags: [sandbox, container, ssh, firecracker, tmp, wasm, rust, lifecycle, manager, provider]
created: 2026-06-17
updated: 2026-08-28
source_count: 7
---

# vol-llm-sandbox Crate

## Overview
`vol-llm-sandbox` is the sandbox abstraction: every command and file operation an agent performs is routed through a `Sandbox`, so the same tool code runs against the local filesystem, a temp directory, or a remote host over SSH without knowing which.

Conceptual material lives in two concept pages; this page is the **API and usage reference**:

- [[sandbox-architecture]] — the four layers, provider matrix, config schema, resolution paths
- [[sandbox-lifecycle]] — lifecycle, state transitions, instance identity, and what is declared vs. reachable

## Crate layout

| Module | Contents |
|---|---|
| `lib.rs` | `SandboxId`, `SandboxStatus`, `SandboxCapabilities`, `Sandbox` trait, `CommandRequest`/`CommandOutput`, `SandboxError` |
| `manager.rs` | `SandboxManager`, `DEFAULT_TMP_PROFILE` |
| `provider.rs` | `SandboxProvider` trait, `BackendSandboxRef`, `SandboxInfo` |
| `store.rs` | `SandboxStore` trait, `InMemorySandboxStore`, `SandboxRecord`, `SandboxFilter` |
| `spec.rs` | `SandboxSpec`, `SandboxProviderConfig`, `SshConfig`, `FirecrackerConfig`, `WasmConfig` — **single schema source** |
| `local.rs` | `LocalSandbox` + `LocalSandboxProvider` |
| `tmp.rs` | `TmpSandbox` + `TmpSandboxProvider` |
| `ssh/` | `SSHSandbox` + `SSHSandboxProvider`, `SshSession` (feature `ssh`) |
| `firecracker.rs` | `FirecrackerSandbox`, `FirecrackerPool` (feature `firecracker`) — **no provider impl** |
| `wasm.rs` | `WasmSandbox` (feature `wasm`) — **no provider impl** |

## Core types

| Type | Notes |
|---|---|
| `SandboxId` | ULID-based instance identifier, e.g. `sb_01JXYZ...`. Distinct from profile name. |
| `SandboxStatus` | 11 declared variants; only `Running` and `Stopped` are ever assigned — see [[sandbox-lifecycle]]. |
| `SandboxCapabilities` | `persistent` / `pausable` / `stoppable` / `destroyable`. **Advisory only** — never enforced. |
| `SandboxSpec` | `{ name, config: SandboxProviderConfig, metadata }` — a profile template. |
| `SandboxRecord` | Store row: `{ id, profile, provider_kind, backend_id, status, created_at, updated_at, metadata }`. |
| `SandboxFilter` | `{ profile, provider_kind, status }` — all optional, ANDed. |
| `BackendSandboxRef` | `{ backend_id, sandbox }` — what a provider returns from `create()`. |

## `Sandbox` trait

```text
fn  id() -> &SandboxId
fn  kind() -> &str
fn  status() -> SandboxStatus
fn  root_path() -> Option<&Path>
fn  resolve_path(rel: &str) -> SandboxResult<PathBuf>
async execute(CommandRequest) -> SandboxResult<CommandOutput>
async read_file(path, ..) / write_file(path, &[u8])
async create_dir_all(path) / read_dir(path) -> Vec<DirEntry> / metadata(path) -> FileMetadata
```

`root_path()` is `Option` because not every backend has a meaningful local path.

Removed in the 2026-08-27 refactor: `name()`, `start()`, `cleanup()`, `bind_metadata()` — these moved to the provider/manager layer.

### Path resolution contract
- `resolve_path` rejects absolute paths (`/...`) and `~` paths in every implementation.
- `ToolContext::resolve_path` converts absolute paths that fall inside the sandbox root to relative before delegating, so tools can keep passing absolute paths (e.g. from `tempfile`) while sandboxes see consistent relative input.

## `SandboxProvider` trait

```text
fn  kind() -> &str
fn  capabilities() -> SandboxCapabilities
async create(spec) -> BackendSandboxRef
async get(backend_id) -> Arc<dyn Sandbox>
async list() -> Vec<SandboxInfo>
async start / pause / resume / stop / destroy (backend_id)
```

`pause` and `resume` exist here but have **no counterpart on `SandboxManager`**, so they are unreachable through normal use.

## `SandboxManager` API

The sole sandbox resolution path since 2026-08-28 (`SandboxRegistry` deleted).

**Setup**

| Method | Purpose |
|---|---|
| `register_provider(provider)` | register a backend by `kind()` |
| `register_profile(spec)` | add a profile programmatically |
| `load_profiles(dir)` | parse `.agents/sandboxes/*.toml` into specs; per-file failures warn+skip |

**Profile-oriented (by name)** — this is what production uses

| Method | Records in store? | Purpose |
|---|---|---|
| `acquire_by_name(name)` | No | cache hit, else create via provider and cache. Returns `Option`. |
| `preload(dir)` | No | `load_profiles()` then eagerly instantiate each; per-profile failures warn+skip |
| `build_inline(spec)` | No | one-off, uncached — for cli-tool inline `[sandbox]` blocks |
| `default_tmp()` | No | fresh `TmpSandbox`, falling back to `LocalSandbox` |
| `default()` | Yes | the idempotent scratch sandbox keyed on `DEFAULT_TMP_PROFILE` |

**Instance-oriented (by `SandboxId`)**

| Method | Purpose |
|---|---|
| `create(profile)` | full create + record + cache. **Zero production callers** — tests only. |
| `get(id)` | record → cache hit → else `provider.get(backend_id)` |
| `list(filter)` | reads the **store**; enriches with `capabilities()` and cached `root_path()` |
| `list_specs()` | reads the **spec map** — the right call for "what is configured" |
| `start(id)` / `stop(id)` | validated transition + provider delegation + status update |
| `destroy(id)` | provider destroy, evict cache, prune name index, delete record. **No validation.** |
| `register_instance(spec, sandbox)` | adopt a pre-existing sandbox into the store |

Internal maps: `specs`, `instances` (by `backend_id`), `store` (by `SandboxId`), and a `name_to_backend` index maintained by `create()` / `acquire_by_name()` / `register_instance()` and pruned by `destroy()`.

## Implementations

| Type | Kind | Provider | Root | Use case |
|---|---|---|---|---|
| `LocalSandbox` | `local` | `LocalSandboxProvider` | `Some(path)`, or random temp if `None` | Development, testing, DP nodes |
| `TmpSandbox` | `tmp` | `TmpSandboxProvider` | `/tmp/{sub_dir}/` | Scratch space, `default()` fallback |
| `SSHSandbox` | `ssh` | `SSHSandboxProvider` | remote `work_dir` | Remote execution |
| `FirecrackerSandbox` | `firecracker` | **none** | VM rootfs | Not currently usable via the manager |
| `WasmSandbox` | `wasm` | **none** | work dir preopened as `/` | Not currently usable via the manager |

## Configuration

`spec.rs` is the single schema source. `SandboxProviderConfig` is a serde internally-tagged enum on `provider`; every variant carries `work_dir`, but there is **no** uniform accessor for it — read it via the variant or the `as_*` helper.

| Variant | Layout |
|---|---|
| `local` | `work_dir` |
| `tmp` | `work_dir`, `sub_dir` |
| `ssh` | fields flattened into the top-level table |
| `firecracker` | nested `[firecracker]` table |
| `wasm` | nested `[wasm]` table + a `wasm.modules` array-of-tables |

Extraction helpers: `as_local()` / `as_tmp()` / `as_ssh()` (return owned config structs), `as_firecracker()` / `as_wasm()` (return references).

Full schema with examples and defaults: [[sandbox-architecture]].

`FirecrackerConfig` / `WasmConfig` / `WasmModuleConfig` live in `spec.rs` **without** feature gates, so config parsing never depends on which features are compiled.

## Usage

### Resolve a named profile

```text
let sandbox = manager.acquire_by_name("ansible-prod").await
    .ok_or("profile not found or provider missing")?;
let out = sandbox.execute(CommandRequest::new("ansible --version")).await?;
```

`acquire_by_name` returns `Option` — `None` covers both "no such profile" and "provider create failed", and logs a warning in the latter case.

### Wire up a manager from scratch

```text
let manager = SandboxManager::new();
manager.register_provider(Arc::new(LocalSandboxProvider)).await;
manager.register_provider(Arc::new(TmpSandboxProvider)).await;
manager.register_provider(Arc::new(SSHSandboxProvider::new())).await;
manager.preload(Path::new(".agents/sandboxes")).await?;
```

Forgetting a `register_provider` is a silent failure: profiles of that kind parse fine, then fail to instantiate, and `preload()` warns and skips them.

### Runtime integration

- `AgentRuntimeBuilder::build()` is the authoritative assembly point: registers `local`/`tmp`/`ssh`, registers a `"local"` profile pointing at `working_dir`, then `preload()`s.
- `"local"` is an ordinary profile via `register_profile()`, not a special case — it resolves through the same path as disk-loaded profiles.
- `DataPlaneServerCore` reuses `runtime.sandbox_manager` rather than constructing its own, so control-plane `sandbox.*` operations and data-plane tool execution observe the same instances.
- `cli-tools-mcp` builds its own manager, registers the same three providers, and calls `preload()`.

### How callers select a sandbox

ReAct tool loop, in precedence order: `ToolConfig.get_sandbox(tool)` → `AgentDef.sandbox` → literal `"local"`; then `acquire_by_name()` with a `default_tmp()` fallback.

cli-tool configs set exactly one of `sandbox_ref = "<profile>"` (via `acquire_by_name`) or an inline `[sandbox]` table (via `build_inline`). Both or neither is a config error.

Agents using `sandbox = "local"` land in `/app/` on DP nodes.

## Known gaps
- `InMemorySandboxStore` is the only store — instance metadata does not survive restart, so any `SandboxId` becomes unresolvable after one.
- No `SandboxProvider` for `firecracker` or `wasm`. Their spec variants parse, then `preload()` warns and skips for lack of a registered provider.
- `SSHSandboxProvider::get(backend_id)` returns `NotFound` unconditionally — only the `instances` cache resolves SSH sandboxes after creation.
- No `pause`/`resume` on `SandboxManager`, so `Pausing`/`Paused` are unreachable.
- `SandboxCapabilities` is never enforced; `destroy()` skips both capability checks and transition validation.
- No lifecycle operation is exposed on the `sandbox.*` RPC surface.
- `preload()`'s eager instantiation is unnecessary and, for SSH, net-negative — see [[sandbox-lifecycle]].

## Timeline
- **2026-06-17**: Initial sandbox abstraction, LocalSandbox, SSHSandbox, FirecrackerSandbox
- **2026-08-11**: TmpSandbox added; resolve_path unified; bind_metadata trait method; registry simplified
- **2026-08-19**: LocalSandbox timeout kill reworked — positive-pid kills only
- **2026-08-25**: SandboxHandler refactored to accept `Arc<SandboxRegistry>` for listing all sandboxes
- **2026-08-27**: **Major refactor** — explicit lifecycle management with SandboxManager, SandboxProvider, SandboxStore, SandboxId, SandboxStatus, SandboxCapabilities. All implementations updated.
- **2026-08-28**: **`SandboxRegistry` deleted** — `SandboxManager` is now the sole resolution path. `spec.rs` became the single schema source: SSH gained `host_key` / `known_hosts_file` / `passphrase` / timeout fields, `Firecracker` / `Wasm` variants added, all variants carry `work_dir`. `SSHSandboxProvider` dropped its `configs: Vec<SshConfig>` workaround and reads spec fields directly. Manager gained `acquire_by_name` / `preload` / `build_inline` / `default_tmp`. Fixes [[schema-drift]] that had `cli-tools-mcp` registering zero tools — see [[sandbox-registry-manager-unification]].
- **2026-08-28**: `SandboxManager::default()` made idempotent — it previously branched on the *total* store record count, so with >=2 records it created and registered a new tmp sandbox on every call (unbounded leak via the handler's six ops), and with exactly 1 record it returned that unrelated instance. Now keyed on the reserved `DEFAULT_TMP_PROFILE` and serialized by `default_lock`. See [[sandbox-default-idempotency]].
- **2026-08-28**: Documentation audit — [[sandbox-architecture]] created as the entry point; [[sandbox-lifecycle]] rewritten around declared-vs-reachable behavior after probing found only `Running`/`Stopped` are ever assigned, no `pause`/`resume` exists on the manager, `destroy()` skips validation, and capabilities are never enforced.
