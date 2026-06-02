# Grep Tool Multi-Backend Redesign

## Overview

Rewrite the built-in `grep` tool with a strategy pattern supporting two backends:
1. **RgCliBackend** — shell out to `rg` (ripgrep) binary when available (fast, full-featured)
2. **RustLibBackend** — pure-Rust fallback using `ignore` + `grep-searcher` + `grep-regex` crates

## Motivation

- Current pure-Rust implementation (`grep` crate + `walkdir`) is slow on large repos and doesn't respect `.gitignore`
- `rg` binary provides best-in-class performance, automatic `.gitignore` support, and feature completeness
- Rust library fallback ensures the tool works in environments without `rg` (minimal Docker images, etc.)
- Two-layer design is simple, maintainable, and covers all deployment scenarios

## Architecture

```
GrepTool (ExecutableTool impl)
  │
  │  on first call: detect rg → cache bool
  │
  ├── has_rg ──► RgCliBackend::search()
  │              (std::process::Command, 30s timeout)
  │
  └── no rg ───► RustLibBackend::search()
                 (ignore::Walk + grep-searcher, tokio::spawn_blocking)
```

## Dependencies

### Added
- `ignore = "0.4"` — .gitignore-aware directory walking (replaces `walkdir`)
- `grep-regex = "0.1"` — regex matcher for the `grep-searcher` crate

### Kept
- `grep-matcher = "0.1"` — already present
- `grep-searcher = "0.1"` — already present
- `tokio` — for timeout and spawn_blocking
- `regex` — for glob-to-regex conversion

### Removed
- `walkdir = "2.4"` — replaced by `ignore`

## Interface

```rust
#[async_trait]
trait GrepBackend: Send + Sync {
    fn is_available() -> bool;
    async fn search(params: &GrepParams, root: &Path) -> Result<Vec<SearchResult>, String>;
}

struct RgCliBackend;    // wraps rg binary
struct RustLibBackend;  // wraps ignore + grep-searcher
```

## Backend Details

### RgCliBackend

- Detection: `which rg` on construction, cached as `AtomicBool`
- Execution: `std::process::Command::new("rg")` with flags built from params
- Timeout: `tokio::time::timeout(30s)` + kill process group on timeout
- Flags:
  - `--no-heading --with-filename --color never`
  - `--max-depth 50 --max-filesize 10M`
  - `-l` (files_with_matches) / `-c` (count) / `-n` (content)
  - `-s` (case_sensitive) / `-i` (case_insensitive, default)
  - `-g <glob>` (file pattern filter)
  - `--` then the pattern, then search path

### RustLibBackend

- Walking: `ignore::WalkBuilder::new(root).max_depth(50).hidden(false)` — auto-respects .gitignore
- Regex: `grep_regex::RegexMatcher::new(pattern)` with `(?i)` prefix if case_insensitive
- Search: `grep_searcher::Searcher::new().search_path(&matcher, path, sink)` — parallel, skips binary
- Output: maps to `SearchResult { path, match_count, line_numbers }`
- Runs inside `tokio::task::spawn_blocking` to avoid blocking the async runtime

## CLI Flag Mapping

| Param | rg flag | RustLib |
|-------|---------|---------|
| `pattern` | `-- <pattern>` | `RegexMatcher::new(pattern)` |
| `path` | positional arg | `WalkBuilder::new(path)` |
| `glob` | `-g <glob>` | `WalkBuilder.types(glob)` |
| `output_mode: files_with_matches` | `-l` | return only paths |
| `output_mode: count` | `-c` | count matches per file |
| `output_mode: content` | `-n` | path:line_number per match |
| `case_sensitive: false` | `-i` | `(?i)` prefix |
| `case_sensitive: true` | `-s` | exact match |

## Behavior

- **rg present**: tool starts instantly, no warm-up. `rg` handles .gitignore, binary skip, parallelism, and multi-line matching natively
- **rg absent**: fallback uses `ignore` crate for .gitignore-aware walking and `grep-searcher` for parallel search (~80% of rg speed)
- **Timeout**: both backends enforce 30-second timeout via `tokio::time::timeout`
- **Output format**: identical between backends — caller cannot distinguish which backend was used

## Files Changed

| File | Change |
|------|--------|
| `crates/vol-llm-tools-builtin/grep-tool/Cargo.toml` | add `ignore`, `grep-regex`; remove `walkdir` |
| `crates/vol-llm-tools-builtin/grep-tool/src/lib.rs` | full rewrite: `GrepBackend` trait + two impls |

## Testing

- Unit tests: verify parameter→flag mapping, glob-to-regex, backend selection
- Integration tests: all existing tests pass with both backends (mock rg binary for CLI tests)
- Manual: test on current environment (rg available) and minimal Docker (rg absent)
