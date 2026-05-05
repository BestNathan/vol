---
type: concept
category: framework
tags: [skills, agent, tool-integration]
created: 2026-05-04
updated: 2026-05-04
source_count: 2
---

# Skill System

**Category:** Agent capability framework
**Related:** [[context-builder]], [[tool-registry]], [[agent-builder-pattern]], [[skills-as-react-native]]

## Definition

A system that allows agents to discover, load, and inject skills (domain-specific capabilities) into their tool registry and prompt context. Skills are native ReActAgent capabilities via `SkillsConfig`.

## Key Points
- `SkillLoader` discovers skills lazily from the working directory [[skills-as-react-native]]
- `SkillTool` is registered into the `ToolRegistry` so the LLM can invoke skills as tools [[skills-as-react-native]]
- `SkillInjector` is a `ContextContributor` that injects skill context into the agent prompt [[skills-as-react-native]]
- `SkillsConfig` provides a unified API for skill integration into any ReActAgent [[skills-as-react-native]]
- Skills are shared across agents via `Arc<SkillLoader>` — multiple agents can use the same skill set [[skills-as-react-native]]

## How It Works

Skills are integrated at agent construction time through two channels:

1. **Tool Registry**: `SkillsConfig::register_tool()` registers a `SkillTool` into the `ToolRegistry`, making skills callable by the LLM via function calling.

2. **Context Builder**: `SkillsConfig::enhance_context_builder()` creates a new `ContextBuilder` that appends a `SkillInjector` contributor. The `SkillInjector` injects skill-specific instructions and context into the agent's system prompt.

```rust
let skills = SkillsConfig::from_workdir(&working_dir);
skills.register_tool(&mut tool_registry);
let context_builder = skills.enhance_context_builder(&base_context);
```

CodingAgent previously managed skills directly with `SkillLoader`, `SkillInjector`, and `SkillTool` imports. After the migration, it uses `SkillsConfig::from_workdir()` and the helper methods, making skills a native ReActAgent capability available to any agent type.

## Examples / Applications

- **Coding Agent**: Skills discovered from the project working directory provide coding-specific capabilities (lint, test, refactor skills)
- **Advice Agent**: Skills provide domain-specific analysis capabilities
- **Any Agent**: Since skills are native to ReActAgent, any agent type can use them via `AgentConfig::with_skills()`

## Related Concepts
- [[context-builder]]: How skills inject context into the prompt
- [[tool-registry]]: How skills register as callable tools
- [[agent-builder-pattern]]: Where skills are configured
- [[skills-as-react-native]]: The migration plan that made skills native
- [[vol-llm-agent-crate]]: Where SkillsConfig is defined
