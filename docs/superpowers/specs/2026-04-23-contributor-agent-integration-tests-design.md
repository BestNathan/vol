# Contributor and ReActAgent Integration Tests Design

> Status: approved — 2026-04-23

## Problem

After adding SessionContributor compression, we need tests to verify the integration chain works correctly: ContextBuilder → SessionContributor → compression → re-contribute. Currently only basic unit tests exist for init_messages and a minimal compression flow test. Missing are full ReActAgent.run() integration tests and compressor strategy comparison tests.

## Solution

Two test files with 6-8 tests total, using MockLlmClient (no real LLM calls).

## Test File 1: `react_agent_integration_test.rs`

**Purpose:** Full ReActAgent.run() integration verifying ContextBuilder + SessionContributor + compression mid-run.

**Scenarios:**

1. **Basic run** — MockLlm answers in 1 iteration, SessionContributor returns empty history (new session), ContextBuilder produces system + user input, agent succeeds.
2. **Compression mid-run** — Pre-populate session with many messages, MockLlm triggers ContextBuilder over budget → compression fires → agent continues with reduced context.
3. **Empty session start** — Brand new session, no contributors needed beyond system prompt, agent succeeds.

## Test File 2: `compression_strategies_test.rs`

**Purpose:** Compressor strategy comparison with realistic conversation patterns.

**Scenarios:**

1. **PositionSampleCompressor** — 20-message conversation (user/tool/assistant mixed), verify first N + sampled + last survive.
2. **RoleFilterCompressor** — Same 20-message conversation, only User+Assistant survive, Tool messages filtered.
3. **Multiple compression cycles** — Compress → add more messages → compress again → verify both cycles work.
4. **Edge cases** — Empty input, single message input.

## Approach

- Use `MockLlmClient` from `vol-llm-core/test-utils`
- Each test is independent (no shared state)
- No real LLM calls
- Tests use `AgentBuilder` to construct ReActAgent with controlled inputs
- Mock LLM responds with predetermined answers in N iterations
