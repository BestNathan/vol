---
type: source
source_type: design
date: 2026-06-10
ingested: 2026-06-10
tags: [agent-server, control-plane, data-plane, implementation-plan, json-rpc, behavior-completion]
---

# Control Plane Behavior Completion Plan

**Authors/Creators:** Claude
**Date:** 2026-06-10
**Link:** `docs/superpowers/plans/2026-06-10-control-plane-behavior-completion-plan.md`
**Feishu/Lark:** https://my.feishu.cn/docx/JRjcd9jnkoKxVyxoQ7zc1aHenue

## TL;DR

Follow-up implementation plan for [[agent-server-control-data-plane]] after final review found behavior gaps in the initial implementation. It completes JSON-RPC notification support, endpoint role allowlists, minimal client-facing control-plane methods, data-plane `control.command` handling, capability revision sync, `control.run_status`, combined-mode local node registration, and final verification.

## Key Takeaways

- `decode_jsonrpc_frame` must accept JSON-RPC notifications without `id` for control-plane reports like `control.heartbeat`.
- `/ws` and `/control/v1/ws` need explicit `ControlConnectionRole` allowlists.
- Control-plane `/ws` needs minimal client-facing handlers so existing clients do not get `UnknownMethod` for `agent.list` and related methods.
- `DataPlaneServerCore` needs a `control.command` handler that returns `CommandAck`.
- Applying capability snapshots should sync `NodeRecord.capability_revision`.
- `control.run_status` needs a handler backed by `RunStore`.
- Combined mode needs local data-plane registration so the local node appears behind the control plane.

## Detailed Summary

The plan contains eight tasks. Task 1 adds JSON-RPC notification decode support. Task 2 introduces role-aware control-plane endpoints and method allowlists. Task 3 adds a minimal client-facing handler, beginning with `agent.list`. Task 4 registers a data-plane `control.command` handler. Task 5 syncs capability snapshot revision into `NodeRecord`. Task 6 adds `control.run_status`. Task 7 registers a local data-plane node in combined mode. Task 8 performs verification, review, docs, Lark, and wiki updates.

## Entities Mentioned

- [[vol-llm-agent-channel-crate]]: notification decode and protocol behavior.
- [[vol-agent-server-crate]]: endpoint roles, client/data-plane handlers, capability revision sync, combined mode registration.

## Concepts Covered

- [[agent-server-control-data-plane]]: behavior-completion plan for the remaining contract gaps after initial implementation.
- [[jsonrpc-transport]]: notification semantics and role-aware endpoint use.

## Notes

The plan intentionally keeps persistent control-plane storage and rich remote command delivery outside scope. It also allows the combined-mode MVP to use in-process local registration instead of loopback JSON-RPC to keep this follow-up deterministic.
