---
type: source
source_type: report
date: 2026-08-17
ingested: 2026-08-17
tags: [coverage, testing, vol-llm-core, vol-llm-context, vol-llm-provider]
---

# Per-Crate Coverage Gate Work (≥80%)

**Authors/Creators:** vol-llm team
**Date:** 2026-08-17
**Link:** commits `f6edf792`, `c0018d89`, `3846cee7`, `9600911d` on `main`

## TL;DR

Test-only work raising per-crate line/region coverage to ≥80% for `vol-llm-context`, `vol-llm-core`, and `vol-llm-provider`. No production code changed. The provider coverage work doubled as bug discovery: it surfaced the four production bugs fixed in [[provider-bugfixes]].

## Key Takeaways

- **`vol-llm-context` (`f6edf792`)**: 65.19% → **88.94%** regions (90.08% lines). +19 tests covering previously dead paths: `replace_contributor`, `token_budget`, `contributor_names`/`contributor_infos` (all zones + errors), `snapshot_by_name` (all roles, None content, not-found, error propagation), `build()` compress path, middle-zone budget drop loop, `Clone` impls, `add_contributors_from`, plus first-ever full test modules for `builtin::simple` (was 0%), `FileContributor`, `UserInputContributor`, and the `ContextContributor` test-double.
- **`vol-llm-core` (`c0018d89`)**: 62.93% → **95.62%** regions (gate reads the region column while labeling it "line coverage"). +7 test modules: `agent_def`, `conversation`, `message`, `model`, `provider`, `stream`, `streaming` — 1241 insertions.
- **`vol-llm-provider` (`3846cee7` + `9600911d`)**: 60.81% region / 63.31% line → **85.79%** pre-bugfix; +40 unit tests and +20 integration tests (`tests/converse_integration.rs`, `tests/proxy_env.rs`). `9600911d` pinned `request.system` wire behavior and fixed a mock `Content-Length` off-by-one in the invalid-UTF-8 streaming tests.
- The `cover-gate` justfile recipe parses column $4 of the llvm-cov TOTAL row, which is **region** coverage printed with a "line coverage" label; all three crates exceed 80% under either metric.
- `openai.rs` was the weakest file (64.82% region / 66.22% line); investigation showed a large share of its "uncovered" lines have no instrumentation regions at all (rustc async-fn lowering + dual `cfg(test)` lib builds), so its reported numbers are a conservative lower bound — the lines are demonstrably executed by integration tests.

## Detailed Summary

The coverage work followed the SDD cover workflow with per-crate reports in `/root/vol/.superpowers/sdd-cover/` (`vol-llm-context-report.md`, `vol-llm-core-report.md`, `vol-llm-provider-report.md`, `provider-bugfix-report.md`). Coverage was measured with llvm-cov via `just cover-gate <crate> 80`. All commits are test-only (`#[cfg(test)]` modules and `tests/` integration tests); no production behavior changed, and no doc tests were added. The provider integration suite grew to exercise both Anthropic and OpenAI converse paths against a local mock server, which is what exposed the four production bugs later fixed in [[provider-bugfixes]] (final gate: 95.41%, 120 tests passing).

## Entities Mentioned

- [[vol-llm-context-crate]]: coverage raised to 88.94% regions / 90.08% lines, gate PASS
- [[vol-llm-core-crate]]: coverage raised to 95.62% regions, gate PASS
- [[vol-llm-provider-crate]]: coverage raised to 85.79% (pre-bugfix), later 95.41% after bugfixes

## Concepts Covered

- [[context-builder]]: previously 0%-covered `builtin::simple` and contributor paths now fully tested
- [[streaming-session]]: core streaming state machine coverage raised to 95.62%; provider streaming tests now pin ToolCallComplete behavior (see [[provider-bugfixes]])

## Notes

- No production code was changed by the coverage commits themselves.
- Known follow-up from the provider suite: multi-tool-call streams in `StreamingSession` only complete the last-started call (see [[provider-bugfixes]] Notes).
