# vol-llm-skill Package Design

> **Goal:** Create `vol-llm-skill` crate providing skill definition, discovery, loading, and a SkillTool that agents can invoke to load skill instructions on demand.

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│                    vol-llm-skill                         │
│                                                          │
│  ┌────────────┐   ┌─────────────┐   ┌────────────────┐  │
│  │  SkillDef  │◀──│ SkillLoader │   │  SkillTool     │  │
│  │  (data)    │   │ (discovery  │   │ (Executable    │  │
│  │            │   │  + parse    │──▶│  Tool)         │  │
│  └────────────┘   │  + cache)   │   └────────────────┘  │
│                   └──────┬──────┘                       │
│                          │                              │
│                    FileSystem                        ReAct Agent
│                  (.agents/skills/)                    ToolRegistry
│                          │                              │
│                   ┌──────────────┐                      │
│                   │ SkillInjector│◀─────────────────────┘
│                   │ (prompt      │  inject metadata before loop
│                   │  prepender)  │
│                   └──────────────┘                      │
└─────────────────────────────────────────────────────────┘
```

## Core Design Principles

1. **File-based primary** — SKILL.md files discovered from `.agents/skills/` directories
2. **Code-based secondary** — call `loader.add_root()` to register custom directories (e.g., plugin-packaged skills)
3. **Progressive disclosure** — inject metadata list into system prompt, load full content via SkillTool
4. **Skills are read-only prompt content** — no code execution, no side effects
5. **Structured path disclosure** — SkillTool output includes absolute file listing so LLM can `read` referenced files

## SKILL.md Format

```markdown
---
name: rust-conventions
version: 1.0.0
description: Rust coding conventions for this project
triggers: ["rust", "conventions", "coding style"]
---

# Rust Conventions

When writing code in this project:
- Use snake_case for functions, PascalCase for types
- See `references/style-guide.md` for full style guide
- Run `scripts/format.sh` before committing
```

## Directory Structure

```
.agents/skills/
├── rust-conventions/
│   ├── SKILL.md              # Required
│   ├── references/           # Reference docs (optional)
│   │   └── style-guide.md
│   ├── scripts/              # Helper scripts (optional)
│   │   └── format.sh
│   └── assets/               # Templates, icons (optional)
├── tdengine-query/
│   └── SKILL.md
└── invalid-skill/            # No SKILL.md → skipped
    └── readme.md
```

## Core Types

### SkillScope

```rust
pub enum SkillScope {
    /// ~/.agents/skills/ — user personal skills
    User,
    /// {working_dir}/.agents/skills/ — project-specific skills
    Repo,
    /// Custom path registered by caller (e.g., plugin-packaged skills)
    Custom(PathBuf),
}
```

Scope prefix for skill IDs:
- `User` → `"user"`
- `Repo` → `"repo"`
- `Code` (direct register) → `"code"`
- `Custom(path)` → `"custom:{path}"` (path as-is, e.g., `"custom:/opt/skills"`)

### SkillDef

```rust
pub struct SkillDef {
    pub id: String,                    // "{scope_prefix}:{name}" e.g., "user:rust-conventions"
    pub name: String,                  // "rust-conventions"
    pub version: String,               // "1.0.0"
    pub description: String,           // "Rust coding conventions"
    pub scope: SkillScope,
    pub triggers: Vec<String>,         // ["rust", "conventions"]
    pub content: String,               // SKILL.md markdown body (after frontmatter)
    pub file_listing: Vec<String>,     // Relative paths: ["references/style-guide.md", "scripts/format.sh"]
}
```

### SkillMetadata (for progressive disclosure)

```rust
pub struct SkillMetadata {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub scope: SkillScope,
    pub triggers: Vec<String>,
}
```

### SkillLoader

```rust
pub struct SkillLoader {
    roots: Vec<(SkillScope, PathBuf)>,
    skills: RwLock<HashMap<String, Arc<SkillDef>>>,
    metadata_cache: RwLock<Vec<SkillMetadata>>,
}

impl SkillLoader {
    /// Creates loader with default roots (User: ~/.agents/skills/, Repo: {working_dir}/.agents/skills/)
    pub fn new(working_dir: Option<PathBuf>) -> Self;

    /// Add a custom discovery root
    pub fn add_root(&mut self, scope: SkillScope, path: PathBuf);

    /// Discover skills from all registered roots
    pub async fn discover_all(&self) -> Result<()>;

    /// List metadata for progressive disclosure
    pub fn list_metadata(&self) -> Vec<SkillMetadata>;

    /// Get full skill by name
    pub fn get(&self, name: &str) -> Option<&SkillDef>;

    /// Find skills whose triggers match the query (keyword match)
    pub fn get_by_trigger(&self, query: &str) -> Vec<&SkillDef>;

    /// Register a skill directly (code-registered)
    pub fn register(&mut self, skill: SkillDef);
}
```

### SkillTool

Implements `ExecutableTool`. When LLM calls `skill` tool with `{ "name": "rust-conventions" }`:

1. Look up skill in `SkillLoader`
2. Format output with structured header, file listing, and content
3. Return as `ToolResult::success()`

```rust
pub struct SkillTool {
    loader: Arc<SkillLoader>,
}

impl SkillTool {
    pub fn new(loader: Arc<SkillLoader>) -> Self;
}

impl ExecutableTool for SkillTool {
    fn name(&self) -> &'static str { "skill" }
    fn description(&self) -> &'static str {
        "Load a skill's full instructions by name. \
         Use the 'read' tool with absolute paths to access files relative to the skill root. \
         Available skills are listed in the system prompt."
    }
    fn parameters(&self) -> serde_json::Value;
    fn sensitivity(&self, _args: &serde_json::Value) -> ToolSensitivity { ToolSensitivity::Safe }
    async fn execute(&self, args: &serde_json::Value, context: &ToolContext) -> ToolResultType<ToolResult>;
}
```

SkillTool output format:

```
=== SKILL: rust-conventions (v1.0.0) ===
Skill root: /home/user/.agents/skills/rust-conventions

Contents:
  SKILL.md
  references/style-guide.md
  scripts/format.sh

Use the `read` tool with absolute paths to access these files.

---
(SKILL.md body content)

When writing code:
- See `references/style-guide.md` for full style guide
- Run `scripts/format.sh` before committing

---
=== END SKILL ===
```

### SkillInjector

Formats skill metadata for system prompt injection:

```rust
pub struct SkillInjector {
    loader: Arc<SkillLoader>,
}

impl SkillInjector {
    pub fn new(loader: Arc<SkillLoader>) -> Self;

    /// Format metadata as prompt string
    pub fn format_metadata(&self) -> String;
}
```

Output:

```
Available skills:
- rust-conventions: Rust coding conventions for this project
- tdengine-query: How to query TDengine time-series database

Use the `skill` tool to load any skill's full instructions.
```

## Discovery Logic

```
for each (scope, root_path) in roots:
    if root_path doesn't exist → skip
    for each subdirectory in root_path:
        skil_md = subdirectory / "SKILL.md"
        if not exists → skip
        read content
        parse frontmatter + body
        if parse fails → skip
        build SkillDef:
            id = "{scope.prefix()}:{frontmatter.name}"
            content = markdown body
            file_listing = scan subdirectory for files (references/, scripts/, assets/)
        insert into skills HashMap
rebuild metadata_cache
```

Priority: first-loaded wins for name conflicts. Order: User → Repo → Custom → Code-registered.

## Integration Points

### CodingAgentConfig

```rust
pub struct CodingAgentConfig {
    // ... existing fields ...
    pub skill_loader: Option<Arc<SkillLoader>>,
}
```

### AgentConfig

```rust
pub struct AgentConfig {
    // ... existing fields ...
    pub skill_loader: Option<Arc<SkillLoader>>,
}
```

### Wiring

```
CodingAgent::new(config)
  │
  ├── if let Some(loader) = config.skill_loader:
  │     loader.discover_all().await
  │
  ├── if let Some(loader) = config.skill_loader:
  │     register SkillTool::new(loader) into ToolRegistry
  │
  └── build ReActAgent

ReActAgent.run(user_input)
  │
  ├── if let Some(loader) = config.skill_loader:
  │     injector = SkillInjector::new(loader)
  │     skill_prompt = injector.format_metadata()
  │     prepend to system prompt
  │
  └── agent loop
      └── LLM may call `skill` tool → loads full instructions
```

### PromptContext Extension

Need `prepend_system_content(&mut self, content: &str)` on `PromptContext` that injects before the base template:

```
[Injected: Available skills listed above]
[Template: You are an expert coding assistant...]
[Conversation: messages...]
```

## Crate Structure

```
crates/vol-llm-skill/
├── Cargo.toml
├── src/
│   ├── lib.rs      # Re-exports
│   ├── def.rs      # SkillDef, SkillScope, SkillMetadata
│   ├── loader.rs   # SkillLoader (discover, parse, cache, register)
│   ├── tool.rs     # SkillTool (ExecutableTool impl)
│   ├── injector.rs # SkillInjector
│   └── parser.rs   # SKILL.md frontmatter + body parser
└── tests/
    └── skill_test.rs
```

## Dependencies

```toml
[package]
name = "vol-llm-skill"
version.workspace = true
edition.workspace = true

[dependencies]
async-trait = { workspace = true }
tokio = { workspace = true }
serde = { workspace = true }
serde_yaml = "0.9"
thiserror = { workspace = true }
vol-llm-tool = { workspace = true }
vol-llm-core = { workspace = true }

[dev-dependencies]
tempfile = "3"
serde_json = { workspace = true }
```

## Error Handling

- **Malformed SKILL.md** — skip with `tracing::warn!`, don't fail discovery
- **Missing frontmatter** — treat entire file as content, use directory name as skill name
- **Empty roots** — non-error, just no skills found
- **Skill not found** — SkillTool returns error listing available skill names
- **Encoding** — UTF-8 only, non-UTF-8 files skipped

## What's NOT in Scope

- File watching / hot-reload
- Embedding-based retrieval
- LLM-based skill installation
- Skill dependencies / ordering
- Remote skill fetching (git, HTTP)
- Skill version management / updates
- Implicit auto-injection based on trigger matching (future)
