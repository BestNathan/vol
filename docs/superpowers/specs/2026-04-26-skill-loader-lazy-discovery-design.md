# SkillLoader Lazy Discovery

## Context

Currently `SkillLoader` requires callers to manually call `discover_all().await` after construction. This forces `async` operations into `new()` contexts like `CodingAgent::new()`, adding awaits that could be deferred until the first actual skill read.

## Design

Make `get()` and `list_metadata()` auto-trigger discovery on first access using a `tokio::sync::OnceCell`. `discover_all()` remains callable explicitly (for cases that want eager discovery).

### Key Changes

**1. `crates/vol-llm-skill/src/loader.rs`**

- Add `discovered: OnceCell<()>` to `SkillLoader`
- In `new()`, initialize `discovered: OnceCell::new()`
- Create internal `ensure_discovered(&self)` that runs `discover_all()` once
- Call `ensure_discovered()` at the start of `get()` and `list_metadata()`
- `discover_all()` itself remains unchanged (idempotent via map-merge semantics)

The `OnceCell::get_or_init()` pattern ensures discovery runs exactly once even under concurrent access.

**2. `crates/vol-llm-agents/src/coding/agent.rs`**

Remove the `discover_all()` call:
```rust
// Before:
let skill_loader = Arc::new(SkillLoader::new(Some(config.working_dir.clone())));
let _ = skill_loader.discover_all().await;

// After:
let skill_loader = Arc::new(SkillLoader::new(Some(config.working_dir.clone())));
```

**3. `crates/vol-llm-skill/src/injector.rs`**

`SkillInjector::from_workdir()` currently calls `discover_all()`. With lazy discovery, the `discover_all()` call becomes optional. Keep it for backward compatibility but no longer needed.

### Tests

Existing tests that call `discover_all()` explicitly continue to work. Tests that rely on lazy discovery (e.g., `register()` followed by `get()` without `discover_all()`) already work since `register()` writes directly to the skills map.
