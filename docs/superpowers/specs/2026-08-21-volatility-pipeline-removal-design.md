# Volatility Pipeline Removal from main — Design

**Date:** 2026-08-21
**Status:** Approved (brainstorming + section-by-section design review)
**Goal:** `main` maintains only the vol-agent-system; the volatility monitoring pipeline is archived to a dedicated branch and all its code/infra/docs/wiki content is removed from `main`.

## Decisions

| # | Decision | Choice |
|---|----------|--------|
| 1 | Archive strategy | Copy current main to archive branch + tag; main proceeds with direct removal commits |
| 2 | Shared crates (vol-tracing/vol-config/vol-core/vol-tdengine/vol-observability) | 彻底切割改名: rename to `vol-llm-*` where agent side depends on them, delete where it does not |
| 3 | Advice agent + TDengine toolchain | Remove both (volatility-domain features inside the agent system) |
| 4 | Docs depth | Delete active docs AND historical superpowers artifacts AND wiki pipeline pages; history lives only in the archive branch |
| 5 | Commit strategy | Direct commits on main (4-5 logical commits), no feature branch |

## Git Strategy

1. From current main HEAD (`f7a3dd2f`), create branch `archive/volatility-pipeline` and tag `volatility-pipeline-2026-08-21`. The branch never advances; it is the complete, buildable pipeline snapshot.
2. On main, commit sequence:
   - ✅ `7e27983d` docs: README restructure (done, pre-work)
   - `refactor!`: remove volatility crates (workspace surgery, renames, advice removal)
   - `chore!`: remove pipeline infra (configs/dockers/k8s/scripts/justfile)
   - `docs!`: remove pipeline docs (docs/, superpowers, CLAUDE.md)
   - `docs(wiki)!`: wiki surgery + dead-link repair
3. Archive completeness check: `git checkout archive/volatility-pipeline && cargo build -p vol-monitor` must succeed.

## Crate Surgery (commit 2)

### Delete (12 crates + 1 module)

| Crate | Reason |
|-------|--------|
| `vol-monitor` | Pipeline binary |
| `vol-deribit` | Deribit WebSocket client (pipeline) |
| `vol-datasource` | Pipeline data sources |
| `vol-alert` | Pipeline alert rules |
| `vol-rules` | Pipeline rule processors |
| `vol-notification` | Pipeline notification handlers (Feishu/Stdout) |
| `vol-engine` | Pipeline engine orchestration |
| `vol-eventbus` | Pipeline event bus (no agent dependents) |
| `vol-core` | Only agent user was the advice module → deletable |
| `vol-config` | Zero agent usage (dead deps in vol-llm-agents/tui/wiki) |
| `vol-tdengine` | Only user was vol-llm-tdengine → both removed |
| `vol-llm-tdengine` | TDengine query tools (volatility domain) |
| `vol-llm-agents/src/advice/` | Advice agent module + its registration in `lib.rs` + its tests |

### Rename (2 crates)

| From | To | Notes |
|------|----|-------|
| `vol-tracing` | `vol-llm-tracing` | `TracedEvent<T>` + tracing utilities; consumers: vol-llm-agent, vol-llm-agents, vol-session. Strip pipeline-only helpers if trivially separable. |
| `vol-observability` | `vol-llm-observability` | LoggingPlugin/MetricsPlugin/otel_init; consumers all agent-side (vol-agent-server, vol-llm-agents, vol-mcp-servers, vol-llm-yaml-agent) |

Rename procedure per crate: move directory, change `[package] name`, update all `Cargo.toml` dependents, update all `use`/path references in Rust source, update workspace members.

### Dependency cleanup

- Remove dead dependency entries (`vol-core`, `vol-config`, `vol-tracing`→new name, `vol-tdengine`) from `vol-session`, `vol-llm-core`, `vol-llm-agents`, `vol-llm-tui`, `vol-llm-wiki` as applicable.
- Remove deleted crates' entries from `[workspace.dependencies]`; remove entries of shared deps that only the deleted crates used (verify with `cargo check`).
- Regenerate `Cargo.lock`.

## Infra Cleanup (commit 3)

| Path | Action |
|------|--------|
| `configs/vol-monitor.env.example`, `configs/vol-monitor.example.toml` | Delete |
| `dockers/vol-monitor.Dockerfile`, `dockers/vol-monitor.cross.Dockerfile` | Delete |
| `k8s/vol-monitor/` | Delete directory (deploy.sh, deployment*.yaml, configmap, secrets) |
| `k8s/namespace.yaml` | Delete (deribit namespace, pipeline-only) |
| `k8s/README.md` | Remove vol-monitor entries; keep agent-server/mcp legacy listing |
| `scripts/build-multiarch.sh`, `scripts/init_tdengine.sql`, `scripts/run-dev.sh`, `scripts/test-agent.sh` | Delete (pipeline-only or dependent on vol-monitor binary) |
| `scripts/check-rust-coverage.sh` | Remove 12 pipeline crate threshold entries |
| `justfile` | Delete `docker-monitor` recipe; sweep test/cover recipes for pipeline crate references |

CI workflows (`.github/workflows/`) verified pipeline-free — no change.

## Docs Cleanup (commit 4)

### Active docs — delete

| Path | Reason |
|------|--------|
| `docs/deribit/` (entire) | Deribit API reference |
| `docs/integration/` (entire) | deribit.md + tdengine.md, both pipeline |
| `docs/tracing.md` | vol-monitor tracing doc |
| `docs/architecture/crates.md`, `docs/architecture/overview.md` | Pipeline-only; agent architecture lives in README + wiki. Remove `docs/architecture/` if empty. |
| `docs/test-results/advice-agent-integration-test-result-2026-04-11.md`, `docs/test-results/coding-agent-deribit-ws-client-test-result-2026-04-15.md` | Pipeline/advice topics |

### Active docs — trim

| Path | Action |
|------|--------|
| `docs/CONFIGURATION.md` | Delete Subsystem A (pipeline env/TOML), section B.3 `[agent_advice]`, pipeline parts of the K8s section |
| `docs/development/common-modifications.md` | Remove pipeline content if any |
| `CLAUDE.md` | Remove pipeline project-structure rows, vol-monitor docker/k8s commands, configs listing; keep agent conventions |

### Superpowers artifacts — delete pipeline-topic files

Rule: a file is deleted iff its subject is pipeline-domain (monitor pipeline, Deribit, options/IV/tenor/greeks monitoring, alert notification templates, monitor tracing/OTel, advice agent, TDengine). Agent-domain files are kept.

Confirmed delete list:

**plans/**: `2026-03-30-multi-channel-dispatch-plan`, `2026-03-30-symbol-specific-iv-config`, `2026-03-31-deribit-auth-portfolio-monitor`, `2026-03-31-notification-template-enrichment`, `2026-03-31-single-connection-dispatcher`, `2026-04-01-channel-monitor-architecture`, `2026-04-01-channel-monitor-config-plan`, `2026-04-02-vol-monitor-k8s-deployment-plan`, `2026-04-04-multi-arch-docker-build`, `2026-04-04-portfolio-greeks-monitoring-plan`, `2026-04-04-tenor-based-cooldown-plan`, `2026-04-05-add-logging-tracing-otel`, `2026-04-05-log-file-naming`, `2026-04-05-span-tracing-implementation`, `2026-04-05-traced-event-implementation`, `2026-04-05-trace-id-in-logs-implementation`, `2026-04-06-agent-alert-advice`, `2026-04-06-agent-notification-handler-integration`, `2026-04-11-advice-agent-integration-test-plan`

**specs/**: `2026-03-30-multi-channel-dispatch-design`, `2026-03-30-symbol-specific-iv-config-design`, `2026-03-31-notification-template-enrichment-design`, `2026-03-31-single-connection-dispatcher-design`, `2026-04-01-channel-monitor-config-design`, `2026-04-02-vol-monitor-k8s-deployment-design`, `2026-04-04-multi-arch-docker-design`, `2026-04-04-portfolio-greeks-monitoring-design`, `2026-04-04-tenor-based-cooldown-design`, `2026-04-05-log-file-naming-design`, `2026-04-05-traced-event-design`, `2026-04-05-trace-id-in-logs-design`, `2026-04-06-agent-notification-handler-design`, `2026-04-11-advice-agent-integration-test-design`

**releases/**: `2026-04-02-vol-feishu-to-openlark-migration`

**requirement/**: content-check candidates: `2026-05-03-k8s-lgtm-observability-requirement`, `2026-06-02-vol-rebrand-requirement` — delete iff pipeline-domain per rule.

**architectures/**: both agent-topic — keep. **examples/**: agent-topic — keep.

## Wiki Surgery (commit 5)

| Action | Pages |
|--------|-------|
| Delete entity | `tdengine` |
| Rename entity | `vol-observability-crate` → `vol-llm-observability-crate` (update in-page refs) |
| Update entities | `vol-llm-agents-crate` (advice module removed), `vol-repository` (archive branch + agent-only main), any page referencing removed crates |
| Update concepts/sources | Fix passing mentions of `vol-tracing`/`vol-core`/`vol-config`/advice/TDengine in the ~24 scanned pages; delete pages whose whole subject is pipeline (none found beyond tdengine entity — verify) |
| Index + log | Remove deleted/renamed rows; add ingest entry for the removal |
| Dead-link sweep | All `[[wikilink]]` resolve (wiki-lint pass) |

## Verification & Success Criteria

1. `cargo build --workspace` green on main; agent crate tests green
2. `just fmt-check clippy boundaries no-doc-tests` green
3. Archive completeness: `git checkout archive/volatility-pipeline && cargo build -p vol-monitor` green
4. Repo-wide grep on main (rs/toml/md/yml/yaml/sh) for `vol-monitor|vol-deribit|vol-alert|vol-engine|vol-datasource|vol-notification|vol-rules|vol-eventbus|vol_core|vol_config|vol_tdengine|TdengineClient` → 0 hits (fresh Cargo.lock)
5. wiki-lint pass (no dead links, no stale refs)
6. Remaining ArgoCD/K8s YAML parse
7. Coverage gate ≥80% for affected crates (vol-llm-agents after advice removal, renamed crates)
8. `frontend/` untouched (grep confirms zero pipeline references)
9. wiki-ingest the removal itself (source page + log entry); upload spec to Lark (specs node) per convention

**Success:** `main` is pure vol-agent-system — the `vol-*` (non-llm) namespace is gone from main; `archive/volatility-pipeline` independently builds the full pipeline.

## Risks

- Advice removal changes vol-llm-agents' public surface (agent registry) — cargo check + tests cover; frontend/UI lists agents dynamically, verify no hardcoded advice references
- Rename churn in imports is mechanical; cargo check is the net
- Superpowers borderline files resolved by the rule above during implementation
- Concurrent commits by other sessions: coordinate via fresh git status before each commit

## Out of Scope

- `vol-llm-ui` (deprecated Dioxus) and its Dockerfile — agent-side deprecation, not volatility
- Legacy `k8s/agent-server` + `k8s/mcp` — agent-side legacy, not volatility
- Cluster-side cleanup (deleting deployed monitor workloads, TDengine DB) — deployment work, not repo work
