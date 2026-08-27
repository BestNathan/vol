---
type: entity
category: product
tags: [repository, rust, cargo-workspace, agent, llm, gitops, documentation, mkdocs]
created: 2026-05-19
updated: 2026-08-27
source_count: 5
---

# vol Repository

**Category:** Rust Cargo workspace — LLM agent platform (volatility pipeline removed 2026-08-21)

**Related:** [[mkdocs-ai-contextual-menu]], [[claude-md-project-overview]], [[vol-llm-ui-crate]], [[vol-llm-agent-crate]], [[vol-llm-agents-crate]], [[vol-llm-agent-protocol-crate]], [[vol-llm-mcp-crate]], [[vol-mcp-servers-crate]]

## Overview

`vol` is a Rust workspace for an LLM agent platform: a ReAct agent runtime, tool/skill/MCP
infrastructure, a JSON-RPC-over-WebSocket agent server (control-plane + data-plane), and a
React web frontend. The former Deribit volatility monitoring pipeline was removed from
`main` on 2026-08-21 and lives on the `archive/volatility-pipeline` branch.

## Key Facts

- Main workspace root: `crates/`
- LLM/agent crate family: `vol-llm-*`
- Web frontend: React app at `frontend/` (active); `crates/vol-llm-ui` Dioxus WASM deprecated 2026-08 — dev via `just web-*`
- Agent backend service: `crates/vol-agent-server`
- Command entry point: `justfile` recipes (no Makefile)
- Project wiki: `docs/wiki`
- GitHub Pages wiki: MkDocs Material with page-level AI actions for copying Markdown, viewing or copying canonical raw Markdown URLs, and opening the page in ChatGPT or Claude [[mkdocs-ai-contextual-menu]]
- Superpowers artifacts: `docs/superpowers/` (requirement / architectures / specs / plans)
- Kubernetes manifests: `deploy/argocd/` self-contained ArgoCD GitOps manifests (primary) [[argocd-gitops-deployment]]; `k8s/` legacy/manual, deprecated
- Cargo mirror config for Docker Rust builds: `.cargo/`

## Module Structure

| Area | Role |
|------|------|
| LLM core and providers | `vol-llm-core`, `vol-llm-provider`, `vol-llm-tool` |
| Agent orchestration | `vol-llm-agent`, `vol-llm-agents`, `vol-llm-yaml-agent`, `vol-llm-agent-tool` |
| Tools & sandboxes | `vol-llm-tools-builtin`, `vol-llm-cli-tool`, `vol-llm-fs`, `vol-llm-task`, `vol-llm-sandbox`, `vol-llm-skill`, `vol-llm-wiki` |
| Context & memory | `vol-llm-context`, `vol-llm-memory`, `vol-session` |
| Observability & tracing | `vol-llm-observability`, `vol-llm-tracing` |
| Runtime & protocol | `vol-llm-runtime`, `vol-llm-agent-protocol` |
| MCP | `vol-llm-mcp`, `vol-mcp-servers` |
| User interfaces | React `frontend/` (active); `vol-llm-ui` (deprecated), `vol-llm-tui` |
| Deployment | `deploy/argocd/` ArgoCD GitOps (primary); `deploy/kustomize/`; `k8s/` legacy/manual, deprecated |
| Documentation and artifacts | `README.md` (agent-only, six sections), `docs/`, `docs/wiki/`, `docs/superpowers/` |

## Timeline

- **2026-08-27**: Added an AI contextual menu to every GitHub Pages wiki page: copy page Markdown, copy/view the canonical Markdown URL, and open the page in ChatGPT or Claude; the integration pins `mkdocs-copy-to-llm` and validates nested `/vol` → `docs/wiki` path mapping [[mkdocs-ai-contextual-menu]]
- **2026-08-21**: Volatility pipeline removed from `main` — advice module and 12 pipeline crates deleted (crate list in [[pipeline-removal-from-main]]), `vol-tracing`→`vol-llm-tracing` and `vol-observability`→`vol-llm-observability` renamed, pipeline infra/docs deleted; the pipeline lives on the `archive/volatility-pipeline` branch [[pipeline-removal-from-main]]
- **2026-08-21**: README restructured to an agent-only six-section layout (agent system / architecture / project structure / install & deploy / AI workflow / tools & commands); volatility pipeline mentions removed entirely [[readme-restructure]]
- **2026-06-16**: Added self-contained ArgoCD App-of-Apps GitOps deployment tree under `deploy/argocd/` plus MCP image build workflow; later refactored into `runtime-config` (namespace + shared .agents ConfigMaps) and `workloads` child Applications [[argocd-gitops-deployment]]
- **2026-05-19**: `CLAUDE.md` gained a Project Overview section summarizing the main repository directories and their roles [[claude-md-project-overview]]

## Related

- [[mkdocs-ai-contextual-menu]]
- [[claude-md-project-overview]]
- [[readme-restructure]]
- [[vol-llm-ui-crate]]
- [[vol-llm-agent-crate]]
- [[vol-llm-agent-protocol-crate]]
- [[vol-llm-mcp-crate]]
