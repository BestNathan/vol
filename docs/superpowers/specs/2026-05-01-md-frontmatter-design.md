# Design: md-frontmatter Crate

## Overview

A generic Rust crate for parsing YAML frontmatter from markdown files. Provides sync core parsing + async I/O convenience methods, replacing ad-hoc implementations in `vol-llm-skill` and `vol-llm-wiki`.

## Architecture

```
┌──────────────────────────────────────────────────────┐
│                 Public API Surface                    │
├──────────────────────┬───────────────────────────────┤
│   Async I/O Layer    │     Sync Core (pure)           │
│   (tokio::fs)        │                                │
│                      │                                │
│  from_path<T>()      │  parse<T>(content)             │
│  scan_dir<T>()       │  update_frontmatter(doc, new)  │
│  write(doc)          │  to_string(doc)                │
├──────────────────────┴───────────────────────────────┤
│              Error Type (line-number aware)           │
│              MdFmError                                │
└──────────────────────────────────────────────────────┘
```

**Separation principle**: The sync core handles `&str -> (T, &str)` — splitting YAML frontmatter from markdown body and deserializing to the user's struct. The async layer wraps `tokio::fs` for file read/write and directory scanning, built on top of the sync core.

## Dependencies

| Crate | Source | Purpose |
|-------|--------|---------|
| `serde` | workspace | Derive Deserialize/Serialize for user types |
| `serde_yaml` | 0.9 | YAML parsing |
| `thiserror` | workspace | Error type derivation |
| `tokio` | workspace | Async file I/O |
| `glob` | crates.io | Directory scanning |

## API Reference

### Core Types

```rust
/// A parsed markdown file with typed frontmatter.
pub struct ParsedDoc<T> {
    pub frontmatter: T,
    pub body: String,
    /// Source file path (None if parsed from string).
    pub path: Option<PathBuf>,
}

/// Error type for all frontmatter operations.
#[derive(Debug, thiserror::Error)]
pub enum MdFmError {
    #[error("frontmatter parse error at line {line}: {message}")]
    ParseError { line: usize, message: String },

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("file is not valid UTF-8: {path}")]
    InvalidUtf8 { path: PathBuf },

    #[error("no frontmatter found: {path}")]
    MissingFrontmatter { path: PathBuf },
}
```

### Sync Core

```rust
/// Parse markdown content, extracting frontmatter into user's type T.
pub fn parse<T: DeserializeOwned>(content: &str) -> Result<ParsedDoc<T>, MdFmError>;

/// Reconstruct full markdown from a ParsedDoc.
pub fn to_string<T: Serialize>(doc: &ParsedDoc<T>) -> Result<String, MdFmError>;

/// Update frontmatter on a ParsedDoc, preserving body byte-for-byte.
pub fn update_frontmatter<T: Serialize>(doc: &mut ParsedDoc<T>, new: &T);
```

### Async I/O

```rust
/// Read a file and parse its frontmatter.
pub async fn from_path<T: DeserializeOwned>(
    path: impl AsRef<Path>,
) -> Result<ParsedDoc<T>, MdFmError>;

/// Write a ParsedDoc back to its file.
pub async fn write<T: Serialize>(doc: &ParsedDoc<T>) -> Result<(), MdFmError>;

/// Recursively scan a directory for .md files, parse each.
/// Returns (successful docs, errors per file).
pub async fn scan_dir<T: DeserializeOwned + Clone>(
    root: impl AsRef<Path>,
) -> Result<Vec<ParsedDoc<T>>, Vec<(PathBuf, MdFmError)>>;
```

## Frontmatter Parsing Algorithm

1. Trim leading whitespace from content
2. If content doesn't start with `---`, return `MissingFrontmatter`
3. Find the closing `\n---` delimiter (first occurrence after opening)
4. Extract YAML between delimiters
5. Parse YAML with `serde_yaml::from_str::<T>`
6. On parse error, extract the line number from `serde_yaml::Error` and wrap in `MdFmError::ParseError`
7. Body is everything after the closing `---`, preserving original bytes

### Edge Cases

| Input | Result |
|-------|--------|
| No `---` | `MissingFrontmatter` |
| Opening `---` only | `MissingFrontmatter` |
| Invalid YAML | `ParseError` with line number |
| Body contains `---` | Unaffected — only first pair is delimiter |
| Leading whitespace before `---` | Accepted after trimming |
| Only frontmatter, no body | `body` is empty string |

## Error Line Numbering

`serde_yaml::Error` provides a `location()` method with line/column. The `ParseError` variant exposes the 1-based line number **within the frontmatter block** (counting from the first `---`).

## Write Semantics

`to_string()` and `write()` reconstruct the file as:

```
---
{serialized YAML}
---
{body}
```

The body is preserved byte-for-byte from the original parse. Frontmatter is re-serialized from the current struct state via `serde_yaml::to_string`.

## Migration Path

### vol-llm-skill/src/parser.rs

Replace ~80 lines of hard-coded parsing:
```rust
// Before: manual split + serde_yaml::from_str::<SkillFrontmatter>
// After: one call to md_frontmatter::parse::<SkillFrontmatter>()
```

### vol-llm-wiki/src/loader.rs

Replace ~15 lines of manual string parsing (`parse_frontmatter_value`):
```rust
// Before: string find/split for title, tags
// After: md_frontmatter::parse::<WikiFrontmatter>(&content)
```

The sync `walk_dir` in WikiLoader can optionally migrate to `scan_dir` in a follow-up.

## Directory Structure

```
crates/md-frontmatter/
├── Cargo.toml
└── src/
    ├── lib.rs          # Public API + re-exports
    ├── parser.rs        # Sync core: parse, to_string, update
    ├── io.rs            # Async I/O: from_path, write, scan_dir
    └── error.rs         # MdFmError definition
```

## Testing Strategy

- **Unit tests** (parser.rs): Valid/invalid/missing frontmatter, body with `---`, leading whitespace, empty body
- **Unit tests** (error.rs): Line number accuracy for parse errors
- **Integration tests** (io.rs): temp directory with test files, verify round-trip (parse → modify → write → parse)
- **Body preservation test**: Parse a file, modify frontmatter, write back, verify body is byte-identical to original
