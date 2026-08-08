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

## Scripts

Each script is standalone — runnable by hooks, CI, or manually.  Every script
returns exit 0 on pass, exit 1 on fail with structured output.

### pre-commit scripts

| Script | Runs | Speed |
|--------|------|-------|
| `scripts/check-rust-fmt.sh` | `cargo fmt --all -- --check` | ~2s |
| `scripts/check-rust-clippy.sh` | `cargo clippy --workspace` | ~5s |
| `scripts/check-fe-format.sh` | `npx prettier --check frontend/src` | ~1s |
| `scripts/check-fe-lint.sh` | `npx eslint frontend/src` | ~3s |
| `scripts/check-fe-type.sh` | `npx tsc -b --noEmit` | ~3s |

### pre-push scripts

| Script | Runs | Speed |
|--------|------|-------|
| `scripts/check-rust-coverage.sh` | Existing, modified to run only changed crates | ~30s-2min |
| `scripts/check-fe-test.sh` | `npx vitest run --coverage` | ~10s-1min |

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

### New devDependencies

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

### Configuration files

- `frontend/.prettierrc` — single quotes, trailing commas, 100 char width
- `frontend/eslint.config.js` — flat config (ESLint 9), TypeScript + React Hooks rules
- `frontend/vitest.config.ts` — add `coverage` plugin, set thresholds
- `frontend/.prettierignore` — node_modules, dist, coverage
- `frontend/.eslintignore` — same

### Format existing code

Run `npx prettier --write frontend/src` once to baseline all existing code.
Include that commit separately before adding the pre-commit hook so the
format commit is clean and bisectable.

### Makefile targets

Add these for manual use:

```makefile
fe-fmt:     npx prettier --write frontend/src
fe-fmt-check: npx prettier --check frontend/src
fe-lint:    npx eslint frontend/src
fe-type:    npx tsc -b --noEmit
fe-test:    npx vitest run --coverage
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
