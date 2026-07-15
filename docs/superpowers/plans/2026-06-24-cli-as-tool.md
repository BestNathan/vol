# CLI-as-Tool Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let the LLM invoke remote CLI commands (ansible, kubectl, …) as named tools via a declarative TOML config, with two deployment paths — an HTTP MCP server (primary) and in-process direct-load (fallback) — sharing one core crate.

**Architecture:** A new `vol-llm-cli-tool` crate owns config parsing, `{{env.VAR}}` interpolation, first-token binaries validation, sandbox command construction, and output formatting. A sub-crate `vol-llm-tools-builtin-cli-tool` wraps the core as an `ExecutableTool` for path A. A new module `vol-mcp-servers::cli_tools` exposes the same core as an HTTP MCP server for path C. Both paths read the same `.agents/cli-tools/*.toml` schema.

**Tech Stack:** Rust, tokio, async-trait, serde/toml 0.8, rmcp 1.6, vol-llm-sandbox (existing), vol-llm-tool (ExecutableTool trait), axum 0.7 (MCP HTTP transport).

**Spec:** `docs/superpowers/specs/2026-06-24-cli-as-tool-design.md`

---

## File Structure

| Action | File | Responsibility |
|---|---|---|
| Create | `crates/vol-llm-cli-tool/Cargo.toml` | New crate manifest |
| Create | `crates/vol-llm-cli-tool/src/lib.rs` | Public module roots + `CliTool::load_dir` |
| Create | `crates/vol-llm-cli-tool/src/config.rs` | `CliToolConfig` TOML schema + parse |
| Create | `crates/vol-llm-cli-tool/src/interpolate.rs` | `{{env.VAR}}` substitution |
| Create | `crates/vol-llm-cli-tool/src/validate.rs` | First-token binaries check |
| Create | `crates/vol-llm-cli-tool/src/exec.rs` | Build `CommandRequest`, format `CommandOutput` |
| Create | `crates/vol-llm-cli-tool/src/error.rs` | `CliToolError` enum |
| Modify | `crates/vol-llm-sandbox/src/registry.rs` | Factor out `pub async fn build_sandbox(config) -> Arc<dyn Sandbox>` |
| Modify | `Cargo.toml` (workspace) | Add `vol-llm-cli-tool` to members + workspace deps |
| Create | `crates/vol-llm-tools-builtin/cli-tool/Cargo.toml` | Direct-load wrapper sub-crate |
| Create | `crates/vol-llm-tools-builtin/cli-tool/src/lib.rs` | `CliToolExecutable` + `register_all` |
| Modify | `crates/vol-llm-tools-builtin/Cargo.toml` | Add `vol-llm-tools-builtin-cli-tool` path dep |
| Modify | `crates/vol-llm-tools-builtin/src/lib.rs` | Re-export `cli_tool` module |
| Modify | `crates/vol-llm-runtime/src/lib.rs` | Call `cli_tool::register_all` in `build()` |
| Create | `crates/vol-mcp-servers/src/cli_tools/mod.rs` | MCP service struct + tool routing |
| Create | `crates/vol-mcp-servers/src/cli_tools/server.rs` | rmcp `#[tool_router]` impl |
| Create | `crates/vol-mcp-servers/src/bin/cli_tools_mcp.rs` | HTTP binary entry point |
| Modify | `crates/vol-mcp-servers/Cargo.toml` | Add `cli-tools-mcp` binary target + `vol-llm-cli-tool` dep |
| Modify | `crates/vol-mcp-servers/src/lib.rs` | Declare `cli_tools` module |
| Create | `.agents/cli-tools/ansible.toml` | Example config |

---

### Task 1: Scaffold `vol-llm-cli-tool` crate + factor `build_sandbox` out of SandboxRegistry

**Files:**
- Create: `crates/vol-llm-cli-tool/Cargo.toml`
- Create: `crates/vol-llm-cli-tool/src/lib.rs` (empty module stubs)
- Modify: `Cargo.toml:3-43` (workspace members)
- Modify: `Cargo.toml:74-143` (workspace deps)
- Modify: `crates/vol-llm-sandbox/src/registry.rs`

- [ ] **Step 1: Create the crate manifest**

Create `crates/vol-llm-cli-tool/Cargo.toml`:

```toml
[package]
name = "vol-llm-cli-tool"
version.workspace = true
edition.workspace = true

[dependencies]
vol-llm-sandbox = { workspace = true, features = ["ssh"] }
async-trait = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
toml = { workspace = true }
thiserror = { workspace = true }
tracing = { workspace = true }
tokio = { workspace = true }

[dev-dependencies]
tempfile = "3"
tokio = { workspace = true, features = ["rt", "macros", "rt-multi-thread"] }
```

- [ ] **Step 2: Create the lib.rs skeleton**

Create `crates/vol-llm-cli-tool/src/lib.rs`:

```rust
//! vol-llm-cli-tool: core abstraction for "CLI-as-Tool".
//!
//! Loads TOML configs that declare a named CLI tool backed by a Sandbox,
//! validates the first command token against a binaries whitelist,
//! interpolates `{{env.VAR}}` placeholders, and formats sandbox output
//! into a tool result. Reused by both the direct-load path
//! (`vol-llm-tools-builtin-cli-tool`) and the MCP server path
//! (`vol-mcp-servers::cli_tools`).

pub mod config;
pub mod error;
pub mod exec;
pub mod interpolate;
pub mod validate;

pub use config::CliToolConfig;
pub use error::CliToolError;
pub use exec::CliTool;
```

- [ ] **Step 3: Add empty module files**

Create each with a one-line doc comment so the crate compiles:

`crates/vol-llm-cli-tool/src/config.rs`:
```rust
//! TOML config schema for a CLI tool.
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct CliToolConfig {
    pub name: String,
    #[serde(default)]
    pub _placeholder: (),
}
```

`crates/vol-llm-cli-tool/src/error.rs`:
```rust
//! Error types for the cli-tool crate.
#[derive(Debug, thiserror::Error)]
pub enum CliToolError {
    #[error("placeholder")]
    Placeholder,
}
```

`crates/vol-llm-cli-tool/src/interpolate.rs`:
```rust
//! `{{env.VAR}}` placeholder substitution.
```

`crates/vol-llm-cli-tool/src/validate.rs`:
```rust
//! First-token binaries whitelist check.
```

`crates/vol-llm-cli-tool/src/exec.rs`:
```rust
//! Command execution and output formatting.
pub struct CliTool;
```

- [ ] **Step 4: Add crate to workspace**

In root `Cargo.toml`, add to the `members` array (after `"crates/vol-llm-sandbox"`):

```toml
    "crates/vol-llm-cli-tool",
```

In the `[workspace.dependencies]` section (after `vol-llm-sandbox = ...`):

```toml
vol-llm-cli-tool = { path = "crates/vol-llm-cli-tool" }
```

- [ ] **Step 5: Verify scaffold compiles**

Run: `cargo check -p vol-llm-cli-tool 2>&1 | tail -20`
Expected: Compiles with no errors. Warnings about unused imports/fields are OK.

- [ ] **Step 6: Factor `build_sandbox` out of SandboxRegistry**

Open `crates/vol-llm-sandbox/src/registry.rs`. Add a new public async function near the top of the `impl SandboxRegistry` block (or as a free function in the same module — your call, but it must be public):

```rust
/// Construct a single sandbox from a parsed config.
///
/// Extracted from `SandboxRegistry::load` so that other crates
/// (e.g. `vol-llm-cli-tool`) can build inline sandboxes without
/// going through the directory loader.
pub async fn build_sandbox(
    config: SandboxConfig,
) -> SandboxResult<Arc<dyn Sandbox>> {
    let sandbox: Arc<dyn Sandbox> = match config.sandbox_type.as_str() {
        "local" => Arc::new(LocalSandbox::new(
            config.work_dir.as_ref().map(std::path::PathBuf::from),
        )),
        #[cfg(feature = "ssh")]
        "ssh" => {
            let ssh_config = config.ssh.ok_or_else(|| {
                SandboxError::Config(format!(
                    "SSH sandbox '{}' requires [ssh] section",
                    config.name
                ))
            })?;
            let sb = crate::ssh::SSHSandbox::new(
                config.name.clone(),
                config.work_dir.clone(),
                ssh_config,
            )?;
            let sandbox: Arc<dyn Sandbox> = Arc::new(sb);
            sandbox.start().await?;
            sandbox
        }
        other => {
            return Err(SandboxError::Config(format!(
                "unsupported sandbox type: {other}"
            )));
        }
    };
    Ok(sandbox)
}
```

Note: `SandboxError::Config` may not exist; check the `SandboxError` enum in `crates/vol-llm-sandbox/src/lib.rs` and use the closest variant (e.g. `SandboxError::Io` or add a new `Config(String)` variant if needed).

- [ ] **Step 7: Refactor SandboxRegistry::load to use build_sandbox**

In the same file, replace the inline `match config.sandbox_type.as_str()` body inside `SandboxRegistry::load` (around lines 210-243) with:

```rust
let sandbox = match Self::build_sandbox(config).await {
    Ok(s) => s,
    Err(e) => {
        tracing::warn!(path = %path.display(), error = %e, "Failed to build sandbox, skipping");
        continue;
    }
};
sandboxes.insert(sandbox.name().to_string(), sandbox);
```

(The firecracker/wasm branches stay inline if you prefer; or extend `build_sandbox` behind their feature gates. Keep them inline for this task to minimize blast radius.)

- [ ] **Step 8: Verify vol-llm-sandbox still compiles and tests pass**

Run:
```
cargo check -p vol-llm-sandbox 2>&1 | tail -10
cargo test -p vol-llm-sandbox --lib 2>&1 | tail -20
```
Expected: Both succeed. `test_registry_always_has_local` and other registry tests still pass.

- [ ] **Step 9: Commit**

```bash
git add Cargo.toml crates/vol-llm-cli-tool crates/vol-llm-sandbox/src/registry.rs
git commit -m "feat(cli-tool): scaffold vol-llm-cli-tool crate and factor build_sandbox

- New empty crate with module stubs
- Factored SandboxRegistry::build_sandbox() for inline sandbox construction
- Added to workspace members + deps"
```

---

### Task 2: Config parsing (TDD)

**Files:**
- Modify: `crates/vol-llm-cli-tool/src/config.rs`

- [ ] **Step 1: Write the failing config tests**

Replace `crates/vol-llm-cli-tool/src/config.rs` with:

```rust
//! TOML config schema for a CLI tool.
use serde::Deserialize;
use vol_llm_sandbox::registry::{SandboxConfig, SshConfig};

#[derive(Debug, Clone, Deserialize)]
pub struct CliToolConfig {
    pub name: String,
    pub description: String,
    pub binaries: Vec<String>,

    /// Inline sandbox config (XOR with `sandbox_ref`).
    #[serde(default)]
    pub sandbox: Option<SandboxConfig>,

    /// Name of a sandbox in `.agents/sandboxes/` (XOR with `sandbox`).
    #[serde(default)]
    pub sandbox_ref: Option<String>,

    /// Remote working directory. Supports `{{env.VAR}}`.
    pub cwd: String,

    /// Shell used to wrap the command. Default: `/bin/sh`.
    #[serde(default = "default_shell")]
    pub shell: String,

    /// Command timeout in seconds. Default: 60.
    #[serde(default = "default_timeout_secs")]
    pub timeout_secs: u64,

    /// Per-stream truncation ceiling in bytes. Default: 65536.
    #[serde(default = "default_max_output_bytes")]
    pub max_output_bytes: usize,

    /// Environment variables passed to the remote process.
    /// Values support `{{env.VAR}}` placeholders.
    #[serde(default)]
    pub env: std::collections::HashMap<String, String>,
}

fn default_shell() -> String { "/bin/sh".to_string() }
fn default_timeout_secs() -> u64 { 60 }
fn default_max_output_bytes() -> usize { 65_536 }

impl CliToolConfig {
    /// Parse a TOML document into a CliToolConfig.
    /// Returns an error if required fields are missing, or if both
    /// `sandbox` and `sandbox_ref` are set.
    pub fn from_toml(s: &str) -> Result<Self, crate::CliToolError> {
        let cfg: CliToolConfig = toml::from_str(s)
            .map_err(|e| crate::CliToolError::Config(e.to_string()))?;
        cfg.validate()?;
        Ok(cfg)
    }

    fn validate(&self) -> Result<(), crate::CliToolError> {
        if self.binaries.is_empty() {
            return Err(crate::CliToolError::Config(
                "`binaries` must be non-empty".into(),
            ));
        }
        match (&self.sandbox, &self.sandbox_ref) {
            (Some(_), Some(_)) => Err(crate::CliToolError::Config(
                "set either `sandbox` or `sandbox_ref`, not both".into(),
            )),
            (None, None) => Err(crate::CliToolError::Config(
                "one of `sandbox` or `sandbox_ref` is required".into(),
            )),
            _ => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINIMAL_REF: &str = r#"
        name = "ansible"
        description = "Ansible suite"
        binaries = ["ansible", "ansible-playbook"]
        sandbox_ref = "ansible-prod"
        cwd = "/opt/ansible"
    "#;

    const FULL_INLINE: &str = r#"
        name = "ansible"
        description = "Ansible suite"
        binaries = ["ansible", "ansible-playbook"]
        cwd = "/opt/ansible"
        shell = "/bin/bash"
        timeout_secs = 300
        max_output_bytes = 131072

        [sandbox]
        name = "ansible-sandbox"
        type = "ssh"
        work_dir = "/"

        [sandbox.ssh]
        host = "ansible-prod.example.com"
        port = 22
        user = "deploy"
        identity_file = "/home/u/.ssh/id_ed25519"

        [env]
        ANSIBLE_CONFIG = "/opt/ansible/ansible.cfg"
    "#;

    #[test]
    fn parse_minimal_with_sandbox_ref() {
        let cfg = CliToolConfig::from_toml(MINIMAL_REF).unwrap();
        assert_eq!(cfg.name, "ansible");
        assert_eq!(cfg.binaries, vec!["ansible", "ansible-playbook"]);
        assert_eq!(cfg.sandbox_ref.as_deref(), Some("ansible-prod"));
        assert!(cfg.sandbox.is_none());
        assert_eq!(cfg.shell, "/bin/sh");       // default
        assert_eq!(cfg.timeout_secs, 60);       // default
        assert_eq!(cfg.max_output_bytes, 65536); // default
        assert!(cfg.env.is_empty());
    }

    #[test]
    fn parse_full_with_inline_sandbox() {
        let cfg = CliToolConfig::from_toml(FULL_INLINE).unwrap();
        assert_eq!(cfg.shell, "/bin/bash");
        assert_eq!(cfg.timeout_secs, 300);
        assert_eq!(cfg.max_output_bytes, 131072);
        assert!(cfg.sandbox.is_some());
        assert!(cfg.sandbox_ref.is_none());
        let sb = cfg.sandbox.as_ref().unwrap();
        assert_eq!(sb.sandbox_type, "ssh");
        assert_eq!(sb.ssh.as_ref().unwrap().host, "ansible-prod.example.com");
        assert_eq!(cfg.env.get("ANSIBLE_CONFIG").map(String::as_str), Some("/opt/ansible/ansible.cfg"));
    }

    #[test]
    fn reject_both_sandbox_and_ref() {
        let bad = r#"
            name = "x"
            description = "x"
            binaries = ["x"]
            cwd = "/"
            sandbox_ref = "foo"
            [sandbox]
            name = "bar"
            type = "local"
        "#;
        let err = CliToolConfig::from_toml(bad).unwrap_err().to_string();
        assert!(err.contains("not both"), "unexpected error: {err}");
    }

    #[test]
    fn reject_neither_sandbox_nor_ref() {
        let bad = r#"
            name = "x"
            description = "x"
            binaries = ["x"]
            cwd = "/"
        "#;
        let err = CliToolConfig::from_toml(bad).unwrap_err().to_string();
        assert!(err.contains("required"), "unexpected error: {err}");
    }

    #[test]
    fn reject_empty_binaries() {
        let bad = r#"
            name = "x"
            description = "x"
            binaries = []
            sandbox_ref = "foo"
            cwd = "/"
        "#;
        let err = CliToolConfig::from_toml(bad).unwrap_err().to_string();
        assert!(err.contains("non-empty"), "unexpected error: {err}");
    }

    #[test]
    fn reject_missing_required_field() {
        let bad = r#"
            name = "x"
            binaries = ["x"]
            sandbox_ref = "foo"
            cwd = "/"
        "#; // missing description
        let err = CliToolConfig::from_toml(bad).unwrap_err().to_string();
        assert!(!err.is_empty());
    }
}
```

- [ ] **Step 2: Update error.rs with the Config variant**

Replace `crates/vol-llm-cli-tool/src/error.rs`:

```rust
//! Error types for the cli-tool crate.

#[derive(Debug, thiserror::Error)]
pub enum CliToolError {
    #[error("config error: {0}")]
    Config(String),

    #[error("binary not allowed: first token `{token}` is not in {allowed:?}")]
    BinaryNotAllowed {
        token: String,
        allowed: Vec<String>,
    },

    #[error("invalid arguments: {0}")]
    InvalidArguments(String),

    #[error("sandbox execution failed: {0}")]
    SandboxFailed(String),

    #[error("command timed out after {0} seconds")]
    Timeout(u64),
}
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test -p vol-llm-cli-tool --lib 2>&1 | tail -30`
Expected: Tests fail to compile — `SandboxConfig` / `SshConfig` types may not be public yet. Check the actual error.

- [ ] **Step 4: Ensure `SandboxConfig` and `SshConfig` are publicly exported**

Check `crates/vol-llm-sandbox/src/lib.rs` — the `registry` module needs to be `pub`. If it is not, change `mod registry;` to `pub mod registry;`. (It already is `pub` per the exploration; verify.)

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p vol-llm-cli-tool --lib 2>&1 | tail -30`
Expected: All 6 tests pass.

- [ ] **Step 6: Commit**

```bash
git add crates/vol-llm-cli-tool/src/config.rs crates/vol-llm-cli-tool/src/error.rs
git commit -m "feat(cli-tool): config parsing with XOR sandbox/sandbox_ref validation"
```

---

### Task 3: Placeholder interpolation (TDD)

**Files:**
- Modify: `crates/vol-llm-cli-tool/src/interpolate.rs`

- [ ] **Step 1: Write failing interpolation tests**

Replace `crates/vol-llm-cli-tool/src/interpolate.rs`:

```rust
//! `{{env.VAR}}` placeholder substitution.
//!
//! Replaces every occurrence of `{{env.VAR}}` in the input string with the
//! value of the local environment variable `VAR`. Unknown vars become empty
//! strings and emit a `tracing::warn!`. Unknown namespaces (e.g. `{{foo.X}}`)
//! are left untouched but emit a warning. Escape literal `{{` with `\{{`.

/// Interpolate `{{env.VAR}}` placeholders in a single string.
///
/// Lookup is performed via the provided closure so callers can inject
/// test doubles without touching process env.
pub fn interpolate_with<F>(input: &str, lookup: F) -> String
where
    F: Fn(&str) -> Option<String>,
{
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();

    while let Some(c) = chars.next() {
        // escaped opening: `\{{` → literal `{{`
        if c == '\\' && chars.peek() == Some(&'{') {
            let mut maybe = chars.clone();
            maybe.next();
            if maybe.peek() == Some(&'{') {
                chars.next(); // consume first `{`
                out.push('{');
                out.push('{');
                continue;
            }
        }

        if c == '{' && chars.peek() == Some(&'{') {
            chars.next(); // consume second `{`
            // read until `}}`
            let mut tag = String::new();
            let mut closed = false;
            while let Some(ch) = chars.next() {
                if ch == '}' && chars.peek() == Some(&'}') {
                    chars.next();
                    closed = true;
                    break;
                }
                tag.push(ch);
            }
            if !closed {
                // malformed: emit as-is
                out.push_str("{{");
                out.push_str(&tag);
                continue;
            }
            // parse `namespace.name`
            if let Some(dot_pos) = tag.find('.') {
                let ns = &tag[..dot_pos];
                let var = &tag[dot_pos + 1..];
                match ns {
                    "env" => match lookup(var) {
                        Some(v) => out.push_str(&v),
                        None => {
                            tracing::warn!(var, "env var not set, substituting empty");
                        }
                    },
                    other => {
                        tracing::warn!(
                            namespace = other,
                            var,
                            "unknown placeholder namespace, leaving intact"
                        );
                        out.push_str("{{");
                        out.push_str(&tag);
                        out.push_str("}}");
                    }
                }
            } else {
                // no dot: unknown form, leave intact
                tracing::warn!(tag = %tag, "malformed placeholder (missing namespace)");
                out.push_str("{{");
                out.push_str(&tag);
                out.push_str("}}");
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Interpolate using the real process environment.
pub fn interpolate(input: &str) -> String {
    interpolate_with(input, |var| std::env::var(var).ok())
}

/// Apply interpolation to every value in a HashMap, returning a new HashMap.
pub fn interpolate_map(
    m: &std::collections::HashMap<String, String>,
) -> std::collections::HashMap<String, String> {
    m.iter()
        .map(|(k, v)| (k.clone(), interpolate(v)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn lookup(var: &str) -> Option<String> {
        match var {
            "HOME" => Some("/home/alice".into()),
            "USER" => Some("alice".into()),
            _ => None,
        }
    }

    #[test]
    fn replaces_known_var() {
        assert_eq!(interpolate_with("path={{env.HOME}}/bin", lookup), "path=/home/alice/bin");
    }

    #[test]
    fn missing_var_becomes_empty() {
        assert_eq!(interpolate_with("x={{env.MISSING}}y", lookup), "x=y");
    }

    #[test]
    fn multiple_vars_in_one_string() {
        assert_eq!(
            interpolate_with("{{env.USER}}@{{env.HOME}}", lookup),
            "alice@/home/alice"
        );
    }

    #[test]
    fn escaped_braces_become_literal() {
        assert_eq!(interpolate_with("literal \\{{env.HOME}}", lookup), "literal {{env.HOME}}");
    }

    #[test]
    fn unknown_namespace_left_intact() {
        let out = interpolate_with("keep {{other.X}} intact", lookup);
        assert_eq!(out, "keep {{other.X}} intact");
    }

    #[test]
    fn malformed_no_closing_braces() {
        let out = interpolate_with("oops {{env.HOME", lookup);
        assert_eq!(out, "oops {{env.HOME");
    }

    #[test]
    fn no_placeholders_passes_through() {
        assert_eq!(interpolate_with("plain text", lookup), "plain text");
    }

    #[test]
    fn interpolate_map_applies_to_all_values() {
        let mut m = HashMap::new();
        m.insert("A".into(), "{{env.HOME}}/a".into());
        m.insert("B".into(), "literal".into());
        let out = interpolate_with(
            "placeholder",
            lookup,
        );
        // use the map helper directly:
        let _ = out;
        let out_map: HashMap<String, String> = m
            .iter()
            .map(|(k, v)| (k.clone(), interpolate_with(v, lookup)))
            .collect();
        assert_eq!(out_map.get("A").map(String::as_str), Some("/home/alice/a"));
        assert_eq!(out_map.get("B").map(String::as_str), Some("literal"));
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p vol-llm-cli-tool interpolate 2>&1 | tail -20`
Expected: All 8 tests pass.

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-cli-tool/src/interpolate.rs
git commit -m "feat(cli-tool): {{env.VAR}} placeholder interpolation"
```

---

### Task 4: Executor + validate + output formatting (TDD)

**Files:**
- Modify: `crates/vol-llm-cli-tool/src/validate.rs`
- Modify: `crates/vol-llm-cli-tool/src/exec.rs`
- Modify: `crates/vol-llm-cli-tool/src/lib.rs` (fill in `CliTool`)

- [ ] **Step 1: Write validate.rs with tests**

Replace `crates/vol-llm-cli-tool/src/validate.rs`:

```rust
//! First-token binaries whitelist check.

/// Extract the first whitespace-delimited token from a command string.
pub fn first_token(command: &str) -> Option<&str> {
    command.split_whitespace().next()
}

/// Validate that the first token is in the allowed list.
pub fn validate_first_token<'a>(
    command: &'a str,
    binaries: &[String],
) -> Result<&'a str, crate::CliToolError> {
    let token = first_token(command).ok_or_else(|| {
        crate::CliToolError::InvalidArguments("command is empty".into())
    })?;
    if binaries.iter().any(|b| b == token) {
        Ok(token)
    } else {
        Err(crate::CliToolError::BinaryNotAllowed {
            token: token.to_string(),
            allowed: binaries.to_vec(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bins() -> Vec<String> {
        vec!["ansible".into(), "ansible-playbook".into()]
    }

    #[test]
    fn valid_first_token() {
        let tok = validate_first_token("ansible-playbook site.yml --limit web", &bins()).unwrap();
        assert_eq!(tok, "ansible-playbook");
    }

    #[test]
    fn invalid_first_token() {
        let err = validate_first_token("rm -rf /", &bins()).unwrap_err();
        match err {
            crate::CliToolError::BinaryNotAllowed { token, allowed } => {
                assert_eq!(token, "rm");
                assert_eq!(allowed, bins());
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn empty_command_rejected() {
        let err = validate_first_token("", &bins()).unwrap_err();
        assert!(matches!(err, crate::CliToolError::InvalidArguments(_)));
    }

    #[test]
    fn whitespace_only_rejected() {
        let err = validate_first_token("   \t  ", &bins()).unwrap_err();
        assert!(matches!(err, crate::CliToolError::InvalidArguments(_)));
    }

    #[test]
    fn leading_whitespace_still_finds_token() {
        let tok = validate_first_token("   ansible all -m ping", &bins()).unwrap();
        assert_eq!(tok, "ansible");
    }
}
```

- [ ] **Step 2: Run validate tests**

Run: `cargo test -p vol-llm-cli-tool validate 2>&1 | tail -20`
Expected: 5 tests pass.

- [ ] **Step 3: Write exec.rs with MockSandbox + tests**

Replace `crates/vol-llm-cli-tool/src/exec.rs`:

```rust
//! Command execution and output formatting.
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use vol_llm_sandbox::{CommandOutput, CommandRequest, Sandbox};

use crate::config::CliToolConfig;
use crate::interpolate;

/// A resolved CLI tool: config + its backing sandbox.
pub struct CliTool {
    pub config: CliToolConfig,
    sandbox: Arc<dyn Sandbox>,
    /// Interpolated env (cached at load time, not per-call).
    env: HashMap<String, String>,
    /// Interpolated cwd (cached at load time).
    cwd: String,
}

impl CliTool {
    /// Build a CliTool from a config + sandbox.
    /// Performs placeholder interpolation on `env` and `cwd` once.
    pub fn new(config: CliToolConfig, sandbox: Arc<dyn Sandbox>) -> Self {
        let env = interpolate::interpolate_map(&config.env);
        let cwd = interpolate::interpolate(&config.cwd);
        Self { config, sandbox, env, cwd }
    }

    /// Run a command string through this tool.
    pub async fn run(
        &self,
        command: &str,
    ) -> Result<ToolOutput, crate::CliToolError> {
        // 1. Validate first token
        crate::validate::validate_first_token(command, &self.config.binaries)?;

        // 2. Build shell command with `cd <cwd> && <command>` so it works
        //    on every sandbox type (SSHSandbox ignores CommandRequest.cwd).
        let shell_body = if self.cwd.is_empty() {
            command.to_string()
        } else {
            format!("cd {} && {}", shell_quote(&self.cwd), command)
        };

        let req = CommandRequest {
            program: self.config.shell.clone(),
            args: vec!["-c".into(), shell_body],
            env: self.env.clone(),
            cwd: None,
            stdin: None,
            timeout: Duration::from_secs(self.config.timeout_secs),
        };

        // 3. Execute via sandbox
        let output = self.sandbox.execute(req).await.map_err(|e| {
            crate::CliToolError::SandboxFailed(e.to_string())
        })?;

        // 4. Format output
        Ok(format_output(&output, self.config.max_output_bytes, self.config.timeout_secs))
    }
}

/// Result of a tool invocation, pre-formatted for the LLM.
pub struct ToolOutput {
    pub success: bool,
    pub content: String,
}

/// Format a CommandOutput into LLM-readable text with per-stream truncation.
pub fn format_output(
    output: &CommandOutput,
    max_bytes: usize,
    _timeout_secs: u64,
) -> ToolOutput {
    let mut text = String::new();
    text.push_str(&format!("exit_code: {}\n", output.exit_code));

    text.push_str("--- stdout ---\n");
    append_truncated(&mut text, &output.stdout, max_bytes);

    text.push_str("\n--- stderr ---\n");
    append_truncated(&mut text, &output.stderr, max_bytes);

    if let Some(sig) = output.killed_by_signal {
        text.push_str(&format!("\n--- killed by signal {sig} ---\n"));
    }

    ToolOutput {
        success: output.exit_code == 0 && output.killed_by_signal.is_none(),
        content: text,
    }
}

fn append_truncated(out: &mut String, bytes: &[u8], max_bytes: usize) {
    // Best-effort UTF-8; replace invalid sequences.
    let s = String::from_utf8_lossy(bytes);
    if s.len() <= max_bytes {
        out.push_str(&s);
    } else {
        let truncated_len = s.len() - max_bytes;
        out.push_str(&s[..max_bytes]);
        out.push_str(&format!("\n... [truncated {truncated_len} bytes]"));
    }
}

/// Shell-quote a string for safe interpolation into `sh -c`.
fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

// ---------------------------------------------------------------------------
// Tests (with MockSandbox)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use std::path::{Path, PathBuf};
    use std::sync::{Arc, Mutex};
    use vol_llm_sandbox::*;

    /// MockSandbox: records the last CommandRequest, returns a preset output.
    struct MockSandbox {
        output: CommandOutput,
        last_request: Arc<Mutex<Option<CommandRequest>>>,
    }

    impl MockSandbox {
        fn new(output: CommandOutput) -> (Self, Arc<Mutex<Option<CommandRequest>>>) {
            let last = Arc::new(Mutex::new(None));
            (Self { output, last_request: Arc::clone(&last) }, last)
        }
    }

    #[async_trait]
    impl Sandbox for MockSandbox {
        fn kind(&self) -> &str { "mock" }
        fn name(&self) -> &str { "mock" }
        async fn start(&self) -> SandboxResult<()> { Ok(()) }
        async fn cleanup(&self) -> SandboxResult<()> { Ok(()) }
        fn root_path(&self) -> &Path { Path::new("/") }
        fn resolve_path(&self, rel: &str) -> SandboxResult<PathBuf> {
            Ok(PathBuf::from(rel))
        }
        async fn execute(&self, req: CommandRequest) -> SandboxResult<CommandOutput> {
            *self.last_request.lock().unwrap() = Some(req);
            Ok(self.output.clone())
        }
        async fn read_file(&self, _p: &Path, _o: Option<u64>, _l: Option<u64>) -> SandboxResult<Vec<u8>> { Ok(vec![]) }
        async fn write_file(&self, _p: &Path, _c: &[u8]) -> SandboxResult<()> { Ok(()) }
        async fn create_dir_all(&self, _p: &Path) -> SandboxResult<()> { Ok(()) }
        async fn read_dir(&self, _p: &Path) -> SandboxResult<Vec<DirEntry>> { Ok(vec![]) }
        async fn metadata(&self, _p: &Path) -> SandboxResult<FileMetadata> {
            Err(SandboxError::Io(std::io::Error::new(std::io::ErrorKind::Other, "mock")))
        }
    }

    fn minimal_config() -> CliToolConfig {
        CliToolConfig {
            name: "ansible".into(),
            description: "Ansible suite".into(),
            binaries: vec!["ansible".into(), "ansible-playbook".into()],
            sandbox: None,
            sandbox_ref: Some("mock".into()),
            cwd: "/opt/ansible".into(),
            shell: "/bin/bash".into(),
            timeout_secs: 60,
            max_output_bytes: 65536,
            env: HashMap::new(),
        }
    }

    #[tokio::test]
    async fn success_flow_builds_correct_request() {
        let output = CommandOutput {
            stdout: b"ok\n".to_vec(),
            stderr: vec![],
            exit_code: 0,
            killed_by_signal: None,
        };
        let (sandbox, last_req) = MockSandbox::new(output);
        let mut cfg = minimal_config();
        cfg.env.insert("ANSIBLE_CONFIG".into(), "/etc/ansible.cfg".into());
        let tool = CliTool::new(cfg, Arc::new(sandbox));

        let out = tool.run("ansible all -m ping").await.unwrap();
        assert!(out.success);
        assert!(out.content.contains("exit_code: 0"));
        assert!(out.content.contains("ok\n"));

        let req = last_req.lock().unwrap().clone().unwrap();
        assert_eq!(req.program, "/bin/bash");
        assert_eq!(req.args, vec!["-c", "cd '/opt/ansible' && ansible all -m ping"]);
        assert_eq!(req.env.get("ANSIBLE_CONFIG").map(String::as_str), Some("/etc/ansible.cfg"));
        assert!(req.cwd.is_none()); // we embed cd in shell body
    }

    #[tokio::test]
    async fn invalid_first_token_rejected() {
        let (sandbox, _) = MockSandbox::new(CommandOutput {
            stdout: vec![], stderr: vec![], exit_code: 0, killed_by_signal: None,
        });
        let tool = CliTool::new(minimal_config(), Arc::new(sandbox));
        let err = tool.run("rm -rf /").await.unwrap_err();
        assert!(matches!(err, crate::CliToolError::BinaryNotAllowed { .. }));
    }

    #[tokio::test]
    async fn non_zero_exit_reports_failure() {
        let output = CommandOutput {
            stdout: vec![],
            stderr: b"ERROR: playbook not found\n".to_vec(),
            exit_code: 4,
            killed_by_signal: None,
        };
        let (sandbox, _) = MockSandbox::new(output);
        let tool = CliTool::new(minimal_config(), Arc::new(sandbox));
        let out = tool.run("ansible-playbook missing.yml").await.unwrap();
        assert!(!out.success);
        assert!(out.content.contains("exit_code: 4"));
        assert!(out.content.contains("ERROR: playbook not found"));
    }

    #[tokio::test]
    async fn stdout_is_truncated_at_max_bytes() {
        let big = vec![b'A'; 1000];
        let output = CommandOutput {
            stdout: big,
            stderr: vec![],
            exit_code: 0,
            killed_by_signal: None,
        };
        let (sandbox, _) = MockSandbox::new(output);
        let mut cfg = minimal_config();
        cfg.max_output_bytes = 100;
        let tool = CliTool::new(cfg, Arc::new(sandbox));
        let out = tool.run("ansible --version").await.unwrap();
        // Count A's in content — should be 100, plus a truncation marker
        let a_count = out.content.matches('A').count();
        assert_eq!(a_count, 100);
        assert!(out.content.contains("[truncated 900 bytes]"));
    }

    #[tokio::test]
    async fn killed_by_signal_appended() {
        let output = CommandOutput {
            stdout: vec![],
            stderr: vec![],
            exit_code: -1,
            killed_by_signal: Some(9),
        };
        let (sandbox, _) = MockSandbox::new(output);
        let tool = CliTool::new(minimal_config(), Arc::new(sandbox));
        let out = tool.run("ansible-playbook slow.yml").await.unwrap();
        assert!(!out.success);
        assert!(out.content.contains("killed by signal 9"));
    }

    #[tokio::test]
    async fn env_placeholders_interpolated_at_construction() {
        std::env::set_var("CLI_TOOL_TEST_TOKEN", "secret-token");
        let (sandbox, last_req) = MockSandbox::new(CommandOutput {
            stdout: vec![], stderr: vec![], exit_code: 0, killed_by_signal: None,
        });
        let mut cfg = minimal_config();
        cfg.env.insert("TOKEN".into(), "{{env.CLI_TOOL_TEST_TOKEN}}".into());
        let tool = CliTool::new(cfg, Arc::new(sandbox));
        let _ = tool.run("ansible --version").await.unwrap();
        let req = last_req.lock().unwrap().clone().unwrap();
        assert_eq!(req.env.get("TOKEN").map(String::as_str), Some("secret-token"));
        std::env::remove_var("CLI_TOOL_TEST_TOKEN");
    }
}
```

- [ ] **Step 4: Run exec tests**

Run: `cargo test -p vol-llm-cli-tool exec 2>&1 | tail -30`
Expected: 6 tests pass. (If `SandboxError::Io` doesn't take `std::io::Error`, adjust to match the real variant — check `crates/vol-llm-sandbox/src/lib.rs`.)

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-cli-tool/src/validate.rs crates/vol-llm-cli-tool/src/exec.rs
git commit -m "feat(cli-tool): executor with validate, cd-in-shell, truncation, MockSandbox tests"
```

---

### Task 5: Loader (load_dir)

**Files:**
- Modify: `crates/vol-llm-cli-tool/src/lib.rs`

- [ ] **Step 1: Write load_dir tests**

Append to `crates/vol-llm-cli-tool/src/lib.rs`:

```rust
use std::path::Path;
use std::sync::Arc;
use vol_llm_sandbox::registry::SandboxRegistry;

/// Load every `*.toml` in `dir` as a CliTool.
///
/// - `sandbox_ref` entries are resolved against `registry`.
/// - Inline `[sandbox]` entries are constructed via
///   `vol_llm_sandbox::registry::build_sandbox`.
/// - Files that fail to parse are logged as warnings and skipped.
/// - Name collisions: if a config's `name` matches an already-loaded tool,
///   returns an error (fail-fast).
pub async fn load_dir(
    dir: &Path,
    registry: &SandboxRegistry,
) -> Result<Vec<CliTool>, CliToolError> {
    let mut tools = Vec::new();
    let mut seen_names = std::collections::HashSet::new();

    if !dir.exists() {
        return Ok(tools);
    }

    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .map_err(|e| CliToolError::Config(format!("read_dir {}: {e}", dir.display())))?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map(|x| x == "toml").unwrap_or(false))
        .collect();
    entries.sort_by_key(|e| e.path());

    for entry in entries {
        let path = entry.path();
        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!(path = %path.display(), error = %e, "cli-tool: read failed, skipping");
                continue;
            }
        };
        let config = match CliToolConfig::from_toml(&content) {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!(path = %path.display(), error = %e, "cli-tool: parse failed, skipping");
                continue;
            }
        };

        if !seen_names.insert(config.name.clone()) {
            return Err(CliToolError::Config(format!(
                "duplicate cli-tool name `{}` in {}",
                config.name,
                path.display()
            )));
        }

        let sandbox: Arc<dyn vol_llm_sandbox::Sandbox> = if let Some(ref name) = config.sandbox_ref {
            registry.get(name).ok_or_else(|| {
                CliToolError::Config(format!(
                    "cli-tool `{}` references unknown sandbox `{}`",
                    config.name, name
                ))
            })?
        } else if let Some(sb_cfg) = config.sandbox.clone() {
            vol_llm_sandbox::registry::build_sandbox(sb_cfg)
                .await
                .map_err(|e| CliToolError::Config(format!(
                    "cli-tool `{}` inline sandbox build failed: {e}",
                    config.name
                )))?
        } else {
            unreachable!("validate() guarantees one of sandbox/sandbox_ref");
        };

        tools.push(CliTool::new(config, sandbox));
    }

    Ok(tools)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[tokio::test]
    async fn load_dir_empty_when_missing() {
        let registry = SandboxRegistry::load(&tempdir().unwrap().path().join("no-such"))
            .await
            .unwrap();
        let tools = load_dir(Path::new("/nonexistent/path/abc123"), &registry)
            .await
            .unwrap();
        assert!(tools.is_empty());
    }

    #[tokio::test]
    async fn load_dir_skips_unparseable_files() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("bad.toml"), "this is not valid toml {{{").unwrap();
        fs::write(dir.path().join("also_bad.toml"), "name = 42").unwrap();
        fs::write(dir.path().join("ignore.txt"), "not a toml").unwrap();

        let sandbox_dir = tempdir().unwrap();
        let registry = SandboxRegistry::load(sandbox_dir.path()).await.unwrap();

        let tools = load_dir(dir.path(), &registry).await.unwrap();
        assert!(tools.is_empty());
    }

    #[tokio::test]
    async fn load_dir_fails_fast_on_unknown_sandbox_ref() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("t.toml"),
            r#"
                name = "foo"
                description = "x"
                binaries = ["x"]
                cwd = "/"
                sandbox_ref = "no-such-sandbox"
            "#,
        )
        .unwrap();

        let sandbox_dir = tempdir().unwrap();
        let registry = SandboxRegistry::load(sandbox_dir.path()).await.unwrap();

        let err = load_dir(dir.path(), &registry).await.unwrap_err().to_string();
        assert!(err.contains("no-such-sandbox"), "unexpected: {err}");
    }

    #[tokio::test]
    async fn load_dir_fails_fast_on_duplicate_name() {
        let dir = tempdir().unwrap();
        let body = r#"
            name = "dup"
            description = "x"
            binaries = ["x"]
            cwd = "/"
            sandbox_ref = "local"
        "#;
        fs::write(dir.path().join("a.toml"), body).unwrap();
        fs::write(dir.path().join("b.toml"), body).unwrap();

        let sandbox_dir = tempdir().unwrap();
        let registry = SandboxRegistry::load(sandbox_dir.path()).await.unwrap();

        let err = load_dir(dir.path(), &registry).await.unwrap_err().to_string();
        assert!(err.contains("duplicate"), "unexpected: {err}");
    }
}
```

- [ ] **Step 2: Run loader tests**

Run: `cargo test -p vol-llm-cli-tool 2>&1 | tail -40`
Expected: All loader tests pass. (The `load_dir_fails_fast_on_unknown_sandbox_ref` test relies on `SandboxRegistry` always having a `local` sandbox — verify by checking `SandboxRegistry::load` behavior on an empty dir.)

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-cli-tool/src/lib.rs
git commit -m "feat(cli-tool): load_dir loader with sandbox resolution and name-collision guard"
```

---

### Task 6: Direct-load sub-crate `vol-llm-tools-builtin-cli-tool`

**Files:**
- Create: `crates/vol-llm-tools-builtin/cli-tool/Cargo.toml`
- Create: `crates/vol-llm-tools-builtin/cli-tool/src/lib.rs`
- Modify: `crates/vol-llm-tools-builtin/Cargo.toml`
- Modify: `crates/vol-llm-tools-builtin/src/lib.rs`
- Modify: `Cargo.toml` (workspace members + deps)

- [ ] **Step 1: Create sub-crate manifest**

Create `crates/vol-llm-tools-builtin/cli-tool/Cargo.toml`:

```toml
[package]
name = "vol-llm-tools-builtin-cli-tool"
version.workspace = true
edition.workspace = true

[dependencies]
vol-llm-tool = { workspace = true }
vol-llm-sandbox = { workspace = true, features = ["ssh"] }
vol-llm-cli-tool = { workspace = true }
async-trait = { workspace = true }
serde_json = { workspace = true }
tracing = { workspace = true }
tokio = { workspace = true }
```

- [ ] **Step 2: Implement `CliToolExecutable`**

Create `crates/vol-llm-tools-builtin/cli-tool/src/lib.rs`:

```rust
//! Wraps `vol-llm-cli-tool::CliTool` as an `ExecutableTool` for the
//! direct-load (path A) deployment.
//!
//! The `name` and `description` fields from the TOML config are leaked
//! once at construction time so that the `&'static str` lifetime
//! required by `ExecutableTool` is satisfied.

use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use vol_llm_cli_tool::{CliTool, CliToolConfig, CliToolError};
use vol_llm_sandbox::registry::SandboxRegistry;
use vol_llm_tool::{ExecutableTool, ToolRegistry, ToolResult, ToolResultType, ToolSensitivity};

pub struct CliToolExecutable {
    name: &'static str,
    description: &'static str,
    parameters: serde_json::Value,
    inner: CliTool,
}

impl CliToolExecutable {
    pub fn from_config(
        config: CliToolConfig,
        sandbox: Arc<dyn vol_llm_sandbox::Sandbox>,
    ) -> Self {
        let name: &'static str =
            Box::leak(config.name.clone().into_boxed_str());
        let description: &'static str =
            Box::leak(config.description.clone().into_boxed_str());
        let parameters = serde_json::json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "CLI command to run. First token must be one of this tool's declared binaries."
                }
            },
            "required": ["command"]
        });
        let inner = CliTool::new(config, sandbox);
        Self { name, description, parameters, inner }
    }
}

#[async_trait]
impl ExecutableTool for CliToolExecutable {
    fn name(&self) -> &'static str { self.name }
    fn description(&self) -> &'static str { self.description }
    fn parameters(&self) -> serde_json::Value { self.parameters.clone() }
    fn sensitivity(&self, _args: &serde_json::Value) -> ToolSensitivity {
        ToolSensitivity::Safe // MVP: no approval gates per spec
    }

    async fn execute(
        &self,
        args: &serde_json::Value,
        _context: &vol_llm_tool::ToolContext,
    ) -> ToolResultType<ToolResult> {
        let command = args
            .get("command")
            .and_then(|v| v.as_str())
            .ok_or_else(|| vol_llm_tool::ToolError::InvalidArguments(
                "missing required parameter: 'command'".into(),
            ))?;

        match self.inner.run(command).await {
            Ok(output) => {
                let mut result = if output.success {
                    ToolResult::success(output.content)
                } else {
                    ToolResult::failure(output.content)
                };
                result.call_id = args
                    .get("call_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                Ok(result)
            }
            Err(e @ CliToolError::InvalidArguments(_))
            | Err(e @ CliToolError::BinaryNotAllowed { .. }) => {
                Err(vol_llm_tool::ToolError::InvalidArguments(e.to_string()))
            }
            Err(e) => Err(vol_llm_tool::ToolError::ExecutionFailed(e.to_string())),
        }
    }
}

/// Load every `*.toml` from `dir` and register each as a `CliToolExecutable`.
///
/// Silently returns Ok(0) if `dir` does not exist (no cli-tools configured).
/// Fails hard on parse errors, name collisions, or missing sandbox refs.
pub async fn register_all(
    registry: &mut ToolRegistry,
    sandbox_registry: &SandboxRegistry,
    dir: &Path,
) -> Result<usize, String> {
    let tools = vol_llm_cli_tool::load_dir(dir, sandbox_registry)
        .await
        .map_err(|e| e.to_string())?;
    let count = tools.len();
    for tool in tools {
        // We need to recover the config + sandbox to build CliToolExecutable.
        // Easiest: extend CliTool with public accessors, or refactor load_dir
        // to return (config, sandbox) tuples. For simplicity here we re-load.
        // (See step 3 for the cleaner refactor.)
        todo!("see step 3 — we split load_dir into (config, sandbox) pairs");
    }
    Ok(count)
}
```

Note: `register_all` above has a `todo!` — we need `CliTool` to expose its config + sandbox so the wrapper can be constructed. The cleanest fix is to refactor `load_dir` to return `(CliToolConfig, Arc<dyn Sandbox>)` tuples, or to add a `CliTool::into_parts()` method.

- [ ] **Step 3: Add `into_parts()` to CliTool**

Add to `crates/vol-llm-cli-tool/src/exec.rs` inside `impl CliTool`:

```rust
/// Decompose back into (config, sandbox) for wrapper layers.
pub fn into_parts(self) -> (CliToolConfig, Arc<dyn vol_llm_sandbox::Sandbox>) {
    (self.config, self.sandbox)
}
```

Also expose `sandbox` as accessible via `pub(crate)` or add an accessor. Easiest: add `pub fn config(&self) -> &CliToolConfig` and `pub fn sandbox(&self) -> Arc<dyn Sandbox>`.

Then update `load_dir` in `lib.rs` so the returned `CliTool` is decomposable, and rewrite `register_all` in the sub-crate:

```rust
pub async fn register_all(
    registry: &mut ToolRegistry,
    sandbox_registry: &SandboxRegistry,
    dir: &Path,
) -> Result<usize, String> {
    let tools = vol_llm_cli_tool::load_dir(dir, sandbox_registry)
        .await
        .map_err(|e| e.to_string())?;
    let count = tools.len();
    for tool in tools {
        let (config, sandbox) = tool.into_parts();
        let exe = CliToolExecutable::from_config(config, sandbox);
        let name = exe.name().to_string();
        if registry.get(&name).is_some() {
            return Err(format!(
                "cli-tool `{name}` collides with an already-registered tool"
            ));
        }
        registry.register(exe);
    }
    Ok(count)
}
```

(If `ToolRegistry::get` doesn't exist, use `registry.list_names()` or a similar inspector — add a one-line helper to `vol-llm-tool` if needed. Check `crates/vol-llm-tool/src/registry.rs`.)

- [ ] **Step 4: Wire sub-crate into vol-llm-tools-builtin**

Append to workspace `Cargo.toml` `members` array:

```toml
    "crates/vol-llm-tools-builtin/cli-tool",
```

Append to `[workspace.dependencies]`:

```toml
vol-llm-tools-builtin-cli-tool = { path = "crates/vol-llm-tools-builtin/cli-tool" }
```

Add to `crates/vol-llm-tools-builtin/Cargo.toml` `[dependencies]`:

```toml
vol-llm-tools-builtin-cli-tool = { path = "cli-tool" }
```

Add to `crates/vol-llm-tools-builtin/src/lib.rs`:

```rust
pub mod cli_tool {
    pub use vol_llm_tools_builtin_cli_tool::*;
}
```

- [ ] **Step 5: Verify compile**

Run:
```
cargo check -p vol-llm-tools-builtin 2>&1 | tail -20
cargo check -p vol-llm-tools-builtin-cli-tool 2>&1 | tail -10
```
Expected: Both compile.

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml crates/vol-llm-tools-builtin/cli-tool crates/vol-llm-tools-builtin/Cargo.toml crates/vol-llm-tools-builtin/src/lib.rs
git commit -m "feat(cli-tool): direct-load sub-crate wrapping core as ExecutableTool"
```

---

### Task 7: Wire direct-load into runtime

**Files:**
- Modify: `crates/vol-llm-runtime/src/lib.rs:~497`

- [ ] **Step 1: Add the register_all call**

Open `crates/vol-llm-runtime/src/lib.rs`. Find the tool-registration block in `AgentRuntimeBuilder::build()` (around line 497):

```rust
vol_llm_task::tools::register_cli(&mut tool_registry, task_store.clone());
```

Immediately after it, insert:

```rust
// Register declarative CLI-as-Tool entries from .agents/cli-tools/*.toml
{
    let cli_tools_dir = self.working_dir.join(".agents").join("cli-tools");
    match vol_llm_tools_builtin::cli_tool::register_all(
        &mut tool_registry,
        &sandbox_registry,
        &cli_tools_dir,
    )
    .await
    {
        Ok(0) => {}
        Ok(n) => tracing::info!(n, "cli-tools registered"),
        Err(e) => return Err(format!("cli-tool registration failed: {e}")),
    }
}
```

- [ ] **Step 2: Verify compile**

Run:
```
cargo check -p vol-llm-runtime 2>&1 | tail -10
cargo check -p vol-agent-server 2>&1 | tail -10
```
Expected: Both compile. If `sandbox_registry` isn't accessible at the insertion point (it's moved into Arc before), adjust: either clone the Arc, or insert the call before the Arc wrap.

- [ ] **Step 3: Smoke-test with no cli-tools configured**

Run: `cargo run -p vol-agent-server -- --help 2>&1 | tail -5`
Expected: Binary still starts, no cli-tools log line (since `.agents/cli-tools/` does not exist in the working dir by default).

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-runtime/src/lib.rs
git commit -m "feat(cli-tool): wire direct-load registration into AgentRuntimeBuilder"
```

---

### Task 8: MCP server module `vol-mcp-servers::cli_tools`

**Files:**
- Create: `crates/vol-mcp-servers/src/cli_tools/mod.rs`
- Create: `crates/vol-mcp-servers/src/cli_tools/server.rs`
- Create: `crates/vol-mcp-servers/src/bin/cli_tools_mcp.rs`
- Modify: `crates/vol-mcp-servers/Cargo.toml`
- Modify: `crates/vol-mcp-servers/src/lib.rs`

- [ ] **Step 1: Add binary target and deps**

Append to `crates/vol-mcp-servers/Cargo.toml`:

```toml
[[bin]]
name = "cli-tools-mcp"
path = "src/bin/cli_tools_mcp.rs"
```

Add to `[dependencies]`:

```toml
vol-llm-cli-tool = { path = "../vol-llm-cli-tool" }
vol-llm-sandbox = { path = "../vol-llm-sandbox", features = ["ssh"] }
```

- [ ] **Step 2: Declare module in lib.rs**

Add to `crates/vol-mcp-servers/src/lib.rs`:

```rust
pub mod cli_tools;
```

- [ ] **Step 3: Implement the MCP server**

Create `crates/vol-mcp-servers/src/cli_tools/mod.rs`:

```rust
//! HTTP MCP server hosting one tool per `.agents/cli-tools/*.toml` config.

pub mod server;

pub use server::CliToolsMcpServer;
```

Create `crates/vol-mcp-servers/src/cli_tools/server.rs`:

```rust
use std::collections::HashMap;
use std::sync::Arc;

use rmcp::{ServerHandler, ServiceExt, tool, tool_router, tool_handler};
use rmcp::model::{Tool as McpToolDef, RawContent};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use vol_llm_cli_tool::CliTool;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandParams {
    /// CLI command to run. First token must match one of the tool's declared binaries.
    pub command: String,
}

struct ToolEntry {
    config_name: String,
    description: String,
    cli_tool: CliTool,
}

pub struct CliToolsMcpServer {
    tools: Vec<ToolEntry>,
    last_call: Arc<Mutex<Option<String>>>, // for diagnostics
}

impl CliToolsMcpServer {
    pub async fn load(
        cli_tools_dir: &std::path::Path,
        sandbox_registry: &vol_llm_sandbox::registry::SandboxRegistry,
    ) -> Result<Self, String> {
        let raw_tools = vol_llm_cli_tool::load_dir(cli_tools_dir, sandbox_registry)
            .await
            .map_err(|e| e.to_string())?;

        let tools = raw_tools
            .into_iter()
            .map(|t| {
                let (cfg, sandbox) = t.into_parts();
                ToolEntry {
                    description: cfg.description.clone(),
                    config_name: cfg.name.clone(),
                    cli_tool: CliTool::new(cfg, sandbox),
                }
            })
            .collect();

        Ok(Self {
            tools,
            last_call: Arc::new(Mutex::new(None)),
        })
    }

    fn find_tool(&self, name: &str) -> Option<&ToolEntry> {
        self.tools.iter().find(|t| t.config_name == name)
    }
}

#[tool_router]
impl CliToolsMcpServer {
    /// Dispatch a command to the named CLI tool.
    ///
    /// `tool_name` is the cli-tool's `name` field from its TOML config.
    pub async fn dispatch(
        &self,
        tool_name: &str,
        params: CommandParams,
    ) -> Result<String, String> {
        let entry = self
            .find_tool(tool_name)
            .ok_or_else(|| format!("unknown cli-tool: {tool_name}"))?;
        let output = entry
            .cli_tool
            .run(&params.command)
            .await
            .map_err(|e| e.to_string())?;
        Ok(output.content)
    }
}

// rmcp's ServerHandler requires us to enumerate tools at the type level OR
// override list_tools dynamically. Since our tool set is runtime-driven,
// we implement ServerHandler directly and delegate list_tools to our
// dynamic registry. See docs-rs for the #[tool_handler] pattern; for a
// dynamic set, skip the macro and write list_tools by hand.

impl ServerHandler for CliToolsMcpServer {
    fn list_tools(
        &self,
        _request: rmcp::request::PaginationRequest,
    ) -> impl std::future::Future<Output = Result<rmcp::model::ListToolsResult, rmcp::Error>> + Send
    {
        let tools: Vec<McpToolDef> = self
            .tools
            .iter()
            .map(|t| {
                McpToolDef {
                    name: t.config_name.clone().into(),
                    description: Some(t.description.clone().into()),
                    input_schema: serde_json::json!({
                        "type": "object",
                        "properties": {
                            "command": {
                                "type": "string",
                                "description": "CLI command to run. First token must match one of the tool's declared binaries."
                            }
                        },
                        "required": ["command"]
                    })
                    .as_object()
                    .cloned()
                    .unwrap_or_default(),
                    annotations: None,
                }
            })
            .collect();
        async move {
            Ok(rmcp::model::ListToolsResult {
                tools,
                next_cursor: None,
            })
        }
    }

    fn call_tool(
        &self,
        request: rmcp::request::CallToolRequest,
    ) -> impl std::future::Future<Output = Result<rmcp::model::CallToolResult, rmcp::Error>> + Send
    {
        let tool_name = request.params.name.to_string();
        let params: CommandParams = serde_json::from_value(
            serde_json::to_value(&request.params.arguments).unwrap_or_default(),
        )
        .unwrap_or(CommandParams { command: String::new() });

        async move {
            // Use the dispatch method via a synchronous wrapper.
            // We must drive the async run() inside this future.
            let entry = self
                .tools
                .iter()
                .find(|t| t.config_name == tool_name)
                .ok_or_else(|| rmcp::Error::invalid_request(format!("unknown tool: {tool_name}")))?;
            let output = entry
                .cli_tool
                .run(&params.command)
                .await
                .map_err(|e| rmcp::Error::internal_error(e.to_string()))?;
            Ok(rmcp::model::CallToolResult {
                content: vec![RawContent::Text(rmcp::model::TextContent {
                    text: output.content.into(),
                    annotations: None,
                })],
                is_error: !output.success,
                structured_content: None,
            })
        }
    }
}
```

Note: the exact `rmcp::request::*`, `rmcp::model::*`, and `ServerHandler` method signatures depend on the rmcp version (1.6 per existing Cargo.toml). If any types don't match, consult the `docs_rs` module in the same crate and mirror its shapes. The key idea is: implement `ServerHandler` directly (not via `#[tool_handler]`) because our tool list is dynamic.

- [ ] **Step 4: Create the binary entry point**

Create `crates/vol-mcp-servers/src/bin/cli_tools_mcp.rs`:

```rust
use std::path::PathBuf;
use std::sync::Arc;

use clap::Parser;
use tokio_util::sync::CancellationToken;
use vol_mcp_servers::cli_tools::CliToolsMcpServer;
use vol_mcp_servers::transport::{self, TransportArgs};
use vol_llm_sandbox::registry::SandboxRegistry;

#[derive(Parser)]
#[command(name = "cli-tools-mcp", about = "CLI-as-Tool MCP server")]
struct Cli {
    #[command(flatten)]
    transport: TransportArgs,

    /// Directory containing .agents/cli-tools/*.toml configs.
    #[arg(long, default_value = ".agents/cli-tools")]
    cli_tools_dir: PathBuf,

    /// Directory containing .agents/sandboxes/*.toml configs (for sandbox_ref).
    #[arg(long, default_value = ".agents/sandboxes")]
    sandboxes_dir: PathBuf,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let otel_config = vol_llm_observability::OtelConfig {
        // mirror docs-rs's OtelConfig setup
        service_name: "cli-tools-mcp".into(),
        ..Default::default()
    };
    let _otel_guards = vol_llm_observability::init(&otel_config, "info")
        .expect("Failed to initialize tracing");

    let cli = Cli::parse();

    let sandbox_registry = SandboxRegistry::load(&cli.sandboxes_dir)
        .await
        .map_err(|e| anyhow::anyhow!("sandbox registry: {e}"))?;
    let sandbox_registry = Arc::new(sandbox_registry);

    let server = CliToolsMcpServer::load(&cli.cli_tools_dir, &sandbox_registry)
        .await
        .map_err(|e| anyhow::anyhow!("cli-tools load: {e}"))?;

    let ct = CancellationToken::new();
    transport::run_server(cli.transport.mode(), server, ct).await
}
```

- [ ] **Step 5: Build the binary**

Run: `cargo build -p vol-mcp-servers --bin cli-tools-mcp 2>&1 | tail -30`
Expected: Compiles. If `rmcp::Error::invalid_request` / `internal_error` don't exist with those names, consult the rmcp docs / existing `docs_rs` for the correct constructors.

- [ ] **Step 6: Smoke-test with empty config dir**

Run:
```
mkdir -p /tmp/empty-cli-tools /tmp/empty-sandboxes
./target/debug/cli-tools-mcp --cli-tools-dir /tmp/empty-cli-tools --sandboxes-dir /tmp/empty-sandboxes --help 2>&1 | tail -10
```
Expected: Prints usage info.

- [ ] **Step 7: Commit**

```bash
git add crates/vol-mcp-servers
git commit -m "feat(cli-tool): HTTP MCP server hosting dynamic cli-tools"
```

---

### Task 9: Example config + workspace smoke test

**Files:**
- Create: `.agents/cli-tools/ansible.toml` (example)

- [ ] **Step 1: Create the example config**

Create `.agents/cli-tools/ansible.toml`:

```toml
name = "ansible"
description = """
Ansible automation suite.

Available CLIs:
- `ansible <pattern> -m <module> -a <args>` - ad-hoc commands
  Examples:
  - `ansible all -m ping`
  - `ansible webservers -a "uptime"`
- `ansible-playbook <playbook.yml> [options]` - run playbooks
  Options: --limit, --tags, --skip-tags, --check, --diff, --extra-vars
  Examples:
  - `ansible-playbook site.yml --limit web`
- `ansible-galaxy <action>` - manage roles/collections
- `ansible-vault <action>` - encrypt/decrypt secrets

For end-to-end workflows, invoke skill `ansible-usage`.
"""

binaries = ["ansible", "ansible-playbook", "ansible-galaxy", "ansible-vault"]

sandbox_ref = "ansible-prod"   # <- requires .agents/sandboxes/ansible-prod.toml

cwd = "/opt/ansible"
shell = "/bin/bash"
timeout_secs = 300
max_output_bytes = 65536

[env]
ANSIBLE_CONFIG = "{{env.HOME}}/ansible/ansible.cfg"
SSH_AUTH_SOCK = "{{env.SSH_AUTH_SOCK}}"
```

Note: this example references a sandbox `ansible-prod` that isn't created here — users must create `.agents/sandboxes/ansible-prod.toml` matching their environment.

- [ ] **Step 2: Workspace-wide compile check**

Run: `cargo check --workspace 2>&1 | tail -30`
Expected: Everything compiles. Any errors are task-level regressions — fix them before claiming done.

- [ ] **Step 3: Run all unit tests in new crate**

Run: `cargo test -p vol-llm-cli-tool 2>&1 | tail -40`
Expected: All tests pass (config × 6, interpolate × 8, validate × 5, exec × 6, loader × 4 = 29 tests).

- [ ] **Step 4: Verify direct-load path end-to-end (no SSH needed)**

Create a local sandbox config and a cli-tool config that uses it:

```
mkdir -p .agents/sandboxes .agents/cli-tools
cat > .agents/sandboxes/local-for-cli.toml <<'EOF'
name = "local-for-cli"
type = "local"
work_dir = "/tmp"
EOF

cat > .agents/cli-tools/echo-tool.toml <<'EOF'
name = "echo-tool"
description = "Test tool that runs echo via local sandbox"
binaries = ["echo", "date"]
sandbox_ref = "local-for-cli"
cwd = "/tmp"
EOF
```

Run `cargo run -p vol-agent-server` (or a test harness that constructs `AgentRuntime`) and observe the log line `cli-tools registered, n=1`.

- [ ] **Step 5: Verify MCP server end-to-end**

In one terminal:
```
./target/debug/cli-tools-mcp \
  --cli-tools-dir .agents/cli-tools \
  --sandboxes-dir .agents/sandboxes \
  --mode http --port 8090
```

In another terminal, probe the MCP endpoint (example via curl using MCP's Streamable HTTP protocol — see `docs_rs` docs for the exact path):
```
curl -X POST http://localhost:8090/mcp \
  -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}'
```
Expected: JSON-RPC response listing `echo-tool`.

- [ ] **Step 6: Commit example**

```bash
git add .agents/cli-tools/ansible.toml
git commit -m "docs(cli-tool): example ansible.toml config"
```

- [ ] **Step 7: Final verification**

Run:
```
cargo test -p vol-llm-cli-tool -p vol-llm-tools-builtin-cli-tool -p vol-mcp-servers 2>&1 | tail -10
cargo build --workspace 2>&1 | tail -5
```
Expected: All tests pass, workspace builds clean.

---

## Self-Review Checklist (for the implementing engineer)

- [ ] Every test passes in isolation (`cargo test -p <crate>`)
- [ ] `cargo check --workspace` is clean (no errors; warnings OK if justified)
- [ ] `load_dir` returns empty vec on missing dir, fails on duplicate name, fails on unknown sandbox_ref
- [ ] First-token validation rejects `rm -rf /` with a `BinaryNotAllowed` error
- [ ] Output truncation kicks in at `max_output_bytes`, with `[truncated N bytes]` marker
- [ ] `{{env.VAR}}` resolves from the loader process env at load time, NOT per-call
- [ ] Both paths (direct-load and MCP) produce the same tool name and parameter schema
- [ ] MCP `tools/list` returns one entry per TOML config; `tools/call` routes by tool name
- [ ] No changes to the `Sandbox` or `ExecutableTool` traits (per spec N1)
- [ ] `.agents/cli-tools/ansible.toml` example parses without error
