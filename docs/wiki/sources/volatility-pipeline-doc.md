---
type: source
source_type: code
date: 2026-08-21
ingested: 2026-08-21
tags: [volatility-pipeline, monitoring, architecture, documentation]
---

# Volatility Pipeline Doc (docs/architecture/volatility-pipeline.md)

**Authors/Creators:** BestNathan (Claude-assisted)
**Date:** 2026-08-21
**Link:** `docs/architecture/volatility-pipeline.md`

## TL;DR

New architecture page that becomes the home for volatility monitoring pipeline
documentation after the README restructure [[readme-restructure]]. Covers pipeline
architecture, the 13 pipeline crates, run instructions, and deployment paths.

## Key Takeaways

- Pipeline: `Config → DataSource (Deribit) → EventBus (broadcast) → Alert Rules →
  Notifications (Feishu/Stdout)`, with TDengine time-series storage
- Event-driven via `TracedEvent<T>` wrappers and plugin traits (`DataSource`,
  `RuleProcessor`, `NotificationHandler`), OpenTelemetry/Jaeger tracing
- 13 pipeline crates: `vol-core`, `vol-config`, `vol-tracing`, `vol-eventbus`,
  `vol-deribit`, `vol-datasource`, `vol-alert`, `vol-rules`, `vol-notification`,
  `vol-engine`, `vol-monitor` (binary), `vol-tdengine`, `vol-observability`
- Run: `cargo run -p vol-monitor -- --config configs/vol-monitor.example.toml`
- Docker: `dockers/vol-monitor.Dockerfile` + `vol-monitor.cross.Dockerfile`
  (amd64 + arm64); `just docker-monitor` builds the cross image
- K8s: ArgoCD GitOps primary (`deploy/argocd/manifests/workloads/`); `k8s/vol-monitor`
  legacy deprecated

## Entities Mentioned

- [[vol-observability-crate]]: Prometheus metrics HTTP server
- [[tdengine]]: time-series database for market data storage
- Pipeline crates without entity pages (follow-up candidates): `vol-monitor`,
  `vol-deribit`, `vol-datasource`, `vol-alert`, `vol-rules`, `vol-notification`,
  `vol-engine`, `vol-eventbus`, `vol-core`, `vol-config`, `vol-tracing`, `vol-tdengine`

## Concepts Covered

- [[argocd-app-of-apps-gitops]]: pipeline deployments ride the same GitOps tree

## Notes

- Per-crate details for the original ten pipeline crates still live in
  `docs/architecture/crates.md`, whose overview header ("10 crates") predates
  `vol-eventbus` / `vol-tdengine` / `vol-observability`
- Pipeline crates have no wiki entity pages yet — creating them is a follow-up
