# Design: Built-in Tools Comprehensive Sandbox Testing

Date: 2026-08-09 | Status: approved

## Goal

Achieve **≥90% line coverage** for all 8 built-in tools in `vol-llm-tools-builtin`,
with comprehensive sandbox workdir integration tests — every tool tested under a
restricted sandbox root (not `/`), cross-tool chain tests, and boundary/error
scenario coverage.

## Current State

All 8 built-in tools are registered via `vol_llm_tools_builtin::register_all` +
`register_web_all`. Every tool that touches the filesystem goes through the
`Sandbox` trait (`context.sandbox.read_file`, `write_file`, `execute`, `read_dir`,
`metadata`, `resolve_path`). The `ToolContext` always carries a `sandbox: SandboxRef`.

**Existing test gaps:**
- Every existing test uses `ToolContext::for_test()` which creates a `LocalSandbox`
  rooted at `/` — paths from `tempfile` work by coincidence, not by design.
- Bash tool's `working_dir` parameter is defined but **never tested**.
- `grep` tool tests don't verify sandbox-mediated search path resolution.
- `glob` tool (most comprehensive suite) still uses `/` root.
- Zero tests exist for `web_search` and `web_fetch` (both provider-based tools).
- No cross-tool chain integration tests exist.
- No tool-level path-traversal rejection tests exist.

## Directory Structure

```
crates/vol-llm-tools-builtin/
├── tests/                                    # NEW — centralized sandbox integration tests
│   ├── mod.rs
│   ├── fixtures.rs                           # Shared sandbox fixtures and helpers
│   ├── sandbox_workdir_tests.rs              # A: Single-tool + different workdirs
│   ├── tool_chain_tests.rs                   # B: Cross-tool chain integration
│   ├── sandbox_boundary_tests.rs             # C: Boundary/error scenarios
│   └── web_tool_tests.rs                     # D: Web tool mock tests
├── bash-tool/tests/   → expand existing
├── read-tool/tests/   → expand existing
├── write-tool/tests/  → expand existing
├── edit-tool/tests/   → expand existing
├── grep-tool/tests/   → expand existing
├── glob-tool/tests/   → minor additions (already comprehensive)
├── web-search-tool/   → NEW tests/
└── web-fetch/         → NEW tests/
```

**Rationale:** Per-tool `tests/` directories keep unit-level tests co-located with
each tool. The new centralized `tests/` directory under `vol-llm-tools-builtin`
hosts cross-cutting integration tests that share a common fixture library. No new
crates are created.

## Shared Fixtures (`tests/fixtures.rs`)

```rust
/// Create a sandbox rooted at a temp dir. Returns (ToolContext, TempDir).
/// The TempDir acts as a cleanup guard — sandbox ops go through the trait,
/// but the actual files live here.
pub fn sandbox_in_tempdir() -> (ToolContext, TempDir)

/// Create a sandbox rooted at tempdir/<subdir>, simulating an agent
/// whose working_dir is a subdirectory of the sandbox root.
pub fn sandbox_in_subdir(subdir: &str) -> (ToolContext, TempDir)

/// Populate files in the sandbox. Each tuple is (relative_path, content).
pub async fn populate_files(sandbox: &SandboxRef, files: &[(&str, &str)])

/// Create a ToolContext with an AgentDef — simulates real agent execution.
pub fn agent_context(sandbox: SandboxRef, working_dir: Option<&str>) -> ToolContext
```

## Test Scenario Matrix

### A. Single Tool + Varied Workdirs

Each tool is tested with sandbox root set to a restricted temp directory
(`sandbox_in_tempdir()`), verifying that:
- Path resolution is containment-checked
- File operations succeed within the sandbox root
- Paths in tool output are relative to sandbox root (where applicable)

| Tool | Scenarios | Est. new tests |
|---|---|---|
| `bash` | `working_dir` param switches cwd; stdout/stderr captured; exit code; timeout with restricted root | 5 |
| `read_file` | Read under restricted root; relative path; offset+limit combo; file not found | 5 |
| `write_file` | Write under restricted root; parent-dir auto-creation; overwrite; empty content | 4 |
| `edit_file` | Edit under restricted root; single replace; replace_all; old_string not found; multi-occurrence error | 5 |
| `grep` | Search under restricted root; content/count/files_with_matches modes; glob filter; case-sensitive; no matches | 5 |
| `glob` | Sandbox root != `/` for pattern matching; relative path return; metadata under restricted root | 3 |

### B. Cross-Tool Chain Integration

Each chain runs entirely within a restricted sandbox:

| Chain | Steps | Verifies |
|---|---|---|
| `glob → grep → read` | Find files by pattern → search for content → read specific lines | End-to-end data flow through sandbox |
| `write → read → edit → read` | Create file → read → edit → read back | Write/read/edit round-trip correctness |
| `bash → write → bash → read` | echo to file → verify written → execute generated script → read output | Shell command + file tool interop |
| `glob → edit → grep` | Find files → batch replace → verify with grep | Bulk edit workflow |

### C. Boundary / Error Scenarios

Focused on tool-level error handling when the sandbox rejects operations:

| Scenario | Tools Covered | Verification |
|---|---|---|
| Path traversal `../../../etc/passwd` | read, write, edit, glob, grep | All return error, no file leaked |
| Absolute path when root ≠ `/` | read, write, edit | Rejected by sandbox containment |
| File not found | read, edit | Clear error message, no panic |
| Empty old_string in edit | edit | Parameter validation error |
| Dangerous bash commands | bash | Security regex blocks (rm -rf /, fork bomb, curl\|bash, /dev/tcp) |
| Search path not found | glob | Returns empty result with message, not error |
| Grep no matches | grep | Returns "No matches found" |
| Grep invalid output_mode | grep | Parameter validation error |

### D. Web Tools

Mock-based tests — no real network:

| Tool | Approach |
|---|---|
| `web_search` | Implement a mock `SearchFn` that returns predefined results; verify formatting and error handling |
| `web_fetch` | Implement a mock `FetchFn` that returns predefined HTML/text; verify content extraction |

### E. Per-Tool Existing Test Expansion

| Tool | Current state | Additions |
|---|---|---|
| `bash` | 5 tests, no `working_dir` | Add workdir param tests, cwd verification |
| `read` | 4 tests | Add restricted-root path resolution tests |
| `write` | 3 tests | Add restricted-root tests, edge cases |
| `edit` | 4 tests | Add restricted-root, error path coverage |
| `grep` | 9 tests | Add sandbox-mediated search root tests |
| `glob` | ~45 tests | Minor sandbox-root-path additions |
| `web_search` | 0 tests | Full mock-based suite |
| `web_fetch` | 0 tests | Full mock-based suite |

## Coverage Targets

| Crate/Tool | Target |
|---|---|
| `vol-llm-tools-builtin` overall | ≥90% line |
| `bash` | ≥90% |
| `read` | ≥90% |
| `write` | ≥90% |
| `edit` | ≥90% |
| `grep` | ≥90% |
| `glob` | ≥90% |
| `web_search` | ≥90% |
| `web_fetch` | ≥90% |

Exclusions (per project convention): none — these are library crates with no
`main.rs`/`app.rs`/`health.rs`.

## Implementation Order

1. **fixtures.rs** — shared test infrastructure (sandbox builders, file helpers)
2. **Expand per-tool tests** — add workdir/restricted-root scenarios to existing
   `bash-tool/tests/`, `read-tool/tests/`, `write-tool/tests/`, `edit-tool/tests/`,
   `grep-tool/tests/`, `glob-tool/tests/`
3. **New web tool tests** — `web-search-tool/tests/` + `web-fetch/tests/`
4. **Centralized integration tests** — `tests/sandbox_workdir_tests.rs`,
   `tests/tool_chain_tests.rs`, `tests/sandbox_boundary_tests.rs`,
   `tests/web_tool_tests.rs`
5. **Coverage gate** — `make coverage-threshold PKG=vol-llm-tools-builtin PCT=90`

## Non-Goals

- Testing `SSHSandbox`, `FirecrackerSandbox`, or `WasmSandbox` backends (those are
  sandbox-crate concerns, not tool-crate concerns)
- Testing MCP tool proxying (`mcp_tool.rs`) — MCP integration is tested separately
- Testing `SkillTool` or `TaskCliTool` — those live in other crates
- Real network calls for web tools — mock providers only
