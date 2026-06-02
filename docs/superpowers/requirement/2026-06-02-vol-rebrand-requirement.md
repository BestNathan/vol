# Requirements: Rebrand project codename `nq-deribit` → `vol`

## Background

The repository's project **codename** is currently `nq-deribit` (a leftover from when
the project began as an "nq" effort and then focused on the Deribit exchange). The project
has been renamed to **`vol`**. We want the live/current surfaces of the repo to reflect the
new `vol` codename instead of the stale `nq-deribit` name.

This is a **branding/codename** change only. It is NOT a functional change and NOT a
package/crate rename.

## Goals

1. Replace the project codename token `nq-deribit` with `vol` in **live/current files**.
2. Keep the change purely textual/branding — no behavior changes, the workspace must still build.

## Non-Goals (explicitly out of scope, per user clarification)

1. **Do NOT rename any crate or package.** `vol-deribit` and all `vol-*` / `vol-llm-*`
   crate names stay exactly as they are.
2. **Do NOT touch the Deribit exchange integration.** API endpoints, host names, the
   `vol-deribit` crate, TDengine table names like `deribit_volatility_index`, and any
   `Deribit` reference that denotes the real exchange remain unchanged.
3. **Do NOT rebrand the product title "Deribit Volatility Monitor."** It describes
   monitoring the Deribit exchange (the integration), which stays. This includes:
   - `crates/vol-monitor/src/main.rs` startup banner
   - `crates/vol-notification/src/feishu.rs` notification card
   - `crates/vol-llm-agent/tests/agent_alert_scenario.rs` system prompt
   - README/CLAUDE.md descriptive prose ("Deribit volatility monitoring")
4. **Do NOT modify historical/frozen records.** Leave `nq-deribit` (incl. `/root/nq-deribit`
   paths) untouched in:
   - `docs/superpowers/plans/*`
   - `docs/superpowers/specs/*`
   - `docs/superpowers/requirement/*` (prior docs)
   - `docs/test-results/*`
   - `openspec/changes/*`
5. **Do NOT rename the git repository directory** (`vol-agent`) or change git history.
6. Leave the workspace `authors = ["Deribit Vol Monitor"]` field as-is (it contains the
   "Deribit" product branding, which we are keeping; it does not contain the `nq-deribit`
   token).

## Scope — exact in-scope edits (live files)

| # | File | Current | New |
|---|------|---------|-----|
| 1 | `README.md:1` | `# nq-deribit` | `# vol` |
| 2 | `crates/vol-llm-ui/index.html:6` | `<title>nq-deribit \| vol-llm-ui</title>` | `<title>vol \| vol-llm-ui</title>` |
| 3 | `docs/wiki/sources/skills-panel-content.md:11` | `nq-deribit team` | `vol team` |
| 4 | `docs/wiki/sources/claude-md-project-overview.md:13` | `/root/nq-deribit/CLAUDE.md` | `/root/vol/CLAUDE.md` |
| 5 | `docs/wiki/sources/claude-md-project-overview.md:38` | `[[nq-deribit-repository]]` | `[[vol-repository]]` |
| 6 | `docs/wiki/sources/rust-lib-backend.md:41` | `[[nq-deribit-repository]]` | `[[vol-repository]]` |
| 7 | `docs/wiki/entities/nq-deribit-repository.md` | filename + `# nq-deribit Repository` + body token | rename file → `vol-repository.md`; heading → `# vol Repository`; body `nq-deribit` → `vol` |
| 8 | `docs/deployment/k8s-deployment.md:53` | `/root/nq-deribit/config.prod.toml` | `/root/vol/config.prod.toml` |

> Note: `docs/wiki/index.md` does **not** reference `nq-deribit-repository` (the entity is
> currently only linked from the two source files in items 5–6), so the index needs no edit.
> The entity being absent from the index is a pre-existing wiki state, not part of this change.

## Decisions (resolved by user 2026-06-02)

- **D1 — `nq-web-dev` skill → RENAME FOLDER.** Rename `.claude/skills/nq-web-dev/` →
  `.claude/skills/vol-web-dev/`, update `SKILL.md` `name:` and description text
  (`nq-deribit` → `vol`). Invocation becomes `/vol-web-dev`.
- **D2 — ignored test path → UPDATE.** `crates/vol-llm-wiki/tests/wiki_integration_test.rs:34`
  `.join("nq-deribit")` → `.join("vol")`.
- **D3 — wiki entity → RENAME + FIX BACKLINKS.** Rename `nq-deribit-repository.md` →
  `vol-repository.md`, update heading/body, fix the 2 `[[nq-deribit-repository]]` backlinks,
  validate with `wiki-lint`.

## Constraints

- Textual change only; `cargo build`/`cargo check` for the workspace must still succeed
  afterward (the only code-adjacent edit is the `#[ignore]`d test string in D2).
- Wiki edits must keep the wiki link graph valid (no dangling `[[...]]`), verified via
  `wiki-lint`.
- Per CLAUDE.md, this `docs/superpowers/requirement/*` doc must be uploaded to Feishu
  (wiki node `PPDZw7LFqiFjMTkAXFocFoO6nce`) — done after content is approved.

## Success Criteria

1. `grep -rn 'nq-deribit'` over live areas (excluding the Non-Goal historical dirs) returns
   **zero** results, except any item the user explicitly chooses to keep (e.g. D1 folder name).
2. `grep -rn 'nq-deribit'` still returns the historical-doc hits (they are intentionally
   preserved).
3. No `vol-*` crate name, the `vol-deribit` crate, or any `deribit_*` table/API identifier
   changed (verifiable by diff: no edits under `crates/*/Cargo.toml` name fields, no edits to
   `vol-deribit/`, no edits to the product-title lines listed in Non-Goals #3).
4. `cargo check` (workspace) succeeds.
5. `wiki-lint` reports no broken links after the wiki entity rename.

## Edge Cases

- **`/root/nq-deribit` in a live operational doc (item 9)** is a filesystem path example,
  not a literal requirement; updating the codename portion keeps the example consistent.
- **Wiki backlinks** must be updated in the same change as the entity rename to avoid a
  dangling-link window.
- **Case sensitivity:** only the lowercase `nq-deribit` token appears; the heading
  "nq-deribit Repository" is the only mixed-context use.

## Open Questions

- None. D1/D2/D3 resolved (see Decisions).
