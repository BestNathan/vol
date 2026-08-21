---
type: source
source_type: report
date: 2026-08-21
ingested: 2026-08-21
tags: [readme, documentation, restructuring, agent-system]
---

# README Restructure — Agent-System-Focused Layout

**Authors/Creators:** BestNathan (Claude-assisted)
**Date:** 2026-08-21
**Link:** `README.md`

## TL;DR

The README was restructured from a mixed pipeline+agent layout into a pure agent-system,
six-section entry document. Per follow-up decision, the volatility pipeline is not mentioned
at all — no intro line, no crate note, no pipeline doc. Stale content fixed:
`make` → `just` recipes (no Makefile exists), `vol-llm-ui` marked deprecated (React
`frontend/` is the active web UI), crate table updated with new crates
(`vol-llm-agent-tool`, `vol-llm-fs`, `vol-llm-sandbox`, `vol-llm-cli-tool`), ArgoCD marked
the primary K8s path (`k8s/` legacy deprecated).

## Key Takeaways

- New six-section structure: 1) The Agent System, 2) Architecture, 3) Project Structure,
  4) Installation & Deployment, 5) AI-Driven Development Workflow, 6) Core Tools & Commands
- Architecture section has five subsections: core concepts (AgentRuntime as single source of
  truth), control/data plane, tools & sandboxes, agent–sub-agent collaboration, deployment
- Lean overview + link style: each subsection summarizes and links to the authoritative wiki
  concept page ([[agent-server-control-data-plane]], [[sandbox-lifecycle]],
  [[agenttool-subagent-dispatch]], …) instead of duplicating wiki content
- Language: English; depth: overview + links (explicitly chosen over self-contained long doc)
- All wiki-link targets in the README verified to exist

## Detailed Summary

Decisions confirmed before writing: (1) README is agent-only — the volatility pipeline is
not mentioned at all (revised from "one link-out line" per follow-up decision); (2) English;
(3) lean overview with links into `docs/` and the wiki.

Section 2 is the core: 2.1 core concepts (AgentRuntime, AgentDef + ReAct loop,
ContextBuilder/Contributor, JSON-RPC-over-WebSocket protocol), 2.2 control/data plane (the
three deployment modes table and the two-core diagram retained from the previous README),
2.3 tools & sandboxes (ToolRegistry, built-ins, CLI-as-tool `fs`/`task`, sandbox lifecycle),
2.4 agent–sub-agent collaboration (built-in `agent` tool, `AgentDef.id` dispatch, depth
guard, name-keyed sessions, AgentInjector), 2.5 deployment architecture (ArgoCD app-of-apps,
runtime-config ConfigMaps, kustomize alternative, legacy `k8s/` deprecated).

Section 6 replaces the old scattered `make` commands with a `just` recipe table grouped by
area (build/check, tests, coverage, guards, web, frontend tests, docker).

## Entities Mentioned

- [[vol-repository]]: README is its front door; key facts corrected in this ingest

## Concepts Covered

- [[agent-server-control-data-plane]]: referenced from Architecture 2.2
- [[tool-registry]], [[sandbox-lifecycle]], [[vol-llm-fs-crate]]: referenced from 2.3
- [[agenttool-subagent-dispatch]]: referenced from 2.4
- [[argocd-app-of-apps-gitops]]: referenced from 2.5

## Notes

- `docs/architecture/crates.md` remains stale in its overview ("10 crates") — follow-up
  candidate if pipeline documentation is revisited (it was: the volatility pipeline was
  removed on 2026-08-21, see [[pipeline-removal-from-main]])
- [[vol-repository]] had stale key facts (Makefile web commands, `vol-agent-manager`,
  OpenSpec) — corrected as part of this ingest
