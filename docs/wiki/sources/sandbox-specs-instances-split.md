---
type: source
source_type: code
date: 2026-08-28
ingested: 2026-08-28
tags: [sandbox, frontend, protocol, rpc]
---

# Sandbox Specs & Instances Split

**Authors/Creators:** Claude + user
**Date:** 2026-08-28
**Link:** commit 599fb6fc

## TL;DR

Split the Sandboxes panel into two sections: **Specs** (loaded profile templates) and **Instances** (running sandbox instances). Added `sandbox.list_specs` RPC method to expose specs from `SandboxManager`.

## Key Takeaways

- `SandboxManager` had both `specs` (HashMap<String, SandboxSpec>) and `store` (instances) but only instances were exposed via RPC
- The `sandbox.list` returning empty list was correct behavior (no running instances) — the UI needed to show specs separately
- New `SandboxOperation::ListSpecs` follows same flat-format decode convention as other sandbox ops

## Entities Mentioned

- [[vol-llm-sandbox-crate]]: added `SandboxManager::list_specs()`
- [[vol-llm-agent-protocol-crate]]: added `SandboxOperation::ListSpecs`, `SandboxSpecInfo` wire type
- [[vol-agent-server-crate]]: handler branch in `SandboxHandler`
- [[frontend-sandboxes-panel]]: split UI into Specs + Instances sections

## Concepts Covered

- [[sandbox-lifecycle-refactor]]: specs vs instances distinction from the lifecycle refactor
- [[json-rpc-websocket]]: new RPC method following existing patterns

## Notes

- Specs are loaded from `.agents/sandboxes/*.toml` via `SandboxManager::load_profiles()`
- Empty specs list shows "No spec profiles configured"
- Empty instances list shows "No running sandbox instances"
