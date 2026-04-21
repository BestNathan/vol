# vol-llm-skill Package Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Create `vol-llm-skill` crate providing skill definition, discovery, loading, and a SkillTool that agents can invoke to load skill instructions on demand.

**Architecture:** File-based SKILL.md discovery from `.agents/skills/` directories with progressive disclosure. Skills are read-only prompt content loaded via a `skill` tool. SkillInjector prepends metadata to system prompt before agent loop.

**Tech Stack:** async-trait, tokio, serde, serde_yaml, thiserror, vol-llm-tool, vol-llm-core

---

## File Structure

| File | Action | Purpose |
|------|--------|---------|
| `crates/vol-llm-skill/Cargo.toml` | Create | Crate manifest |
| `crates/vol-llm-skill/src/lib.rs` | Create | Re-exports, error type |
| `crates/vol-llm-skill/src/def.rs` | Create | SkillDef, SkillScope, SkillMetadata |
| `crates/vol-llm-skill/src/parser.rs` | Create | SKILL.md frontmatter + body parser |
| `crates/vol-llm-skill/src/loader.rs` | Create | SkillLoader (discover, cache, register) |
| `crates/vol-llm-skill/src/tool.rs` | Create | SkillTool (ExecutableTool impl) |
| `crates/vol-llm-skill/src/injector.rs` | Create | SkillInjector (prompt prepender) |
| `crates/vol-llm-skill/tests/skill_test.rs` | Create | Integration tests |
| `Cargo.toml` (root) | Modify | Add workspace member + deps |

---

### Task 1: Create vol-llm-skill Crate Skeleton

**Files:**
- Create: `crates/vol-llm-skill/Cargo.toml`
- Create: `crates/vol-llm-skill/src/lib.rs`
- Create: `crates/vol-llm-skill/src/def.rs` (empty)
- Create: `crates/vol-llm-skill/src/parser.rs` (empty)
- Create: `crates/vol-llm-skill/src/loader.rs` (empty)
- Create: `crates/vol-llm-skill/src/tool.rs` (empty)
- Create: `crates/vol-llm-skill/src/injector.rs` (empty)
- Modify: `Cargo.toml` (workspace root)

- [ ] **Step 1: Create Cargo.toml**

```toml
[package]
name = "vol-llm-skill"
version.workspace = true
edition.workspace = true

[dependencies]
async-trait = { workspace = true }
tokio = { workspace = true }
serde = { workspace = true }
thiserror = { workspace = true }
tracing = { workspace = true }
serde_yaml = "0.9"
vol-llm-tool = { workspace = true }
vol-llm-core = { workspace = true }

[dev-dependencies]
tempfile = "3"
serde_json = { workspace = true }
```

- [ ] **Step 2: Create lib.rs**

```rust
//! vol-llm-skill: Skill definition, discovery, loading, and invocation for LLM agents.
//!
//! # Architecture
//!
//! File-based SKILL.md discovery from `.agents/skills/` directories with progressive disclosure.
//! Skills are read-only prompt content loaded via a `skill` tool.
//!
//! # Quick Start
//!
//! ```rust
//! use vol_llm_skill::{SkillLoader, SkillTool, SkillInjector};
//!
//! #[tokio::main]
//! async fn main() {
//!     let mut loader = SkillLoader::new(None);
//!     loader.discover_all().await.unwrap();
//!
//!     let tool = SkillTool::new(std::sync::Arc::new(loader));
//!     // Register tool in ToolRegistry
//! }
//! ```

mod def;
mod injector;
mod loader;
mod parser;
mod tool;

pub use def::{SkillDef, SkillMetadata, SkillScope};
pub use injector::SkillInjector;
pub use loader::SkillLoader;
pub use tool::SkillTool;

/// Result type for skill operations
pub type Result<T> = std::result::Result<T, SkillError>;

/// Error type for skill operations
#[derive(Debug, thiserror::Error)]
pub enum SkillError {
    #[error("Skill not found: {0}")]
    NotFound(String),
    #[error("Discovery error: {0}")]
    Discovery(String),
    #[error("Parse error: {0}")]
    Parse(String),
}
```

- [ ] **Step 3: Add workspace member and dependency**

Add `"crates/vol-llm-skill"` to `members` array in root `Cargo.toml`.

Add to `[workspace.dependencies]`:
```toml
vol-llm-skill = { path = "crates/vol-llm-skill" }
```

- [ ] **Step 4: Verify compilation**

Run: `cargo check -p vol-llm-skill`
Expected: Compiles with warnings for unused imports/types (will fill modules in subsequent tasks).

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-skill/ Cargo.toml
git commit -m "feat: create vol-llm-skill crate skeleton"
```

---

### Task 2: Implement SkillDef, SkillScope, SkillMetadata

**Files:**
- Modify: `crates/vol-llm-skill/src/def.rs`

- [ ] **Step 1: Write def.rs**

```rust
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Discovery scope for skills.
#[derive(Debug, Clone)]
pub enum SkillScope {
    /// ~/.agents/skills/ — user personal skills
    User,
    /// {working_dir}/.agents/skills/ — project-specific skills
    Repo,
    /// Custom path registered by caller (e.g., plugin-packaged skills)
    Custom(PathBuf),
}

impl SkillScope {
    /// Returns the scope prefix string for skill IDs.
    pub fn prefix(&self) -> String {
        match self {
            SkillScope::User => "user".to_string(),
            SkillScope::Repo => "repo".to_string(),
            SkillScope::Custom(path) => format!("custom:{}", path.display()),
        }
    }
}

impl std::fmt::Display for SkillScope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SkillScope::User => write!(f, "User"),
            SkillScope::Repo => write!(f, "Repo"),
            SkillScope::Custom(path) => write!(f, "Custom({})", path.display()),
        }
    }
}

/// A skill definition loaded from a SKILL.md file or registered directly.
#[derive(Debug, Clone)]
pub struct SkillDef {
    /// Unique ID: "{scope_prefix}:{name}" e.g., "user:rust-conventions"
    pub id: String,
    /// Skill name from frontmatter
    pub name: String,
    /// Version from frontmatter
    pub version: String,
    /// Description from frontmatter
    pub description: String,
    /// Discovery scope
    pub scope: SkillScope,
    /// Trigger keywords for implicit matching
    pub triggers: Vec<String>,
    /// SKILL.md markdown body (after frontmatter)
    pub content: String,
    /// Relative file paths within the skill directory
    pub file_listing: Vec<String>,
}

impl SkillDef {
    /// Create a new skill with minimal fields.
    pub fn new(name: &str, content: impl Into<String>) -> Self {
        let content_str = content.into();
        Self {
            id: format!("code:{}", name),
            name: name.to_string(),
            version: "1.0.0".to_string(),
            description: String::new(),
            scope: SkillScope::Custom(PathBuf::new()),
            triggers: Vec::new(),
            content: content_str,
            file_listing: Vec::new(),
        }
    }

    /// Set triggers for implicit matching.
    pub fn with_triggers(mut self, triggers: Vec<String>) -> Self {
        self.triggers = triggers;
        self
    }

    /// Set description.
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }

    /// Set version.
    pub fn with_version(mut self, version: impl Into<String>) -> Self {
        self.version = version.into();
        self
    }

    /// Set file listing.
    pub fn with_file_listing(mut self, files: Vec<String>) -> Self {
        self.file_listing = files;
        self
    }
}

/// Metadata for progressive disclosure (injected into system prompt).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillMetadata {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub scope: SkillScope,
    pub triggers: Vec<String>,
}

impl From<&SkillDef> for SkillMetadata {
    fn from(def: &SkillDef) -> Self {
        Self {
            id: def.id.clone(),
            name: def.name.clone(),
            version: def.version.clone(),
            description: def.description.clone(),
            scope: def.scope.clone(),
            triggers: def.triggers.clone(),
        }
    }
}
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check -p vol-llm-skill`
Expected: Compiles with warnings for unused imports (SkillDef, SkillMetadata not yet used elsewhere).

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-skill/src/def.rs
git commit -m "feat: add SkillDef, SkillScope, SkillMetadata types"
```

---

### Task 3: Implement SKILL.md Parser

**Files:**
- Modify: `crates/vol-llm-skill/src/parser.rs`

- [ ] **Step 1: Write the failing test**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_skill_with_frontmatter() {
        let content = "---
name: rust-conventions
version: 1.0.0
description: Rust coding conventions
triggers: [rust, conventions]
---

# Rust Conventions

When writing code:
- Use snake_case for functions.
";
        let result = parse_skill_content(content).unwrap();
        assert_eq!(result.name, "rust-conventions");
        assert_eq!(result.version, "1.0.0");
        assert_eq!(result.description, "Rust coding conventions");
        assert_eq!(result.triggers, vec!["rust", "conventions"]);
        assert!(result.body.contains("# Rust Conventions"));
    }

    #[test]
    fn test_parse_skill_without_frontmatter() {
        let content = "# Plain Skill\n\nJust markdown, no frontmatter.";
        let result = parse_skill_content(content).unwrap();
        assert_eq!(result.name, "default");
        assert_eq!(result.version, "1.0.0");
        assert!(result.body.contains("# Plain Skill"));
    }

    #[test]
    fn test_parse_invalid_frontmatter() {
        let content = "---\ninvalid: yaml: : :\n---\nbody";
        let result = parse_skill_content(content);
        // Should fail to parse frontmatter, treated as no frontmatter
        assert!(result.is_err() || result.unwrap().name == "default");
    }

    #[test]
    fn test_scan_files() {
        // Will be verified in integration tests with temp dirs
        let files = vec![
            "SKILL.md".to_string(),
            "scripts/format.sh".to_string(),
            "references/style.md".to_string(),
        ];
        let filtered = filter_skill_files(&files);
        assert!(filtered.contains(&"SKILL.md".to_string()));
        assert!(filtered.contains(&"scripts/format.sh".to_string()));
    }
}
```

Run: `cargo test -p vol-llm-skill parser`
Expected: FAIL — `parse_skill_content` not defined.

- [ ] **Step 2: Write parser.rs**

```rust
use std::path::Path;

use serde::Deserialize;

use crate::{Result, SkillError};

/// Parsed frontmatter from SKILL.md.
#[derive(Debug, Clone, Deserialize)]
pub struct SkillFrontmatter {
    pub name: String,
    #[serde(default = "default_version")]
    pub version: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub triggers: Vec<String>,
}

fn default_version() -> String {
    "1.0.0".to_string()
}

/// Result of parsing a SKILL.md file.
#[derive(Debug, Clone)]
pub struct ParsedSkill {
    pub name: String,
    pub version: String,
    pub description: String,
    pub triggers: Vec<String>,
    pub body: String,
}

/// Parse SKILL.md content into frontmatter + body.
///
/// If frontmatter is missing or invalid, treats entire content as body
/// with default name "default".
pub fn parse_skill_content(content: &str) -> Result<ParsedSkill> {
    // Try to find frontmatter boundaries
    let trimmed = content.trim_start();
    if !trimmed.starts_with("---") {
        return Ok(ParsedSkill {
            name: "default".to_string(),
            version: "1.0.0".to_string(),
            description: String::new(),
            triggers: Vec::new(),
            body: content.to_string(),
        });
    }

    // Find the second "---"
    let rest = &trimmed[3..]; // skip first "---"
    if let Some(end_idx) = rest.find("\n---") {
        let frontmatter_str = &rest[..end_idx];
        let body = &rest[end_idx + 4..]; // skip "\n---"

        match serde_yaml::from_str::<SkillFrontmatter>(frontmatter_str) {
            Ok(fm) => Ok(ParsedSkill {
                name: fm.name,
                version: fm.version,
                description: fm.description,
                triggers: fm.triggers,
                body: body.trim_start().to_string(),
            }),
            Err(e) => {
                tracing::warn!(error = %e, "Failed to parse SKILL.md frontmatter, treating as plain body");
                Ok(ParsedSkill {
                    name: "default".to_string(),
                    version: "1.0.0".to_string(),
                    description: String::new(),
                    triggers: Vec::new(),
                    body: content.to_string(),
                })
            }
        }
    } else {
        // No closing "---", treat as no frontmatter
        Ok(ParsedSkill {
            name: "default".to_string(),
            version: "1.0.0".to_string(),
            description: String::new(),
            triggers: Vec::new(),
            body: content.to_string(),
        })
    }
}

/// Scan a skill directory for files, returning relative paths.
///
/// Returns all files (not directories) relative to the skill root,
/// sorted alphabetically.
pub fn scan_skill_files(root: &Path) -> Vec<String> {
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

/// Keep only relevant skill files (for file_listing output).
///
/// Returns all files since the LLM may want to read any of them.
pub fn filter_skill_files(files: &[String]) -> Vec<String> {
    files.to_vec()
}

fn collect_files_recursive(path: &Path, root: &Path, files: &mut Vec<String>) {
    if path.is_file() {
        if let Ok(rel) = path.strip_prefix(root) {
            let rel_str = rel.to_string_lossy().to_string();
            files.push(rel_str);
        }
    } else if path.is_dir() {
        if let Ok(entries) = std::fs::read_dir(path) {
            for entry in entries.flatten() {
                collect_files_recursive(&entry.path(), root, files);
            }
        }
    }
}
```

- [ ] **Step 3: Run tests to verify they pass**

Run: `cargo test -p vol-llm-skill parser`
Expected: All 4 tests pass.

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-skill/src/parser.rs
git commit -m "feat: add SKILL.md frontmatter parser and file scanner"
```

---

### Task 4: Implement SkillLoader

**Files:**
- Modify: `crates/vol-llm-skill/src/loader.rs`
- Modify: `crates/vol-llm-skill/src/def.rs` (add `use serde::{Serialize, Deserialize};` to SkillScope)

- [ ] **Step 1: Make SkillScope serializable**

Add `#[derive(Serialize, Deserialize)]` to `SkillScope` in `def.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SkillScope {
    User,
    Repo,
    Custom(PathBuf),
}
```

- [ ] **Step 2: Write the failing test**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[tokio::test]
    async fn test_discover_skills_from_temp_dir() {
        let temp_dir = tempfile::tempdir().unwrap();
        let skills_dir = temp_dir.path().join(".agents").join("skills");
        std::fs::create_dir_all(&skills_dir).unwrap();

        // Create a valid skill
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

        // Create an invalid skill (no SKILL.md)
        let invalid_dir = skills_dir.join("invalid-skill");
        std::fs::create_dir_all(&invalid_dir).unwrap();

        let mut loader = SkillLoader::new(None);
        loader.add_root(SkillScope::User, skills_dir.clone());
        loader.discover_all().await.unwrap();

        let metadata = loader.list_metadata();
        assert_eq!(metadata.len(), 1);
        assert_eq!(metadata[0].name, "rust-conventions");

        let skill = loader.get("rust-conventions");
        assert!(skill.is_some());
        assert!(skill.unwrap().content.contains("# Rust Conventions"));
    }

    #[tokio::test]
    async fn test_discover_empty_root() {
        let temp_dir = tempfile::tempdir().unwrap();
        let mut loader = SkillLoader::new(None);
        loader.add_root(SkillScope::User, temp_dir.path().join("nonexistent"));
        let result = loader.discover_all().await;
        assert!(result.is_ok());
        assert!(loader.list_metadata().is_empty());
    }

    #[test]
    fn test_get_by_trigger() {
        let mut loader = SkillLoader::new(None);
        let mut skill = SkillDef::new("test-skill", "some content")
            .with_triggers(vec!["rust".to_string(), "coding".to_string()]);
        skill.id = "code:test-skill".to_string();
        loader.register(skill);

        let results = loader.get_by_trigger("rust");
        assert_eq!(results.len(), 1);

        let no_results = loader.get_by_trigger("python");
        assert_eq!(no_results.len(), 0);
    }

    #[test]
    fn test_register_overwrites() {
        let mut loader = SkillLoader::new(None);
        let mut skill1 = SkillDef::new("dup", "content1");
        skill1.id = "user:dup".to_string();
        loader.register(skill1);

        let mut skill2 = SkillDef::new("dup", "content2");
        skill2.id = "repo:dup".to_string();
        loader.register(skill2);

        // Later registration overwrites (last wins)
        let skill = loader.get("dup").unwrap();
        assert_eq!(skill.content, "content2");
    }
}
```

Run: `cargo test -p vol-llm-skill loader`
Expected: FAIL — `SkillLoader` not defined.

- [ ] **Step 3: Write loader.rs**

```rust
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::RwLock;

use crate::def::{SkillDef, SkillMetadata, SkillScope};
use crate::parser::{parse_skill_content, scan_skill_files};
use crate::{Result, SkillError};

/// Discovers, loads, and caches skills from registered roots.
pub struct SkillLoader {
    roots: Vec<(SkillScope, PathBuf)>,
    skills: Arc<RwLock<HashMap<String, Arc<SkillDef>>>>,
    metadata_cache: Arc<RwLock<Vec<SkillMetadata>>>,
}

impl SkillLoader {
    /// Creates a loader with default roots.
    ///
    /// Default roots:
    /// - User: `~/.agents/skills/`
    /// - Repo: `{working_dir}/.agents/skills/` (if working_dir provided)
    pub fn new(working_dir: Option<PathBuf>) -> Self {
        let mut loader = Self {
            roots: Vec::new(),
            skills: Arc::new(RwLock::new(HashMap::new())),
            metadata_cache: Arc::new(RwLock::new(Vec::new())),
        };

        // User root
        if let Some(home) = dirs::home_dir() {
            let user_root = home.join(".agents").join("skills");
            loader.add_root(SkillScope::User, user_root);
        }

        // Repo root
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

                let parsed = match parse_skill_content(&content) {
                    Ok(p) => p,
                    Err(e) => {
                        tracing::warn!(path = %skill_md.display(), error = %e, "Failed to parse SKILL.md, skipping");
                        continue;
                    }
                };

                let file_listing = scan_skill_files(&dir_path);
                let id = format!("{}:{}", scope.prefix(), parsed.name);

                let def = SkillDef {
                    id: id.clone(),
                    name: parsed.name.clone(),
                    version: parsed.version,
                    description: parsed.description,
                    scope: scope.clone(),
                    triggers: parsed.triggers,
                    content: parsed.body,
                    file_listing,
                };

                // First-loaded wins: don't overwrite existing
                if !skills_map.contains_key(&parsed.name) {
                    skills_map.insert(parsed.name, Arc::new(def));
                } else {
                    tracing::warn!(skill = %parsed.name, "Duplicate skill name, keeping existing");
                }
            }
        }

        // Merge into main map (discover_all can be called multiple times)
        let mut guard = self.skills.write().await;
        for (name, def) in skills_map {
            guard.insert(name, def);
        }
        drop(guard);

        // Rebuild metadata cache
        self.rebuild_metadata().await;

        Ok(())
    }

    /// List metadata for progressive disclosure.
    pub async fn list_metadata(&self) -> Vec<SkillMetadata> {
        self.metadata_cache.read().await.clone()
    }

    /// Get full skill by name.
    pub async fn get(&self, name: &str) -> Option<Arc<SkillDef>> {
        self.skills.read().await.get(name).cloned()
    }

    /// Find skills whose triggers match the query (case-insensitive keyword match).
    pub async fn get_by_trigger(&self, query: &str) -> Vec<Arc<SkillDef>> {
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

    /// Register a skill directly (code-registered).
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
```

Wait — I need to check if `dirs` crate is available. Let me add it to dependencies.

- [ ] **Step 3b: Add `dirs` dependency**

Add to `crates/vol-llm-skill/Cargo.toml`:
```toml
dirs = "5"
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p vol-llm-skill loader`
Expected: All 4 tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-skill/src/loader.rs crates/vol-llm-skill/src/def.rs
git commit -m "feat: add SkillLoader with discovery, caching, and registration"
```

---

### Task 5: Implement SkillTool

**Files:**
- Modify: `crates/vol-llm-skill/src/tool.rs`

- [ ] **Step 1: Write the failing test**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::def::{SkillDef, SkillScope};
    use std::sync::Arc;
    use vol_llm_tool::{ExecutableTool, ToolContext};

    fn make_test_loader() -> SkillLoader {
        let mut loader = SkillLoader::new(None);
        let mut skill = SkillDef::new("test-skill", "# Test Skill\n\nThis is a test.")
            .with_description("A test skill")
            .with_version("1.0.0")
            .with_triggers(vec!["test".to_string()])
            .with_file_listing(vec!["SKILL.md".to_string()]);
        skill.id = "code:test-skill".to_string();
        // We need async register, so we'll use a sync path for the tool test
        // The tool itself only needs Arc<SkillLoader>, not the register method
        // For now, construct directly via inner fields
        loader
    }

    #[tokio::test]
    async fn test_skill_tool_execute() {
        let loader = make_test_loader();
        loader.discover_all().await.unwrap();
        // We need to register the skill first
        let mut skill = SkillDef::new("test-skill", "# Test Skill\n\nThis is a test.")
            .with_description("A test skill")
            .with_file_listing(vec!["SKILL.md".to_string()]);
        skill.id = "code:test-skill".to_string();
        loader.register(skill).await;

        let tool = SkillTool::new(Arc::new(loader));
        let args = serde_json::json!({ "name": "test-skill" });
        let result = tool.execute(&args, &ToolContext::default()).await;
        assert!(result.is_ok());
        let content = result.unwrap().content;
        assert!(content.contains("=== SKILL: test-skill"));
        assert!(content.contains("# Test Skill"));
    }

    #[tokio::test]
    async fn test_skill_tool_not_found() {
        let loader = SkillLoader::new(None);
        let tool = SkillTool::new(Arc::new(loader));
        let args = serde_json::json!({ "name": "nonexistent" });
        let result = tool.execute(&args, &ToolContext::default()).await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("not found"));
        assert!(err.contains("Available skills"));
    }
}
```

Run: `cargo test -p vol-llm-skill tool`
Expected: FAIL — `SkillTool` not defined.

- [ ] **Step 2: Write tool.rs**

```rust
use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::loader::SkillLoader;
use vol_llm_tool::{ExecutableTool, ToolContext, ToolError, ToolResult, ToolResultType, ToolSensitivity};

/// Parameters for the Skill tool.
#[derive(Debug, Deserialize, Serialize)]
pub struct SkillToolParams {
    /// Skill name to load
    pub name: String,
}

/// Tool that loads skill instructions by name.
pub struct SkillTool {
    loader: Arc<SkillLoader>,
}

impl SkillTool {
    pub fn new(loader: Arc<SkillLoader>) -> Self {
        Self { loader }
    }

    /// Format a skill as tool output.
    fn format_skill_output(&self, def: &crate::def::SkillDef) -> String {
        let mut output = String::new();

        // Header with skill identity
        output.push_str(&format!("=== SKILL: {} (v{}) ===\n", def.name, def.version));

        // Skill root absolute path (from scope)
        let root_path = match &def.scope {
            crate::def::SkillScope::User => {
                dirs::home_dir().map(|p| p.join(".agents").join("skills").join(&def.name))
            }
            crate::def::SkillScope::Repo => {
                // We don't have working_dir here, skip absolute path for Repo
                None
            }
            crate::def::SkillScope::Custom(path) => Some(path.join(&def.name)),
        };

        if let Some(ref root) = root_path {
            output.push_str(&format!("Skill root: {}\n", root.display()));
        }

        // File listing
        if !def.file_listing.is_empty() {
            output.push_str("\nContents:\n");
            for file in &def.file_listing {
                output.push_str(&format!("  {}\n", file));
            }
            output.push_str("\nUse the `read` tool with absolute paths to access these files.\n");
        }

        // Separator and body
        output.push_str("\n---\n");
        output.push_str(&def.content);
        output.push_str("\n---\n");

        output.push_str("=== END SKILL ===");
        output
    }

    /// Format error with available skills list.
    async fn format_not_found(&self, name: &str) -> String {
        let metadata = self.loader.list_metadata().await;
        let mut output = format!("Skill '{}' not found.\n\n", name);
        if metadata.is_empty() {
            output.push_str("No skills available.");
        } else {
            output.push_str("Available skills:\n");
            for m in &metadata {
                output.push_str(&format!("- {}: {}\n", m.name, m.description));
            }
        }
        output.push_str("\nUse the `read` tool with absolute paths to access files relative to the skill root.");
        output
    }
}

#[async_trait]
impl ExecutableTool for SkillTool {
    fn name(&self) -> &'static str {
        "skill"
    }

    fn description(&self) -> &'static str {
        "Load a skill's full instructions by name. \
         Use the 'read' tool with absolute paths to access files relative to the skill root. \
         Available skills are listed in the system prompt."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "Name of the skill to load"
                }
            },
            "required": ["name"]
        })
    }

    fn sensitivity(&self, _args: &serde_json::Value) -> ToolSensitivity {
        ToolSensitivity::Safe
    }

    async fn execute(
        &self,
        args: &serde_json::Value,
        _context: &ToolContext,
    ) -> ToolResultType<ToolResult> {
        let params: SkillToolParams = serde_json::from_value(args.clone()).map_err(|e| {
            ToolError::InvalidArguments(format!("Failed to parse arguments: {}", e))
        })?;

        match self.loader.get(&params.name).await {
            Some(def) => {
                let content = self.format_skill_output(&def);
                Ok(ToolResult::success(content))
            }
            None => {
                let error_msg = self.format_not_found(&params.name).await;
                Err(ToolError::ExecutionFailed(error_msg))
            }
        }
    }
}
```

- [ ] **Step 3: Run tests to verify they pass**

Run: `cargo test -p vol-llm-skill tool`
Expected: All 2 tests pass.

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-skill/src/tool.rs
git commit -m "feat: add SkillTool as ExecutableTool for loading skills"
```

---

### Task 6: Implement SkillInjector

**Files:**
- Modify: `crates/vol-llm-skill/src/injector.rs`

- [ ] **Step 1: Write the failing test**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::def::SkillDef;
    use std::sync::Arc;

    #[tokio::test]
    async fn test_format_metadata_empty() {
        let loader = SkillLoader::new(None);
        let injector = SkillInjector::new(Arc::new(loader));
        let output = injector.format_metadata().await;
        assert!(output.is_empty());
    }

    #[tokio::test]
    async fn test_format_metadata_with_skills() {
        let loader = SkillLoader::new(None);
        let mut skill = SkillDef::new("rust-conventions", "# Rust")
            .with_description("Rust coding conventions")
            .with_triggers(vec!["rust".to_string()]);
        skill.id = "user:rust-conventions".to_string();
        loader.register(skill).await;

        let injector = SkillInjector::new(Arc::new(loader));
        let output = injector.format_metadata().await;

        assert!(output.contains("Available skills:"));
        assert!(output.contains("rust-conventions"));
        assert!(output.contains("Rust coding conventions"));
        assert!(output.contains("skill tool"));
    }
}
```

Run: `cargo test -p vol-llm-skill injector`
Expected: FAIL — `SkillInjector` not defined.

- [ ] **Step 2: Write injector.rs**

```rust
use std::sync::Arc;

use crate::loader::SkillLoader;

/// Formats skill metadata for system prompt injection.
pub struct SkillInjector {
    loader: Arc<SkillLoader>,
}

impl SkillInjector {
    pub fn new(loader: Arc<SkillLoader>) -> Self {
        Self { loader }
    }

    /// Format metadata as prompt string for system prompt injection.
    ///
    /// Returns empty string if no skills are available.
    pub async fn format_metadata(&self) -> String {
        let metadata = self.loader.list_metadata().await;
        if metadata.is_empty() {
            return String::new();
        }

        let mut output = String::from("Available skills:\n");
        for m in &metadata {
            output.push_str(&format!("- {}: {}\n", m.name, m.description));
        }
        output.push_str("\nUse the `skill` tool to load any skill's full instructions.");
        output
    }
}
```

- [ ] **Step 3: Run tests to verify they pass**

Run: `cargo test -p vol-llm-skill injector`
Expected: All 2 tests pass.

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-skill/src/injector.rs
git commit -m "feat: add SkillInjector for system prompt injection"
```

---

### Task 7: Full Integration Tests

**Files:**
- Create: `crates/vol-llm-skill/tests/skill_test.rs`

- [ ] **Step 1: Write skill_test.rs**

```rust
use std::io::Write;
use std::sync::Arc;

use vol_llm_skill::{SkillDef, SkillLoader, SkillMetadata, SkillScope, SkillInjector, SkillTool};
use vol_llm_tool::{ExecutableTool, ToolContext};

#[tokio::test]
async fn test_full_skill_lifecycle() {
    let temp_dir = tempfile::tempdir().unwrap();
    let skills_dir = temp_dir.path().join(".agents").join("skills");
    std::fs::create_dir_all(&skills_dir).unwrap();

    // Create a skill with files
    let rust_dir = skills_dir.join("rust-conventions");
    std::fs::create_dir_all(&rust_dir).unwrap();

    let mut f = std::fs::File::create(rust_dir.join("SKILL.md")).unwrap();
    writeln!(f, "---").unwrap();
    writeln!(f, "name: rust-conventions").unwrap();
    writeln!(f, "version: 1.0.0").unwrap();
    writeln!(f, "description: Rust coding conventions").unwrap();
    writeln!(f, "triggers: [rust, conventions]").unwrap();
    writeln!(f, "---").unwrap();
    writeln!(f, "# Rust Conventions").unwrap();
    writeln!(f, "").unwrap();
    writeln!(f, "When writing Rust code:").unwrap();
    writeln!(f, "- Use snake_case for functions.").unwrap();

    // Create a reference file
    let refs_dir = rust_dir.join("references");
    std::fs::create_dir_all(&refs_dir).unwrap();
    std::fs::write(refs_dir.join("style.md"), "# Style Guide").unwrap();

    let mut loader = SkillLoader::new(None);
    loader.add_root(SkillScope::User, skills_dir.clone());
    loader.discover_all().await.unwrap();

    // Verify discovery
    let metadata = loader.list_metadata().await;
    assert_eq!(metadata.len(), 1);
    assert_eq!(metadata[0].name, "rust-conventions");

    // Verify file listing
    let skill = loader.get("rust-conventions").await.unwrap();
    assert!(skill.file_listing.contains(&"SKILL.md".to_string()));
    assert!(skill.file_listing.contains(&"references/style.md".to_string()));

    // Verify content
    assert!(skill.content.contains("# Rust Conventions"));
    assert!(skill.content.contains("snake_case"));

    // Verify trigger matching
    let matched = loader.get_by_trigger("rust coding").await;
    assert_eq!(matched.len(), 1);

    let no_match = loader.get_by_trigger("python").await;
    assert_eq!(no_match.len(), 0);
}

#[tokio::test]
async fn test_skill_tool_loads_skill() {
    let temp_dir = tempfile::tempdir().unwrap();
    let skills_dir = temp_dir.path().join(".agents").join("skills");
    std::fs::create_dir_all(&skills_dir).unwrap();

    let test_dir = skills_dir.join("test-skill");
    std::fs::create_dir_all(&test_dir).unwrap();

    let mut f = std::fs::File::create(test_dir.join("SKILL.md")).unwrap();
    writeln!(f, "---").unwrap();
    writeln!(f, "name: test-skill").unwrap();
    writeln!(f, "version: 2.0.0").unwrap();
    writeln!(f, "description: A test skill").unwrap();
    writeln!(f, "---").unwrap();
    writeln!(f, "# Test Skill Body").unwrap();

    let mut loader = SkillLoader::new(None);
    loader.add_root(SkillScope::User, skills_dir.clone());
    loader.discover_all().await.unwrap();

    let tool = SkillTool::new(Arc::new(loader));
    let args = serde_json::json!({ "name": "test-skill" });
    let result = tool.execute(&args, &ToolContext::default()).await.unwrap();

    assert!(result.content.contains("=== SKILL: test-skill (v2.0.0)"));
    assert!(result.content.contains("# Test Skill Body"));
}

#[tokio::test]
async fn test_injector_formats_prompt() {
    let loader = SkillLoader::new(None);

    let mut skill1 = SkillDef::new("rust", "# Rust")
        .with_description("Rust conventions");
    skill1.id = "user:rust".to_string();
    loader.register(skill1).await;

    let mut skill2 = SkillDef::new("python", "# Python")
        .with_description("Python conventions");
    skill2.id = "user:python".to_string();
    loader.register(skill2).await;

    let injector = SkillInjector::new(Arc::new(loader));
    let output = injector.format_metadata().await;

    assert!(output.contains("Available skills:"));
    assert!(output.contains("rust"));
    assert!(output.contains("Python conventions"));
    assert!(output.contains("skill tool"));
}

#[tokio::test]
async fn test_mixed_file_and_code_skills() {
    let temp_dir = tempfile::tempdir().unwrap();
    let skills_dir = temp_dir.path().join(".agents").join("skills");
    std::fs::create_dir_all(&skills_dir).unwrap();

    // File-based skill
    let file_dir = skills_dir.join("file-skill");
    std::fs::create_dir_all(&file_dir).unwrap();
    let mut f = std::fs::File::create(file_dir.join("SKILL.md")).unwrap();
    writeln!(f, "---").unwrap();
    writeln!(f, "name: file-skill").unwrap();
    writeln!(f, "version: 1.0.0").unwrap();
    writeln!(f, "description: From file").unwrap();
    writeln!(f, "---").unwrap();
    writeln!(f, "# File Skill").unwrap();

    let mut loader = SkillLoader::new(None);
    loader.add_root(SkillScope::User, skills_dir.clone());
    loader.discover_all().await.unwrap();

    // Code-registered skill
    let mut code_skill = SkillDef::new("code-skill", "# Code Skill")
        .with_description("From code registration");
    code_skill.id = "code:code-skill".to_string();
    loader.register(code_skill).await;

    let metadata = loader.list_metadata().await;
    assert_eq!(metadata.len(), 2);

    // Both should be accessible
    assert!(loader.get("file-skill").await.is_some());
    assert!(loader.get("code-skill").await.is_some());
}

#[tokio::test]
async fn test_discover_non_utf8_file() {
    let temp_dir = tempfile::tempdir().unwrap();
    let skills_dir = temp_dir.path().join(".agents").join("skills");
    std::fs::create_dir_all(&skills_dir).unwrap();

    let bad_dir = skills_dir.join("bad-skill");
    std::fs::create_dir_all(&bad_dir).unwrap();
    // Write binary content that is not valid UTF-8
    std::fs::write(bad_dir.join("SKILL.md"), &[0xff, 0xfe, 0x00, 0x01]).unwrap();

    let mut loader = SkillLoader::new(None);
    loader.add_root(SkillScope::User, skills_dir.clone());
    // Should not fail, just skip the invalid file
    let result = loader.discover_all().await;
    assert!(result.is_ok());
    assert!(loader.list_metadata().await.is_empty());
}

#[test]
fn test_skill_scope_prefix() {
    assert_eq!(SkillScope::User.prefix(), "user");
    assert_eq!(SkillScope::Repo.prefix(), "repo");
    let custom = SkillScope::Custom(std::path::PathBuf::from("/opt/skills"));
    assert!(custom.prefix().starts_with("custom:"));
}

#[test]
fn test_skill_def_builder() {
    let skill = SkillDef::new("my-skill", "# Content")
        .with_description("My skill")
        .with_version("2.0.0")
        .with_triggers(vec!["test".to_string()])
        .with_file_listing(vec!["SKILL.md".to_string()]);

    assert_eq!(skill.name, "my-skill");
    assert_eq!(skill.content, "# Content");
    assert_eq!(skill.description, "My skill");
    assert_eq!(skill.version, "2.0.0");
    assert_eq!(skill.triggers, vec!["test"]);
    assert_eq!(skill.file_listing, vec!["SKILL.md"]);
}
```

- [ ] **Step 2: Run all tests**

Run: `cargo test -p vol-llm-skill`
Expected: All 11 tests pass (4 from parser, 4 from loader, 2 from tool, 2 from injector, minus any overlap).

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-skill/tests/skill_test.rs
git commit -m "test: add integration tests for vol-llm-skill"
```

---

### Task 8: Full Workspace Verification

- [ ] **Step 1: Full workspace check**

Run: `cargo check --workspace`
Expected: Compiles without errors.

- [ ] **Step 2: Run all workspace tests**

Run: `cargo test --workspace --lib`
Expected: All existing tests pass.

---

## Summary of Changes

| Crate | Files Changed | Purpose |
|-------|---------------|---------|
| `vol-llm-skill` | **new** | Skill system crate |
| `Cargo.toml` (root) | Modify | Add workspace member + dependency |

### vol-llm-skill Internal Structure

| File | Purpose |
|------|---------|
| `src/def.rs` | SkillDef, SkillScope, SkillMetadata |
| `src/parser.rs` | SKILL.md frontmatter parser + file scanner |
| `src/loader.rs` | SkillLoader (discover, parse, cache, register) |
| `src/tool.rs` | SkillTool (ExecutableTool impl) |
| `src/injector.rs` | SkillInjector (prompt prepender) |
| `tests/skill_test.rs` | Integration tests (11 tests) |
