---
type: source
source_type: code
date: 2026-08-19
ingested: 2026-08-19
tags: [observability, logging, otel, file-appender, backend]
---

# Agent Log File Location Fix: `logs/` Instead of CWD

**Authors/Creators:** BestNathan / Claude
**Date:** 2026-08-19
**Link:** `crates/vol-llm-observability/src/otel_init.rs`

## TL;DR

The OTel file appender wrote hourly `agent.YYYY-MM-DD-HH.log` files into the process CWD (`.build(".")`), so dev servers launched from the repo root littered the root directory with log files (up to 168 of them). Extracted `build_agent_file_appender()` and pointed it at `logs/` (created on demand, already gitignored); the `/tmp` fallback path is unchanged.

## Key Takeaways

- Root cause was `RollingFileAppender::builder()...build(".")` in `otel_init.rs` — CWD-relative, not configurable (vol-agent-server config has no log-dir option).
- `build_agent_file_appender()` keeps the same settings: `Rotation::HOURLY`, prefix `agent`, suffix `log`, `max_log_files(168)`; primary dir `logs`, fallback `/tmp`.
- Behavior test (TDD): writes a unique marker through the appender via `std::io::Write`, asserts it lands in `logs/agent*.log` and does NOT appear in any agent log in the CWD.
- Gates: fmt / clippy / no-doc-tests / boundaries clean; 58/58 unit tests; line coverage 88.24% (≥ 80%).

## Detailed Summary

`init()` in `otel_init.rs` builds the tracing file layer from the rolling appender; the appender construction was inlined with `build(".")`. Dev backends (`just web-backend` = `cargo watch -x "run -p vol-agent-server"`) run from the repo root, so logs landed at `/root/vol/agent.*.log`. The fix moves the primary directory to `logs`; note the running dev backend (started 2026-08-07, not under cargo watch) keeps writing the old root file until restarted.

## Entities Mentioned

- [[vol-llm-observability-crate]]: `otel_init.rs` appender directory fix

## Concepts Covered

- [[agent-observability]]: file appender placement within the OTel init stack

## Notes

- The test creates `crates/vol-llm-observability/logs/agent.*.log` during runs — gitignored (`logs/` pattern).
- `logs/` root directory had been deleted as junk in the same cleanup; the fix recreates it automatically.
