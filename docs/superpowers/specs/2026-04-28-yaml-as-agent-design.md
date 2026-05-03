# YAML as Agent — Spec

## Goal

Allow users to define a ReActAgent instance purely via a YAML template file. Parsing the YAML produces a fully configured `ReActAgent`, ready to `run()`.

## YAML Format

```yaml
# .agents/agents/<name>.yaml
name: coding              # Agent identifier
llm: anthropic-main       # References llm_provider_id
max_iterations: 20
max_history_messages: 20

# System prompt: inline string
system: "You are an expert coding assistant."
# System prompt: load from files (content appended after inline system)
system_files:
  - .agents/AGENT.md
  - .agents/INSTRUCTION.md

# Tools by name
tools:
  - read
  - write
  - edit
  - glob
  - grep
  - bash

# Tool parameter configs (optional, keyed by tool name)
tool_configs:
  web_search:
    provider: tavily
    api_key: "${TAVILY_API_KEY}"

# Plugins by name
plugins:
  - logger

# Working directory (optional, default ".")
working_dir: "."
```

## Directory Convention

YAML files live in `{working_dir}/.agents/agents/<name>.yaml`.

The loader discovers all YAML files in this directory at startup.

## Core Types

```rust
/// Parsed YAML config
struct YamlAgentConfig {
    name: String,
    llm: String,                       // llm_provider_id
    max_iterations: Option<u32>,
    max_history_messages: Option<usize>,
    system: Option<String>,            // Inline system prompt
    system_files: Option<Vec<String>>, // File paths to load
    tools: Vec<String>,                // Tool name strings
    tool_configs: Option<ToolConfigs>, // Per-tool parameter configs
    plugins: Option<Vec<String>>,      // Plugin name strings
    working_dir: Option<PathBuf>,
}

/// Tool parameter configs (opaque, forwarded to tools)
#[derive(Debug, Deserialize)]
struct ToolConfigs(HashMap<String, serde_yaml::Value>);
```

All derive `Deserialize`. `serde_yaml` is used directly — no custom parsing.

## Build Flow

```
YAML file → serde_yaml::from_str → YamlAgentConfig
  → YamlAgentBuilder::from_config(config, llm_registry)
    → ReActAgent
```

### Step-by-step

1. **LLM resolution** — Look up `config.llm` in the `LLMProviderRegistry`
2. **Tool registration** — Create `ToolRegistry`, register tools by name from `vol_llm_tools_builtin`
3. **Tool configuration** — If `tool_configs` present, apply per-tool configs via tool-specific setters
4. **Context building**:
   - `config.system` → `SimpleContributor::system(inline)`
   - `config.system_files` → Read each file, concatenate content → `SimpleContributor::system(files_content)`
   - Both contributors added in order (inline first, then files)
5. **Plugin registration** — Create plugins by name, register into `PluginRegistry`
6. **Agent assembly** — Assemble `AgentConfig` + `Session` → `ReActAgent`

## Supported Tool Names

| Name | Tool | Has Config |
|------|------|------------|
| `read` | `ReadTool` | No |
| `write` | `WriteTool` | No |
| `edit` | `EditTool` | No |
| `glob` | `GlobTool` | No |
| `grep` | `GrepTool` | No |
| `bash` | `BashTool` | No |
| `web_search` | `WebSearchTool` | Yes (provider, api_key) |
| `web_fetch` | `WebFetchTool` | Yes (max_content_length, proxy) |

## Supported Plugin Names

| Name | Plugin | Description |
|------|--------|-------------|
| `logger` | `LoggerPlugin` | JSONL event logs to `store_dir/logs/` |

## Error Handling

- Missing required field (`name`, `llm`) → `YamlAgentError::MissingField`
- Unknown tool name → `YamlAgentError::UnknownTool(name)`
- Unknown plugin name → `YamlAgentError::UnknownPlugin(name)`
- LLM not found in registry → `YamlAgentError::LlmNotFound(id)`
- System file not found → warning logged, file skipped

## Crate Placement

New crate: `vol-llm-yaml-agent` (or add to existing `vol-llm-agents`).

Dependencies: `serde`, `serde_yaml`, `vol-llm-core`, `vol-llm-provider`, `vol-llm-agent`, `vol-llm-tools-builtin`, `vol-llm-observability`.

## API

```rust
pub struct YamlAgentBuilder {
    config: YamlAgentConfig,
    llm_registry: LLMProviderRegistry,
}

impl YamlAgentBuilder {
    /// Load YAML from file path
    pub fn from_file(path: &Path) -> Result<Self, YamlAgentError>;

    /// Load YAML from string
    pub fn from_yaml(yaml: &str) -> Result<Self, YamlAgentError>;

    /// Build the ReActAgent
    pub fn build(self) -> Result<ReActAgent, YamlAgentError>;
}
```

## Example Usage

```rust
// Discover and build
let agents_dir = working_dir.join(".agent").join("agents");
for entry in std::fs::read_dir(&agents_dir)? {
    let path = entry?.path();
    if path.extension() == Some("yaml") {
        let agent = YamlAgentBuilder::from_file(&path)?.build()?;
        let response = agent.run("Hello!").await?;
        println!("{}", response.content);
    }
}
```
