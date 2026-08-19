---
type: source
source_type: design
date: 2026-08-19
ingested: 2026-08-19
tags: [testing, unit-test, integration-test, e2e, git-hooks, just, ci, playwright, sandbox, wasmtime]
---

# Three-Tier Test Completion: E2E Landing, Broken-Test Fixes, Sandbox Kill Rework

**Authors/Creators:** BestNathan + Claude
**Date:** 2026-08-19
**Link:** `justfile`, `.githooks/pre-*`, `.github/workflows/{quality,e2e}.yml`, `crates/vol-llm-sandbox/src/local.rs`, e2e test files across 8 crates
**Follows:** [[test-tiering-hooks]]

## TL;DR

Completed the three-tier test split established on 2026-08-18. All `#[ignore]`d tests are now genuine e2e tests with a standardized `#[ignore = "e2e: <requirement>"]` marker and in-test environment guards that skip cleanly (empty env vars treated as missing — GitHub Actions passes empty strings for unconfigured secrets). Fixed all broken/ignored tests (wasmtime exit-code test, brittle CodeAgentSimulator test, runtime inline-ignored test, 2 bash-timeout tests). Added a manual `e2e.yml` workflow (secrets-gated Rust e2e + frontend Playwright) and wired frontend Playwright into the PR gate. Reworked `LocalSandbox` timeout kill: positive-pid kills only — process-group kills (`kill -TERM -pgid`) kill the caller's whole process tree in sandboxed environments (verified: the Claude Code bash sandbox).

## Key Takeaways

- **E2E marker convention**: `#[ignore]` now means ONLY "needs external service", reason standardized to `e2e: ` prefix for grep-auditability. Files that are entirely e2e keep `*_e2e.rs` naming; mixed files keep `*_integration.rs` with per-test ignore. 22 e2e tests across 13 files/9 crates.
- **Env guards in every e2e test**: check env vars (non-empty) or TCP-probe required services (TDengine :6041, model service :31693, SSH host :2222, control-plane) and `return` with a `SKIP (e2e): ...` message. Guards tolerate empty env values — GH Actions sets unconfigured secrets to `""`.
- **`LocalSandbox` timeout kill rework**: replaced `kill -TERM -pgid` + fixed 5s sleep + `kill -KILL -pgid` with `pkill -TERM -P <pid>` (descendants) + `kill -TERM <pid>` (direct child) + 2s grace poll + KILL escalation. Root cause of the bash-timeout failures: sandboxes kill the caller's tree when a group signal is actually delivered (reproduced with a minimal python script in the Claude Code sandbox — exit 144, whole script killed).
- **MockLlmClient gained `set_stream_event_queue`**: per-call stream scripts (VecDeque), first call pops first script, exhausted queue → empty stream. Added to `vol-llm-core/src/test_utils.rs` with a unit test.
- **wasmtime 22 exit-code test**: the bug was NOT the I32Exit import (still at crate root in wasmtime-wasi 22.0.1). The WAT module lacked a `memory` export — the wiggle shim bails with "missing required memory export" before calling any host function. Fixed the WAT (`(memory (export "memory") 1)`) and deduplicated the inline WAT copy in the test (the test used its own inline copy, not the shared `EXIT42_WAT` constant — why the fix initially looked ineffective).
- **`react_mock_test` slow test**: was ignored because the loop mock triggered real TDengine tool calls (~2min connection timeouts each). Rewrote with a local no-op `LoopIndexPriceTool` (ExecutableTool) — no network, runs in the integration tier.
- **`code_agent_simulation` brittle test**: rewritten with MockLlmClient + scripted stream events — no query-text parsing (the old simulator matched "What is the current BTC price?" to the volatility branch).
- **runtime inline-ignored test**: un-ignored with a `~/.mcp.json` presence guard — the test's assertions are hermetic, but a user-level MCP config makes the build slow (16s) and noisy; CI (no user config) runs it, dev machines with user MCP config skip fast.
- **`jsonrpc_e2e_test.rs` renamed to `jsonrpc_operation_coverage_test.rs`** — it was an in-process handler-dispatch coverage test, not e2e (name now matches tier).
- **Frontend**: `npm run test:e2e` (playwright, mock backend, self-contained) added to `package.json`, `fe-e2e` just recipe, Playwright step in quality.yml PR gate, plus manual `e2e.yml` job.

## Detailed Summary

### E2E inventory (all `#[ignore = "e2e: ..."]`, all with guards)

| Crate | File | Tests | Requires |
|---|---|---|---|
| vol-llm-agents | coding_e2e_test.rs | 3 | ANTHROPIC_AUTH_TOKEN |
| vol-llm-agents | e2e_log_counter_cli.rs | 2 | ANTHROPIC_AUTH_TOKEN |
| vol-llm-agents | coding_deribit_ws_e2e.rs | 2 | ANTHROPIC_AUTH_TOKEN / DERIBIT_WS_CLIENT_DIR |
| vol-llm-agents | coding_web_tools_integration.rs | 2 | ANTHROPIC_AUTH_TOKEN |
| vol-llm-agents | observer_integration.rs | 1 | ANTHROPIC_AUTH_TOKEN |
| vol-llm-agents | ppt_agent_integration.rs | 1 | ANTHROPIC_AUTH_TOKEN |
| vol-llm-agents | advice_agent_integration.rs | 1 | ANTHROPIC_AUTH_TOKEN + TDengine + FEISHU_* |
| vol-llm-agent | agent_llm_integration.rs | 2 | ANTHROPIC_AUTH_TOKEN |
| vol-llm-agent | agent_alert_scenario.rs | 1 | ANTHROPIC_AUTH_TOKEN |
| vol-llm-sandbox | ssh_integration.rs | 3 | Docker SSH host :2222 |
| vol-llm-task | task_cli_llm_e2e.rs | 2 | model service 192.168.2.162:31693 |
| vol-llm-wiki | wiki_integration_test.rs | 1 | ANTHROPIC_AUTH_TOKEN + real session file |
| vol-agent-server | agent_run_client.rs | 1 | running control-plane |

### Fixed broken/ignored tests (removed from the e2e tier)

| Test | Problem | Fix |
|---|---|---|
| wasm_sandbox test_execute_nonzero_exit | "missing required memory export" — WAT module missing memory export | added `(memory (export "memory") 1)`; un-ignored |
| code_agent_simulation test_code_agent_market_data_query | brittle query-text parsing mock | rewritten with MockLlmClient scripted events; un-ignored |
| runtime register_agent_with_mcps_resolves_filtered_tools | environment-sensitive (user MCP config) | `~/.mcp.json` guard; un-ignored |
| bash_tool_test test_bash_timeout / test_bash_timeout_kills_process | racy short sleeps + group-kill escalation (sandbox kills caller tree) | `sleep 30`/`sleep 60` + positive-pid kill rework in LocalSandbox; anchored pkill/pgrep patterns |
| agent_alert_scenario test_agent_alert_scenario | mock's `converse_stream` was `unimplemented!()` — would panic on any run with a valid token | `converse_stream` implemented as a replay of the scripted `converse()` response (tool-call + content stream events) |
| frontend Playwright suite (14 tests) | stale locators vs the post-shadcn-migration UI: Nodes(N) button, tabs as buttons, DebugPanel close button, FileTree color classes | locators updated to the current UI (NodesDropdown auto-select trigger, `role=tab`, Radix dialog Close, `text-foreground/80`); 14/14 green |

### CI

- `quality.yml` quality-frontend job: + Playwright install (`--with-deps chromium`) + `npm run test:e2e` (PR gate).
- New `e2e.yml` (workflow_dispatch, optional `crate` input): `e2e-rust` runs `just test-e2e` / `just test-e2e-crate <crate>` with secrets → env (guards handle empties); `e2e-frontend` = manual Playwright fallback.

### Verification

- `cargo test -p vol-llm-tools-builtin-bash --test bash_tool_test` — 11/11 in 0.88s (was: permanent hang).
- `cargo test -p vol-llm-sandbox --lib` — 42/42.
- `cargo test -p vol-llm-sandbox --features wasm --test wasm_sandbox` — 18/18, 0 ignored.
- `cargo test -p vol-llm-agent --test code_agent_simulation` — 5/5, 0 ignored.
- `cargo test -p vol-llm-core --features test-utils test_mock` — 6/6 (incl. new queue test).
- Runtime guard verified: `~/.mcp.json` present → skip in 0.00s (was 16s).
- Unit tier (10 changed crates): 796/796 passed.
- Integration tier (`-E 'kind(test)'`): **399/399 passed, 22 skipped, 0 failed** (was 396 passed / 2 failed / 24 skipped before this work).
- E2E tier (`--ignored`): with ANTHROPIC_AUTH_TOKEN unset, all tests clean-skip (verified per-test `SKIP (e2e): ...` messages). With a stale token the LLM tests fail loudly with 401 (correct e2e semantics — a configured-but-invalid credential must not silently skip). `agent_run_client` runs against whatever is on :3001 — a bare dev server without test agents fails its assertions (dedicated e2e control-plane needed).
- Frontend: vitest 140/140; Playwright e2e suite was stale (written pre-shadcn-migration: Nodes(N) button, tabs as buttons, DebugPanel close button, FileTree color classes) — fixed locators against the current UI (NodesDropdown auto-select, role=tab, Radix dialog close, `text-foreground/80`), now **14/14**.
- Coverage gates: `just cover-gate vol-llm-sandbox 80` PASS 88.67%; `just cover-gate vol-llm-core 80` PASS 96.89%.
- fmt clean; clippy (workspace + strict -D warnings) clean; no-doc-tests ✓; boundary check ✓; no-clippy-allow ✓; justfile parses; both workflows valid YAML.

## Entities Mentioned

- [[vol-llm-sandbox-crate]]: LocalSandbox timeout kill rework (positive-pid only).
- [[vol-llm-core-crate]]: MockLlmClient `set_stream_event_queue`.
- [[vol-repository]]: justfile recipes `test-e2e-crate`, `fe-e2e`; CI workflows.

## Concepts Covered

- [[test-tiers]]: e2e tier now landed: marker convention, guards, workflows.
- [[coverage-gate-work]]: unchanged (CI-only llvm-cov gate).

## Notes

- Process-group kills are a landmine in sandboxed environments: `kill -TERM -<pgid>` that actually delivers a signal kills the caller's whole process tree (reproduced: minimal python `setpgid` + `/usr/bin/kill -TERM -pid` → exit 144, zero output). Any future code touching process signals must use positive pids.
- The sandbox's `process_group(0)` on the child is retained (harmless, useful for job control outside sandboxes), but the kill path no longer depends on it.
- E2E tests with a real LLM key still need a network path to the provider; `HTTPS_PROXY` is passed through in e2e.yml.
- `docs/superpowers/specs/2026-08-08-git-hooks-quality-gates.md` remains historical; the tiering docs are [[test-tiering-hooks]] (pipeline) and this source (completion).
