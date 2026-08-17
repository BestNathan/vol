---
type: source
source_type: code
date: 2026-08-17
ingested: 2026-08-17
tags: [crate, provider, openai, bugfix, secret, streaming, tool-call, tdd]
---

# vol-llm-provider Production Bugfixes (TDD)

**Authors/Creators:** vol-llm team
**Date:** 2026-08-17
**Link:** commits `72277b0a`, `9fa770f0`, `6f56f327`, `de04fe83` on `main`

## TL;DR

Four production bugs in `vol-llm-provider` were found during the coverage-raising work and fixed with TDD (RED → GREEN, one commit per bug): raw string tool-call arguments in OpenAI non-streaming converse, `request.system` silently dropped by the OpenAI provider, `Secret` JSON serde asymmetry breaking `LLMConfig` round-trips, and streamed OpenAI tool calls never emitting `ToolCallComplete`. All changes confined to `crates/vol-llm-provider/`; gate re-verified at 95.41% with 120 tests passing.

## Key Takeaways

- **Bug 1 (`72277b0a`)** — non-streaming OpenAI converse mapped the wire `function.arguments` value through `ToString::to_string`, so a plain JSON string of arguments arrived double-quoted and escaped (`"{\"city\":\"Beijing\"}"`) instead of raw (`{"city":"Beijing"}`). Fix: `parse_tool_arguments()` passes `Value::String` through as content, serializes other JSON values normally. Anthropic path already correct (serializes the `input` object).
- **Bug 2 (`9fa770f0`)** — `OpenaiProvider::converse`/`converse_stream` converted only `request.messages`; `ConversationRequest.system` never reached the model. Fix: `apply_system_prompt()` embeds it as the first `messages[0]` with role `"system"` (OpenAI wire convention), exactly once, without duplicating a caller-provided leading system message. No top-level `system` key (that is the Anthropic convention).
- **Bug 3 (`6f56f327`)** — `Secret` serialized as a tagged enum (`{"Literal":...}` / `{"Env":...}`) but deserialized only from a plain string, so `LLMConfig` JSON round-trips failed. Fix: a `SecretVisitor` accepts both the tagged form (kind-preserving) and the plain-string back-compat form including the `${VAR}` / `${VAR:default}` env-reference pattern; TOML config path untouched; `PartialEq`/`Eq` derived for round-trip equality tests.
- **Bug 4 (`de04fe83`)** — OpenAI SSE has no per-block stop marker for tool calls, and `StreamingSession::finalize()` flushes only content/thinking buffers, so a streamed tool call was started and fed argument deltas but never completed. Fix (confined to the provider): the `converse_stream` task flushes the pending tool call via `session.apply(&ParsedEvent::ContentBlockStop)` before `finalize()` — the same primitive the Anthropic `content_block_stop` path uses — emitting `ToolCallComplete` exactly once; a no-op when no tool call is pending.
- Completion arrives at stream end (after `ResponseComplete`), which is safe for the ReAct consumer (`vol-llm-agent/src/react/agent.rs` collects `ToolCallComplete` events for use post-stream).

## Detailed Summary

The four bugs were uncovered while raising `vol-llm-provider` coverage to ≥80% ([[coverage-gate-work]]): the coverage work pinned several behaviors with tests, and three of those pins (raw tool-call args, dropped `request.system`, streamed tool-call completion) exposed real bugs, plus the `Secret` serde asymmetry found by JSON round-trip testing.

Per-bug detail from the TDD report (`/root/vol/.superpowers/sdd-cover/provider-bugfix-report.md`):

- Bug 1 RED: `assertion failed: left: "\"{\\\"city\\\":\\\"Beijing\\\"}\"" right: "{\"city\":\"Beijing\"}"`; fix at `src/openai.rs:184-189` (`parse_tool_arguments`), used at the parser (`src/openai.rs:398`).
- Bug 2 RED: wire carried only the user message; fix `apply_system_prompt` (`src/openai.rs:197-209`) applied in both `converse` (`:237`) and `converse_stream` (`:466`).
- Bug 3 RED: `invalid type: map, expected a string`; `SecretVisitor` at `src/secret.rs:36-46, 83-125`, `secret_from_env_pattern` at `:59-81`, `EnvSecretWire` at `:51-55`; 5 new secret tests + real `LLMConfig` round-trip in `config.rs` (removed the `json.replace(...)` workaround).
- Bug 4 RED: `tool_call_completes == 0`; flush via `session.apply(&ParsedEvent::ContentBlockStop)` at `src/openai.rs:646-659`; event type `StreamEventData::ToolCallComplete { tool_call }` in `vol-llm-core/src/stream.rs:42-44`.

Final verification: `cargo test -p vol-llm-provider` → 94 lib + 20 converse_integration + 3 proxy_env + 3 stream_integration = **120 tests, 0 failures**; `just cover-gate vol-llm-provider 80` → **PASS 95.41%**; no doc tests; rustfmt + clippy-strict green.

Deviations from the brief: (1) coverage gate initially failed at 79.11% due to stale instrumented artifacts in the shared `target/llvm-cov-target` — cleaned, gate stable at 95.41% (recommendation: clean `target/llvm-cov-target` before coverage gating); (2) `StreamingSession::finalize()` not modified (lives in vol-llm-core, out of scope) — the provider flushes via the public `session.apply` primitive instead; (3) `PartialEq`/`Eq` derive on `Secret` is additive, no behavioral change; (4) test `openai_converse_drops_request_system_field` renamed/rewritten to pin correct behavior.

## Entities Mentioned

- [[vol-llm-provider-crate]]: all four fixes in `src/openai.rs`, `src/secret.rs`, `src/config.rs`; integration tests in `tests/converse_integration.rs`
- [[vol-llm-core-crate]]: `StreamEventData::ToolCallComplete` and `ParsedEvent::ContentBlockStop` primitives used by the fix; `StreamingSession` not modified

## Concepts Covered

- [[streaming-session]]: stream-end tool-call flush pattern for providers whose SSE lacks per-block stop markers; ToolCallComplete emitted after ResponseComplete is safe for post-stream consumers

## Notes

- Known follow-up: multi-tool-call streams in vol-llm-core `StreamingSession` only complete the last-started call — `current_tool_call: Option<ToolCallBuilder>` is a single slot, so a new `ToolCallStart` replaces the previous builder before completion. Pre-existing limitation, now surfaced; single-call streams work correctly.
- Future coverage runs should `rm -rf target/llvm-cov-target` before gating.
