# Design: Skip Streaming Delta Events in LoggerPlugin

## Problem

The `LoggerPlugin` in `vol-llm-observability` logs every `AgentStreamEvent` variant, including high-frequency streaming delta events (`ThinkingDelta`, `ContentDelta`, `ToolCallArgumentDelta`). Each fires once per streaming token/chunk, generating hundreds of log lines per LLM response. This bloats log files without adding diagnostic value — the `Complete` variants already capture the full accumulated content.

The `SessionListener` already filters out deltas correctly.

## Decision

Skip all three delta event types in `LoggerPlugin.listen()`. The `Complete` variants (`ThinkingComplete`, `ContentComplete`, `ToolCallBegin`/`ToolCallComplete`) provide the full content needed for debugging and analysis.

## Implementation

- Add `should_log()` method that returns `false` for the three delta variants
- Early-return in `listen()` when `should_log()` is `false`
- Keep the delta arms in `create_log_entry()` and `event_name()` as `unreachable!()` branches (required for exhaustive pattern matching since `AgentStreamEvent` is non-exhaustive)
- Update tests to verify `should_log()` behavior

## Trade-offs

| Aspect | Before | After |
|--------|--------|-------|
| Log volume | Hundreds of lines per LLM response (one per token) | One line per complete content segment |
| Debuggability | Can trace incremental streaming | Only complete content visible (already sufficient for debugging) |
| TUI compatibility | Deltas still render in TUI (unaffected) | Unchanged — TUI uses its own `EventBuffer` |
