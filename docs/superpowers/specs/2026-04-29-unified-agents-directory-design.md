# Unified `.agents/` Directory Convention

**Date**: 2026-04-29
**Status**: Draft
**Author**: Claude Code

## Goal

Consolidate all agent-related files (skills, wikis, YAML agents, schema) under a single `.agents/` directory, eliminating the inconsistency between `.agents/` (used by vol-llm-skill) and `.agents/` (used by vol-llm-wiki, vol-llm-yaml-agent).

## Directory Structure

### Project-level (`{working_dir}/.agents/`)

```
{working_dir}/.agents/
├── skills/              # vol-llm-skill discovery root
│   └── <skill-name>/SKILL.md
├── wikis/               # vol-llm-wiki discovery root
│   ├── INDEX.md
│   ├── LOG.md
│   ├── entities/
│   ├── concepts/
│   ├── sources/
│   └── synthesis.md
├── agents/              # YAML agent definitions
│   └── <name>.yaml
└── schema.md            # Wiki/skill conventions (shared)
```

### User-level (`~/.agents/`)

```
~/.agents/
├── wikis/               # User-level wiki pages
│   └── ...
├── agents/              # User-level YAML agent definitions
│   └── ...
└── schema.md            # User-level conventions

# Skills remain in ~/.claude/skills/ (Claude Code native)
```

## Key Decisions

- **`.agents/` is plural** — matches the existing skills convention in vol-llm-skill
- **Flat subdirectories** — each subsystem gets a direct child of `.agents/`
- **`schema.md` at root** — shared conventions for wikis and skills, not wiki-specific
- **`INDEX.md`/`LOG.md` inside `wikis/`** — wiki-internal, not at `.agents/` root
- **Discovery order** — user-level `~/.agents/<type>/` first, repo-level `{working_dir}/.agents/<type>/` second. First-loaded wins for name conflicts (existing vol-llm-skill behavior)
- **No backward compatibility** — all `.agents/` references renamed atomically

## Code Changes

### Path Updates (`.agents/` → `.agents/`)

| Crate | File | Path Change |
|-------|------|-------------|
| `vol-llm-wiki` | `src/config.rs` | `.agents/wikis/` → `.agents/wikis/` |
| `vol-llm-wiki` | `src/loader.rs` | `.agents/wikis/` → `.agents/wikis/` |
| `vol-llm-wiki` | `src/injector.rs` | `.agents/wikis/` → `.agents/wikis/` |
| `vol-llm-wiki` | `src/lib.rs` | `.agents/wikis/` → `.agents/wikis/` |
| `vol-llm-yaml-agent` | `src/lib.rs` | `.agents/agents/` → `.agents/agents/` |
| `vol-llm-yaml-agent` | `src/discovery.rs` | `.agents/agents/` → `.agents/agents/` |

Plus test files (~5 files) and documentation (~10 files).

### No Change Required

- `vol-llm-skill` — already uses `.agents/skills/`
- `vol-llm-agents` tests — already reference `.agents/skills/`
- CLAUDE.md — uses `.agents/skills/` paths

## Migration Plan

**Phase 1: Update code references**
- Replace `.agents/wikis/` → `.agents/wikis/` across `crates/`
- Replace `.agents/agents/` → `.agents/agents/` across `crates/`
- Update doc comments in affected files
- Run `cargo check --workspace` to verify compilation

**Phase 2: Move existing files**
- Move `.agents/wikis/*` → `.agents/wikis/`
- Move any `.agents/agents/*.yaml` → `.agents/agents/`
- Remove empty `.agents/` directory
- Update wiki INDEX.md to reflect new paths if needed

**Phase 3: Update specs and plans**
- Update spec/plans referencing `.agents/` paths (grep in `docs/superpowers/`)
- Update wiki pages if they reference old paths

## Out of Scope

- Session directories (remain in `temp/.vol-sessions/` or current location)
- Log directories (remain in `crates/*/logs/` or current location)
- `.claude/` directory (Claude Code tool-specific, not affected)
- `~/.claude/skills/` (user-level skills remain here)
