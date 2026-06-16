---
type: entity
category: product
tags: [repository, rust, cargo-workspace, deribit, llm, gitops]
created: 2026-05-19
updated: 2026-06-16
source_count: 2
---

# vol Repository

**Category:** Rust Cargo workspace — Deribit volatility monitoring and LLM agent tooling

**Related:** [[claude-md-project-overview]], [[vol-llm-ui-crate]], [[vol-llm-agent-crate]], [[vol-llm-agents-crate]], [[vol-llm-agent-channel-crate]], [[vol-llm-mcp-crate]], [[vol-mcp-servers-crate]], [[tdengine]]

## Overview

`vol` is a Rust workspace that combines a Deribit volatility monitoring service with a broader LLM agent platform. The monitoring side follows an event-driven pipeline: configuration feeds data sources, data sources publish through an event bus, alert handlers evaluate conditions, and notification handlers deliver alerts.

## Key Facts

- Main workspace root: `crates/`
- Monitoring crate family: `vol-*`
- LLM/agent crate family: `vol-llm-*`
- Web frontend: `crates/vol-llm-ui`, built and served through Makefile web commands
- Agent backend service for the web UI: `crates/vol-agent-manager`
- Project wiki: `docs/wiki`
- OpenSpec artifacts: `openspec/`
- Kubernetes manifests: `k8s/`
- Self-contained ArgoCD GitOps manifests: `deploy/argocd/` [[argocd-gitops-deployment]]
- Cargo mirror config for Docker Rust builds: `.cargo/`

## Module Structure

| Area | Role |
|------|------|
| Monitoring core | `vol-core`, `vol-config`, `vol-datasource`, `vol-deribit`, `vol-tdengine` |
| Monitoring runtime | `vol-eventbus`, `vol-engine`, `vol-alert`, `vol-notification`, `vol-monitor` |
| LLM core and providers | `vol-llm-core`, `vol-llm-provider`, `vol-llm-tool` |
| Agent orchestration | `vol-llm-agent`, `vol-llm-agents` |
| Agent communication and MCP | `vol-llm-agent-channel`, `vol-llm-mcp`, `vol-mcp-servers` |
| User interfaces | `vol-llm-ui`, `vol-llm-tui` |
| Deployment | `k8s/` legacy/manual manifests; `deploy/argocd/` self-contained ArgoCD GitOps manifests |
| Documentation and artifacts | `docs/`, `docs/wiki/`, `docs/superpowers/`, `openspec/` |

## Timeline

- **2026-06-16**: Added self-contained ArgoCD App-of-Apps GitOps deployment tree under `deploy/argocd/` plus MCP image build workflow [[argocd-gitops-deployment]]
- **2026-05-19**: `CLAUDE.md` gained a Project Overview section summarizing the main repository directories and their roles [[claude-md-project-overview]]

## Related

- [[claude-md-project-overview]]
- [[vol-llm-ui-crate]]
- [[vol-llm-agent-crate]]
- [[vol-llm-agent-channel-crate]]
- [[vol-llm-mcp-crate]]
