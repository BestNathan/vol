---
type: source
source_type: code
date: 2026-06-01
ingested: 2026-06-01
tags: [grep, search, backend, rust-library, ignore, grep-searcher]
---

# RustLibBackend — Pure-Rust Grep Fallback

**Authors/Creators:** Nathan
**Date:** 2026-06-01
**Link:** `crates/vol-llm-tools-builtin/grep-tool/src/lib_impl.rs`

## TL;DR
Added `RustLibBackend`, a pure-Rust library-based grep backend using the `ignore` crate for .gitignore-aware directory walking and `grep-searcher` for parallel regex search. This backend is the universal fallback when the `rg` (ripgrep) CLI binary is unavailable, completing the multi-backend grep tool architecture.

## Key Takeaways
- `RustLibBackend` is the second backend in the Strategy-pattern grep tool design, alongside `RgCliBackend`
- Uses `ignore::WalkBuilder` for .gitignore-aware directory traversal (depth limit 50)
- Uses `grep_regex::RegexMatcher` for pattern compilation with case-insensitive support via `(?i)` prefix
- Uses `grep_searcher::Searcher` for per-file parallel search with built-in binary detection
- Custom `MatchCollector` implements the `grep_searcher::Sink` trait to collect line numbers and match counts
- Glob filtering via `ignore::types::TypesBuilder` (primary) and fallback regex-based path matching (secondary)
- `is_available()` always returns `true` — this is the universal fallback backend
- CPU-bound search work runs on `tokio::task::spawn_blocking`
- `Arc<Mutex<Vec>>` for thread-safe result collection across walker iterations

## Detailed Summary

The `RustLibBackend` provides the same `GrepBackend` trait interface as `RgCliBackend`, enabling seamless fallback. Key implementation decisions:

1. **Walker**: `ignore::WalkBuilder` with `max_depth(50)`, `hidden(false)`, `git_ignore(true)` — respects .gitignore and includes hidden files
2. **Glob filtering**: Two-layer approach — primary via `ignore::types::TypesBuilder` for early walker-level filtering, secondary via `glob_to_regex()` for path-name/regex matching
3. **Glob-to-regex conversion**: `glob_to_regex()` handles `*.ext`, `**/*.rs`, `prefix*`, `*suffix` patterns by escaping with `regex::escape` and replacing glob wildcards
4. **Match collection**: Custom `MatchCollector` struct implementing `grep_searcher::Sink` — collects `line_numbers` vector and `count` for each matching file
5. **Output**: Returns `Vec<SearchResult>` with path, match_count, and line_numbers — the same format consumed by `format_results()` in `lib.rs`

## Entities Mentioned
- [[vol-llm-tool-crate]]: Tool definition and registry framework using ExecutableTool trait
- [[nq-deribit-repository]]: Parent repository containing the grep-tool crate

## Concepts Covered
- [[tool-registry]]: GrepTool registered as a built-in tool with multi-backend strategy

## Notes
- 5 unit tests in `lib_impl::tests` cover: basic search, no matches, glob filter, count mode, content mode
- All 17 tests pass (8 unit + 9 integration) across both backends
- The `output_mode` parameter is passed through for API consistency but the library backend always collects all data (mode-specific formatting happens in `format_results()`)
