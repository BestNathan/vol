---
type: source
source_type: report
date: 2026-08-21
ingested: 2026-08-21
tags: [pipeline-removal, refactor, workspace, observability, tracing]
---

# Volatility Pipeline Removal from Main

**Authors/Creators:** BestNathan + Claude
**Date:** 2026-08-21
**Link:** commits `bed84465` (advice module + dead deps), `d928329c` (12 pipeline crates), `936f59d2` (`vol-tracing` → `vol-llm-tracing`), `e4cf35f0` (`vol-observability` → `vol-llm-observability`), `fb25b69f` (pipeline infra), `9a80bba4` (pipeline docs), `7e27983d` (README restructure)

## TL;DR

The Deribit volatility monitoring pipeline was removed from `main`: the advice agent
module and 12 pipeline crates were deleted, `vol-tracing`/`vol-observability` were renamed
to `vol-llm-tracing`/`vol-llm-observability`, and the pipeline's infra, docs, and README
mentions were stripped. The pipeline remains intact on the `archive/volatility-pipeline`
branch.

## Key Takeaways

- **Advice agent removed** (2026-08-21): the `advice/` module in `vol-llm-agents`, its
  integration test, and the coding-deribit WebSocket e2e test were deleted; dead
  dependencies on `vol-core`/`vol-config`/`vol-notification`/`vol-tdengine`/
  `vol-llm-tdengine` were stripped from `vol-llm-agents`, `vol-llm-agent`, `vol-session`,
  and `vol-llm-core`.
- **12 pipeline crates deleted** from the workspace: `vol-monitor`, `vol-deribit`,
  `vol-datasource`, `vol-alert`, `vol-rules`, `vol-notification`, `vol-engine`,
  `vol-eventbus`, `vol-core`, `vol-config`, `vol-tdengine`, `vol-llm-tdengine` — workspace
  members and `workspace.dependencies` entries removed.
- **2 crates renamed** to the agent-side namespace: `vol-tracing` → `vol-llm-tracing`
  (TracedEvent is agent-side infrastructure, consumed only by `vol-llm-agent`) and
  `vol-observability` → `vol-llm-observability` (all consumers are agent-side:
  agent-server, llm-agents, mcp-servers, yaml-agent).
- **Pipeline infra and docs deleted**: monitoring deployment artifacts, Dockerfiles,
  pipeline documentation, and README pipeline mentions (README was restructured to a pure
  agent six-section layout — [[readme-restructure]]).
- **Wiki surgery (this ingest)**: the `tdengine` entity page was deleted and
  `vol-observability-crate` was renamed to `vol-llm-observability-crate`; entity/concept/
  source pages referencing removed crates were updated; dead wikilinks repaired.

## Entities Touched

- [[vol-repository]]: workspace members, dependency graph, README, and timeline updated
  for the agent-only `main`; pipeline preserved on `archive/volatility-pipeline`
- [[vol-llm-agents-crate]]: advice agent removed; coding/ppt/qa/wiki remain
- [[vol-llm-observability-crate]]: renamed entity page (was `vol-observability-crate`)
- `tdengine` entity: deleted (pipeline data source)

## Concepts Covered

- [[agent-observability]], [[pull-based-metrics]], [[otel-log-routing]]: crate references
  updated to `vol-llm-observability`
- [[tool-registry]]: the four TDengine-backed built-in tools (`market_data`,
  `alert_history`, `iv_curve`, `rule_info`) are gone — registry is populated dynamically
- [[tool-context]]: `alert` field and its `vol_core` type removed
- [[skill-system]], [[agent-builder-pattern]], [[otel-dependency-upgrade]],
  [[loki-plugin-otel-migration-design]]: removed-crate mentions cleaned up

## Notes

- The wiki source pages for the removed pipeline (OTel init, observability refactor,
  test tiering, etc.) remain as historical records with past-tense references.
- **Cluster-side cleanup is out of scope** for this removal: the in-cluster TDengine
  service and any leftover monitoring resources are not part of the `main` removal.
- The `archive/volatility-pipeline` branch keeps the full pipeline (crates, infra, docs)
  for future reference.
