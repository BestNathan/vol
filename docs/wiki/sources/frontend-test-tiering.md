---
type: source
source_type: design
date: 2026-08-19
ingested: 2026-08-19
tags: [testing, frontend, vitest, playwright, integration-test, unit-test, e2e, jsdom, testing-library]
---

# Frontend Test Tiering: Vitest Projects Split (Unit + Integration)

**Authors/Creators:** BestNathan + Claude
**Date:** 2026-08-19
**Link:** `frontend/vite.config.ts`, `frontend/tests/{unit,integration,e2e}`, `frontend/package.json`, `justfile`, `.github/workflows/quality.yml`
**Follows:** [[test-tiering-e2e-completion]]

## TL;DR

The React frontend now mirrors the Rust three-tier split. Vitest runs as two **projects**: `unit` (node environment, `tests/unit/**`, the 18 pre-existing pure-logic tests) and `integration` (jsdom + @testing-library/react, `tests/integration/**`). Playwright remains the separate e2e toolchain (unchanged — the "dedicated e2e package" proposal was explicitly dropped). Four new component integration tests (InputArea, TabBar, StatusBar, CapabilityBar) render real components against a real jotai store with a mocked panel client — no live WS. Tier commands: `fe-test-unit` / `fe-test-integration` / `fe-e2e`; CI runs the two vitest projects as separate steps for per-tier failure attribution.

## Key Takeaways

- **Vitest projects** (`extends: true` from the root test config so exclude/coverage are inherited): `unit` = node + `tests/unit/**/*.test.ts`; `integration` = jsdom + `tests/integration/**/*.test.{ts,tsx}` + setup file. Select with `--project <name>`; plain `vitest run` executes both.
- **Integration setup file** (`tests/integration/setup.ts`): jest-dom matchers, RTL `cleanup()` after each test (vitest globals are off), and jsdom stubs for `ResizeObserver` + `matchMedia` (Radix primitives require them).
- **Mock strategy for component tests**: real component + real jotai `createStore()` with atoms pre-set via `store.set(...)`, `@/lib/panel-client` mocked with `vi.mock` (call spy returns canned RPC results). No WS connection, no network.
- **First batch (14 tests)**: InputArea (submit flow + optimistic UserInput entry + empty/no-agent guards + isRunning disable + approval banner), TabBar (7 tabs, active state, atom update — must be wrapped in a `Tabs` root like App.tsx does), StatusBar (counters/elapsed/Running badge/debug-panel toggle — NodesDropdown renders null in non-ControlPlane mode), CapabilityBar (capability counts fetch, drawer open, disabled without agent).
- **`test:coverage` is unchanged** (all projects, v8, `src/**` include) — coverage gate now benefits from component tests too.
- Playwright e2e untouched: the standalone-package restructure was considered and explicitly rejected ("先忽略这个提议") — the current same-project + separate config/scripts/CI job setup stays.

## Detailed Summary

### Tier map (frontend)

| Tier | Tool | Location | Command | CI |
|---|---|---|---|---|
| unit | vitest (node) | `tests/unit/` | `just fe-test-unit` | quality.yml step 1 |
| integration | vitest (jsdom + testing-library) | `tests/integration/` | `just fe-test-integration` | quality.yml step 2 |
| e2e | Playwright (chromium) | `tests/e2e/` | `just fe-e2e` | quality.yml step 3 (+ e2e.yml manual) |

### Files

- `frontend/vite.config.ts` — root test config (exclude, coverage thresholds) + `projects: [unit, integration]`
- `frontend/tests/integration/setup.ts` — jest-dom, cleanup, ResizeObserver/matchMedia stubs
- `frontend/tests/integration/{input-area,tab-bar,status-bar,capability-bar}.test.tsx` — 14 tests
- `frontend/package.json` — `test:unit`, `test:integration` scripts; new devDeps (@testing-library/react 16, user-event 14, jest-dom 7, jsdom 29)
- `justfile` — `fe-test-unit`, `fe-test-integration`; `fe-test` unchanged (all projects + coverage)
- `quality.yml` — Vitest split into two steps
- `.claude/skills/vol-backend-dev/SKILL.md` — tier table gained frontend rows

### Verification

- `npx vitest run --project unit` — 140/140.
- `npx vitest run --project integration` — 14/14 (first run).
- `npx vitest run --coverage` — 22 files, 154/154, thresholds pass.
- `npx tsc -b --noEmit` — clean; `npm run lint` — 0 errors.
- `npm run test:e2e` — 14/14 (Playwright regression after vite.config.ts change).
- justfile parses; quality.yml valid YAML.

## Entities Mentioned

- [[vol-llm-ui-crate]]: the React frontend the tests cover (via `frontend/`).

## Concepts Covered

- [[test-tiers]]: frontend tiers added to the tier table.

## Notes

- Component tests must wrap Radix consumers in their required providers (e.g. `TabBar` inside `<Tabs>`), mirroring App.tsx composition.
- `NodesDropdown` auto-returns null unless `serverMode === 'ControlPlane'` — component tests get it for free.
- The 39 eslint warnings in `npm run lint` are pre-existing (src/ only; test files are outside the lint glob).
