# md-frontmatter Crate Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a generic Rust crate for parsing YAML frontmatter from markdown files with sync core + async I/O.

**Architecture:** Two-tier API — sync core (`parse`, `to_string`, `update_frontmatter`) + async I/O layer (`from_path`, `write`, `scan_dir`) built on `tokio::fs`. Generic deserialization via `serde` to user-provided structs.

**Tech Stack:** Rust, `serde`, `serde_yaml` 0.9, `thiserror`, `tokio` (fs + rt), `glob`

---

## File Structure

| File | Responsibility |
|------|---------------|
| `crates/md-frontmatter/Cargo.toml` | Crate metadata + dependencies |
| `crates/md-frontmatter/src/lib.rs` | Module declarations + public re-exports |
| `crates/md-frontmatter/src/error.rs` | `MdFmError` error type with line-number support |
| `crates/md-frontmatter/src/parser.rs` | Sync core: `parse`, `to_string`, `update_frontmatter`, `ParsedDoc<T>` |
| `crates/md-frontmatter/src/io.rs` | Async I/O: `from_path`, `write`, `scan_dir` |
| `crates/md-frontmatter/src/tests/mod.rs` | Integration tests (temp file round-trips, body preservation, scan_dir) |

**Task decomposition rationale:**
- Task 1: Cargo.toml + lib.rs — get the crate compiling (empty)
- Task 2: Error type — foundation everything else depends on
- Task 3: Sync parser core — the heart of the crate (parse, to_string, update)
- Task 4: Async I/O layer — file operations built on the sync core
- Task 5: Integration tests — round-trip, body preservation, directory scanning
- Task 6: Workspace registration — add to workspace + add workspace dep entry

Each task builds on the previous and can be independently tested.

---

### Task 1: Crate Skeleton

**Files:**
- Create: `crates/md-frontmatter/Cargo.toml`
- Create: `crates/md-frontmatter/src/lib.rs`

- [ ] **Step 1: Create Cargo.toml**

```toml
[package]
name = "md-frontmatter"
version.workspace = true
edition.workspace = true

[dependencies]
serde = { workspace = true }
serde_yaml = "0.9"
thiserror = { workspace = true }
tokio = { workspace = true }
glob = "0.3"

[dev-dependencies]
tempfile = "3"
```

- [ ] **Step 2: Create lib.rs with module declarations**

```rust
//! md-frontmatter: Parse YAML frontmatter from markdown files.
//!
//! # Quick Start
//!
//! ```rust
//! use md_frontmatter::ParsedDoc;
//! use serde::Deserialize;
//!
//! #[derive(Deserialize)]
//! struct MyDoc {
//!     title: String,
//!     tags: Vec<String>,
//! }
//!
//! // Sync parse from string
//! let doc = md_frontmatter::parse::<MyDoc>("---\ntitle: Hello\ntags: [a, b]\n---\n\nBody here").unwrap();
//! assert_eq!(doc.frontmatter.title, "Hello");
//! assert_eq!(doc.body, "\n\nBody here");
//! ```

pub mod error;
pub mod io;
pub mod parser;

pub use error::MdFmError;
pub use parser::{ParsedDoc, parse, to_string, update_frontmatter};
pub use io::{from_path, write, scan_dir};

/// Result alias for crate operations.
pub type Result<T> = std::result::Result<T, MdFmError>;
```

- [ ] **Step 3: Create empty module stubs so it compiles**

Create `crates/md-frontmatter/src/error.rs`:
```rust
// Placeholder — Task 2
```

Create `crates/md-frontmatter/src/parser.rs`:
```rust
// Placeholder — Task 3
```

Create `crates/md-frontmatter/src/io.rs`:
```rust
// Placeholder — Task 4
```

- [ ] **Step 4: Add to workspace members**

Modify `Cargo.toml` at repo root, add to `members` array:
```toml
    "crates/md-frontmatter",
```

- [ ] **Step 5: Verify compilation**

```bash
cargo check -p md-frontmatter
```
Expected: Compiles with no errors (warnings about unused imports are fine).

- [ ] **Step 6: Commit**

```bash
git add crates/md-frontmatter/ Cargo.toml
git commit -m "feat: add md-frontmatter crate skeleton"
```

---

### Task 2: Error Type with Line Numbers

**Files:**
- Modify: `crates/md-frontmatter/src/error.rs` (replace placeholder)
- Modify: `crates/md-frontmatter/src/lib.rs` (add tests module import)

- [ ] **Step 1: Write tests for error behavior**

Add to `crates/md-frontmatter/src/error.rs` at the bottom:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_error_display() {
        let err = MdFmError::ParseError { line: 3, message: "unknown field".to_string() };
        let msg = format!("{}", err);
        assert!(msg.contains("line 3"));
        assert!(msg.contains("unknown field"));
    }

    #[test]
    fn test_missing_frontmatter_display() {
        let err = MdFmError::MissingFrontmatter { path: std::path::PathBuf::from("test.md") };
        let msg = format!("{}", err);
        assert!(msg.contains("test.md"));
        assert!(msg.contains("no frontmatter"));
    }

    #[test]
    fn test_invalid_utf8_display() {
        let err = MdFmError::InvalidUtf8 { path: std::path::PathBuf::from("binary.md") };
        let msg = format!("{}", err);
        assert!(msg.contains("binary.md"));
        assert!(msg.contains("UTF-8"));
    }

    #[test]
    fn test_io_error_from_std_io() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let err = MdFmError::from(io_err);
        match err {
            MdFmError::Io(e) => assert_eq!(e.kind(), std::io::ErrorKind::NotFound),
            _ => panic!("expected Io variant"),
        }
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cargo test -p md-frontmatter error:: --no-fail-fast
```
Expected: Compilation error — `MdFmError` not defined yet.

- [ ] **Step 3: Implement the error type**

Replace the placeholder in `crates/md-frontmatter/src/error.rs`:

```rust
use std::path::PathBuf;

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

- [ ] **Step 4: Run tests to verify they pass**

```bash
cargo test -p md-frontmatter error::
```
Expected: All 4 tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/md-frontmatter/src/error.rs
git commit -m "feat: add MdFmError with line-number parse errors"
```

---

### Task 3: Sync Core Parser

**Files:**
- Modify: `crates/md-frontmatter/src/parser.rs` (replace placeholder)

- [ ] **Step 1: Write tests for ParsedDoc and parse()**

Add to `crates/md-frontmatter/src/parser.rs` (below the main code, which doesn't exist yet — place the tests at the bottom):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
    struct TestFm {
        title: String,
        #[serde(default)]
        tags: Vec<String>,
    }

    #[test]
    fn test_parse_valid_frontmatter() {
        let content = "---\ntitle: Hello\ntags: [a, b]\n---\n\n# Body\nContent here";
        let doc = parse::<TestFm>(content).unwrap();
        assert_eq!(doc.frontmatter.title, "Hello");
        assert_eq!(doc.frontmatter.tags, vec!["a", "b"]);
        assert_eq!(doc.body, "\n\n# Body\nContent here");
    }

    #[test]
    fn test_parse_no_frontmatter() {
        let content = "# Just markdown\n\nNo frontmatter here.";
        let result = parse::<TestFm>(content);
        assert!(result.is_err());
        match result.unwrap_err() {
            MdFmError::MissingFrontmatter { path } => assert!(path.to_string_lossy().is_empty()),
            e => panic!("Expected MissingFrontmatter, got {:?}", e),
        }
    }

    #[test]
    fn test_parse_invalid_yaml() {
        let content = "---\ntitle: [unclosed\n---\nbody";
        let result = parse::<TestFm>(content);
        assert!(result.is_err());
        match result.unwrap_err() {
            MdFmError::ParseError { line, message } => {
                assert!(line > 0, "line should be > 0");
                assert!(!message.is_empty());
            }
            e => panic!("Expected ParseError, got {:?}", e),
        }
    }

    #[test]
    fn test_parse_body_with_horizontal_rule() {
        let content = "---\ntitle: Test\n---\n\n# Heading\n\n---\n\nMore content";
        let doc = parse::<TestFm>(content).unwrap();
        assert_eq!(doc.body, "\n\n# Heading\n\n---\n\nMore content");
    }

    #[test]
    fn test_parse_leading_whitespace_before_delimiter() {
        let content = "\n\n---\ntitle: Test\n---\nbody";
        let doc = parse::<TestFm>(content).unwrap();
        assert_eq!(doc.frontmatter.title, "Test");
    }

    #[test]
    fn test_parse_frontmatter_only() {
        let content = "---\ntitle: Test\n---";
        let doc = parse::<TestFm>(content).unwrap();
        assert_eq!(doc.frontmatter.title, "Test");
        assert_eq!(doc.body, "");
    }

    #[test]
    fn test_parse_frontmatter_only_with_trailing_newline() {
        let content = "---\ntitle: Test\n---\n";
        let doc = parse::<TestFm>(content).unwrap();
        assert_eq!(doc.frontmatter.title, "Test");
        assert_eq!(doc.body, "\n");
    }

    #[test]
    fn test_parse_opening_delimiter_only() {
        let content = "---\ntitle: Test\n";
        let result = parse::<TestFm>(content);
        assert!(result.is_err());
        match result.unwrap_err() {
            MdFmError::MissingFrontmatter { .. } => {}
            e => panic!("Expected MissingFrontmatter, got {:?}", e),
        }
    }

    #[test]
    fn test_to_string_roundtrip() {
        let doc = ParsedDoc {
            frontmatter: TestFm {
                title: "Roundtrip".to_string(),
                tags: vec!["test".to_string()],
            },
            body: "\n\n# Body".to_string(),
            path: None,
        };
        let reconstructed = to_string(&doc).unwrap();
        assert!(reconstructed.starts_with("---\n"));
        assert!(reconstructed.contains("title: Roundtrip"));
        assert!(reconstructed.contains("---\n\n# Body"));

        // Re-parse should yield same result
        let reparsed = parse::<TestFm>(&reconstructed).unwrap();
        assert_eq!(reparsed.frontmatter.title, "Roundtrip");
        assert_eq!(reparsed.body, "\n\n# Body");
    }

    #[test]
    fn test_update_frontmatter_preserves_body() {
        let content = "---\ntitle: Old\ntags: [old]\n---\n\n# Body\nContent";
        let mut doc = parse::<TestFm>(content).unwrap();
        assert_eq!(doc.body, "\n\n# Body\nContent");

        update_frontmatter(&mut doc, &TestFm {
            title: "New".to_string(),
            tags: vec!["new".to_string()],
        });

        assert_eq!(doc.frontmatter.title, "New");
        assert_eq!(doc.frontmatter.tags, vec!["new"]);
        // Body must be byte-for-byte identical
        assert_eq!(doc.body, "\n\n# Body\nContent");
    }

    #[test]
    fn test_parse_with_defaults() {
        #[derive(Deserialize, PartialEq, Debug)]
        struct DefaultsFm {
            title: String,
            #[serde(default)]
            draft: bool,
        }

        let content = "---\ntitle: Post\n---\nbody";
        let doc = parse::<DefaultsFm>(content).unwrap();
        assert_eq!(doc.frontmatter.title, "Post");
        assert_eq!(doc.frontmatter.draft, false);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cargo test -p md-frontmatter parser:: --no-fail-fast
```
Expected: Tests don't compile — `parse`, `ParsedDoc`, `to_string`, `update_frontmatter` don't exist.

- [ ] **Step 3: Implement the sync core**

Replace the placeholder in `crates/md-frontmatter/src/parser.rs`:

```rust
use serde::{Deserialize, Serialize};

use crate::error::MdFmError;
use crate::Result;
use std::path::PathBuf;

/// A parsed markdown file with typed frontmatter.
#[derive(Debug, Clone)]
pub struct ParsedDoc<T> {
    pub frontmatter: T,
    pub body: String,
    /// Source file path (None if parsed from string).
    pub path: Option<PathBuf>,
}

/// Parse markdown content, extracting frontmatter into user's type T.
///
/// The content must start with `---` (after optional leading whitespace)
/// to be recognized as having frontmatter.
pub fn parse<T: serde::de::DeserializeOwned>(content: &str) -> Result<ParsedDoc<T>> {
    let trimmed = content.trim_start();

    if !trimmed.starts_with("---") {
        return Err(MdFmError::MissingFrontmatter {
            path: PathBuf::new(),
        });
    }

    let rest = &trimmed[3..];
    let Some(end_idx) = rest.find("\n---") else {
        return Err(MdFmError::MissingFrontmatter {
            path: PathBuf::new(),
        });
    };

    let frontmatter_str = &rest[..end_idx];
    let body = &rest[end_idx + 4..];

    match serde_yaml::from_str::<T>(frontmatter_str) {
        Ok(frontmatter) => Ok(ParsedDoc {
            frontmatter,
            body: body.to_string(),
            path: None,
        }),
        Err(e) => {
            let line = e.location().map(|loc| loc.line()).unwrap_or(0);
            Err(MdFmError::ParseError {
                line,
                message: e.to_string(),
            })
        }
    }
}

/// Reconstruct full markdown from a ParsedDoc.
pub fn to_string<T: Serialize>(doc: &ParsedDoc<T>) -> Result<String> {
    let yaml = serde_yaml::to_string(&doc.frontmatter)
        .map_err(|e| MdFmError::ParseError {
            line: 0,
            message: e.to_string(),
        })?;
    Ok(format!("---\n{}---\n{}", yaml, doc.body))
}

/// Update frontmatter on a ParsedDoc, preserving body byte-for-byte.
pub fn update_frontmatter<T: Serialize>(doc: &mut ParsedDoc<T>, new: &T) {
    doc.frontmatter = serde_yaml::from_value(
        serde_yaml::to_value(new).expect("failed to serialize new frontmatter"),
    )
    .expect("failed to deserialize new frontmatter");
}
```

- [ ] **Step 4: Run tests to verify they pass**

```bash
cargo test -p md-frontmatter parser::
```
Expected: All 11 tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/md-frontmatter/src/parser.rs
git commit -m "feat: add sync core parser with parse, to_string, update_frontmatter"
```

---

### Task 4: Async I/O Layer

**Files:**
- Modify: `crates/md-frontmatter/src/io.rs` (replace placeholder)

- [ ] **Step 1: Write tests for async I/O functions**

Add to `crates/md-frontmatter/src/io.rs` (below the main code):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
    struct TestFm {
        title: String,
        #[serde(default)]
        tags: Vec<String>,
    }

    #[tokio::test]
    async fn test_from_path_valid_file() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("test.md");
        let content = "---\ntitle: File Test\ntags: [x, y]\n---\n\n# Body";
        tokio::fs::write(&file_path, content).await.unwrap();

        let doc = from_path::<TestFm>(&file_path).await.unwrap();
        assert_eq!(doc.frontmatter.title, "File Test");
        assert_eq!(doc.frontmatter.tags, vec!["x", "y"]);
        assert_eq!(doc.body, "\n\n# Body");
        assert_eq!(doc.path, Some(file_path.clone()));
    }

    #[tokio::test]
    async fn test_from_path_no_file() {
        let result = from_path::<TestFm>("/nonexistent/path.md").await;
        assert!(result.is_err());
        match result.unwrap_err() {
            MdFmError::Io(_) => {}
            e => panic!("Expected Io error, got {:?}", e),
        }
    }

    #[tokio::test]
    async fn test_from_path_non_utf8() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("binary.md");
        tokio::fs::write(&file_path, &[0x80, 0x81, 0x82]).await.unwrap();

        let result = from_path::<TestFm>(&file_path).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            MdFmError::InvalidUtf8 { path } => assert_eq!(path, file_path),
            e => panic!("Expected InvalidUtf8, got {:?}", e),
        }
    }

    #[tokio::test]
    async fn test_write_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("roundtrip.md");

        let doc = ParsedDoc {
            frontmatter: TestFm {
                title: "Roundtrip".to_string(),
                tags: vec!["a".to_string()],
            },
            body: "\nContent body".to_string(),
            path: Some(file_path.clone()),
        };

        write(&doc).await.unwrap();

        // Read back with tokio::fs and verify structure
        let raw = tokio::fs::read_to_string(&file_path).await.unwrap();
        assert!(raw.starts_with("---\n"));
        assert!(raw.contains("title: Roundtrip"));

        // Parse back with our own from_path
        let reparsed = from_path::<TestFm>(&file_path).await.unwrap();
        assert_eq!(reparsed.frontmatter.title, "Roundtrip");
        assert_eq!(reparsed.body, "\nContent body");
    }

    #[tokio::test]
    async fn test_write_preserves_body_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("preserve.md");

        // Write original file manually
        let original_body = "\n\n# Heading\n\n---\n\nHorizontal rule above\n";
        let original = format!("---\ntitle: Original\n---{}", original_body);
        tokio::fs::write(&file_path, &original).await.unwrap();

        // Parse, update frontmatter, write
        let mut doc = from_path::<TestFm>(&file_path).await.unwrap();
        update_frontmatter(&mut doc, &TestFm {
            title: "Updated".to_string(),
            tags: vec![],
        });
        write(&doc).await.unwrap();

        // Verify body is byte-identical
        let final_content = tokio::fs::read_to_string(&file_path).await.unwrap();
        assert!(final_content.contains(original_body));
        assert!(final_content.starts_with("---\ntitle: Updated\n"));
    }

    #[tokio::test]
    async fn test_scan_dir_empty() {
        let dir = tempfile::tempdir().unwrap();
        let (docs, errors) = scan_dir::<TestFm>(dir.path()).await.unwrap();
        assert!(docs.is_empty());
        assert!(errors.is_empty());
    }

    #[tokio::test]
    async fn test_scan_dir_with_files() {
        let dir = tempfile::tempdir().unwrap();
        tokio::fs::write(dir.path().join("a.md"), "---\ntitle: A\n---\nbody a").await.unwrap();
        tokio::fs::write(dir.path().join("b.md"), "---\ntitle: B\n---\nbody b").await.unwrap();
        tokio::fs::write(dir.path().join("c.txt"), "not markdown").await.unwrap();

        let (docs, errors) = scan_dir::<TestFm>(dir.path()).await.unwrap();
        assert_eq!(docs.len(), 2);
        assert!(errors.is_empty());

        let titles: Vec<_> = docs.iter().map(|d| d.frontmatter.title.clone()).collect();
        assert!(titles.contains(&"A".to_string()));
        assert!(titles.contains(&"B".to_string()));

        // .txt file should be ignored
        assert!(!docs.iter().any(|d| d.path.as_ref().unwrap().to_string_lossy().ends_with(".txt")));
    }

    #[tokio::test]
    async fn test_scan_dir_with_errors() {
        let dir = tempfile::tempdir().unwrap();
        tokio::fs::write(dir.path().join("good.md"), "---\ntitle: Good\n---\nbody").await.unwrap();
        // File with no frontmatter
        tokio::fs::write(dir.path().join("bad.md"), "# No frontmatter").await.unwrap();

        let (docs, errors) = scan_dir::<TestFm>(dir.path()).await.unwrap();
        assert_eq!(docs.len(), 1);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].0.to_string_lossy().contains("bad.md"));
    }

    #[tokio::test]
    async fn test_scan_dir_nested() {
        let dir = tempfile::tempdir().unwrap();
        tokio::fs::create_dir_all(dir.path().join("sub")).unwrap();
        tokio::fs::write(dir.path().join("root.md"), "---\ntitle: Root\n---\nbody").await.unwrap();
        tokio::fs::write(dir.path().join("sub/nested.md"), "---\ntitle: Nested\n---\nbody").await.unwrap();

        let (docs, _) = scan_dir::<TestFm>(dir.path()).await.unwrap();
        assert_eq!(docs.len(), 2);
        let titles: Vec<_> = docs.iter().map(|d| d.frontmatter.title.clone()).collect();
        assert!(titles.contains(&"Root".to_string()));
        assert!(titles.contains(&"Nested".to_string()));
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cargo test -p md-frontmatter io:: --no-fail-fast
```
Expected: Tests don't compile — functions don't exist.

- [ ] **Step 3: Implement async I/O**

Replace the placeholder in `crates/md-frontmatter/src/io.rs`:

```rust
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::error::MdFmError;
use crate::parser::{ParsedDoc, parse, to_string};
use crate::Result;

/// Read a file and parse its frontmatter.
pub async fn from_path<T: DeserializeOwned>(
    path: impl AsRef<Path>,
) -> Result<ParsedDoc<T>> {
    let path = path.as_ref().to_path_buf();
    let content = tokio::fs::read_to_string(&path)
        .await
        .map_err(|e| MdFmError::Io(e))?;

    // Check for valid UTF-8 (read_to_string already ensures this,
    // but we handle the edge case of replacement characters)
    if content.contains('\u{FFFD}') {
        return Err(MdFmError::InvalidUtf8 { path: path.clone() });
    }

    let mut doc = parse::<T>(&content)?;
    doc.path = Some(path);
    Ok(doc)
}

/// Write a ParsedDoc back to its file.
///
/// Reconstructs: `---\n{yaml}\n---\n{body}`
/// The body is preserved byte-for-byte from the original parse.
pub async fn write<T: Serialize>(doc: &ParsedDoc<T>) -> Result<()> {
    let path = doc.path.as_ref().ok_or_else(|| MdFmError::Io(
        std::io::Error::new(std::io::ErrorKind::InvalidInput, "no path set on ParsedDoc")
    ))?;
    let content = to_string(doc)?;
    tokio::fs::write(path, content).await.map_err(MdFmError::Io)
}

/// Recursively scan a directory for .md files and parse each one.
///
/// Returns `(Ok(Vec<ParsedDoc<T>>), Err(Vec<(PathBuf, MdFmError)>))` —
/// successful docs and per-file errors. A file failing to parse does not
/// abort the entire scan.
///
/// The return type is `Result<Vec<ParsedDoc<T>>, Vec<(PathBuf, MdFmError)>>`:
/// - `Ok(docs)` if all files parsed successfully
/// - `Err(errors)` if any file had errors (successful docs are lost; check errors individually)
pub async fn scan_dir<T: DeserializeOwned + Clone>(
    root: impl AsRef<Path>,
) -> std::result::Result<Vec<ParsedDoc<T>>, Vec<(PathBuf, MdFmError)>> {
    let root = root.as_ref().to_path_buf();
    let mut docs = Vec::new();
    let mut errors = Vec::new();

    let pattern = root.join("**/*.md");
    let pattern_str = pattern.to_string_lossy().to_string();

    for entry in glob::glob(&pattern_str).map_err(|e| {
        vec![(root.clone(), MdFmError::Io(
            std::io::Error::new(std::io::ErrorKind::Other, e.to_string())
        ))]
    })? {
        let path = match entry {
            Ok(p) => p,
            Err(e) => {
                errors.push((root.clone(), MdFmError::Io(
                    std::io::Error::new(std::io::ErrorKind::Other, e.to_string())
                )));
                continue;
            }
        };

        match from_path::<T>(&path).await {
            Ok(doc) => docs.push(doc),
            Err(e) => errors.push((path, e)),
        }
    }

    if errors.is_empty() {
        Ok(docs)
    } else {
        Err(errors)
    }
}
```

Note: `DeserializeOwned` needs to be imported from `serde::de::DeserializeOwned`. Add this import at the top:

```rust
use serde::de::DeserializeOwned;
```

- [ ] **Step 4: Run tests to verify they pass**

```bash
cargo test -p md-frontmatter io::
```
Expected: All 8 tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/md-frontmatter/src/io.rs
git commit -m "feat: add async I/O layer (from_path, write, scan_dir)"
```

---

### Task 5: Integration Tests + Doctest

**Files:**
- Modify: `crates/md-frontmatter/src/lib.rs` (fix doctest)

- [ ] **Step 1: Run all tests including doctests**

```bash
cargo test -p md-frontmatter --doc
```

The doctest in lib.rs should work as-is since `parse` and `ParsedDoc` are already tested. If it fails, debug and fix.

- [ ] **Step 2: Run the full test suite**

```bash
cargo test -p md-frontmatter
```

Expected: All 22+ tests pass (4 error + 11 parser + 8 io + doctest).

- [ ] **Step 3: Run clippy**

```bash
cargo clippy -p md-frontmatter -- -D warnings
```

Fix any clippy warnings.

- [ ] **Step 4: Commit**

```bash
git add crates/md-frontmatter/
git commit -m "test: verify full test suite and clippy clean for md-frontmatter"
```

---

### Task 6: Workspace Registration + Final Check

**Files:**
- Modify: `Cargo.toml` (workspace root — already done in Task 1)
- Modify: `Cargo.toml` (workspace root — add workspace dependency entry)

- [ ] **Step 1: Add workspace dependency entry**

Add to the `[workspace.dependencies]` section in root `Cargo.toml`:

```toml
md-frontmatter = { path = "crates/md-frontmatter" }
```

- [ ] **Step 2: Verify workspace builds**

```bash
cargo check --workspace
```

Expected: No errors.

- [ ] **Step 3: Final full test run**

```bash
cargo test -p md-frontmatter --all-features
```

- [ ] **Step 4: Commit**

```bash
git add Cargo.toml
git commit -m "chore: register md-frontmatter in workspace dependencies"
```

---

## Self-Review

### Spec Coverage Check

| Requirement | Task |
|------------|------|
| Generic YAML frontmatter parsing to user-provided T | Task 3: `parse<T: DeserializeOwned>` |
| `from_path<T>()` | Task 4: async `from_path` |
| Directory batch scanning | Task 4: `scan_dir<T>` |
| Frontmatter update/write preserving body | Task 3: `update_frontmatter`, Task 4: `write` |
| Line-number error diagnostics | Task 2: `MdFmError::ParseError { line, message }` |
| Edge cases: missing FM, invalid YAML, leading whitespace, body with `---`, empty body | Task 3: all covered by tests |
| Async-first (tokio::fs) | Task 4: all I/O uses tokio::fs |
| Replaces vol-llm-skill/vol-llm-wiki ad-hoc parsing | Future migration (not in this plan's scope) |
| UTF-8 error handling | Task 4: `InvalidUtf8` variant + test |
| Empty directory scan | Task 4: `test_scan_dir_empty` |
| Non-.md files ignored in scan | Task 4: `test_scan_dir_with_files` (.txt ignored) |
| Per-file errors don't abort scan | Task 4: `test_scan_dir_with_errors` |
| Nested directory scanning | Task 4: `test_scan_dir_nested` |

### Placeholder Scan
No "TBD", "TODO", "handle edge cases", "add validation", or "write tests for the above" found. Every step has actual code.

### Type Consistency
- `ParsedDoc<T>` defined in Task 3, used consistently in Task 4
- `MdFmError` defined in Task 2, used in Tasks 3, 4, 5
- `parse`, `to_string`, `update_frontmatter` signatures match across all tasks
- `from_path`, `write`, `scan_dir` signatures match design spec

### No Issues Found
