---
type: source
source_type: plan
date: 2026-04-26
ingested: 2026-05-04
tags: [skills, react-agent, vol-llm-skill]
---

# Skills as Native ReActAgent Capability

**Authors/Creators:** vol-monitor team
**Date:** 2026-04-26
**Link:** `docs/superpowers/plans/2026-04-26-skills-as-react-agent-native.md`

## TL;DR
Plan to move skill initialization from CodingAgent into ReActAgent as native helpers, so any agent gets skill support automatically via `SkillsConfig`.

## Key Takeaways
- `SkillsConfig` struct holds a shared `SkillLoader` and provides methods to register the `SkillTool` and enhance the `ContextBuilder` with `SkillInjector`
- `SkillsConfig::from_workdir(path)` creates a lazy skill loader (no I/O at construction)
- `SkillsConfig::register_tool(registry)` registers the SkillTool into the tool registry
- `SkillsConfig::enhance_context_builder(existing)` appends a SkillInjector to a ContextBuilder
- `AgentConfig::with_skills(self, working_dir)` convenience method for config enhancement
- CodingAgent's direct skill imports are replaced with `SkillsConfig` helpers, making skills a native ReActAgent capability
- Tech stack: Rust, vol-llm-agent, vol-llm-skill, vol-llm-context

## Detailed Summary

The plan addresses a design concern where skill initialization was tightly coupled to CodingAgent. By introducing `SkillsConfig` in the react module, skills become a first-class capability of ReActAgent itself. The `SkillsConfig` struct wraps an `Arc<SkillLoader>` and provides two key methods: `register_tool()` for tool registry integration, and `enhance_context_builder()` for context integration.

The `enhance_context_builder` method uses `ContextBuilderBuilder` to create a new builder that copies all existing contributors from the original and appends a `SkillInjector`. This allows skills to inject their context into the agent's prompt without requiring agent-specific code.

CodingAgent migration involves removing direct `vol_llm_skill` imports and replacing the manual skill loader/injector setup with `SkillsConfig::from_workdir()` calls. The function `build_tools_and_context()` in CodingAgent becomes simpler: `skills.register_tool(&mut tool_registry)` replaces the manual registration, and `skills.enhance_context_builder(&base_context)` replaces the manual context builder construction.

## Entities Mentioned
- [[vol-llm-agent-crate]]: Where SkillsConfig is defined
- [[vol-llm-agents-crate]]: CodingAgent that uses SkillsConfig

## Concepts Covered
- [[skill-system]]: Skills as native ReActAgent capability via SkillsConfig
- [[context-builder]]: How skills integrate via ContextBuilder enhancement
- [[agent-builder-pattern]]: Agent configuration with skills

## Notes
- Skills are discovered lazily by SkillLoader on first access
- The plan follows subagent-driven development workflow with 3 tasks
