# CodingAgent workdir/storedir Separation Design

**Date**: 2026-04-25
**Status**: Draft

## Summary

Split `CodingAgent`'s single `working_dir` into two paths: `workdir` (code, skills, sandbox) and `store_dir` (sessions, logs). `store_dir` defaults to `~/.vol-coding/{workdir_basename}/` and is derived from `workdir` when not explicitly set.

---

## 1. Current Problem

`working_dir` conflates three concerns:

1. **Sandbox root** — where tools read/write/execute code
2. **Skill loading** — `{working_dir}/.agents/skills/`
3. **Session storage** — `{working_dir}/.vol-sessions/`

This means sessions are scattered across project directories and can't be managed centrally.

---

## 2. New Configuration

```rust
pub struct CodingAgentConfig {
    // Existing fields...
    pub working_dir: PathBuf,  // code, skills, sandbox root
    pub store_dir: PathBuf,    // sessions, logs (new field)
}
```

### Defaults

- `working_dir` defaults to `"."` (current directory)
- `store_dir` defaults to `~/.vol-coding/{workdir_basename}/`
- Setting `working_dir()` on the builder auto-derives `store_dir` unless explicitly overridden

### Builder

```rust
pub struct CodingAgentBuilder {
    config: CodingAgentConfig,
    sandbox: Option<SandboxRef>,
    store_dir_set: bool,  // tracks whether store_dir was explicitly set
}

impl CodingAgentBuilder {
    pub fn working_dir(mut self, path: PathBuf) -> Self {
        self.config.working_dir = path;
        if !self.store_dir_set {
            let basename = path.file_name().unwrap_or_default().to_string_lossy();
            let home = std::env::var("HOME").unwrap_or_default();
            self.config.store_dir = PathBuf::from(home)
                .join(".vol-coding")
                .join(basename.as_ref());
        }
        self
    }

    pub fn store_dir(mut self, path: PathBuf) -> Self {
        self.config.store_dir = path;
        self.store_dir_set = true;
        self
    }
}
```

---

## 3. Agent Changes

- `CodingAgent` gains `store_dir: PathBuf` field (stored directly)
- `resume(session_id)` reads from `{store_dir}/sessions/` instead of `{working_dir}/.vol-sessions/`
- `AgentConfig` continues to use `working_dir` for sandbox/tool execution — `store_dir` is not passed to it

---

## 4. Storage Layout

```
~/.vol-coding/my-project/
├── sessions/     # FileSessionEntryStore (JSONL)
├── logs/         # Agent run / observability logs
└── reports/      # HTML reports (fallback)
```

---

## 5. TUI Changes

`vol-llm-tui/src/main.rs` currently uses `{cwd}/.vol-sessions/`. Updated to derive `store_dir` from the project name and use `{store_dir}/sessions/`.

---

## 6. Files Changed

| File | Change |
|------|--------|
| `vol-llm-agents/src/coding/config.rs` | Add `store_dir` field, update Default |
| `vol-llm-agents/src/coding/agent.rs` | Add `store_dir` field, update `new()`, `resume()`, builder |
| `vol-llm-agents/src/coding/tests.rs` | Update tests for new field |
| `vol-llm-tui/src/main.rs` | Derive store_dir, use for session storage |

No public API breaking changes — `working_dir()` remains, `store_dir()` is additive.
