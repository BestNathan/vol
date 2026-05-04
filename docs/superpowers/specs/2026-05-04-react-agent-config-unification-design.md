# Design Spec: ReAct Agent Config Unification

## Architecture

### `AgentConfig` — Single Source of Truth

All agent configuration lives in one struct. `ReActAgent::new(config)` takes a single parameter.

```rust
pub struct AgentConfig {
    pub def: Option<AgentDef>,
    pub llm: Arc<dyn LLMClient>,
    pub tools: Arc<ToolRegistry>,
    pub session: Arc<Session>,
    pub sandbox: Option<SandboxRef>,
    pub context_builder: ContextBuilder,
    pub plugin_registry: PluginRegistry,
}
```

No more `ReActAgent::new(llm, tools, config, session)`. No more `AgentBuilder`.

### `AgentDef` — Declarative Constraints

`AgentDef` holds the agent's identity and behavioral constraints. Runtime values from `def` override defaults.

```rust
pub struct AgentDef {
    // Identity (existing)
    pub id: String,
    pub name: String,
    pub r#type: String,
    pub description: String,
    pub scope: AgentScope,

    // Tool constraints (existing)
    pub tools: Option<Vec<String>>,
    pub disallowed_tools: Option<Vec<String>>,

    // Runtime behavior (new)
    pub max_iterations: Option<u32>,
    pub max_history_messages: Option<usize>,

    // Model (existing)
    pub model: Option<String>,

    // Context (existing)
    pub content: String,
}
```

### Component Hierarchy

```
AgentFrontmatter (.md frontmatter, subset)
    ↓ parse
AgentDef (complete declarative description)
    ↓ combine with runtime components
AgentConfig (def + llm + tools + session + sandbox + context + plugins)
```

### Components

#### `AgentConfig::builder()` — Chain Builder

Replaces `AgentBuilder`. Provides `with_def()`, `with_llm()`, `with_tool()`, `with_tools()`, `with_session()`, `with_sandbox()`, `with_context_builder()`, `with_plugin()`, `with_plugin_registry()`. `build()` validates `llm` is set, creates default `session` if missing.

#### `ToolRegistry::filter()` — Runtime Tool Filtering

New method on `ToolRegistry`:

```rust
impl ToolRegistry {
    pub fn filter(
        &self,
        allowed: Option<&[&str]>,
        disallowed: Option<&[&str]>,
    ) -> Arc<Self> {
        // allowed=None → all tools; Some → whitelist
        // disallowed=Some → exclude from result
        // Both → whitelist first, then blacklist
    }
}
```

#### `ReActAgent.run()` — Apply Def at Runtime

Inside `run()`, before calling `tools.definitions()`:

```rust
let effective_tools = self.config.def.as_ref()
    .and_then(|def| {
        let allowed = def.tools.as_ref().map(|t| t.iter().map(|s| s.as_str()).collect::<Vec<_>>());
        let disallowed = def.disallowed_tools.as_ref().map(|t| t.iter().map(|s| s.as_str()).collect::<Vec<_>>());
        Some(self.config.tools.filter(allowed.as_deref(), disallowed.as_deref()))
    })
    .unwrap_or_else(|| self.config.tools.clone());

let tools_defs = effective_tools.definitions();
```

Same for `max_iterations` and `max_history_messages` — read from `def` if set, otherwise use config defaults.

## Data Flow

```
AgentConfig::builder()
    .with_def(agent_def)          // optional: .md parsed def
    .with_llm(llm_client)
    .with_tools(tool_registry)    // full set of registered tools
    .with_session(session)
    .with_sandbox(sandbox)
    .build()
        ↓
ReActAgent::new(config)
        ↓
config.run(user_input)
        ↓
Inside run(): read def.tools/disallowed_tools → filter tool registry → use filtered definitions for LLM
```

## Migration: Downstream Callers

| Caller | Current | After |
|--------|---------|-------|
| `AgentTool` | `AgentConfig { ... }; ReActAgent::new(llm, tools, config, session)` | `AgentConfig::builder().with_def(def).with_llm(llm).with_tools(tools).build()` |
| `CodingAgent` | `ReActAgent::new(llm, tools, agent_config, session)` | Same builder pattern |
| `YamlAgentBuilder` | Same 4-param constructor | Same builder pattern |
| `AdviceAgent` | `AgentBuilder::new()...` | `AgentConfig::builder()...` |
| Tests | `ReActAgent::builder()...` | `AgentConfig::builder()...` |

## Files Changed

| File | Change |
|------|--------|
| `vol-llm-agent/src/react/agent.rs` | Expand `AgentConfig`, remove `AgentBuilder` usage, add `ToolRegistry::filter()` call in `run()` |
| `vol-llm-agent/src/react/builder.rs` | Delete — replaced by `AgentConfig::builder()` |
| `vol-llm-agent/src/react/mod.rs` | Remove `AgentBuilder` re-export, add builder module inline |
| `vol-llm-agent/src/react/tests.rs` | Update to use `AgentConfig::builder()` |
| `vol-llm-agent/src/agent_tool.rs` | Update to use new `AgentConfig` |
| `vol-llm-agent/src/lib.rs` | Remove `AgentBuilder` re-export |
| `vol-llm-tool/src/registry.rs` | Add `filter()` method |
| `vol-llm-agents/src/coding/agent.rs` | Update `CodingAgent` to use new `AgentConfig` |
| `vol-llm-agents/src/advice/service.rs` | Update to use new `AgentConfig` |
| `vol-llm-yaml-agent/src/builder.rs` | Update to use new `AgentConfig` |
| `vol-llm-wiki/src/agent.rs` | Update to use new `AgentConfig` |
| All test/example files | Update builder usage |

## Error Handling

- `AgentConfig::builder().build()` returns error if `llm` is missing
- `ToolRegistry::filter()` never errors — invalid tool names are silently ignored
- `def` is `None` → no filtering, all tools available
