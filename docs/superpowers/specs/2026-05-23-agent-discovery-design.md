# Design: Directory-Based Agent Discovery + Frontend Selection

## Summary

Replace manual `AgentDef::new()` + `register_agent()` in `jsonrpc_agent_service.rs` with `discover_agents()`. Create 3 agent definition files under `.agents/agents/`. Fix `agent.list` to return full metadata (type, description, scope). Add frontend agent selector dropdown.

## Agent definition files

Create `.agents/agents/` with 3 markdown files using YAML frontmatter + markdown body as system prompt:

```markdown
---
name: general-purpose
type: general-purpose
description: General-purpose AI assistant for conversation and task help
max_iterations: 30
---

You are a helpful AI assistant. Answer questions clearly and concisely.
```

```markdown
---
name: explore
type: explore
description: Code exploration specialist — search, grep, read, navigate codebases
tools: [read_file, glob, grep]
max_iterations: 30
---

You are a code exploration specialist. Your job is to understand and navigate
codebases. Use read_file, glob, and grep tools to search and read code.
Report findings clearly with file paths and line numbers.
```

```markdown
---
name: review
type: review
description: Code review specialist — analyze code quality, find issues, suggest improvements
tools: [read_file, glob, grep, bash]
max_iterations: 40
---

You are a code review specialist. Review code for bugs, security issues,
performance problems, and style violations. Use tools to read and understand
the code. Provide clear, actionable feedback with specific file locations.
```

## Backend

### `jsonrpc_agent_service.rs`

Replace manual registration:
```rust
// Remove:
let def = AgentDef::new("general-assistant", "...").with_type("general-assistant");
core.register_agent("general-assistant", def).await?;

// Replace with:
core.discover_agents().await?;
```

### `server_core.rs`

Store `HashMap<String, AgentDef>` alongside holders so `agent.list` can return metadata:

```rust
// New field on AgentServerCore:
agent_defs: Arc<std::sync::RwLock<HashMap<String, AgentDef>>>,
```

Populated during `discover_agents()` before registration.

### `domain/agent.rs`

Update `agent.list` handler to return type, description, scope:
```rust
// Before: { "id": k, "name": k }
// After:  { "id": k, "name": k, "type": "...", "description": "...", "scope": "..." }
```

## Frontend

### `client.rs` — `submit()` add target param
```rust
pub fn submit(&self, input: &str, target: Option<&str>) -> Result<String, String> {
    params: { "input": input, "target": target },
}
```

### `input_area.rs` + `app.rs` — agent selector
- `AgentsState` gets new field `selected: Option<String>`
- `InputArea` renders a `<select>` dropdown populated from `AgentsState.agents`
- On submit, passes selected agent id as `target`

## Files

| File | Change |
|------|--------|
| `.agents/agents/general-purpose.md` | New |
| `.agents/agents/explore.md` | New |
| `.agents/agents/review.md` | New |
| `examples/jsonrpc_agent_service.rs` | Use `discover_agents()` |
| `src/server_core.rs` | Store agent defs, update `discover_agents()` |
| `src/domain/agent.rs` | Return type/description/scope in `agent.list` |
| `src/web/client.rs` | `submit()` add optional target |
| `src/web/components/input_area.rs` | Agent selector dropdown |
| `src/web/components/app.rs` | Wire selected agent to submit |
| `src/state/mod.rs` | `AgentsState.selected` field |
