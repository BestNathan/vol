---
type: source
source_type: code
date: 2026-08-28
ingested: 2026-08-28
tags: [sandbox, manager, registry, spec, ssh, cli-tool, mcp, refactor, schema-drift]
---

# Sandbox Registry → Manager Unification

**Authors/Creators:** BestNathan
**Date:** 2026-08-28
**Link:** `crates/vol-llm-sandbox/`, `crates/vol-llm-cli-tool/`, `crates/vol-llm-runtime/`, `crates/vol-agent-server/`, `crates/vol-mcp-servers/`

## TL;DR

`cli-tools-mcp` was registering **zero tools** in production. Root cause was not a single bug but a two-system schema drift: commit `11a28f09` (2026-08-27) rewrote all four `.agents/sandboxes/*.toml` files to the new `SandboxSpec` format (`provider = "..."` + flattened SSH fields), but `SandboxRegistry::load()` — still the loader used by `AgentRuntime` and `cli-tools-mcp` — parsed the *old* `SandboxConfig` format (`type = "..."` + `[ssh]` subtable). Every sandbox failed to parse, so every `sandbox_ref` lookup missed, so all three cli-tools were silently skipped. This ingest deletes `SandboxRegistry` entirely and routes all sandbox resolution through `SandboxManager`, closing the drift at its source.

## Key Takeaways

- **Two parallel sandbox systems existed.** `SandboxRegistry` (old, data-plane: `AgentRuntime`, `cli-tools-mcp`, ReAct agent) and `SandboxManager` (new, control-plane: frontend sandbox panel, `sandbox.list_specs`). Only the TOML files had been migrated to the new schema.
- **Failure was silent by design.** `SandboxRegistry::load()` logs parse failures at `warn` and skips; `cli_tool::load_dir()` logs missing `sandbox_ref` at `warn` and skips. Neither returns an error, so startup succeeded with zero tools registered.
- **The new spec was incomplete.** `SandboxProviderConfig::Ssh` was missing `host_key`, `known_hosts_file`, `passphrase`, `idle_timeout_secs`, `connect_timeout_secs` — all fields the production `ansible-prod` / `ssh-dev` configs actually use. `SSHSandboxProvider::create()` worked around this with a `configs: Mutex<Vec<registry::SshConfig>>` field and a comment reading *"For now, use the first registered config"*.
- **No TOML changes were needed.** The four config files were already correct; the code was wrong. This is the diagnostic signature of schema drift — configs and parser disagree, and the configs are the newer artifact.
- **`SandboxManager` gained the registry's lookup semantics** rather than callers being rewritten around instance IDs: `acquire_by_name()`, `preload()`, `build_inline()`, `default_tmp()`.

## Detailed Summary

### Root cause chain

```
SandboxRegistry::load(.agents/sandboxes/*.toml)
  → toml::from_str::<SandboxConfig>()   requires `type`
  → TOML says `provider = "ssh"`
  → parse error "missing field `type`" → warn + skip (all 4)
        ↓
registry empty (ansible-prod / gh-sandbox / local-for-cli / ssh-dev all absent)
        ↓
cli_tool::load_dir(.agents/cli-tools/*.toml, registry)
  → ansible.toml   sandbox_ref="ansible-prod"   → get() = None → warn + skip
  → gh.toml        sandbox_ref="gh-sandbox"     → get() = None → warn + skip
  → echo-tool.toml sandbox_ref="local-for-cli"  → get() = None → warn + skip
        ↓
0 cli-tools registered, in both AgentRuntime and cli-tools-mcp
```

Verified by extracting `SandboxConfig` into a standalone parse harness and feeding it the production TOMLs — both `gh-sandbox.toml` and `ansible-prod.toml` failed with `missing field 'type'`.

### `spec.rs` — completed the provider config

`SandboxProviderConfig::Ssh` gained the five missing fields, plus an `identity_file` alias for backward-compatible TOML (`as_ssh()` resolves `key_path.or(identity_file)`). `Firecracker` and `Wasm` variants were added, carrying `FirecrackerConfig` / `WasmConfig` / `WasmModuleConfig` — moved from `registry.rs` into `spec.rs` **without** feature gates, so type definitions stay available regardless of which features are compiled.

All five variants now carry a shared `work_dir`, and `SandboxProviderConfig::work_dir()` reads it uniformly. Serde's internally-tagged enum places `[firecracker]` / `[wasm]` nested tables under the field whose name matches the provider tag, so the existing nested-table TOML layout is preserved.

`spec::SshConfig` (the `as_ssh()` extraction type) became the canonical SSH config, absorbing `registry::SshConfig`. It uses `key_path: Option<PathBuf>` rather than the old required `identity_file: String`, which let `SshSandboxConfig` support ssh-agent-only auth.

### `SSHSandboxProvider` — removed the config hack

The `configs: Mutex<Vec<SshConfig>>` field, its `add_config()` method, and the "use the first registered config" behavior are gone. `create()` now reads `spec.config.as_ssh()` directly, so each spec produces a correctly-configured sandbox instead of all SSH specs sharing whichever config was registered first.

`SSHSandbox::new()` dropped its redundant `work_dir: Option<String>` parameter — `work_dir` comes from the spec's SSH config.

`session.rs::authenticate()` was restructured for the now-optional key path: try key+passphrase, then ssh-agent, then bare key file. Previously a missing `identity_file` was impossible (it was a required `String`).

### `SandboxManager` — absorbed registry lookup semantics

| New method | Replaces | Behavior |
|---|---|---|
| `acquire_by_name(name)` | `SandboxRegistry::acquire(name)` / `get(name)` | Profile-name lookup; cache hit, else create via provider and cache |
| `preload(dir)` | `SandboxRegistry::load(dir)` | `load_profiles()` then eagerly instantiate every profile; per-profile failures warn+skip |
| `build_inline(spec)` | `SandboxRegistry::build_sandbox(cfg)` | One-off sandbox from a spec, not cached or tracked (for cli-tool inline `[sandbox]`) |
| `default_tmp()` | `SandboxRegistry::default()` | Fresh `TmpSandbox`, falling back to `LocalSandbox` |

A `name_to_backend: RwLock<HashMap<String, String>>` index maps profile name → `backend_id` so `acquire_by_name` is a cache hit on the common path. It is maintained by `create()`, `acquire_by_name()`, and `register_instance()`, and pruned by `destroy()`.

`preload()` is safe to run at startup even for SSH profiles because `SshSession::new()` only stores config — the TCP connection is established lazily on first use.

> **Superseded (2026-08-30):** `preload()` was deleted; callers use `load_profiles()`, which instantiates nothing. The claim above was also incomplete: while `SshSession::new()` does not connect, `SSHSandbox::new()` *does* spawn a per-instance background idle task, and that task was never aborted — so eagerly instantiating SSH profiles leaked a task plus an `Arc<SshSession>` per profile. See [[sandbox-ssh-idle-task-lifecycle]].

### Call-site migration

- `CliToolConfig.sandbox` — `Option<SandboxConfig>` → `Option<SandboxSpec>`
- `cli_tool::load_dir(dir, &SandboxRegistry)` → `load_dir(dir, &SandboxManager)`, using `acquire_by_name()` / `build_inline()`
- `AgentRuntime.sandbox_registry` → `sandbox_manager: Arc<SandboxManager>`; the builder registers Local/Tmp/SSH providers, registers a `"local"` profile pointing at `working_dir`, then calls `preload()`
- `AgentConfigBuilder::with_sandbox_registry()` → `with_sandbox_manager()`; the ReAct tool loop uses `acquire_by_name().await` with `default_tmp().await` fallback
- `DataPlaneServerCore` dropped its own `SandboxManager::new()` + provider registration (lines 562-582) and now reuses `runtime.sandbox_manager`, so control-plane `sandbox.*` operations and data-plane tool execution observe the same instances
- `cli-tools-mcp` binary builds a `SandboxManager`, registers providers, and calls `preload()`

`"local"` is registered as an ordinary profile via `register_profile()` rather than being special-cased, so it resolves through the same path as disk-loaded profiles.

### Verification

- `spec.rs` gained 5 parse tests covering local / ssh-with-`key_path` / ssh-with-`identity_file`-alias / firecracker / wasm
- `vol-llm-sandbox` 116 tests pass (`--all-features`); `vol-agent-server` 199/199 pass
- Whole-workspace `--lib` suite green; `clippy`, `fmt-check`, `no-doc-tests`, `boundaries`, `no-clippy-allow` all pass
- The four production TOMLs and their generated ConfigMaps required no edits

## Entities Mentioned

- [[vol-llm-sandbox-crate]]: `registry.rs` deleted; `spec.rs` became the single schema source; `SandboxManager` gained name-based lookup
- [[vol-mcp-servers-crate]]: `cli-tools-mcp` binary switched to `SandboxManager` + `preload()`, fixing zero-tool registration
- [[vol-llm-runtime-crate]]: `AgentRuntime.sandbox_registry` → `sandbox_manager`; builder registers providers and preloads
- [[vol-llm-agent-crate]]: `AgentConfig.sandbox_registry` → `sandbox_manager`; tool loop uses `acquire_by_name`
- [[vol-agent-server-crate]]: `DataPlaneServerCore` reuses the runtime's manager instead of constructing its own

## Concepts Covered

- [[sandbox-lifecycle]]: `SandboxManager` is now the sole sandbox resolution path; profile-name lookup added alongside instance-ID lookup
- [[schema-drift]]: the failure mode this ingest diagnoses and closes — config files and parser diverging, with warn-and-skip error handling hiding it
- [[cli-style-tool-pattern]]: cli-tool sandbox resolution moved from registry to manager
- [[lifecycle-state-machine]]: unchanged, but `destroy()` now also prunes the profile-name index

## Notes

- `SandboxManager` still uses `InMemorySandboxStore`; instance metadata does not survive restart. Out of scope here.
- No `SandboxProvider` implementation exists for `firecracker` or `wasm` — their spec variants parse, but `preload()` will warn-and-skip those profiles for lack of a registered provider. Same behavior as the old registry.
- `SSHSandboxProvider::get(backend_id)` still returns `NotFound` unconditionally; only the `instances` cache resolves SSH sandboxes after creation.
- The warn-and-skip error handling that hid this bug is still in place in both `preload()` and `load_dir()`. A startup assertion — "N cli-tools configured but M registered" — would surface a recurrence, and is worth considering.
