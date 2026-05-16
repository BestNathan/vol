---
type: source
source_type: report
date: 2026-05-16
ingested: 2026-05-16
tags: [skills, json-rpc, web-ui, dioxus, backend]
---

# Skills Panel Content — Backend JSON-RPC + Web UI Detail Dialog

**Authors/Creators:** nq-deribit team
**Date:** 2026-05-16
**Link:** docs/superpowers/specs/2026-05-16-skills-panel-content-design.md

## TL;DR

Populated the empty Skills panel by exposing `SkillLoader` discovery results via two new JSON-RPC methods (`skill.list`, `skill.get`), and added a detail dialog component in the Dioxus web frontend for viewing full skill metadata.

## Key Takeaways

- Skills panel showed "No skills discovered" because no RPC methods existed to list/retrieve skills
- `SkillLoader` already discovers and caches skills from `.agents/skills/` — only needed exposure
- Two JSON-RPC methods added: `skill.list` (Vec<SkillListEntry>) and `skill.get` (SkillDetail by name)
- `JsonRpcServer::new()` gained `Option<Arc<SkillLoader>>` — returns empty array when unconfigured
- Frontend `SkillsPanel` fetches on mount via `rpc_client.skill_list()`, shows error with retry
- Row click opens `SkillDetailDialog` modal — view-only display of name, version, scope, triggers, SKILL.md body, file listing
- `SkillLoader` wired up in `jsonrpc_agent_service.rs` example at server startup
- 3 new unit tests for RPC request parsing, 49 backend tests total passing

## Detailed Summary

### Backend Changes

Two new RPC data types defined in `serde_helpers.rs`:
- `SkillListEntry`: `id`, `name`, `version`, `scope`, `description`, `triggers`
- `SkillDetail`: all ListEntry fields plus `content` (SKILL.md body) and `file_listing`

`JsonRpcServer` struct and `new()` gained `skill_loader: Option<Arc<SkillLoader>>` parameter, threaded through `handle_ws` to `JsonRpcConnection`.

`handle_skill_list()` — when no loader configured returns `[]`; otherwise calls `loader.list_metadata()` and serializes to `Vec<SkillListEntry>`.

`handle_skill_get(name)` — calls `loader.get(name)`, returns full `SkillDetail` or JSON-RPC error `{"code": -32000, "message": "Skill 'X' not found"}`.

In `jsonrpc_agent_service.rs`, `SkillLoader::new(Some(PathBuf::from(".")))` created, `discover_all()` spawned in background task, wrapped in `Arc`, passed to server.

### Frontend Changes

`SkillsState` gained `error: Option<String>` for fetch failures. `SkillDialogState` struct: `open`, `skill`, `loading`.

`SkillsPanel` rewritten: fetches via `rpc_client.skill_list()` in `use_effect` on mount, shows retry button on error, row click calls `rpc_client.skill_get()` and opens dialog.

`SkillDetailDialog` new component — modal overlay with header (name + version badge + scope badge), description, trigger pills, scrollable `pre` block for SKILL.md body, file listing table.

Dialog signal managed at `App` level, passed via context to `SkillsPanel`, rendered at root level.

## Entities Mentioned

- [[vol-llm-agent-channel-crate]]: Added `skill.list`/`skill.get` RPC methods, `SkillLoader` integration
- [[vol-llm-ui-crate]]: Added `SkillDialogState`, `SkillDetail` types, `SkillsPanel` rewrite, `SkillDetailDialog` component

## Concepts Covered

- [[skill-system]]: Native skill discovery mechanism powering the panel
- [[jsonrpc-transport]]: Transport layer carrying new skill RPC methods
- [[dioxus-web-pattern]]: Detail dialog modal pattern added to web components

## Notes

- Dialog is view-only — no activate/install button (design decision for initial scope)
- Pre-existing web compile errors (53) in other components do not affect these changes
- `cargo check -p vol-llm-ui --no-default-features --features web` passes cleanly for changed files
