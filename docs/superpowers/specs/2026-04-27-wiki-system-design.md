# Wiki System Design

## Overview

Build `vol-llm-wiki` — a new crate that provides LLM-powered wiki compression and management. Wiki pages live in `.agents/wikis/` with progressive loading (index + directory injected, model reads pages on demand via `read` tool). `WikiAgent` analyzes session conversations and creates/updates wiki pages.

## Architecture

### Crate Structure

```
crates/vol-llm-wiki/
├── src/
│   ├── lib.rs           # Public types: WikiLoader, WikiInjector, WikiAgent
│   ├── loader.rs        # Scan .agents/wikis/ roots, build page listing
│   ├── injector.rs      # WikiInjector: ContextContributor for system prompt injection
│   ├── agent.rs         # WikiAgent: ReActAgent-based compression agent
│   ├── config.rs        # WikiAgentConfig
│   └── error.rs         # WikiAgentError
```

### Wiki Loading (mirrors Skills pattern)

**`WikiLoader`** discovers wiki pages from multiple roots:
- `~/.agents/wikis/` — user-level wiki
- `{working_dir}/.agents/wikis/` — project-level wiki

Scans all `.md` files, builds a flat list of `WikiPage` (path, title, frontmatter).

**`WikiInjector`** implements `ContextContributor`, injects into system prompt:
```
# Wiki

Available pages:
- INDEX.md: Main index and directory
- entities.md: Known entities and concepts
- decisions.md: Project decisions

Use the `read` tool to load any page. Use `write`/`edit` to update.
```

### WikiAgent

Built on ReActAgent with tools: `read`, `write`, `edit`, `bash`, `glob`, `grep`.

```rust
pub struct WikiAgent {
    config: WikiAgentConfig,
    llm: Arc<dyn LLMClient>,
    tool_registry: Arc<ToolRegistry>,
    context_builder: ContextBuilder,
}
```

**Input:** Session messages (`Vec<SessionMessage>`)
**Process:** ReActAgent analyzes messages, extracts entities/decisions/tasks, creates/updates wiki pages, maintains `INDEX.md`
**Output:** `WikiCompressResult { pages_created, pages_updated, summary }`

### System Prompt

WikiAgent's prompt instructs it to:
1. Analyze the provided conversation
2. Extract entities, concepts, decisions, TODOs
3. Create or update wiki pages in `.agents/wikis/`
4. Keep `INDEX.md` current
5. Pages use frontmatter (`title`, `tags`, `updated_at`) and link via relative paths

### Page Format

```markdown
---
title: Project Entities
tags: [entities, concepts]
updated_at: 2026-04-27
---

# Project Entities

## DeribitClient
Deribit WebSocket client for market data. See [architecture](projects/deribit.md).
```

## Files Changed

| File | Change |
|------|--------|
| `crates/vol-llm-wiki/` | **New crate** — full implementation |
| `crates/vol-llm-wiki/Cargo.toml` | Dependencies: vol-llm-agent, vol-llm-context, vol-llm-tool, vol-llm-core, vol-session, vol-config |
| `crates/vol-llm-agents/Cargo.toml` | Add `vol-llm-wiki` dependency |
| `crates/vol-llm-agents/src/lib.rs` | Add `pub mod wiki` |
| `Cargo.lock` | Updated by build |

## Key Decisions

- **No dedicated `wiki` tool** — WikiAgent uses existing `read`/`write`/`edit` tools. The wiki content is injected as context (index + directory listing), and the model navigates via standard tools.
- **Progressive loading mirrors skills** — Same pattern: loader discovers, injector injects summary, model loads full content on demand.
- **WikiAgent is a ReActAgent** — Reuses `ReActAgent` infrastructure (tool registry, sandbox, plugin system). No custom agent loop.
- **Wiki pages are flat Markdown** — No subdirectory nesting in v1. All `.md` files in `.agents/wikis/` root. `INDEX.md` provides virtual hierarchy.

## Error Handling

- `WikiLoader` silently skips unreadable files/directories (matching `SkillLoader` behavior)
- `WikiAgent` returns error if LLM call fails; wiki files are not modified on failure
- Missing `.agents/wikis/` directory is created on first write by WikiAgent

## Testing

- Unit tests for `WikiLoader` (temp directory with markdown files)
- Unit tests for `WikiInjector` (format_metadata output)
- Integration tests: load real session JSONL files, run `WikiAgent.compress()`, verify wiki pages are created
