# Git Hooks Quality Gates

## Summary

Replace the monolithic `.githooks/pre-commit` with a two-layer quality gate
system: pre-commit for fast checks (fmt, lint, typecheck), pre-push for slow
checks (coverage).  Both layers are hard gates with no bypass.

## Motivation

The current pre-commit hook runs fmt → clippy → coverage for every commit.
Coverage is too slow for pre-commit (can exceed 10 minutes on a cold cache)
and discourages frequent commits.  Moving coverage to pre-push keeps commits
fast while still guarding pushes.

Frontend code (React/TypeScript) has no quality gating at all today — no
formatter, no linter, no type checker running automatically.  This adds it.

## Architecture

```
git commit ──→ pre-commit ──→ Rust:     fmt + clippy            (~7s)
                             Frontend: prettier + eslint + tsc  (~8s)
                             │
                             └── fail → file:line + fix cmd → hard block

git push   ──→ pre-push    ──→ Rust:     coverage (changed crate)  (~30s-2min)
                             Frontend: vitest --coverage           (~10s-1min)
                             │
                             └── fail → coverage gap → hard block
```

### Change detection

A shared `scripts/detect-changes.sh` checks staged files (pre-commit) or
the push range (pre-push) and outputs which language checks to run:

```
.rs files staged          → RUST_CHANGED=true
frontend/src/** staged    → FRONTEND_CHANGED=true
```

Hooks call this first, skip checks for unchanged languages.

## Configuration ownership

Tools are configured where they belong.  Root-level `scripts/` and
`.githooks/` only orchestrate — they call into the toolchain, they don't
configure it.

| Layer | Owns |
|-------|------|
| `Cargo.toml` / `rustfmt.toml` / `clippy.toml` | Rust fmt, clippy, test config |
| `frontend/package.json` npm scripts | Frontend format, lint, typecheck, test commands |
| `frontend/.prettierrc` / `eslint.config.js` / `tsconfig.json` | Frontend tool config |
| `frontend/vitest.config.ts` | Frontend test + coverage config |
| `scripts/check-*.sh` | Thin wrappers that call the above, format error output |
| `.githooks/pre-commit` / `pre-push` | Change detection → dispatch to scripts |

Scripts under `scripts/` never contain tool-specific configuration (no
prettier rules, no eslint settings, no coverage thresholds for frontend).

## Scripts

Each script is a thin wrapper.  It calls the underlying tool, formats
failure output with file:line + fix suggestion, and returns exit 0 or 1.

### pre-commit scripts

| Script | Delegates to |
|--------|-------------|
| `scripts/check-rust-fmt.sh` | `cargo fmt --all -- --check` |
| `scripts/check-rust-clippy.sh` | `cargo clippy --workspace` |
| `scripts/check-fe-format.sh` | `npm --prefix frontend run format:check` |
| `scripts/check-fe-lint.sh` | `npm --prefix frontend run lint` |
| `scripts/check-fe-type.sh` | `npm --prefix frontend run typecheck` |

### pre-push scripts

| Script | Delegates to |
|--------|-------------|
| `scripts/check-rust-coverage.sh` | Existing, modified to accept changed crate names |
| `scripts/check-fe-test.sh` | `npm --prefix frontend run test:coverage` |

### Frontend npm scripts (defined in `frontend/package.json`)

```json
{
  "scripts": {
    "dev": "vite",
    "build": "tsc -b && vite build",
    "preview": "vite preview",
    "test": "vitest",
    "test:run": "vitest run",
    "format": "prettier --write src/",
    "format:check": "prettier --check src/",
    "lint": "eslint src/",
    "typecheck": "tsc -b --noEmit",
    "test:coverage": "vitest run --coverage"
  }
}
```

### Speed

| Script | Speed |
|--------|-------|
| `check-rust-fmt.sh` | ~2s |
| `check-rust-clippy.sh` | ~5s |
| `check-fe-format.sh` | ~1s |
| `check-fe-lint.sh` | ~3s |
| `check-fe-type.sh` | ~3s |
| `check-rust-coverage.sh` | ~30s-2min (per changed crate) |
| `check-fe-test.sh` | ~10s-1min |

## Fail output format

Every script uses a consistent format on failure:

```
✗ <check-name> failed
─────────────────────────────────
  <file>:<line>
  error: <one-line description>
  fix:   <actionable fix command or suggestion>
─────────────────────────────────
  <file>:<line>
  warning: <description>

This check must pass before pushing. No bypass available.
```

## Hooks

### `.githooks/pre-commit`

Rewrite existing hook.  Remove coverage.  Add frontend checks.

```
detect-changes.sh → for Rust:    check-rust-fmt.sh → check-rust-clippy.sh
                    for Frontend: check-fe-format.sh → check-fe-lint.sh → check-fe-type.sh
```

All checks run.  First failure stops the chain and blocks the commit.

### `.githooks/pre-push` (new)

```
detect-changes.sh → for Rust:     check-rust-coverage.sh <changed crates>
                    for Frontend: check-fe-test.sh
```

Runs against the push range (commits being pushed), not just staged files.

## Frontend tooling setup

All frontend config lives under `frontend/`.  Root-level scripts and hooks
only invoke npm scripts — they don't configure prettier, eslint, or vitest.

### New devDependencies (in `frontend/package.json`)

```json
{
  "prettier": "^3.x",
  "eslint": "^9.x",
  "@eslint/js": "^9.x",
  "typescript-eslint": "^8.x",
  "eslint-plugin-react-hooks": "^5.x",
  "eslint-plugin-react-refresh": "^0.4.x",
  "@vitest/coverage-v8": "^3.x"
}
```

### Configuration files (all under `frontend/`)

| File | Purpose |
|------|---------|
| `frontend/.prettierrc` | Single quotes, trailing commas, 100 char width |
| `frontend/eslint.config.js` | Flat config (ESLint 9), TypeScript + React Hooks |
| `frontend/.prettierignore` | node_modules, dist, coverage |
| `frontend/.eslintignore` | Same |

Existing files to modify:

| File | Change |
|------|--------|
| `frontend/tsconfig.json` | Already exists — verify `noEmit` is set |
| `frontend/vitest.config.ts` | Add `coverage` provider (v8), set thresholds |

### Format existing code

Run `npm --prefix frontend run format` once to baseline all existing code.
Include that commit separately before adding the pre-commit hook so the
format commit is clean and bisectable.

### Makefile targets

Add these for manual use — thin wrappers around npm scripts:

```makefile
fe-fmt:       npm --prefix frontend run format
fe-fmt-check: npm --prefix frontend run format:check
fe-lint:      npm --prefix frontend run lint
fe-type:      npm --prefix frontend run typecheck
fe-test:      npm --prefix frontend run test:coverage
```

## Hard gate policy

No `--no-verify` bypass.  No skip flag.  No environment variable override.
If a check fails, the code must be fixed before committing or pushing.

Rationale: a hotfix that can't pass fmt/lint/test is not a hotfix — it's a
regression waiting to happen.

## Implementation plan

1. Install frontend devDependencies (prettier, eslint, vitest/coverage)
2. Create frontend config files (.prettierrc, eslint.config.js, vitest.config.ts)
3. Format existing frontend code with prettier, fix lint issues
4. Create scripts: check-rust-fmt.sh, check-rust-clippy.sh, check-fe-format.sh, check-fe-lint.sh, check-fe-type.sh, check-fe-test.sh, detect-changes.sh
5. Modify check-rust-coverage.sh to accept crate names as args
6. Rewrite .githooks/pre-commit to use new scripts
7. Create .githooks/pre-push
8. Add Makefile targets for frontend quality commands
9. Verify: commit triggers pre-commit, push triggers pre-push
10. Run full CI to confirm nothing breaks
