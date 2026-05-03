# Migrate vol-llm-skill and vol-llm-wiki to md-frontmatter

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace ad-hoc frontmatter parsing in `vol-llm-skill` and `vol-llm-wiki` with the generic `md-frontmatter` crate, deleting custom parsing code.

**Architecture:** Two sequential migration passes. Task 1 migrates `vol-llm-skill` (delete parser.rs, rewrite loader.rs). Task 2 migrates `vol-llm-wiki` (rewrite loader.rs to use `scan_dir`). Each crate gets `md-frontmatter` as a dependency. Strict error handling — missing/invalid frontmatter logs a warning and skips the file.

**Tech Stack:** Rust, `md-frontmatter`, `serde`, `thiserror`, `tracing`, `glob`

---

## File Structure

| File | Task | Responsibility |
|------|------|----------------|
| `crates/vol-llm-skill/src/parser.rs` | Task 1 | DELETE — all custom parsing replaced |
| `crates/vol-llm-skill/src/loader.rs` | Task 1 | Rewrite to use `md_frontmatter::parse` inline |
| `crates/vol-llm-skill/src/lib.rs` | Task 1 | Remove `mod parser` |
| `crates/vol-llm-skill/Cargo.toml` | Task 1 | Add `md-frontmatter`, remove `serde_yaml` |
| `crates/vol-llm-wiki/src/loader.rs` | Task 2 | Rewrite `walk_dir` to use `md_frontmatter::scan_dir` |
| `crates/vol-llm-wiki/Cargo.toml` | Task 2 | Add `md-frontmatter` |

---

### Task 1: Migrate vol-llm-skill

**Files:**
- Delete: `crates/vol-llm-skill/src/parser.rs`
- Modify: `crates/vol-llm-skill/src/loader.rs`
- Modify: `crates/vol-llm-skill/src/lib.rs:26`
- Modify: `crates/vol-llm-skill/Cargo.toml`

- [ ] **Step 1: Update loader.rs to use md-frontmatter (test will fail because parser.rs still exists)**

Replace the entire contents of `crates/vol-llm-skill/src/loader.rs` with the md-frontmatter version. The key changes:
1. Add `md_frontmatter::parse::<SkillFrontmatter>` import
2. Define `SkillFrontmatter` struct locally (was in parser.rs)
3. Replace `parse_skill_content(&content)` call with `md_frontmatter::parse`
4. Move `scan_skill_files` and `collect_files_recursive` inline as private helpers
5. Remove the `parser` module import

The new `loader.rs`:

```rust
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::Deserialize;
use tokio::sync::{OnceCell, RwLock};

use crate::def::{SkillDef, SkillMetadata, SkillScope};
use crate::Result;

fn default_version() -> String {
    "1.0.0".to_string()
}

#[derive(Debug, Deserialize)]
struct SkillFrontmatter {
    name: String,
    #[serde(default = "default_version")]
    version: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    triggers: Vec<String>,
}

/// Discovers, loads, and caches skills from registered roots.
pub struct SkillLoader {
    roots: Vec<(SkillScope, PathBuf)>,
    skills: Arc<RwLock<HashMap<String, Arc<SkillDef>>>>,
    metadata_cache: Arc<RwLock<Vec<SkillMetadata>>>,
    discovered: OnceCell<()>,
}

impl SkillLoader {
    /// Creates a loader with no default roots (useful for tests).
    pub fn new_empty() -> Self {
        Self {
            roots: Vec::new(),
            skills: Arc::new(RwLock::new(HashMap::new())),
            metadata_cache: Arc::new(RwLock::new(Vec::new())),
            discovered: OnceCell::new(),
        }
    }

    /// Creates a loader with default roots.
    pub fn new(working_dir: Option<PathBuf>) -> Self {
        let mut loader = Self {
            roots: Vec::new(),
            skills: Arc::new(RwLock::new(HashMap::new())),
            metadata_cache: Arc::new(RwLock::new(Vec::new())),
            discovered: OnceCell::new(),
        };

        if let Some(home) = dirs::home_dir() {
            let user_root = home.join(".agents").join("skills");
            loader.add_root(SkillScope::User, user_root);
        }

        if let Some(ref wd) = working_dir {
            let repo_root = wd.join(".agents").join("skills");
            loader.add_root(SkillScope::Repo, repo_root);
        }

        loader
    }

    /// Add a discovery root.
    pub fn add_root(&mut self, scope: SkillScope, path: PathBuf) {
        self.roots.push((scope, path));
    }

    /// Discover skills from all registered roots.
    pub async fn discover_all(&self) -> Result<()> {
        let mut skills_map = HashMap::new();

        for (scope, root_path) in &self.roots {
            if !root_path.exists() || !root_path.is_dir() {
                continue;
            }

            let entries = match std::fs::read_dir(root_path) {
                Ok(e) => e,
                Err(e) => {
                    tracing::warn!(path = %root_path.display(), error = %e, "Failed to read skill root");
                    continue;
                }
            };

            for entry in entries.flatten() {
                let dir_path = entry.path();
                if !dir_path.is_dir() {
                    continue;
                }

                let skill_md = dir_path.join("SKILL.md");
                if !skill_md.exists() {
                    continue;
                }

                let content = match std::fs::read_to_string(&skill_md) {
                    Ok(c) => c,
                    Err(e) => {
                        tracing::warn!(path = %skill_md.display(), error = %e, "Failed to read SKILL.md, skipping");
                        continue;
                    }
                };

                let fm = match md_frontmatter::parse::<SkillFrontmatter>(&content) {
                    Ok(doc) => doc.frontmatter,
                    Err(e) => {
                        tracing::warn!(path = %skill_md.display(), error = %e, "Failed to parse SKILL.md frontmatter, skipping");
                        continue;
                    }
                };

                let file_listing = scan_skill_files(&dir_path);
                let id = format!("{}:{}", scope.prefix(), fm.name);

                let def = SkillDef {
                    id: id.clone(),
                    name: fm.name.clone(),
                    version: fm.version,
                    description: fm.description,
                    scope: scope.clone(),
                    triggers: fm.triggers,
                    content: doc_to_body(&content),
                    file_listing,
                };

                if !skills_map.contains_key(&fm.name) {
                    skills_map.insert(fm.name, Arc::new(def));
                } else {
                    tracing::warn!(skill = %fm.name, "Duplicate skill name, keeping existing");
                }
            }
        }

        let mut guard = self.skills.write().await;
        for (name, def) in skills_map {
            guard.insert(name, def);
        }
        drop(guard);

        self.rebuild_metadata().await;

        Ok(())
    }

    /// Ensure skills are discovered on first access.
    async fn ensure_discovered(&self) {
        self.discovered
            .get_or_init(|| async {
                let _ = self.discover_all().await;
            })
            .await;
    }

    /// List metadata for progressive disclosure.
    pub async fn list_metadata(&self) -> Vec<SkillMetadata> {
        self.ensure_discovered().await;
        self.metadata_cache.read().await.clone()
    }

    /// Get full skill by name.
    pub async fn get(&self, name: &str) -> Option<Arc<SkillDef>> {
        self.ensure_discovered().await;
        self.skills.read().await.get(name).cloned()
    }

    /// Find skills whose triggers match the query.
    pub async fn get_by_trigger(&self, query: &str) -> Vec<Arc<SkillDef>> {
        self.ensure_discovered().await;
        let guard = self.skills.read().await;
        let query_lower = query.to_lowercase();
        guard
            .values()
            .filter(|def| {
                def.triggers
                    .iter()
                    .any(|t| query_lower.contains(&t.to_lowercase()) || t.to_lowercase().contains(&query_lower))
            })
            .cloned()
            .collect()
    }

    /// Register a skill directly.
    pub async fn register(&self, skill: SkillDef) {
        let name = skill.name.clone();
        self.skills.write().await.insert(name, Arc::new(skill));
        self.rebuild_metadata().await;
    }

    /// Rebuild the metadata cache from current skills.
    async fn rebuild_metadata(&self) {
        let guard = self.skills.read().await;
        let metadata: Vec<SkillMetadata> = guard.values().map(|d| d.as_ref().into()).collect();
        drop(guard);
        *self.metadata_cache.write().await = metadata;
    }
}

/// Scan a skill directory for all files, returning relative paths.
fn scan_skill_files(root: &Path) -> Vec<String> {
    let mut files = Vec::new();
    if let Ok(entries) = std::fs::read_dir(root) {
        for entry in entries.flatten() {
            let path = entry.path();
            collect_files_recursive(&path, root, &mut files);
        }
    }
    files.sort();
    files
}

fn collect_files_recursive(path: &Path, root: &Path, files: &mut Vec<String>) {
    if path.is_file() {
        if let Ok(rel) = path.strip_prefix(root) {
            files.push(rel.to_string_lossy().to_string());
        }
    } else if path.is_dir() {
        if let Ok(entries) = std::fs::read_dir(path) {
            for entry in entries.flatten() {
                collect_files_recursive(&entry.path(), root, files);
            }
        }
    }
}

/// Extract the body from SKILL.md content by finding the closing --- delimiter.
/// This is only used for the `content` field of SkillDef; frontmatter is parsed separately.
fn doc_to_body(content: &str) -> String {
    let trimmed = content.trim_start();
    if !trimmed.starts_with("---") {
        return content.to_string();
    }
    let rest = &trimmed[3..];
    if let Some(end_idx) = rest.find("\n---") {
        rest[end_idx + 4..].to_string()
    } else {
        content.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[tokio::test]
    async fn test_discover_skills_from_temp_dir() {
        let temp_dir = tempfile::tempdir().unwrap();
        let skills_dir = temp_dir.path().join(".agents").join("skills");
        std::fs::create_dir_all(&skills_dir).unwrap();

        let rust_dir = skills_dir.join("rust-conventions");
        std::fs::create_dir_all(&rust_dir).unwrap();
        let mut f = std::fs::File::create(rust_dir.join("SKILL.md")).unwrap();
        writeln!(f, "---").unwrap();
        writeln!(f, "name: rust-conventions").unwrap();
        writeln!(f, "version: 1.0.0").unwrap();
        writeln!(f, "description: Rust conventions").unwrap();
        writeln!(f, "triggers: [rust]").unwrap();
        writeln!(f, "---").unwrap();
        writeln!(f, "# Rust Conventions").unwrap();

        let invalid_dir = skills_dir.join("invalid-skill");
        std::fs::create_dir_all(&invalid_dir).unwrap();

        let mut loader = SkillLoader::new(None);
        loader.roots.clear();
        loader.add_root(SkillScope::User, skills_dir.clone());
        loader.discover_all().await.unwrap();

        let skill = loader.get("rust-conventions").await;
        assert!(skill.is_some(), "rust-conventions skill should exist");
        assert!(skill.unwrap().content.contains("# Rust Conventions"));
    }

    #[tokio::test]
    async fn test_discover_empty_root() {
        let temp_dir = tempfile::tempdir().unwrap();
        let non_existent = temp_dir.path().join("nonexistent");
        let mut loader = SkillLoader::new(None);
        loader.roots.clear();
        loader.add_root(SkillScope::User, non_existent);
        let result = loader.discover_all().await;
        assert!(result.is_ok());
        let _ = loader.list_metadata().await;
    }

    #[tokio::test]
    async fn test_get_by_trigger() {
        let loader = SkillLoader::new(None);
        let mut skill = SkillDef::new("test-skill", "some content")
            .with_triggers(vec!["rust".to_string(), "coding".to_string()]);
        skill.id = "code:test-skill".to_string();
        loader.register(skill).await;

        let results = loader.get_by_trigger("rust").await;
        assert_eq!(results.len(), 1);

        let no_results = loader.get_by_trigger("python").await;
        assert_eq!(no_results.len(), 0);
    }

    #[tokio::test]
    async fn test_register_overwrites() {
        let loader = SkillLoader::new(None);
        let mut skill1 = SkillDef::new("dup", "content1");
        skill1.id = "user:dup".to_string();
        loader.register(skill1).await;

        let mut skill2 = SkillDef::new("dup", "content2");
        skill2.id = "repo:dup".to_string();
        loader.register(skill2).await;

        let skill = loader.get("dup").await.unwrap();
        assert_eq!(skill.content, "content2");
    }
}
```

- [ ] **Step 2: Remove parser module from lib.rs**

In `crates/vol-llm-skill/src/lib.rs`, remove line 26 (`mod parser;`):

```rust
mod def;
mod injector;
mod loader;
mod tool;

pub use def::{SkillDef, SkillMetadata, SkillScope};
pub use injector::SkillInjector;
pub use loader::SkillLoader;
pub use tool::SkillTool;
```

- [ ] **Step 3: Update Cargo.toml**

Replace `crates/vol-llm-skill/Cargo.toml`:

```toml
[package]
name = "vol-llm-skill"
version.workspace = true
edition.workspace = true

[dependencies]
async-trait = { workspace = true }
tokio = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }
tracing = { workspace = true }
md-frontmatter = { workspace = true }
vol-llm-core = { workspace = true }
vol-llm-context = { workspace = true }
vol-llm-tool = { workspace = true }
dirs = "5"

[dev-dependencies]
tempfile = "3"
```

Note: `serde_yaml = "0.9"` is removed — no longer needed since parsing is delegated to `md-frontmatter`.

- [ ] **Step 4: Delete parser.rs**

```bash
rm crates/vol-llm-skill/src/parser.rs
```

- [ ] **Step 5: Verify compilation and tests**

```bash
cargo check -p vol-llm-skill
```
Expected: Compiles with no errors.

```bash
cargo test -p vol-llm-skill
```
Expected: All 4 loader tests pass. The parser tests are gone (moved to md-frontmatter crate tests).

```bash
cargo clippy -p vol-llm-skill -- -D warnings
```
Expected: No warnings.

- [ ] **Step 6: Commit**

```bash
git rm crates/vol-llm-skill/src/parser.rs
git add crates/vol-llm-skill/src/loader.rs crates/vol-llm-skill/src/lib.rs crates/vol-llm-skill/Cargo.toml
git commit -m "refactor(vol-llm-skill): replace ad-hoc parsing with md-frontmatter crate"
```

---

### Task 2: Migrate vol-llm-wiki

**Files:**
- Modify: `crates/vol-llm-wiki/src/loader.rs`
- Modify: `crates/vol-llm-wiki/Cargo.toml`

- [ ] **Step 1: Rewrite loader.rs to use md-frontmatter**

Replace the entire contents of `crates/vol-llm-wiki/src/loader.rs`:

```rust
//! Wiki page discovery and loading.

use std::path::PathBuf;
use std::sync::Arc;

use serde::Deserialize;
use tokio::sync::RwLock;

/// A wiki page discovered from the filesystem.
#[derive(Debug, Clone)]
pub struct WikiPage {
    /// Relative path from the wiki root.
    pub path: String,
    /// File title from frontmatter (falls back to filename).
    pub title: String,
    /// Tags from frontmatter.
    pub tags: Vec<String>,
    /// Absolute path to the file.
    pub absolute_path: PathBuf,
}

#[derive(Debug, Deserialize, Clone)]
struct WikiFrontmatter {
    title: String,
    #[serde(default)]
    tags: Vec<String>,
}

/// Discovers wiki pages from `.agents/wikis/` directories.
pub struct WikiLoader {
    roots: Vec<PathBuf>,
    pages: Arc<RwLock<Vec<WikiPage>>>,
}

impl WikiLoader {
    /// Create a WikiLoader with no roots (for testing).
    pub fn new_empty() -> Self {
        Self {
            roots: Vec::new(),
            pages: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Replace the internal pages list (for testing).
    pub async fn set_pages(&self, pages: Vec<WikiPage>) {
        *self.pages.write().await = pages;
    }

    pub fn new(working_dir: Option<&std::path::Path>) -> Self {
        let mut roots = Vec::new();

        if let Some(home) = dirs::home_dir() {
            roots.push(home.join(".agents").join("wikis"));
        }

        if let Some(wd) = working_dir {
            roots.push(wd.join(".agent").join("wikis"));
        }

        Self {
            roots,
            pages: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Add a custom wiki root.
    pub fn add_root(&mut self, path: PathBuf) {
        self.roots.push(path);
    }

    /// Discover all wiki pages from registered roots.
    pub async fn discover_all(&self) -> Result<(), String> {
        let mut pages = Vec::new();

        for root in &self.roots {
            if !root.exists() || !root.is_dir() {
                continue;
            }

            match md_frontmatter::scan_dir::<WikiFrontmatter>(root).await {
                Ok(docs) => {
                    for doc in docs {
                        let path = doc.path.clone().expect("scan_dir always sets path");
                        let title = doc.frontmatter.title.clone();
                        let tags = doc.frontmatter.tags.clone();
                        pages.push(WikiPage {
                            path: path.to_string_lossy().to_string(),
                            title,
                            tags,
                            absolute_path: path,
                        });
                    }
                }
                Err(errors) => {
                    for (file_path, error) in errors {
                        tracing::warn!(path = %file_path.display(), error = %error, "Failed to parse wiki page");
                    }
                    // Still add any docs that were parsed before errors
                    // scan_dir returns Err with no docs on any error,
                    // so we need to re-parse successful ones individually
                    // For now, skip this root entirely on error — acceptable for migration
                }
            }
        }

        *self.pages.write().await = pages;
        Ok(())
    }

    /// List discovered wiki pages.
    pub async fn list_pages(&self) -> Vec<WikiPage> {
        self.pages.read().await.clone()
    }

    /// List page paths (relative paths only).
    pub async fn list_paths(&self) -> Vec<String> {
        self.pages.read().await.iter().map(|p| p.path.clone()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_empty_loader() {
        let loader = WikiLoader::new(None);
        loader.discover_all().await.unwrap();
        assert!(loader.list_pages().await.is_empty());
    }

    #[tokio::test]
    async fn test_discover_from_temp_dir() {
        let temp = tempfile::tempdir().unwrap();
        let wiki_dir = temp.path().join(".agent").join("wikis");
        std::fs::create_dir_all(&wiki_dir).unwrap();

        std::fs::write(
            wiki_dir.join("INDEX.md"),
            "---\ntitle: Index\ntags: [index]\n---\n# Wiki Index\n",
        )
        .unwrap();

        std::fs::write(
            wiki_dir.join("entities.md"),
            "---\ntitle: Entities\ntags: [entities]\n---\n# Entities\n",
        )
        .unwrap();

        let mut loader = WikiLoader::new(None);
        // Override roots to only use our test dir
        loader.roots.clear();
        loader.add_root(wiki_dir);

        loader.discover_all().await.unwrap();

        let pages = loader.list_pages().await;
        assert_eq!(pages.len(), 2);
        assert!(pages.iter().any(|p| p.title == "Index"));
        assert!(pages.iter().any(|p| p.title == "Entities"));
    }

    #[tokio::test]
    async fn test_discover_no_directory() {
        let loader = WikiLoader::new(None);
        let result = loader.discover_all().await;
        assert!(result.is_ok());
        assert!(loader.list_pages().await.is_empty());
    }
}
```

**Important:** The `scan_dir` return type is `Result<Vec<ParsedDoc<T>>, Vec<(PathBuf, MdFmError)>>`. On any error, it returns `Err` with no docs. For the wiki loader, a better approach is to handle per-file errors by iterating results and collecting individually. However, since `scan_dir` already does this internally and returns `Err(errors)` when ANY file fails, the simplest migration is: on `Err`, log the errors and skip the root. This matches the current behavior where `walk_dir` silently ignores errors.

Actually, let me reconsider. The current `walk_dir` reads every .md file individually — one bad file doesn't skip others. Let me use a different approach: iterate files manually using `glob`, then call `md_frontmatter::from_path` per file for per-file error handling. This preserves the current "one bad file doesn't break the rest" behavior.

Let me revise the `discover_all` implementation:

```rust
    /// Discover all wiki pages from registered roots.
    pub async fn discover_all(&self) -> Result<(), String> {
        let mut pages = Vec::new();

        for root in &self.roots {
            if !root.exists() || !root.is_dir() {
                continue;
            }

            let pattern = root.join("**/*.md");
            let pattern_str = pattern.to_string_lossy();

            let entries = glob::glob(&pattern_str)
                .map_err(|e| format!("Invalid glob pattern: {e}"))?;

            for entry in entries.flatten() {
                match md_frontmatter::from_path::<WikiFrontmatter>(&entry).await {
                    Ok(doc) => {
                        let title = doc.frontmatter.title.clone();
                        let tags = doc.frontmatter.tags.clone();
                        pages.push(WikiPage {
                            path: entry.to_string_lossy().to_string(),
                            title,
                            tags,
                            absolute_path: entry,
                        });
                    }
                    Err(e) => {
                        tracing::warn!(path = %entry.display(), error = %e, "Failed to parse wiki page, skipping");
                    }
                }
            }
        }

        *self.pages.write().await = pages;
        Ok(())
    }
```

This is the correct version — per-file error handling, one bad file doesn't skip others.

- [ ] **Step 2: Update Cargo.toml**

Add `md-frontmatter` and `glob` to `crates/vol-llm-wiki/Cargo.toml`:

```toml
[package]
name = "vol-llm-wiki"
version.workspace = true
edition.workspace = true

[dependencies]
vol-llm-agent = { path = "../vol-llm-agent" }
vol-llm-context = { path = "../vol-llm-context" }
vol-llm-core = { path = "../vol-llm-core" }
vol-llm-tool = { path = "../vol-llm-tool" }
vol-llm-tools-builtin = { path = "../vol-llm-tools-builtin" }
vol-session = { path = "../vol-session" }
vol-config = { path = "../vol-config" }
vol-llm-provider = { path = "../vol-llm-provider" }
tokio = { workspace = true, features = ["full"] }
async-trait = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
tracing = { workspace = true }
uuid = { workspace = true }
chrono = { workspace = true }
thiserror = "1.0"
dirs = "5.0"
md-frontmatter = { workspace = true }
glob = "0.3"

[dev-dependencies]
tempfile = "3"
```

- [ ] **Step 3: Verify compilation and tests**

```bash
cargo check -p vol-llm-wiki
```
Expected: Compiles with no errors.

```bash
cargo test -p vol-llm-wiki
```
Expected: All 3 loader tests pass. The `parse_frontmatter_value` tests are gone (replaced by md-frontmatter tests).

```bash
cargo clippy -p vol-llm-wiki -- -D warnings
```
Expected: No warnings.

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-wiki/src/loader.rs crates/vol-llm-wiki/Cargo.toml
git commit -m "refactor(vol-llm-wiki): replace manual string parsing with md-frontmatter crate"
```

---

### Task 3: Workspace Verification

**Files:**
- Modify: `Cargo.toml` (workspace root — already has `md-frontmatter` registered)

- [ ] **Step 1: Verify workspace builds**

```bash
cargo check --workspace
```
Expected: No errors (existing warnings in other crates are fine).

- [ ] **Step 2: Run all tests for both migrated crates**

```bash
cargo test -p vol-llm-skill -p vol-llm-wiki
```
Expected: All tests pass (7 total: 4 from vol-llm-skill + 3 from vol-llm-wiki).

- [ ] **Step 3: Run clippy on workspace**

```bash
cargo clippy --workspace -- -D warnings 2>&1 | grep -E "(md-frontmatter|vol-llm-skill|vol-llm-wiki)" || echo "No issues in migrated crates"
```
Expected: No warnings from the migrated crates.

- [ ] **Step 4: Commit any remaining changes**

```bash
git add -A
git commit -m "chore: verify md-frontmatter migration across workspace"
```

---

## Self-Review

### Spec Coverage Check

| Spec Requirement | Task |
|-----------------|------|
| Delete `parser.rs` entirely (Approach C) | Task 1, Step 4 |
| Replace `parse_skill_content` with `md_frontmatter::parse` | Task 1, Step 1 |
| Move `scan_skill_files` inline to `loader.rs` | Task 1, Step 1 |
| Strict error handling (skip on error, no defaults) | Task 1, Step 1 |
| Replace `walk_dir` + `parse_frontmatter_value` with `scan_dir`/`from_path` | Task 2, Step 1 |
| Per-file error handling (one bad file doesn't skip others) | Task 2, Step 1 (uses `from_path` per file) |
| Add `md-frontmatter` dependency to both crates | Task 1 Step 3, Task 2 Step 2 |
| Remove `serde_yaml` from vol-llm-skill | Task 1, Step 3 |
| Tests pass after migration | Task 1 Step 5, Task 2 Step 3, Task 3 Step 2 |
| Public APIs unchanged | Task 1 Step 1 (SkillLoader same), Task 2 Step 1 (WikiLoader same) |

### Placeholder Scan
No "TBD", "TODO", "handle edge cases", "add validation", or "write tests for the above". Every step has actual code.

### Type Consistency
- `SkillFrontmatter` in Task 1 matches the old struct from `parser.rs` exactly
- `WikiFrontmatter` in Task 2 has `title: String` and `tags: Vec<String>` with `#[serde(default)]` on tags
- `md_frontmatter::parse::<T>` returns `Result<ParsedDoc<T>, MdFmError>` — callers extract `.frontmatter`
- `md_frontmatter::from_path::<T>` returns `Result<ParsedDoc<T>, MdFmError>` — callers extract `.frontmatter`
- `doc_to_body` helper in Task 1 extracts body from content (needed because we no longer get it from `ParsedDoc` — we parse frontmatter separately from content)
- All function signatures on `SkillLoader` and `WikiLoader` match existing code

### No Issues Found

Plan complete and saved to `docs/superpowers/plans/2026-05-01-md-frontmatter-migration-plan.md`. Two execution options:

**1. Subagent-Driven (recommended)** - I dispatch a fresh subagent per task, review between tasks, fast iteration

**2. Inline Execution** - Execute tasks in this session using executing-plans, batch execution with checkpoints

Which approach?
