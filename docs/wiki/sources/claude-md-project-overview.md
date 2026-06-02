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

`CLAUDE.md` now includes a Project Overview section that summarizes the repository as a Rust Cargo workspace for Deribit volatility monitoring and LLM agent tooling, with a directory map covering workspace crates, documentation, OpenSpec artifacts, deployment manifests, scripts, and Cargo mirror configuration.

## Key Takeaways

- The repository combines a Deribit volatility monitoring pipeline with LLM agent infrastructure.
- The original monitoring pipeline flows from configuration to data sources, event bus, alert handlers, and notification handlers.
- `crates/` is the main workspace root and contains both `vol-*` monitoring crates and `vol-llm-*` agent/tooling crates.
- `crates/vol-llm-ui` is the Dioxus WASM web frontend and must use the Makefile web commands.
- `docs/wiki` is the persistent project wiki for future agents.
- `.cargo/` contains the Cargo mirror configuration required by Docker Rust builds.

## Detailed Summary

The new Project Overview section gives future Claude Code sessions a compact orientation before diving into specific crates. It identifies the repository-level split between the volatility monitor and the LLM agent system, then lists the major directories and selected high-level crate groups.

The monitoring side centers on shared models and traits in `vol-core`, configuration in `vol-config`, market data ingestion/storage via `vol-datasource`, `vol-deribit`, and `vol-tdengine`, and runtime behavior through `vol-eventbus`, `vol-engine`, `vol-alert`, `vol-notification`, and `vol-monitor`.

The LLM side is summarized through provider/core/tool/agent crates (`vol-llm-core`, `vol-llm-provider`, `vol-llm-tool`, `vol-llm-agent`, `vol-llm-agents`), communication and MCP crates (`vol-llm-agent-channel`, `vol-llm-mcp`, `vol-mcp-servers`), UI crates (`vol-llm-ui`, `vol-llm-tui`), and the backend agent service (`vol-agent-manager`).

## Entities Mentioned

- [[vol-repository]]: repository-level Cargo workspace and directory map
- [[vol-llm-ui-crate]]: Dioxus WASM web frontend called out in the overview
- [[vol-llm-agent-crate]]: ReAct orchestration crate grouped under LLM agent infrastructure
- [[vol-llm-agents-crate]]: higher-level agent implementations grouped under LLM agent infrastructure
- [[vol-llm-agent-channel-crate]]: agent communication and JSON-RPC/MCP-related layer
- [[vol-llm-mcp-crate]]: MCP client protocol layer
- [[vol-mcp-servers-crate]]: MCP server collection
- [[tdengine]]: market-data storage integration

## Concepts Covered

- [[react-pattern]]: used by the LLM agent orchestration layer
- [[tool-registry]]: part of the LLM tool infrastructure summarized by the overview
- [[mcp-transport-pattern]]: related to the MCP crate group
- [[dioxus-web-pattern]]: used by the web frontend crate

## Notes

This source documents the repository orientation guidance in `CLAUDE.md`, not a code behavior change.
