# CLI-as-Tool Design

**Date:** 2026-06-24
**Status:** Design Draft

---

## Overview

Expose remote CLI commands (e.g. `ansible`, `ansible-playbook`, `kubectl`) as **named tools** the LLM can invoke, with each tool defined declaratively via a TOML config. SSH is the transport tunnel, not the abstraction — the core design is a `Sandbox`-backed tool with a working directory, an env block, and a whitelist of allowed CLI binaries.

Two deployment paths share the same config format:

| Path | Where the tool runs | Agent-side reference |
|---|---|---|
| **C (MCP, primary)** | Remote HTTP MCP server (`vol-mcp-servers/cli_tools`) | `agent_def.mcps` / `.mcp.json` |
| **A (direct-load)** | In-process inside agent-server | `agent_def.tools.allow` |

From the LLM's perspective the two paths are indistinguishable — both expose a tool named e.g. `ansible` that takes a single `command: string` argument.

---

## Requirements

### Functional

| ID | Requirement | Priority |
|---|---|---|
| R1 | Declare a CLI tool via TOML (name, description, binaries, sandbox, cwd, env, shell, timeout) | High |
| R2 | Support multiple CLI binaries per tool (e.g. `ansible` + `ansible-playbook` + `ansible-galaxy`) | High |
| R3 | Validate first token of command against `binaries` whitelist before execution | High |
| R4 | Interpolate `{{env.VAR}}` placeholders in all string config fields from the local process env | High |
| R5 | Execute the command in the configured sandbox with `cwd`, `env`, `shell`, `timeout` applied | High |
| R6 | Support two sandbox-reference styles: inline SSH config, or `sandbox_ref` to `.agents/sandboxes/*.toml` | High |
| R7 | Deploy the tool via HTTP MCP server (path C) OR direct-load in agent-server (path A) | High |
| R8 | Description is hand-written; no auto-generation, no skill-reference validation | High |
| R9 | Truncate stdout/stderr at 64 KB per stream with a clear truncation marker | High |

### Non-Functional

| ID | Requirement | Priority |
|---|---|---|
| N1 | Zero changes to `Sandbox` or `ExecutableTool` traits | High |
| N2 | Config parse errors fail agent-server startup (fail-fast) | High |
| N3 | Core logic lives in a dedicated `vol-llm-cli-tool` crate, reused by both paths | High |
| N4 | No `sensitivity` / approval gates in MVP; both paths default to unguarded execution | Medium |
| N5 | Coverage ≥ 80 % on `vol-llm-cli-tool` core modules (config, interpolate, validate, exec) | Medium |

### Out of Scope

- Streaming output (request/response only for MVP)
- Interactive stdin (CommandRequest.stdin is always `None`; tools like `ansible --ask-pass` that prompt are not supported)
- Per-call timeout / cwd / env overrides from the LLM
- Sensitivity / human-approval gates
- Automatic description generation from `--help` output
- Automatic skill association
- Multi-token / sub-command validation
- Whole-environment forwarding (only explicitly declared `[env]` keys are passed)

---

## Config Model

### File Location

- **Path A (direct-load):** `.agents/cli-tools/<name>.toml` (agent-server local filesystem)
- **Path C (MCP):** loaded by the remote `cli-tools-server`; the config files can live anywhere the server reads from (typically the same `.agents/cli-tools/` layout on the server host)

The schema is identical in both paths.

### Schema

```toml
# .agents/cli-tools/ansible.toml
name = "ansible"

description = """
Ansible automation suite.

Available CLIs:
- `ansible <pattern> -m <module> -a <args>` — ad-hoc commands
  - `ansible all -m ping`
  - `ansible webservers -a "uptime"`
- `ansible-playbook <playbook.yml> [options]` — run playbooks
  Options: --limit, --tags, --skip-tags, --check, --diff, --extra-vars
  - `ansible-playbook site.yml --limit web`
- `ansible-galaxy <action>` — manage roles/collections (install / list / info)
- `ansible-vault <action>` — encrypt / decrypt (encrypt / decrypt / view / edit / rekey)

For end-to-end workflows and best practices, invoke skill `ansible-usage`.
"""

# Whitelist: the first whitespace-delimited token of `command` must be in this list.
binaries = ["ansible", "ansible-playbook", "ansible-galaxy", "ansible-vault"]

# --- Sandbox (transport) ---
# Two styles; pick ONE. If both are present, config loading fails.

# Style 1: inline SSH config (self-contained)
[sandbox]
type = "ssh"
host = "ansible-prod.example.com"
port = 22
user = "deploy"
key_file = "{{env.HOME}}/.ssh/id_ed25519"

# Style 2: reference an existing sandbox (shares connection config across tools)
# sandbox_ref = "ansible-prod"       # points to .agents/sandboxes/ansible-prod.toml

# --- Semantics ---
cwd = "/opt/ansible"                  # remote working directory
shell = "/bin/bash"                   # default: /bin/sh
timeout_secs = 300                    # default: 60
max_output_bytes = 65536              # per-stream truncation; default: 65536 (64 KB)

[env]
# Values support {{env.VAR}} placeholders resolved from the loader process env.
ANSIBLE_CONFIG = "{{env.HOME}}/ansible/ansible.cfg"
SSH_AUTH_SOCK = "{{env.SSH_AUTH_SOCK}}"
ANSIBLE_INVENTORY = "inventories/production"   # literal values are also fine
```

### Field Semantics

| Field | Required | Default | Notes |
|---|---|---|---|
| `name` | yes | — | Tool name the LLM sees. Must not collide with any built-in tool name. |
| `description` | yes | — | Hand-written. The only place to hint at skills, subcommands, examples. |
| `binaries` | yes | — | Non-empty list. First-token whitelist. |
| `sandbox` xor `sandbox_ref` | yes | — | Inline config (Style 1) OR reference (Style 2). Both present → load error. |
| `cwd` | yes | — | Remote working directory. Also supports `{{env.VAR}}`. |
| `shell` | no | `/bin/sh` | Wrap program; invoked as `<shell> -c "<command>"`. |
| `timeout_secs` | no | 60 | Pass-through to `CommandRequest.timeout`. |
| `max_output_bytes` | no | 65536 | Per-stream truncation ceiling. |
| `env` | no | `{}` | Key/value pairs; values support `{{env.VAR}}`. |

### Placeholder Interpolation

- Syntax: `{{env.VAR}}` — replaced with the value of the local env var `VAR` at load time.
- Scope: every string-valued field in the config (`host`, `user`, `key_file`, `cwd`, each `env` value, etc.).
- Missing variable: replaced with empty string and a `warn!` at load time. No hard error by default.
- Escaping: literal `{{` must be written as `\{{` (one level of backslash escape).
- Reserved for future expansion: `{{config.X}}`, `{{sandbox.X}}`, `{{agent.X}}`. The interpolator must reject unknown namespaces with a warning.

---

## Execution Flow

```
execute(args):
    1. command ← args["command"] as string
       if missing / not a string → InvalidArguments

    2. first_token ← command.split_whitespace().next()
       if first_token not in config.binaries:
           return InvalidArguments(
               "first token '{first_token}' is not in allowed binaries: {binaries:?}"
           )

    3. env ← interpolate(config.env)        # resolve {{env.VAR}} from local process env
       cwd ← interpolate(config.cwd)        # (cached at load time, not per-call)

    4. req ← CommandRequest {
               program: config.shell,
               args:    ["-c", command],
               cwd:     Some(cwd),
               env,
               timeout: Duration::from_secs(config.timeout_secs),
               stdin:   None,
           }

    5. output ← self.sandbox.execute(req)

    6. format output into ToolResult:
         success ← output.exit_code == 0
         text    ← format!("{exit}\n--- stdout ---\n{stdout}\n--- stderr ---\n{stderr}")
         apply per-stream truncation at max_output_bytes with marker:
             "\n... [truncated {N} bytes]"
         if let Some(sig) = output.killed_by_signal:
             text += "\n--- killed by signal {sig} ---"

    7. return ToolResult { call_id, success, content: text, error: None, data: None }
```

Interpolation of `env` and `cwd` happens **once at config load time**, not on every tool call. This keeps per-call cost constant and means misconfigured env vars surface at startup, not mid-flight.

---

## Architecture

### Crate Layout

```
crates/
├── vol-llm-cli-tool/                       # NEW — core abstraction
│   ├── src/
│   │   ├── lib.rs                          # pub mod {config, interpolate, validate, exec, error}
│   │   ├── config.rs                       # CliToolConfig + TOML parse + load_dir()
│   │   ├── interpolate.rs                  # {{env.VAR}} substitution
│   │   ├── validate.rs                     # first-token binaries check
│   │   ├── exec.rs                         # build CommandRequest, call sandbox.execute, format output
│   │   └── error.rs                        # CliToolError enum
│   └── Cargo.toml                          # deps: toml, serde, thiserror, vol-llm-sandbox
│
├── vol-mcp-servers/
│   └── src/
│       ├── ... (existing: docs_rs, etc.)
│       └── cli_tools/                      # NEW module
│           ├── mod.rs
│           └── server.rs                   # rmcp service: loads dir, exposes one MCP tool per config
│
├── vol-llm-tools-builtin/
│   └── src/
│       ├── ... (existing: bash-tool, etc.)
│       └── cli_tool.rs                     # NEW — CliToolExecutable wraps vol-llm-cli-tool as ExecutableTool
│
└── vol-llm-runtime/
    └── src/lib.rs                          # build() gains one line: CliToolExecutable::register_all(...)
```

### Dependency Direction

```
vol-mcp-servers ──────┐
                       ├──► vol-llm-cli-tool ──► vol-llm-sandbox
vol-llm-tools-builtin ─┘
```

`vol-llm-cli-tool` deliberately does **not** depend on `vol-llm-tool` (the `ExecutableTool` trait). This keeps it usable by the MCP server path, which does not go through `ExecutableTool`.

### Path C — HTTP MCP Server

A long-running HTTP MCP server hosted as a new module inside `vol-mcp-servers`:

- Binary: `vol-mcp-servers` gains a new subcommand or a new binary target `vol-mcp-cli-tools`.
- Loads every `*.toml` from a configurable directory (default: `.agents/cli-tools/`).
- Each config becomes **one MCP tool** with the tool's `name` equal to `config.name`.
- Exposes a single HTTP endpoint (e.g. `http://cli-tools.internal:8080/mcp`) serving all tools.
- Uses the existing `McpTransport::Http` on the client side.

Agent config:

```json
// .mcp.json
{
  "cli-tools-prod": {
    "transport": {
      "type": "http",
      "url": "http://cli-tools.internal:8080/mcp",
      "headers": { "Authorization": "Bearer {{env.CLI_TOOLS_TOKEN}}" }
    }
  }
}
```

Agent-side filtering works unchanged: `agent_def.mcps = ["cli-tools-prod"]` pulls in every tool hosted by that server.

### Path A — Direct Load

`vol-llm-tools-builtin::cli_tool::CliToolExecutable` wraps `vol-llm-cli-tool`'s executor as an `ExecutableTool`:

```rust
pub struct CliToolExecutable {
    inner: vol_llm_cli_tool::CliTool,   // config + resolved sandbox
}

impl ExecutableTool for CliToolExecutable {
    fn name(&self)        -> &'static str { &self.inner.config.name }
    fn description(&self) -> &'static str { &self.inner.config.description }
    fn parameters(&self)  -> serde_json::Value { /* single "command" string */ }
    async fn execute(&self, args, _ctx) -> ToolResultType<ToolResult> {
        self.inner.run(args).await
    }
}
```

`&'static str` constraint is solved the same way `McpTool` does it today (leak on registration or `Box::leak` of a `String` — follow the existing pattern).

Registration in `AgentRuntimeBuilder::build()`:

```rust
let sandbox_registry = SandboxRegistry::load(&working_dir.join(".agents/sandboxes"))?;
vol_llm_tools_builtin::cli_tool::register_all(
    &mut tool_registry,
    &sandbox_registry,
    &working_dir.join(".agents/cli-tools"),
)?;
```

### Name Collision Rule

- Tool name collisions between a CLI tool and a built-in tool → startup error.
- Tool name collisions between a path-A direct-load and a path-C MCP tool → startup error.
- Rule of thumb: fail-fast with a clear message naming both contenders.

---

## Parameter & Output Contract

### Tool Parameter Schema

```json
{
  "type": "object",
  "properties": {
    "command": {
      "type": "string",
      "description": "The CLI command to run. First token must be one of this tool's declared binaries."
    }
  },
  "required": ["command"]
}
```

The LLM cannot override `cwd`, `env`, `shell`, or `timeout` per call. All runtime context comes from the config.

### Output Format

```
<exit_code>
--- stdout ---
<stdout content — possibly truncated>
--- stderr ---
<stderr content — possibly truncated>
```

- `success = (exit_code == 0)`.
- Each of stdout/stderr truncated independently at `max_output_bytes`. Trailing marker: `\n... [truncated {N} bytes]`.
- If the sandbox killed the process: append `\n--- killed by signal {sig} ---`.

---

## Error Model

| Failure | Error type | Surfaced as |
|---|---|---|
| Missing `command` arg | `CliToolError::InvalidArguments` | `ToolResult.success = false`, error message in content |
| First token not in `binaries` | `CliToolError::BinaryNotAllowed` | `ToolResult.success = false`, message lists allowed binaries |
| Sandbox exec failure (connect error, auth failure) | `CliToolError::SandboxFailed` | `ToolResult.success = false` |
| Timeout (sandbox kills the process) | `CliToolError::Timeout` | `ToolResult.success = false`, `killed_by_signal` reported |
| Config load error (TOML parse, missing required field, `sandbox` AND `sandbox_ref` both set) | startup panic / `Result::Err` from `build()` | agent-server refuses to start |
| Env-var placeholder references unknown namespace | `warn!` at load, empty-string substitution | no failure, logged |

---

## Testing Strategy

| Layer | Test kind | Coverage target |
|---|---|---|
| `vol-llm-cli-tool::config` | unit: minimal TOML / full TOML / inline sandbox / sandbox_ref / both-set rejection / missing required field | ≥ 80 % |
| `vol-llm-cli-tool::interpolate` | unit: existing var / missing var / multiple vars / escaped `\{{` / unknown namespace warning | ≥ 80 % |
| `vol-llm-cli-tool::validate` | unit: in-list / not-in-list / empty command / whitespace-only / multi-token with valid first | ≥ 80 % |
| `vol-llm-cli-tool::exec` | integration with `MockSandbox` (in-crate test double implementing `Sandbox`): assert `CommandRequest` fields, output formatting, truncation, exit-code mapping | ≥ 80 % |
| `vol-llm-tools-builtin::cli_tool` | integration: `CliToolExecutable` + `MockSandbox` → assert `ToolResult` shape, `&'static str` lifetime handling | ≥ 80 % |
| `vol-mcp-servers::cli_tools` | integration: in-memory rmcp transport, exercise `tools/list` and `tools/call` end-to-end | ≥ 80 % |
| E2E (feature-gated `ssh`) | real SSH sandbox + real remote binary, conditional in CI | best-effort |

---

## Deployment Notes

- `vol-mcp-cli-tools` binary is deployed like any other MCP HTTP server (k8s Deployment + Service, or sidecar). The agent-server connects via `McpTransport::Http`.
- Path A is the fallback for single-box / developer setups: drop TOMLs into `.agents/cli-tools/`, agent-server picks them up on next start.
- Both paths are mutually exclusive per tool name — never register the same tool through both paths at the same time.

---

## Open Questions

None at time of approval.
