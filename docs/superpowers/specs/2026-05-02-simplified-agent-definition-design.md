# Design: Simplified Agent Definition System

## Overview

Add agent definition support to `vol-llm-agent` as three new modules: `agent_def.rs`, `agent_loader.rs`, and `agent_tool.rs`. Agents are defined as `.md` files with YAML frontmatter in `.agents/agents/` directories. An `AgentTool` allows LLMs to dispatch sub-agents by type, running a full ReAct loop and returning the result.

No AgentBus, no inter-agent communication. Pure definition → execution → return.

## Architecture

### Module Structure

```
crates/vol-llm-agent/src/
├── agent_def.rs       # AgentDef struct, AgentScope, AgentPath, frontmatter types
├── agent_loader.rs    # AgentLoader: discovery from user/repo .agents/agents/
└── agent_tool.rs      # AgentTool: ExecutableTool that spawns sub-agents
```

### Data Flow

```
User/LLM calls AgentTool(type, prompt, description)
  → AgentTool checks agent_path depth vs max_depth
  → AgentTool looks up AgentDef by type from AgentLoader
  → Creates ReActAgent via AgentBuilder with:
      - System prompt from AgentDef markdown body
      - Tools filtered by AgentDef.tools / disallowed_tools
      - max_iterations from AgentDef
      - working_dir inherited from parent
      - agent_path extended with this agent's name
  → Runs ReActAgent.run(prompt)
  → Returns final answer content
```

## Data Types

### AgentDef

```rust
pub struct AgentDef {
    pub id: String,                      // "{scope}:{name}"
    pub name: String,                    // Unique identifier
    pub r#type: String,                  // Dispatch key (defaults to name)
    pub description: String,             // Short description
    pub scope: AgentScope,               // User or Repo
    pub tools: Option<Vec<String>>,      // Allowed tools (None = inherit)
    pub disallowed_tools: Option<Vec<String>>, // Blacklisted tools
    pub model: Option<String>,           // Model override
    pub max_iterations: Option<u32>,     // Max ReAct iterations
    pub content: String,                 // Markdown body (system prompt)
}
```

### AgentScope

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AgentScope {
    User,  // ~/.agents/agents/
    Repo,  // {working_dir}/.agents/agents/
}
```

### AgentPath

```rust
#[derive(Debug, Clone)]
pub struct AgentPath {
    segments: Vec<String>,
}
```

- `AgentPath::root()` → path = `"root"`
- `.push("test-runner")` → path = `"root/test-runner"`
- `.depth()` → number of segments
- `Display` → segments joined by `/`

### Frontmatter

```yaml
---
name: test-runner
type: test-runner              # Optional, defaults to name
description: "Run tests and fix failures"
tools: [Bash, Read, Glob]
disallowed_tools: [Write]
max_iterations: 20
---

System prompt / instructions in markdown body...
```

## AgentLoader

Follows the `SkillLoader` pattern:

- `new(working_dir: Option<PathBuf>)` — registers user (`~/.agents/agents`) and repo (`{working_dir}/.agents/agents`) roots
- `discover_all()` — scans all roots, parses frontmatter, builds `HashMap<String, Arc<AgentDef>>`
- `get_by_type(type: &str)` — returns agents matching `type` field
- `list_metadata()` — returns lightweight metadata for context injection
- Repo scope overrides user scope on duplicate names (with warning)
- Uses `md_frontmatter::parse::<AgentFrontmatter>()` for strict parsing — invalid files are skipped with a warning
- `OnceCell` for lazy discovery on first access

## AgentTool

### Struct

```rust
pub struct AgentTool {
    loader: Arc<AgentLoader>,
    llm: Arc<dyn LLMClient>,
    agent_path: AgentPath,
    max_depth: u32,
    parent_tools: Arc<ToolRegistry>,
    working_dir: PathBuf,
}
```

### Parameters

```json
{
  "type": { "type": "string", "description": "Agent type to dispatch" },
  "prompt": { "type": "string", "description": "Full task instructions" },
  "description": { "type": "string", "description": "Short task description" }
}
```

### Execution Flow

1. **Depth check**: If `agent_path.depth() >= max_depth`, return error `"Dispatch depth exceeded (max {max}, path: {path})"`
2. **Lookup**: Find agents matching `type` via `loader.get_by_type()`. If none found, return error with available types list
3. **Build tool registry**:
   - If `AgentDef.tools` is `Some` → register only those tools (minus `disallowed_tools`)
   - If `AgentDef.tools` is `None` → copy `parent_tools`
   - Filter out unknown tool names with a warning
4. **Build AgentConfig**:
   - System prompt: `AgentDef.content` (or default prompt if empty)
   - `max_iterations`: `AgentDef.max_iterations.unwrap_or(5)`
   - `working_dir`: inherited from parent
   - `agent_id`: generated with prefix from agent_path
5. **Create ReActAgent** via `AgentBuilder`:
   - LLM: cloned from `self.llm`
   - Tools: the registry built in step 3
   - Session: new in-memory session (sub-agents are independent)
   - System prompt: via `with_system_prompt(AgentDef.content)`
6. **Run**: `agent.run(prompt)` → get `AgentResponse`
7. **Return**: `AgentResponse.content` to the calling LLM

### AgentPath propagation

When a sub-agent runs, its internal `AgentTool` (if registered) is created with:
```rust
AgentTool {
    agent_path: parent_path.push(agent_name),
    // ... other fields
}
```

Each dispatch level extends the path. The depth check prevents infinite recursion.

## Error Handling

### AgentDefError

```rust
#[derive(Debug, thiserror::Error)]
pub enum AgentDefError {
    #[error("Agent type '{0}' not found")]
    TypeNotFound(String),
    #[error("Dispatch depth exceeded (max {0}, path: {1})")]
    DepthExceeded(u32, String),
    #[error("Invalid agent definition: {0}")]
    InvalidDef(String),
    #[error("Loader error: {0}")]
    Loader(String),
}
```

### LLM-facing errors

- Type not found: `"Agent type 'X' not found. Available types: A, B, C."`
- Depth exceeded: `"Cannot dispatch: maximum dispatch depth (3) reached at path 'root/test-runner/debugger'."`
- No agents defined: `"No agents are defined. Create .md files in .agents/agents/ to define custom agents."`
- Agent with empty body: Uses default system prompt `"You are a specialized AI agent. Follow the instructions provided."`

## Integration

### Re-export from vol-llm-agent

```rust
// crates/vol-llm-agent/src/lib.rs
pub use agent_def::{AgentDef, AgentScope, AgentPath};
pub use agent_loader::AgentLoader;
pub use agent_tool::AgentTool;
```

### Usage in CodingAgent or TUI

```rust
let loader = Arc::new(AgentLoader::new(Some(working_dir.clone())));
let agent_tool = AgentTool::new(
    loader,
    llm.clone(),
    AgentPath::root(),
    3,                        // max_depth
    tool_registry.clone(),    // tools to inherit
    working_dir.clone(),
);
tool_registry.register(agent_tool);
```

## Testing Strategy

### Unit tests

- `AgentPath`: `root()`, `push()`, `depth()`, `Display`
- Frontmatter parsing: valid, missing fields, invalid YAML
- `AgentLoader`: discover from temp dir, empty dir, duplicate names, scope priority
- Tool filtering: `tools` whitelist, `disallowed_tools` blacklist, unknown tool filtering

### Integration test

1. Create temp dir with agent definition `.md` file
2. Create `AgentLoader` pointing to temp dir
3. Create `AgentTool` with mock LLM client
4. Call `agent_tool.execute({type: "test", prompt: "hello"})`
5. Verify ReActAgent runs and returns content
6. Verify `agent_path` was extended correctly
7. Verify depth limit works (chain 3 agents, 4th should fail)

Mock LLM client to avoid real API calls — use a stub that returns a fixed response.
