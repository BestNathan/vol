---
name: wiki-context-hook-design
description: Design for SessionStart hook that injects wiki index.md and wiki-query skill content into Claude Code context
type: design
---

# Wiki Context Hook Design

## Overview

Replace the current `wiki-context.sh` hook (19 lines, reads only index.md) with a complete version that injects both `docs/wiki/index.md` and `.claude/skills/wiki-query/SKILL.md` content at session start, outputting valid `hookSpecificOutput` JSON for Claude Code.

## Goal

At session start, LLM receives:
1. The full wiki index — understanding the wiki structure and available pages
2. The full wiki-query methodology — knowing how to properly retrieve, cite, and synthesize wiki content

This enables the LLM to effectively search the wiki when needed during the session.

## Architecture

### Input Files
- `docs/wiki/index.md` — wiki index/catalog (required for hook to produce output)
- `.claude/skills/wiki-query/SKILL.md` — wiki-query skill methodology (optional, skipped if missing)

### Output Format

```json
{
  "hookSpecificOutput": {
    "hookEventName": "SessionStart",
    "additionalContext": "<escaped wiki content>"
  }
}
```

### String Escaping

Use bash parameter substitution for JSON escaping (`escape_for_json` function), matching the official superpowers session-start hook pattern. No external `jq` dependency.

### Fallback Behavior

- If `index.md` doesn't exist: exit 0 silently
- If `wiki-query/SKILL.md` doesn't exist: inject only index.md with a note
- If both exist: inject both contents concatenated
