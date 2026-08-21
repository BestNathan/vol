---
type: source
source_type: code
date: 2026-05-19
ingested: 2026-05-19
tags: [claude-md, project-overview, repository-structure]
---

# CLAUDE.md Project Overview

**Authors/Creators:** Claude Code
**Date:** 2026-05-19
**Link:** `/root/vol/CLAUDE.md`

## TL;DR

`CLAUDE.md` includes a Project Overview section that summarizes the repository as a Rust Cargo workspace for the LLM agent system (volatility-monitor pipeline removed 2026-08-21), with a directory map covering workspace crates, documentation, deployment manifests, scripts, and Cargo mirror configuration.

## Key Takeaways

- The repository is an LLM agent platform: ReAct agent runtime, tool/skill/MCP infrastructure, agent server (control-plane + data-plane), and web frontend.
- The former Deribit volatility monitoring pipeline was removed from `main` on 2026-08-21 (see [[pipeline-removal-from-main]]); the 2026-05-19 overview predates that and described the pipeline crates.
- `crates/` is the main workspace root and contains `vol-llm-*` agent/tooling crates (plus `vol-agent-server`, `vol-session`).
- `crates/vol-llm-ui` is the Dioxus WASM web frontend (deprecated 2026-08 — React `frontend/` is active) and must use the `just web-*` commands.
- `docs/wiki` is the persistent project wiki for future agents.
- `.cargo/` contains the Cargo mirror configuration required by Docker Rust builds.

## Detailed Summary

The Project Overview section gives future Claude Code sessions a compact orientation before diving into specific crates. It identifies the repository as an agent system, then lists the major directories and selected high-level crate groups.

The LLM side is summarized through provider/core/tool/agent crates (`vol-llm-core`, `vol-llm-provider`, `vol-llm-tool`, `vol-llm-agent`, `vol-llm-agents`), MCP crates (`vol-llm-mcp`, `vol-mcp-servers`), UI crates (`vol-llm-ui`, `vol-llm-tui`), and the backend agent service (`vol-agent-server`).

## Entities Mentioned

- [[vol-repository]]: repository-level Cargo workspace and directory map
- [[vol-llm-ui-crate]]: Dioxus WASM web frontend called out in the overview
- [[vol-llm-agent-crate]]: ReAct orchestration crate grouped under LLM agent infrastructure
- [[vol-llm-agents-crate]]: higher-level agent implementations grouped under LLM agent infrastructure
- [[vol-llm-agent-protocol-crate]]: agent communication and JSON-RPC/MCP-related layer
- [[vol-llm-mcp-crate]]: MCP client protocol layer
- [[vol-mcp-servers-crate]]: MCP server collection

## Concepts Covered

- [[react-pattern]]: used by the LLM agent orchestration layer
- [[tool-registry]]: part of the LLM tool infrastructure summarized by the overview
- [[mcp-transport-pattern]]: related to the MCP crate group
- [[dioxus-web-pattern]]: used by the web frontend crate

## Notes

This source documents the repository orientation guidance in `CLAUDE.md`, not a code behavior change.
