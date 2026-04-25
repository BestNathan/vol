# CodingAgent workdir/storedir Separation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `store_dir` to `CodingAgentConfig` and `CodingAgent`, separating session/log storage from the working directory.

**Architecture:** `store_dir` is a new config field with `~/.vol-coding/{workdir_basename}/` as the builder default. The builder tracks whether `store_dir` was explicitly set via a `store_dir_set` flag. `CodingAgent` stores `store_dir` directly and uses it in `resume()`.

**Tech Stack:** Rust, cargo, tokio test

---

### Task 1: Add `store_dir` to `CodingAgentConfig`

**Files:**
- Modify: `crates/vol-llm-agents/src/coding/config.rs`

- [ ] **Step 1: Add `store_dir` field and update Default**

In `crates/vol-llm-agents/src/coding/config.rs`, add the field to the struct (after `working_dir`):

```rust
/// Directory for persistent agent state (sessions, logs).
/// Defaults to `.vol-coding` in the Default impl.
/// Builder's `working_dir()` auto-derives this to `~/.vol-coding/{name}/`.
pub store_dir: PathBuf,
```

Update the `Default` impl to add:

```rust
store_dir: PathBuf::from(".vol-coding"),
```

- [ ] **Step 2: Update `Debug` impl**

Add the field to the `Debug` struct builder (after `working_dir`):

```rust
.field("store_dir", &self.store_dir)
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p vol-llm-agents -- --test-threads=1`
Expected: FAIL — `store_dir` field missing in test struct literals (fixed in Task 3)

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-agents/src/coding/config.rs
git commit -m "feat: add store_dir field to CodingAgentConfig"
```

---

### Task 2: Add `store_dir` to `CodingAgent` struct and `new()`

**Files:**
- Modify: `crates/vol-llm-agents/src/coding/agent.rs`

- [ ] **Step 1: Add `store_dir` field to `CodingAgent` struct**

In the struct definition (around line 27-34):

```rust
pub struct CodingAgent {
    config: CodingAgentConfig,
    llm: Arc<dyn vol_llm_core::LLMClient>,
    tool_registry: Arc<ToolRegistry>,
    context_builder: ContextBuilder,
    observer: Option<Arc<dyn EventObserver>>,
    sandbox: Option<vol_llm_core::SandboxRef>,
    store_dir: PathBuf,
}
```

- [ ] **Step 2: Store `store_dir` in `new()`**

In the `Ok(Self { ... })` block of `new()`, add:

```rust
store_dir: config.store_dir.clone(),
```

- [ ] **Step 3: Update `resume()` to use `store_dir`**

In the `resume()` method, replace:

```rust
let session_dir = self.config.working_dir.join(".vol-sessions");
```

With:

```rust
let session_dir = self.store_dir.join("sessions");
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p vol-llm-agents -- --test-threads=1`
Expected: FAIL — builder doesn't set `store_dir` yet (fixed in Task 3)

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-agents/src/coding/agent.rs
git commit -m "feat: add store_dir field to CodingAgent, update resume()"
```

---

### Task 3: Update `CodingAgentBuilder` with `store_dir` support

**Files:**
- Modify: `crates/vol-llm-agents/src/coding/agent.rs` (builder section, lines ~272-351)

- [ ] **Step 1: Add `store_dir_set` flag to builder**

Replace the builder struct definition:

```rust
pub struct CodingAgentBuilder {
    config: CodingAgentConfig,
    sandbox: Option<vol_llm_core::SandboxRef>,
    store_dir_set: bool,
}
```

Update `new()`:

```rust
pub fn new() -> Self {
    Self {
        config: CodingAgentConfig::default(),
        sandbox: None,
        store_dir_set: false,
    }
}
```

- [ ] **Step 2: Update `working_dir()` builder method**

Replace the existing `working_dir` method:

```rust
pub fn working_dir(mut self, path: PathBuf) -> Self {
    self.config.working_dir = path;
    if !self.store_dir_set {
        let basename = path
            .file_name()
            .unwrap_or(std::ffi::OsStr::new("default"))
            .to_string_lossy();
        let home = std::env::var("HOME").unwrap_or_default();
        self.config.store_dir =
            PathBuf::from(home).join(".vol-coding").join(basename.as_ref());
    }
    self
}
```

- [ ] **Step 3: Add `store_dir()` builder method**

Add after `working_dir()`:

```rust
pub fn store_dir(mut self, path: PathBuf) -> Self {
    self.config.store_dir = path;
    self.store_dir_set = true;
    self
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p vol-llm-agents -- --test-threads=1`
Expected: All tests pass

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-agents/src/coding/agent.rs
git commit -m "feat: add store_dir builder method, auto-derive from workdir"
```

---

### Task 4: Update tests for `store_dir`

**Files:**
- Modify: `crates/vol-llm-agents/src/coding/tests.rs`

- [ ] **Step 1: Update struct literal tests**

In tests that construct `CodingAgentConfig` directly (around lines 460 and 479), add `store_dir` to the struct literal:

```rust
store_dir: temp_dir.path().join("vol-coding"),
```

- [ ] **Step 2: Update `test_config_defaults` and `test_config_clone`**

In `test_config_defaults` (around line 31), add assertion:

```rust
assert_eq!(config.store_dir, std::path::PathBuf::from(".vol-coding"));
```

In `test_config_clone` (around line 71), add assertion:

```rust
assert_eq!(cloned.store_dir, config.store_dir);
```

- [ ] **Step 3: Add builder test for store_dir derivation**

Add a new test after the existing builder tests (around line 370):

```rust
#[test]
fn test_builder_store_dir_derived_from_workdir() {
    use vol_llm_agents::coding::agent::CodingAgentBuilder;
    let tmp = std::env::temp_dir();
    let project_name = "test_project";
    let project_dir = tmp.join(project_name);

    let builder = CodingAgentBuilder::new()
        .working_dir(project_dir.clone());

    // store_dir should be derived from workdir basename
    assert_eq!(
        builder.config.store_dir,
        PathBuf::from(std::env::var("HOME").unwrap_or_default())
            .join(".vol-coding")
            .join(project_name)
    );
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p vol-llm-agents -- --test-threads=1`
Expected: All tests pass

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-agents/src/coding/tests.rs
git commit -m "test: update tests for store_dir field and derive behavior"
```

---

### Task 5: Update TUI to use store_dir

**Files:**
- Modify: `crates/vol-llm-tui/src/main.rs`

- [ ] **Step 1: Derive store_dir in TUI**

In `main()`, find the session_dir construction (around line 105):

```rust
let session_dir = std::env::current_dir()
    .unwrap_or_default()
    .join(".vol-sessions");
```

Replace with:

```rust
let working_dir = std::env::current_dir().unwrap_or_default();
let project_name = working_dir
    .file_name()
    .unwrap_or(std::ffi::OsStr::new("default"))
    .to_string_lossy();
let home = std::env::var("HOME").unwrap_or_default();
let store_dir = PathBuf::from(home)
    .join(".vol-coding")
    .join(project_name.as_ref());
let session_dir = store_dir.join("sessions");
```

- [ ] **Step 2: Pass store_dir to CodingAgentConfig if TUI builds one**

Check if the TUI constructs `CodingAgentConfig` directly (around line 351-367). If so, add `store_dir` field to the config construction. Look for patterns like `working_dir` being set and add `store_dir` alongside it.

- [ ] **Step 3: Build TUI to verify compilation**

Run: `cargo build -p vol-llm-tui`
Expected: Compiles without errors

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-tui/src/main.rs
git commit -m "feat: update TUI to use store_dir for session storage"
```
