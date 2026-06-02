---
type: concept
category: pattern
tags: [json-rpc, skills, backend, discovery, rpc-method]
created: 2026-05-16
updated: 2026-05-18 (mobile-ui-refinements)
source_count: 2
---

# Skills Panel JSON-RPC Pattern

**Category:** Pattern — Exposing backend skill discovery via JSON-RPC

**Related:** [[skill-system]], [[jsonrpc-transport]], [[vol-llm-agent-channel-crate]], [[vol-llm-ui-crate]], [[dioxus-web-pattern]], [[mobile-ui-refinements]]

## Overview

Pattern for exposing discovered skills through JSON-RPC methods so that frontend UI panels can populate with skill listings and fetch full details on demand.

## How It Works

Two RPC methods:

| Method | Returns |
|--------|---------|
| `skill.list` | `Vec<SkillListEntry>` — id, name, version, scope, description, triggers |
| `skill.get` | `SkillDetail` — all list fields plus content (SKILL.md body) and file_listing |

The `JsonRpcServer` holds `Option<Arc<SkillLoader>>`. When configured, handlers call `loader.list_metadata()` and `loader.get(name)`. When `None`, `skill.list` returns `[]` — panel shows "No skills discovered".

## Key Points

- Graceful degradation: unconfigured server returns empty array, not error
- Two-tier loading: lightweight list for panel, full detail on demand (lazy load)
- `SkillLoader` discovers from `.agents/skills/` directories at server startup
- Frontend fetches list on mount, detail on row click
- Dialog signal managed at App level, passed via context
- Mobile presentation uses cards (`SkillCard`) instead of the desktop table, while both call the same `skill.get` detail flow

## Related Concepts

- [[skill-system]]: Underlying skill discovery mechanism
- [[jsonrpc-transport]]: Transport carrying the RPC methods
- [[dioxus-web-pattern]]: Frontend component pattern for detail dialog
- [[mobile-ui-refinements]]: Mobile card layout for the skills list
