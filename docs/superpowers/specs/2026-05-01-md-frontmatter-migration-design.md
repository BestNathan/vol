# Design: Migrate vol-llm-skill and vol-llm-wiki to md-frontmatter

## Overview

Replace ad-hoc frontmatter parsing in `vol-llm-skill` and `vol-llm-wiki` with the generic `md-frontmatter` crate. Each crate gets its own migration pass, starting with `vol-llm-skill`.

**Core principle:** Delete custom parsing code, use `md-frontmatter::parse<T>()` directly. On parse error, log a warning and skip the file (strict mode — no graceful fallback to defaults).

## Architecture Changes

### vol-llm-skill (Task 1)

| Action | File | Detail |
|--------|------|--------|
| Delete | `src/parser.rs` | All custom parsing removed |
| Modify | `src/loader.rs` | Use `md_frontmatter::parse::<SkillFrontmatter>` inline |
| Modify | `src/lib.rs` | Remove `mod parser` and `pub use parser::` |
| Modify | `Cargo.toml` | Add `md-frontmatter`, remove `serde_yaml` if unused |

**Key behavior change:** `parse_skill_content` previously returned defaults on missing/invalid frontmatter. New code: `md_frontmatter::parse<T>` returns `Err`, caller logs warning and `continue`s to skip the file.

**`scan_skill_files` moves inline to `loader.rs`** since it's only called from one place and `parser.rs` is being deleted. The function itself is unchanged — it still returns ALL files (not just .md) for the LLM's file listing.

### vol-llm-wiki (Task 2)

| Action | File | Detail |
|--------|------|--------|
| Modify | `src/loader.rs` | Replace `walk_dir` + `parse_frontmatter_value` with `md_frontmatter::scan_dir::<WikiFrontmatter>` |
| Modify | `Cargo.toml` | Add `md-frontmatter` |

**Key simplification:** `extract_title`, `extract_tags`, and `parse_frontmatter_value` (15 lines of manual string parsing) are replaced by one `scan_dir` call that returns `ParsedDoc<WikiFrontmatter>` with typed `title` and `tags` fields.

## Frontmatter Types

### SkillFrontmatter (vol-llm-skill)

```rust
#[derive(Deserialize)]
struct SkillFrontmatter {
    name: String,
    #[serde(default = "default_version")]
    version: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    triggers: Vec<String>,
}
```

### WikiFrontmatter (vol-llm-wiki)

```rust
#[derive(Deserialize, Clone)]
struct WikiFrontmatter {
    title: String,
    #[serde(default)]
    tags: Vec<String>,
}
```

## Error Handling

| Scenario | vol-llm-skill | vol-llm-wiki |
|----------|---------------|--------------|
| Missing frontmatter | `tracing::warn!`, skip file | File not in scan results (scan_dir returns per-file errors) |
| Invalid YAML | `tracing::warn!`, skip file | Error collected in scan_dir errors vec, logged |
| IO error (unreadable) | `tracing::warn!`, skip file | Error collected in scan_dir errors vec, logged |

## Migration Order

1. **vol-llm-skill** — delete parser.rs, rewrite loader.rs, verify tests
2. **vol-llm-wiki** — rewrite loader.rs walk_dir + title/tag extraction, verify tests

Each migration is independently testable. If vol-llm-skill passes all tests, vol-llm-wiki can be done in a follow-up.

## Testing Strategy

- Existing loader integration tests should pass (they create valid files with frontmatter)
- Removed parser.rs tests are covered by md-frontmatter crate tests
- `cargo test -p vol-llm-skill` and `cargo test -p vol-llm-wiki` should pass
- `cargo clippy --workspace -- -D warnings` should pass
