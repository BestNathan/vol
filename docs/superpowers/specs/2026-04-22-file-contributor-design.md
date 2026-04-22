# FileContributor Design Spec

## Overview

Replace `RoleContributor`, `RulesContributor`, and `TaskContributor` with a single `FileContributor` that reads markdown files from disk. Each file produces one `ContextBlock` with an explicitly configured `AttentionAnchor`.

## Problem

Three separate contributor types (`RoleContributor`, `RulesContributor`, `TaskContributor`) each do the same thing: hold a string, wrap it in a `Message`, assign an anchor. Adding a new source type means adding another contributor file. This is boilerplate that scales poorly.

## Solution

A single `FileContributor` that takes a list of `(path, anchor)` pairs, reads each file, and returns one `ContextBlock` per successful read. Files that don't exist are silently skipped.

## Architecture

```
FileContributor {
    specs: Vec<FileSpec>
}

FileSpec {
    path: String          // relative or absolute path to .md file
    anchor: AttentionAnchor
}
```

### contribute()

Iterates over specs. For each `(path, anchor)`:
1. `std::fs::read_to_string(path)`
2. If exists → `Message::system(content)` → `ContextBlock { messages: [msg], anchor }`
3. If not exists → `tracing::warn!` → skip
4. Return all successful blocks

Each file produces one independent block, preserving its own anchor.

### compress()

No-op. File content is non-compressible. The builder's drop logic handles budget overflow by removing lowest-priority middle blocks.

### estimate_size()

Returns 0 until `contribute()` has been called (no cached content). After first contribute, returns the total estimated token size of cached content.

### Caching

After the first `contribute()`, file contents are cached in the contributor. Subsequent `contribute()` calls return cached data. This avoids repeated disk I/O.

## Usage

```rust
use vol_llm_context::builtin::{FileContributor, FileSpec};
use vol_llm_context::AttentionAnchor;

let contributor = FileContributor::new(vec![
    FileSpec::new(".claude/ROLE.md", AttentionAnchor::Head(0)),
    FileSpec::new(".claude/CONVENTIONS.md", AttentionAnchor::Head(10)),
    FileSpec::new(".claude/TASK.md", AttentionAnchor::Tail(0)),
]);
```

## File Changes

| File | Action | Reason |
|------|--------|--------|
| `crates/vol-llm-context/src/builtin/file.rs` | Create | `FileContributor`, `FileSpec` |
| `crates/vol-llm-context/src/builtin/mod.rs` | Modify | Export `FileContributor`, `FileSpec`; remove role/rules/task exports |
| `crates/vol-llm-context/src/builtin/role.rs` | Delete | Replaced by FileContributor |
| `crates/vol-llm-context/src/builtin/rules.rs` | Delete | Replaced by FileContributor |
| `crates/vol-llm-context/src/builtin/task.rs` | Delete | Replaced by FileContributor |

## Error Handling

- File not found: `tracing::warn!` + skip. No error propagates.
- Read error (permissions, etc.): `tracing::warn!` + skip.
- This is intentional — missing files are common (e.g., no `TASK.md` in a fresh project) and should not fail the entire context build.

## Tests

1. `test_file_contributor_single_file` — write a temp .md, verify it returns a block with correct anchor
2. `test_file_contributor_multiple_files` — write two temp .md files, verify two blocks returned
3. `test_file_contributor_missing_file` — reference a nonexistent file, verify it returns empty blocks (no panic)
4. `test_file_contributor_mixed_exists` — one file exists, one doesn't, verify only existing file returns a block
5. `test_file_contributor_compress_noop` — verify compress() does nothing
6. `test_file_contributor_estimate_size` — verify estimate_size() returns 0 before contribute, > 0 after

## Preserved

`SkillsContributor` is NOT affected — it uses `SkillLoader`, not file paths.
