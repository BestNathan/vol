# Volatility Pipeline Removal from main — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove all volatility-pipeline code, infra, docs, and wiki content from `main`; preserve a buildable snapshot on the `archive/volatility-pipeline` branch.

**Architecture:** Direct commits on main, one per task for bisectability (refinement of the spec's 5-commit grouping — same logic, finer granularity). Crate surgery order matters: advice module + dead deps first (workspace stays green), then directory deletions, then the two renames, then infra/docs/wiki.

**Tech Stack:** Rust workspace (cargo), justfile recipes, Markdown docs, wiki (docs/wiki), YAML manifests.

**Spec:** `docs/superpowers/specs/2026-08-21-volatility-pipeline-removal-design.md`

## Global Constraints

- **Do NOT use git worktrees** (user preference) — execute in the main working tree.
- Archive branch `archive/volatility-pipeline` and tag `volatility-pipeline-2026-08-21` must point at `f7a3dd2f` exactly.
- Repo-wide grep on main (rs/toml/md/yml/yaml/sh) for `vol-monitor|vol-deribit|vol-alert|vol-engine|vol-datasource|vol-notification|vol-rules|vol-eventbus|vol_core|vol_config|vol_tdengine|TdengineClient` must return **0 hits** by the final task.
- `cargo build --workspace`, `cargo test` (agent crates), and `just fmt-check clippy boundaries no-doc-tests` must be green at the end of every task that touches Rust code.
- Coverage gate ≥80% for `vol-llm-agents`, `vol-llm-tracing`, `vol-llm-observability` (exempt: main.rs / app.rs / health.rs).
- `frontend/` must not be modified.
- After completion: wiki-ingest the removal; upload the new spec to Lark specs node (`Og7twpiP0iVbjk2EzvcqX92nsb`) per CLAUDE.md convention.
- Commit messages use conventional commits and end with `Co-Authored-By: Claude <noreply@anthropic.com>`.

---

### Task 0: Create archive branch and tag

**Files:** none (git refs only)

**Interfaces:**
- Produces: branch `archive/volatility-pipeline` @ `f7a3dd2f`, tag `volatility-pipeline-2026-08-21` @ `f7a3dd2f`

- [ ] **Step 1: Verify clean working tree**

Run: `git status --short`
Expected: empty output. If not, stop and surface to the user.

- [ ] **Step 2: Create archive branch**

Run: `git branch archive/volatility-pipeline f7a3dd2f`
Expected: no output, exit 0.

- [ ] **Step 3: Create archive tag**

Run: `git tag volatility-pipeline-2026-08-21 f7a3dd2f`
Expected: no output, exit 0.

- [ ] **Step 4: Verify both refs**

Run: `git log --oneline -1 archive/volatility-pipeline && git log --oneline -1 volatility-pipeline-2026-08-21`
Expected: both print `f7a3dd2f Merge pull request #57 from BestNathan/feat/wiki-github-pages`.

- [ ] **Step 5: No commit needed** (refs are local; mention to user that `git push origin archive/volatility-pipeline volatility-pipeline-2026-08-21` publishes them).

---

### Task 1: Remove advice module + dead pipeline deps (workspace stays green)

**Files:**
- Modify: `crates/vol-llm-agents/src/lib.rs` (remove lines 3, 9, 10, 11 — `pub mod advice;` and the three re-exports)
- Delete: `crates/vol-llm-agents/src/advice/` (mod.rs, service.rs, prompt.rs, limiter.rs)
- Delete: `crates/vol-llm-agents/tests/advice_agent_integration.rs`, `crates/vol-llm-agents/tests/coding_deribit_ws_e2e.rs` (volatility-domain test)
- Modify: `crates/vol-llm-agents/Cargo.toml` (remove dep lines: `vol-core`, `vol-tracing`, `vol-notification`, `vol-config`, `vol-llm-tdengine`, `vol-tdengine`)
- Modify: `crates/vol-llm-agent/Cargo.toml` (remove `vol-llm-tdengine` and `vol-tdengine` dep lines — dead, confirmed by zero src usage)
- Modify: `crates/vol-session/Cargo.toml` (remove `vol-core`, `vol-tracing`, `vol-config` dep lines if present — dead, zero src usage)
- Modify: `crates/vol-llm-core/Cargo.toml` (remove `vol-core` dep line if present — dead, zero src usage)

**Interfaces:**
- Consumes: nothing (all deps listed are confirmed dead or advice-only)
- Produces: a workspace where no agent crate references `vol_core`, `vol_config`, `vol_notification`, `vol_tdengine`, `vol_llm_tdengine`, or `vol_tracing`

- [ ] **Step 1: Baseline check (red)**

Run: `cargo check --workspace 2>&1 | tail -3`
Expected: PASS (current main is green). This is the baseline, not the test — the test in Step 4 is the grep.

- [ ] **Step 2: Delete advice module files**

Run:
```bash
rm -r crates/vol-llm-agents/src/advice
rm crates/vol-llm-agents/tests/advice_agent_integration.rs
rm crates/vol-llm-agents/tests/coding_deribit_ws_e2e.rs
```

- [ ] **Step 3: Edit vol-llm-agents/src/lib.rs — remove advice exports**

Remove exactly these lines:
```rust
pub mod advice;
```
and
```rust
pub use advice::system_prompt;
pub use advice::FrequencyLimiter;
pub use advice::{AdviceAgent, AdviceAgentConfig};
```

- [ ] **Step 4: Strip the six dead deps from vol-llm-agents/Cargo.toml**

Remove these lines under `[dependencies]`:
```toml
vol-core = { workspace = true }
vol-tracing = { workspace = true }
vol-notification = { workspace = true }
vol-config = { workspace = true }
vol-llm-tdengine = { path = "../vol-llm-tdengine" }
vol-tdengine = { path = "../vol-tdengine" }
```

- [ ] **Step 5: Strip dead deps from vol-llm-agent, vol-session, vol-llm-core Cargo.tomls**

Inspect each file first: `grep -nE "vol-core|vol-config|vol-tracing|vol-tdengine|vol-llm-tdengine|vol-notification" crates/vol-llm-agent/Cargo.toml crates/vol-session/Cargo.toml crates/vol-llm-core/Cargo.toml`
Remove every matched line. (Confirmed: vol-llm-agent has vol-llm-tdengine + vol-tdengine; vol-session has vol-core + vol-tracing + vol-config; vol-llm-core has vol-core. If a matched line is absent, skip it.)

- [ ] **Step 6: Verify no agent code references removed crates (the test)**

Run:
```bash
grep -rn "vol_core\|vol_config\|vol_notification\|vol_tdengine\|vol_llm_tdengine" crates/vol-llm-agent/src crates/vol-llm-agents/src crates/vol-session/src crates/vol-llm-core/src
```
Expected: no matches (exit 1). `vol_tracing` is expected to still match in `crates/vol-llm-agent/src/react/run_context.rs` and `crates/vol-llm-agent/src/react/tests.rs` — that is fine (renamed in Task 3).

- [ ] **Step 7: Verify workspace compiles + vol-llm-agents tests pass**

Run:
```bash
cargo check --workspace 2>&1 | tail -3
cargo test -p vol-llm-agents 2>&1 | tail -5
```
Expected: check PASS (0 errors); tests pass (advice/coding-deribit tests are gone).

- [ ] **Step 8: Commit**

```bash
git add -A crates/vol-llm-agents crates/vol-llm-agent/Cargo.toml crates/vol-session/Cargo.toml crates/vol-llm-core/Cargo.toml
git commit -m "refactor!: remove advice agent module and dead pipeline deps

Advice agent (volatility domain) removed from vol-llm-agents along with
its integration test and the coding-deribit WS e2e test. Dead deps on
vol-core/vol-config/vol-notification/vol-tdengine/vol-llm-tdengine
stripped from vol-llm-agents/vol-llm-agent/vol-session/vol-llm-core.

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 2: Delete 12 pipeline crate directories + workspace entries

**Files:**
- Delete directories: `crates/vol-monitor`, `crates/vol-deribit`, `crates/vol-datasource`, `crates/vol-alert`, `crates/vol-rules`, `crates/vol-notification`, `crates/vol-engine`, `crates/vol-eventbus`, `crates/vol-core`, `crates/vol-config`, `crates/vol-tdengine`, `crates/vol-llm-tdengine`
- Modify: `Cargo.toml` — remove the 13 `members` entries (12 above + none extra; the member list contains each once; vol-tracing/vol-observability stay, renamed later)
- Modify: `Cargo.toml` — remove `[workspace.dependencies]` lines 83-84 (`vol-tdengine`, `vol-llm-tdengine`) and 120-129 (`vol-core`, `vol-eventbus`, `vol-config`, `vol-datasource`, `vol-deribit`, `vol-alert`, `vol-notification`, `vol-engine`, `vol-rules`, `vol-tracing`→ kept for Task 3 rename — DO NOT remove `vol-tracing` line yet; instead Task 3 renames it)

**Interfaces:**
- Consumes: Task 1 (no agent crate depends on these 12 crates anymore)
- Produces: workspace with 12 fewer members; `vol-tracing`/`vol-observability` untouched

- [ ] **Step 1: Delete the 12 crate directories**

```bash
rm -r crates/vol-monitor crates/vol-deribit crates/vol-datasource crates/vol-alert \
      crates/vol-rules crates/vol-notification crates/vol-engine crates/vol-eventbus \
      crates/vol-core crates/vol-config crates/vol-tdengine crates/vol-llm-tdengine
```

- [ ] **Step 2: Remove members from Cargo.toml**

Remove these lines from the `members = [` list:
```toml
    "crates/vol-core",
    "crates/vol-eventbus",
    "crates/vol-config",
    "crates/vol-datasource",
    "crates/vol-deribit",
    "crates/vol-alert",
    "crates/vol-notification",
    "crates/vol-monitor",
    "crates/vol-engine",
    "crates/vol-rules",
    "crates/vol-tdengine",
    "crates/vol-llm-tdengine",
```
(Keep `"crates/vol-tracing"` and `"crates/vol-observability"` — renamed in Tasks 3-4. Note: `"crates/vol-llm-tools-builtin/cli-tool"` and the duplicate `"crates/vol-llm-runtime"` member lines are pre-existing quirks — do not touch.)

- [ ] **Step 3: Remove workspace.dependencies entries**

Remove lines 83-84 and 120-129 from `[workspace.dependencies]`:
```toml
vol-tdengine = { path = "crates/vol-tdengine" }
vol-llm-tdengine = { path = "crates/vol-llm-tdengine" }
vol-core = { path = "crates/vol-core" }
vol-eventbus = { path = "crates/vol-eventbus" }
vol-config = { path = "crates/vol-config" }
vol-datasource = { path = "crates/vol-datasource" }
vol-deribit = { path = "crates/vol-deribit" }
vol-alert = { path = "crates/vol-alert" }
vol-notification = { path = "crates/vol-notification" }
vol-engine = { path = "crates/vol-engine" }
vol-rules = { path = "crates/vol-rules" }
```
Keep `vol-tracing = { path = "crates/vol-tracing" }` — Task 3 renames it.

- [ ] **Step 4: Regenerate lockfile + verify build (the test)**

Run:
```bash
cargo check --workspace 2>&1 | tail -5
```
Expected: PASS. If a missing-crate error names one of the 12 deleted crates, a dependency remains — go back to Task 1 Step 6's grep and strip it.

- [ ] **Step 5: Run full test suite**

Run: `just test-unit 2>&1 | tail -5 && just test-integration 2>&1 | tail -5`
Expected: both PASS.

- [ ] **Step 6: Commit**

```bash
git add -A Cargo.toml Cargo.lock crates/
git commit -m "refactor!: delete 12 volatility pipeline crates from workspace

Remove vol-monitor/vol-deribit/vol-datasource/vol-alert/vol-rules/
vol-notification/vol-engine/vol-eventbus/vol-core/vol-config/vol-tdengine/
vol-llm-tdengine crates, workspace members, and workspace.dependencies.

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 3: Rename vol-tracing → vol-llm-tracing

**Files:**
- Move: `crates/vol-tracing/` → `crates/vol-llm-tracing/` (via `git mv`)
- Modify: `crates/vol-llm-tracing/Cargo.toml` (package name)
- Modify: `Cargo.toml` (member entry + workspace.dependencies entry)
- Modify: `crates/vol-llm-agent/Cargo.toml` (dep rename)
- Modify: `crates/vol-llm-agent/src/react/run_context.rs`, `crates/vol-llm-agent/src/react/tests.rs` (imports `vol_tracing::` → `vol_llm_tracing::`)

**Interfaces:**
- Consumes: Task 2 (vol-tracing is now orphaned — only vol-llm-agent depends on it)
- Produces: crate `vol-llm-tracing` exporting `TracedEvent` (same public API, new crate name); all consumers migrated

- [ ] **Step 1: Inspect modules for pipeline-only helpers**

Run: `ls crates/vol-tracing/src/ && grep -n "pub " crates/vol-tracing/src/lib.rs`
Expected: `lib.rs`, `macros.rs`, `traced_event.rs`, `with_span.rs`. If any module is NOT referenced by agent code after the rename (check with `grep -rn "with_span\|macros::" crates/vol-llm-agent/src`), delete that module file and its `mod` declaration during Step 2.

- [ ] **Step 2: git mv + rename package**

```bash
git mv crates/vol-tracing crates/vol-llm-tracing
sed -i 's/^name = "vol-tracing"$/name = "vol-llm-tracing"/' crates/vol-llm-tracing/Cargo.toml
```
If Step 1 found unused modules (e.g. `with_span.rs`, `macros.rs` with no agent consumers), remove them now: `rm crates/vol-llm-tracing/src/<unused>.rs` and remove their `mod` lines from `lib.rs`.

- [ ] **Step 3: Update workspace references**

In root `Cargo.toml`:
- member: `"crates/vol-tracing",` → `"crates/vol-llm-tracing",`
- workspace.dependencies: `vol-tracing = { path = "crates/vol-tracing" }` → `vol-llm-tracing = { path = "crates/vol-llm-tracing" }`

- [ ] **Step 4: Update consumer dep + imports**

In `crates/vol-llm-agent/Cargo.toml`: `vol-tracing = { workspace = true }` → `vol-llm-tracing = { workspace = true }`.
In `crates/vol-llm-agent/src/react/run_context.rs` and `crates/vol-llm-agent/src/react/tests.rs`:
`vol_tracing::` → `vol_llm_tracing::` (run `grep -rn "vol_tracing" crates/ --include="*.rs"` to confirm these are the only two files).

- [ ] **Step 5: Verify (the test)**

Run:
```bash
grep -rn "vol_tracing" crates/ --include="*.rs" --include="*.toml"; cargo check --workspace 2>&1 | tail -3
```
Expected: grep exits 1 (no matches — every reference now says `vol_llm_tracing`); check PASS.

- [ ] **Step 6: Test the crate**

Run: `cargo test -p vol-llm-tracing 2>&1 | tail -5 && cargo test -p vol-llm-agent 2>&1 | tail -5`
Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add -A Cargo.toml Cargo.lock crates/
git commit -m "refactor!: rename vol-tracing to vol-llm-tracing

TracedEvent is agent-side infrastructure; only vol-llm-agent consumes it.
Unused pipeline-oriented helper modules removed.

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 4: Rename vol-observability → vol-llm-observability

**Files:**
- Move: `crates/vol-observability/` → `crates/vol-llm-observability/` (via `git mv`)
- Modify: `crates/vol-llm-observability/Cargo.toml` (package name)
- Modify: `Cargo.toml` (member entry)
- Modify dependents' Cargo.tomls + imports: `crates/vol-agent-server`, `crates/vol-llm-agents`, `crates/vol-mcp-servers`, `crates/vol-llm-yaml-agent` (dep key + `vol_observability::` → `vol_llm_observability::`)

**Interfaces:**
- Consumes: Task 3 (workspace green)
- Produces: crate `vol-llm-observability` (LoggingPlugin/MetricsPlugin/otel_init, same API)

- [ ] **Step 1: Find all references**

Run:
```bash
grep -rln "vol_observability\|vol-observability" crates/ --include="*.rs" --include="*.toml" | grep -v "crates/vol-observability/"
```
Expected: exactly `crates/vol-agent-server`, `crates/vol-llm-agents`, `crates/vol-mcp-servers`, `crates/vol-llm-yaml-agent` (Cargo.toml and src files).

- [ ] **Step 2: git mv + rename package**

```bash
git mv crates/vol-observability crates/vol-llm-observability
sed -i 's/^name = "vol-observability"$/name = "vol-llm-observability"/' crates/vol-llm-observability/Cargo.toml
```

- [ ] **Step 3: Update workspace member + dependents**

Root `Cargo.toml`: `"crates/vol-observability",` → `"crates/vol-llm-observability",`.
In each of the 4 dependent crates: replace dep key `vol-observability` → `vol-llm-observability` (both `{ workspace = true }` and `{ path = ... }` forms), and `vol_observability::` → `vol_llm_observability::` in src.

- [ ] **Step 4: Verify (the test)**

Run:
```bash
grep -rn "vol_observability" crates/ --include="*.rs" --include="*.toml"; cargo check --workspace 2>&1 | tail -3
```
Expected: grep exits 1; check PASS.

- [ ] **Step 5: Test affected crates**

Run: `cargo test -p vol-llm-observability -p vol-agent-server 2>&1 | tail -5`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add -A Cargo.toml Cargo.lock crates/
git commit -m "refactor!: rename vol-observability to vol-llm-observability

All consumers are agent-side (agent-server, llm-agents, mcp-servers,
yaml-agent).

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 5: Remove pipeline infra (configs, dockers, k8s, scripts, justfile)

**Files:**
- Delete: `configs/vol-monitor.env.example`, `configs/vol-monitor.example.toml`
- Delete: `dockers/vol-monitor.Dockerfile`, `dockers/vol-monitor.cross.Dockerfile`
- Delete: `k8s/vol-monitor/` (directory), `k8s/namespace.yaml`
- Modify: `k8s/README.md` (remove vol-monitor tree entries + "Shared namespace" line; keep agent-server/mcp)
- Delete: `scripts/build-multiarch.sh`, `scripts/init_tdengine.sql`, `scripts/run-dev.sh`, `scripts/test-agent.sh`
- Modify: `scripts/check-rust-coverage.sh` (remove lines 24-27, 31-37, 76 — the 12 pipeline crate thresholds; keep agent entries)
- Modify: `justfile` (delete `docker-monitor:` recipe — lines ~273-276 with its comment)

**Interfaces:**
- Consumes: Task 4 (no repo path may reference deleted crates/binary anymore)
- Produces: no file outside `docs/` mentions the pipeline

- [ ] **Step 1: Delete files and directories**

```bash
rm configs/vol-monitor.env.example configs/vol-monitor.example.toml
rm dockers/vol-monitor.Dockerfile dockers/vol-monitor.cross.Dockerfile
rm -r k8s/vol-monitor
rm k8s/namespace.yaml
rm scripts/build-multiarch.sh scripts/init_tdengine.sql scripts/run-dev.sh scripts/test-agent.sh
```

- [ ] **Step 2: Edit scripts/check-rust-coverage.sh**

Remove the `["vol-..."]=80` lines for: `vol-core`, `vol-config`, `vol-eventbus`, `vol-tracing`→ replace with `vol-llm-tracing`, `vol-datasource`, `vol-deribit`, `vol-alert`, `vol-notification`, `vol-rules`, `vol-engine`, `vol-tdengine`, and the `["vol-monitor"]=40` line. Keep the `vol-llm-*` entries and add `["vol-llm-tracing"]=80`, `["vol-llm-observability"]=80` if the file lists observability.

- [ ] **Step 3: Edit k8s/README.md**

Remove the `namespace.yaml` line and the entire `vol-monitor/` subtree block; keep the agent-server/mcp blocks. Verify the tree diagram still parses as a valid code block.

- [ ] **Step 4: Edit justfile**

Delete the `# Build vol-monitor Docker image` comment and the `docker-monitor:` recipe body (`docker build -f dockers/vol-monitor.cross.Dockerfile -t vol-monitor .`).

- [ ] **Step 5: Verify (the test)**

Run:
```bash
grep -rn "vol-monitor\|vol_monitor" justfile scripts/ k8s/ configs/ dockers/ 2>/dev/null; just --list 2>&1 | head -5
```
Expected: grep exits 1 (no matches); `just --list` prints recipes without error.

- [ ] **Step 6: Commit**

```bash
git add -A configs/ dockers/ k8s/ scripts/ justfile
git commit -m "chore!: remove volatility pipeline infra

Delete vol-monitor configs/dockerfiles/k8s manifests, monitor dev/test
scripts, TDengine init SQL, docker-monitor recipe; trim coverage script.

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 6: Remove pipeline docs (docs/, superpowers, CLAUDE.md)

**Files:**
- Delete directories: `docs/deribit/`, `docs/integration/`
- Delete: `docs/tracing.md`, `docs/architecture/crates.md`, `docs/architecture/overview.md` (then `rmdir docs/architecture` if empty)
- Delete: `docs/test-results/advice-agent-integration-test-result-2026-04-11.md`, `docs/test-results/coding-agent-deribit-ws-client-test-result-2026-04-15.md`
- Modify: `docs/CONFIGURATION.md` (delete "Subsystem A" section — everything between line 33 `## Subsystem A` and line 254 `## Subsystem B`, and the `### B.3 [agent_advice]` block ending at B.4; re-check K8s section for pipeline-only content)
- Modify: `docs/development/common-modifications.md` (remove pipeline content if any — inspect first)
- Delete superpowers files (exact lists from spec section "Superpowers artifacts"):
  - `docs/superpowers/plans/`: `2026-03-30-multi-channel-dispatch-plan.md`, `2026-03-30-symbol-specific-iv-config.md`, `2026-03-31-deribit-auth-portfolio-monitor.md`, `2026-03-31-notification-template-enrichment.md`, `2026-03-31-single-connection-dispatcher.md`, `2026-04-01-channel-monitor-architecture.md`, `2026-04-01-channel-monitor-config-plan.md`, `2026-04-02-vol-monitor-k8s-deployment-plan.md`, `2026-04-04-multi-arch-docker-build.md`, `2026-04-04-portfolio-greeks-monitoring-plan.md`, `2026-04-04-tenor-based-cooldown-plan.md`, `2026-04-05-add-logging-tracing-otel.md`, `2026-04-05-log-file-naming.md`, `2026-04-05-span-tracing-implementation.md`, `2026-04-05-traced-event-implementation.md`, `2026-04-05-trace-id-in-logs-implementation.md`, `2026-04-06-agent-alert-advice.md`, `2026-04-06-agent-notification-handler-integration.md`, `2026-04-11-advice-agent-integration-test-plan.md`
  - `docs/superpowers/specs/`: `2026-03-30-multi-channel-dispatch-design.md`, `2026-03-30-symbol-specific-iv-config-design.md`, `2026-03-31-notification-template-enrichment-design.md`, `2026-03-31-single-connection-dispatcher-design.md`, `2026-04-01-channel-monitor-config-design.md`, `2026-04-02-vol-monitor-k8s-deployment-design.md`, `2026-04-04-multi-arch-docker-design.md`, `2026-04-04-portfolio-greeks-monitoring-design.md`, `2026-04-04-tenor-based-cooldown-design.md`, `2026-04-05-log-file-naming-design.md`, `2026-04-05-traced-event-design.md`, `2026-04-05-trace-id-in-logs-design.md`, `2026-04-06-agent-notification-handler-design.md`, `2026-04-11-advice-agent-integration-test-design.md`
  - `docs/superpowers/releases/`: `2026-04-02-vol-feishu-to-openlark-migration.md`
  - Content-check then delete iff pipeline-domain: `docs/superpowers/requirement/2026-05-03-k8s-lgtm-observability-requirement.md`, `docs/superpowers/requirement/2026-06-02-vol-rebrand-requirement.md` (rule: subject is monitor pipeline / Deribit / options monitoring / advice / TDengine → delete; agent-domain → keep)
- Modify: `CLAUDE.md` (remove: project-structure rows for pipeline crates, `configs/vol-monitor.env.example` / `vol-monitor.example.toml` mentions, `docker build -f dockers/vol-monitor.cross.Dockerfile` line, `k8s/vol-monitor/deploy.sh` legacy line; keep agent commands, Model Service, conventions)

**Interfaces:**
- Consumes: Task 5 (all code refs gone)
- Produces: `docs/` with zero pipeline-topic content (except this plan + the removal spec, which document the agent-side change)

- [ ] **Step 1: Delete directories and standalone files**

```bash
rm -r docs/deribit docs/integration
rm docs/tracing.md docs/architecture/crates.md docs/architecture/overview.md
rm docs/test-results/advice-agent-integration-test-result-2026-04-11.md \
   docs/test-results/coding-agent-deribit-ws-client-test-result-2026-04-15.md
rmdir docs/architecture 2>/dev/null || true
```

- [ ] **Step 2: Delete the enumerated superpowers files**

Run the exact `rm` list from the Files section (19 plans + 14 specs + 1 release). Content-check the two requirement candidates and delete if pipeline-domain.

- [ ] **Step 3: Trim docs/CONFIGURATION.md**

Delete "Subsystem A — Volatility Monitoring Pipeline" (line 33 through just before line 254 `## Subsystem B`), delete the `### B.3 [agent_advice]` subsection (through just before `### B.4`), and renumber if section numbers become non-contiguous. Inspect the "Kubernetes Deployment" section and remove any vol-monitor-only paragraphs.

- [ ] **Step 4: Inspect + trim docs/development/common-modifications.md**

Run: `grep -inE "vol-monitor|deribit|pipeline|volatility" docs/development/common-modifications.md`
Remove pipeline-topic sections if any.

- [ ] **Step 5: Update CLAUDE.md**

Remove the items listed in Files. Do NOT touch the web/shadcn/TDD conventions, Model Service block, or Lark table.

- [ ] **Step 6: Verify (the test)**

Run:
```bash
grep -rinE "vol-monitor|vol-deribit|vol-alert|vol-engine|vol-datasource|vol-notification|vol-rules|vol-eventbus|vol_core|vol_config|vol_tdengine|TdengineClient" docs/ CLAUDE.md --include="*.md"
```
Expected: no matches EXCEPT this plan and the removal spec (check their paths; they document the change and may cite crate names — allowed by spec).

- [ ] **Step 7: Commit**

```bash
git add -A docs/ CLAUDE.md
git commit -m "docs!: remove volatility pipeline documentation

Delete docs/deribit, docs/integration, pipeline architecture/tracing
docs, advice/deribit test results, pipeline-topic superpowers history,
and Subsystem A of CONFIGURATION.md; update CLAUDE.md.

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 7: Wiki surgery + dead-link repair

**Files:**
- Delete: `docs/wiki/entities/tdengine.md`
- Move: `docs/wiki/entities/vol-observability-crate.md` → `docs/wiki/entities/vol-llm-observability-crate.md` (update title + in-page refs)
- Modify: `docs/wiki/entities/vol-llm-agents-crate.md` (remove advice mentions), `docs/wiki/entities/vol-repository.md` (archive branch + agent-only main), any entity/concept/source with refs to removed crates
- Create: `docs/wiki/sources/pipeline-removal-from-main.md` (ingest of this removal — type: report)
- Modify: `docs/wiki/index.md`, `docs/wiki/log.md`

**Interfaces:**
- Consumes: Task 6
- Produces: wiki where every `[[wikilink]]` resolves and no page references removed crates (`vol-tracing`→`vol-llm-tracing`, `vol-observability`→`vol-llm-observability`, deleted names gone)

- [ ] **Step 1: Delete tdengine entity + rename observability entity**

```bash
rm docs/wiki/entities/tdengine.md
git mv docs/wiki/entities/vol-observability-crate.md docs/wiki/entities/vol-llm-observability-crate.md
```
Update the moved page: title line and any self-references.

- [ ] **Step 2: Fix crate-name references across wiki (the sweep)**

Run:
```bash
grep -rlE "vol-monitor|vol-deribit|vol-alert|vol-engine|vol-datasource|vol-notification|vol-rules|vol-eventbus|vol_core|vol_config|vol_tdengine|vol_tracing|vol-observability|advice|tdengine" docs/wiki/entities docs/wiki/concepts docs/wiki/sources
```
For each hit: if the whole page's subject is pipeline (e.g. advice, TDengine tools), delete the page; otherwise edit mentions: `vol-tracing`→`vol-llm-tracing`, `vol-observability`→`vol-llm-observability`, deleted crates→remove the mention. Known pages needing edit (from scan): `agent-builder-pattern`, `loki-plugin-otel-migration-design`, `otel-dependency-upgrade`, `otel-log-routing`, `skill-system`, `tool-registry`, `vol-llm-agents-crate`, `vol-repository`, `agent-tool-design`, `ci-workflow-restructure`, `claude-md-project-overview`, `docs-rs-mcp-impl`, `http-transport-impl`, `observability-pull-metrics-refactor`, `otel-029-log-init`, `otel-agent-log-dir-fix`, `react-agent-docs`, `readme-restructure`, `session-ssot-redesign`, `skills-as-react-native`, `test-tiering-e2e-completion`, `vol-mcp-servers-dockerfile`.

- [ ] **Step 3: Create the ingest source page**

Create `docs/wiki/sources/pipeline-removal-from-main.md` with frontmatter (`type: source, source_type: report, date/ingested: 2026-08-21`), TL;DR (archive branch + 12 crates deleted + 2 renamed + advice removed), Key Takeaways, Entities/Concepts touched, and Notes (cluster-side cleanup out of scope). Link: [[vol-repository]], [[vol-llm-agents-crate]], [[vol-llm-tracing]]/[[vol-llm-observability]] pages if they exist — otherwise link the entity pages you just created/renamed.

- [ ] **Step 4: Update index.md and log.md**

index.md: remove `[[tdengine]]` and `[[vol-observability-crate]]` rows; add `[[vol-llm-observability-crate]]` row; add `[[pipeline-removal-from-main]]` source row; update `Last updated` line and `[[vol-repository]]` / `[[vol-llm-agents-crate]]` summaries.
log.md: prepend `## [2026-08-21] ingest | Volatility pipeline removal from main` entry (created sources, deleted entities, renamed entities, index changes).

- [ ] **Step 5: Dead-link sweep (the test)**

Run: `git ls-files docs/wiki | grep -E "\.md$" | xargs grep -hoE '\[\[[a-z0-9-]+\]\]' | tr -d '[]' | sort -u | while read l; do find docs/wiki -name "$l.md" -print -quit | grep -q . || echo "DEAD: $l"; done`
Expected: no DEAD output. Fix any dead links by removing the mention or pointing to the correct page.

- [ ] **Step 6: Commit**

```bash
git add -A docs/wiki/
git commit -m "docs(wiki)!: remove pipeline pages, rename observability entity

Delete tdengine entity, rename vol-observability-crate to
vol-llm-observability-crate, fix crate references, ingest
pipeline-removal-from-main source, repair dead links.

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 8: Final verification

**Files:** fixups only, whatever the checks below surface

**Interfaces:**
- Consumes: Tasks 0-7
- Produces: verified green main + verified archive branch

- [ ] **Step 1: Workspace build + tests**

Run:
```bash
cargo build --workspace 2>&1 | tail -3
just test-unit 2>&1 | tail -3
just test-integration 2>&1 | tail -3
```
Expected: all PASS.

- [ ] **Step 2: Quality gates**

Run: `just fmt-check && just clippy && just boundaries && just no-doc-tests`
Expected: all PASS.

- [ ] **Step 3: Coverage gates**

Run:
```bash
just cover-gate vol-llm-agents 80
just cover-gate vol-llm-tracing 80
just cover-gate vol-llm-observability 80
```
Expected: each ≥80%.

- [ ] **Step 4: Repo-wide grep sweep**

Run:
```bash
grep -rnE "vol-monitor|vol-deribit|vol-alert|vol-engine|vol-datasource|vol-notification|vol-rules|vol-eventbus|vol_core|vol_config|vol_tdengine|TdengineClient" --include="*.rs" --include="*.toml" --include="*.md" --include="*.yml" --include="*.yaml" --include="*.sh" . 2>/dev/null | grep -v "^\./docs/superpowers/plans/2026-08-21-volatility-pipeline-removal\|^\./docs/superpowers/specs/2026-08-21-volatility-pipeline-removal\|^\./\.git/"
```
Expected: 0 hits (plan + spec documenting the removal are the only allowed mentions). Fix any hit.

- [ ] **Step 5: Archive completeness check**

Run:
```bash
git stash list | grep -q . && git stash push -u -m "pre-archive-check" || true
git checkout archive/volatility-pipeline
cargo build -p vol-monitor 2>&1 | tail -3
git checkout main
git stash pop 2>/dev/null || true
```
Expected: archive builds vol-monitor successfully; back on main with working tree intact.

- [ ] **Step 6: YAML parse remaining manifests**

Run:
```bash
python3 - <<'EOF'
import yaml, glob
for f in glob.glob('deploy/**/*.yaml', recursive=True) + glob.glob('k8s/**/*.yaml', recursive=True):
    yaml.safe_load(open(f))
print("all YAML ok")
EOF
```
Expected: `all YAML ok`.

- [ ] **Step 7: Frontend untouched check**

Run: `git diff --stat HEAD -- frontend/ ; grep -rinE "vol-monitor|deribit|advice|tdengine" frontend/src 2>/dev/null | head -5`
Expected: no diff; grep exits 1.

- [ ] **Step 8: Commit fixups (if any)**

If Steps 1-7 surfaced fixes, commit: `git add -A && git commit -m "fix: verification fixups for pipeline removal\n\nCo-Authored-By: Claude <noreply@anthropic.com>"`.

- [ ] **Step 9: Lark upload of the spec (per CLAUDE.md convention)**

Run:
```bash
lark-cli docs +create --api-version v2 --doc-format markdown \
  --content @docs/superpowers/specs/2026-08-21-volatility-pipeline-removal-design.md \
  --wiki-node "Og7twpiP0iVbjk2EzvcqX92nsb" --as user
```
Expected: success (or surface auth errors to user).

- [ ] **Step 10: Report**

Report to user: commit list (`git log --oneline 7e27983d..HEAD`), verification results, archive branch push command (`git push origin archive/volatility-pipeline volatility-pipeline-2026-08-21`), and out-of-scope note (cluster-side cleanup: deployed monitor workloads, TDengine DB, deribit namespace).
