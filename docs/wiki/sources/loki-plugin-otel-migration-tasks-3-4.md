---
type: source
category: implementation
tags: [otel, migration, tracing, loki, agent-observability]
created: 2026-05-06
updated: 2026-05-06
---

# LokiPlugin OTel Migration — Tasks 3+4

## Summary

Tasks 3 and 4 of the LokiPlugin to OTel SDK migration design spec. Task 3 rewrites `LokiPlugin` to use `tracing::info!` with structured fields instead of HTTP POST to Loki. Task 4 adds a `model` field to `RunContext` so the plugin can include the LLM model name in each log entry.

## What Changed

### LokiPlugin (`crates/vol-llm-observability/src/loki/plugin.rs`)

- **Stateless struct**: `LokiPlugin` now has no fields (previously held `Arc<LokiWriter>`).
- **`new()` signature**: Takes no arguments (previously took `LokiConfig`).
- **`listen()`**: Calls `tracing::info!` with structured fields (`namespace`, `session_id`, `agent_id`, `agent_type`, `run_id`, `model`, `event`).
- **`create_event_json()`**: Renamed from `create_loki_entry()` — returns flat JSON string instead of `LokiEntry`.
- **`should_send()`**: Unchanged — skips `ThinkingDelta`, `ContentDelta`, `ToolCallArgumentDelta`.
- **Removed dependencies**: `LokiEntry`, `LokiWriter`, `LokiConfig`, `LokiLabels` (deleted in Task 2).

### RunContext (`crates/vol-llm-agent/src/react/run_context.rs`)

- **New field**: `pub model: String` added after `session_id`.
- **`new()` signature**: Gains `model: String` as 8th parameter. Empty string normalized to `"unknown"`.
- **Clone impl**: Copies `model` field.
- **All call sites updated**: 12+ test helpers across 9 files, plus the production call in `agent.rs` (uses `config.llm.model()`).

### Agent (`crates/vol-llm-agent/src/react/agent.rs`)

- Passes `config.llm.model().to_string()` to `RunContext::new()`.

### Dev-dependency fix

- Added `tempfile = "3"` to `vol-llm-observability` `[dev-dependencies]` (pre-existing test compilation issue).

## Verification

- `cargo check -p vol-llm-observability -p vol-llm-agent` — clean
- `cargo test -p vol-llm-observability --lib` — 20 tests pass
- `cargo test -p vol-llm-agent --lib` — 141 tests pass

## Design Spec

- [[loki-plugin-otel-migration-design]] — full design specification
- [[loki-raw-event-serialization-design]] — event serialization format
