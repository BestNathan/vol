---
type: concept
category: architecture
tags: [sandbox, registry, lifecycle, tmp-sandbox, bind-metadata]
created: 2026-08-11
updated: 2026-08-11
source_count: 1
---

# Sandbox Lifecycle & Registry Design

## Overview
The sandbox system follows a strict lifecycle: definition → construction → registration → acquisition → metadata binding → initialization → use → cleanup. The `SandboxRegistry` is a pure configuration loader with no built-in entries; programmatic sandboxes are added via `register()`.

## Lifecycle

```
1. Definition    → TOML config or code construction
2. Construction  → Sandbox struct created, root_path determined (or deferred)
3. Registration  → registry.register(name, sandbox)  — programmatic
                 → or load() from .agents/sandboxes/*.toml
4. Acquisition   → registry.acquire(name) — pure name lookup
5. Bind metadata → sandbox.bind_metadata({"sub_dir": ...})  — TmpSandbox sets root
6. Initialization→ sandbox.start() — create dirs, establish connections
7. Use           → execute(), read_file(), write_file(), ...
8. Cleanup       → sandbox.cleanup() — remove dirs, disconnect
```

## Key design decisions

### No built-in sandboxes
`SandboxRegistry::load()` only reads TOML configs. Systems that need specific sandboxes (e.g. `LocalSandbox` at the server's working directory) call `register()` after `load()`.

```
let mut registry = SandboxRegistry::load(&sandboxes_dir).await?;
registry.register("local", LocalSandbox::new(Some(working_dir)));
```

### Pure acquire
`acquire(name)` does simple name lookup. No special-casing for sandbox types. The only exception is `FirecrackerSandbox` which returns a fresh instance from the pool.

### TmpSandbox as default fallback
`registry.default()` returns a fresh `TmpSandbox::new()` with a random subdir. Agents that don't specify a sandbox get an isolated temp directory.

### bind_metadata is sandbox-level, not agent-level
The `bind_metadata` method uses generic key-value pairs. `TmpSandbox` reads `sub_dir` to set its root path. The agent runtime provides `sub_dir = agent_id` for debuggability, but the sandbox doesn't know about agents — it only knows about its own directory.

```rust
// agent.rs — sandbox resolution
let mut meta = HashMap::new();
meta.insert("sub_dir".to_string(), agent_id.clone());
sandbox.bind_metadata(&meta);
```

### Consistent path resolution
All `Sandbox` implementations reject absolute paths and `~` in `resolve_path()`. `ToolContext::resolve_path()` converts absolute paths within the sandbox root to relative before delegation. This keeps tools working with absolute paths from `tempfile` while ensuring consistent sandbox behavior.

## Related
- [[vol-llm-sandbox-crate]] — implementation details
- [[tool-registry]] — tools use sandbox for all I/O
- [[vol-agent-server-crate]] — runtime registers "local" sandbox
