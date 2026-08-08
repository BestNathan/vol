# Git Hooks Quality Gates Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the monolithic pre-commit hook with a two-layer system: fast checks (fmt, lint, typecheck) at pre-commit, slow checks (coverage) at pre-push. Add frontend quality gating (prettier + eslint + tsc + vitest coverage).

**Architecture:** Root-level `scripts/check-*.sh` are thin wrappers that call into Rust tooling (cargo fmt/clippy/coverage) and frontend npm scripts. `.githooks/pre-commit` and `pre-push` detect changes and dispatch to scripts. All frontend config lives under `frontend/`.

**Tech Stack:** bash, cargo/rustc, prettier, eslint (flat config), tsc, vitest, cargo-llvm-cov

## Global Constraints

- Pre-commit must complete in <15s (fmt + clippy + prettier + eslint + tsc)
- Pre-push runs only changed crates/targets, not the full workspace
- No `--no-verify` bypass in hooks
- All frontend config goes under `frontend/`, not repo root
- Root scripts never contain tool-specific configuration
- Thresholds: Rust per-crate (already defined in `check-rust-coverage.sh`), frontend TBD in vitest.config.ts
- No doc tests: verify with `./scripts/check-no-doc-tests.sh`

---

### Task 1: Install frontend devDependencies

**Files:**
- Modify: `frontend/package.json`

**Interfaces:**
- Produces: npm packages available for prettier, eslint, tsc, vitest coverage

- [ ] **Step 1: Install packages**

Run:
```bash
cd frontend && npm install --save-dev \
  prettier@^3 \
  eslint@^9 \
  @eslint/js@^9 \
  typescript-eslint@^8 \
  eslint-plugin-react-hooks@^5 \
  eslint-plugin-react-refresh@^0.4 \
  @vitest/coverage-v8@^3
```

- [ ] **Step 2: Verify install**

Run: `cd frontend && npx prettier --version && npx eslint --version && npx tsc --version`
Expected: All three print version numbers without errors.

- [ ] **Step 3: Commit**

```bash
git add frontend/package.json frontend/package-lock.json
git commit -m "chore(frontend): add prettier, eslint, vitest coverage deps"
```

---

### Task 2: Create .prettierrc and .prettierignore

**Files:**
- Create: `frontend/.prettierrc`
- Create: `frontend/.prettierignore`

**Interfaces:**
- Produces: prettier format config consumed by npm scripts `format` and `format:check`

- [ ] **Step 1: Write .prettierrc**

```json
{
  "semi": false,
  "singleQuote": true,
  "trailingComma": "all",
  "printWidth": 100,
  "tabWidth": 2
}
```

- [ ] **Step 2: Write .prettierignore**

```
node_modules
dist
coverage
```

- [ ] **Step 3: Verify prettier reads config**

Run: `cd frontend && npx prettier --check .prettierrc`
Expected: "All matched files use Prettier code style!" or similar success message.

- [ ] **Step 4: Commit**

```bash
git add frontend/.prettierrc frontend/.prettierignore
git commit -m "chore(frontend): add prettier config"
```

---

### Task 3: Create eslint.config.js and .eslintignore

**Files:**
- Create: `frontend/eslint.config.js`
- Create: `frontend/.eslintignore`

**Interfaces:**
- Produces: eslint flat config consumed by npm script `lint`

- [ ] **Step 1: Check existing tsconfig path**

Run: `ls frontend/tsconfig.json frontend/tsconfig.app.json frontend/tsconfig.node.json 2>/dev/null`

- [ ] **Step 2: Write eslint.config.js**

```javascript
import js from '@eslint/js'
import tseslint from 'typescript-eslint'
import reactHooks from 'eslint-plugin-react-hooks'
import reactRefresh from 'eslint-plugin-react-refresh'

export default tseslint.config(
  { ignores: ['dist', 'coverage', 'node_modules'] },
  {
    extends: [js.configs.recommended, ...tseslint.configs.recommended],
    files: ['src/**/*.{ts,tsx}'],
    plugins: {
      'react-hooks': reactHooks,
      'react-refresh': reactRefresh,
    },
    rules: {
      ...reactHooks.configs.recommended.rules,
      'react-refresh/only-export-components': ['warn', { allowConstantExport: true }],
      '@typescript-eslint/no-unused-vars': ['error', { argsIgnorePattern: '^_' }],
    },
  },
)
```

- [ ] **Step 3: Write .eslintignore**

```
node_modules
dist
coverage
```

- [ ] **Step 4: Verify eslint reads config**

Run: `cd frontend && npx eslint --help 2>&1 | head -1`
Expected: prints eslint help text.

- [ ] **Step 5: Commit**

```bash
git add frontend/eslint.config.js frontend/.eslintignore
git commit -m "chore(frontend): add eslint flat config"
```

---

### Task 4: Add npm scripts to frontend/package.json

**Files:**
- Modify: `frontend/package.json`

**Interfaces:**
- Produces: `format`, `format:check`, `lint`, `typecheck`, `test:coverage` npm scripts
- Consumed by: `scripts/check-fe-*.sh` wrappers and Makefile targets

- [ ] **Step 1: Read current scripts block**

Read `frontend/package.json` to see exact current content.

- [ ] **Step 2: Add scripts**

The current scripts block:
```json
"scripts": {
  "dev": "vite",
  "build": "tsc -b && vite build",
  "preview": "vite preview",
  "test": "vitest",
  "test:run": "vitest run"
}
```

Add these new entries:
```json
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
```

- [ ] **Step 3: Verify new scripts are valid**

Run each:
```bash
cd frontend && npm run format:check 2>&1 | tail -3
cd frontend && npm run typecheck 2>&1 | tail -5
```
Expected: format:check may show unformatted files (expected before Task 5). typecheck should pass or show pre-existing type errors.

- [ ] **Step 4: Commit**

```bash
git add frontend/package.json
git commit -m "chore(frontend): add format, lint, typecheck, test:coverage npm scripts"
```

---

### Task 5: Format existing frontend code with prettier

**Files:**
- Modify: all `frontend/src/**/*.{ts,tsx,css,json}` (format only, no logic changes)

**Interfaces:**
- Consumes: `npm run format` from Task 4
- Produces: all frontend source files formatted consistently

- [ ] **Step 1: Run format**

```bash
cd frontend && npm run format
```

- [ ] **Step 2: Verify formatting**

```bash
cd frontend && npm run format:check
```
Expected: "All matched files use Prettier code style!"

- [ ] **Step 3: Commit**

```bash
git add frontend/src/
git commit -m "style(frontend): apply prettier formatting to all source files"
```

---

### Task 6: Update vitest.config.ts with coverage

**Files:**
- Modify: `frontend/vitest.config.ts`

**Interfaces:**
- Produces: coverage configuration consumed by `npm run test:coverage`

- [ ] **Step 1: Read current vitest config**

Read `frontend/vitest.config.ts` to see exact current content.

- [ ] **Step 2: Add coverage config**

Append to the existing config's `test` block (or add it if not present):

```typescript
  test: {
    // ... existing config ...
    coverage: {
      provider: 'v8',
      reporter: ['text', 'lcov'],
      include: ['src/**/*.{ts,tsx}'],
      exclude: [
        'src/components/ui/**',  // shadcn/ui generated primitives
        'src/**/*.d.ts',
      ],
      thresholds: {
        lines: 60,
        functions: 60,
        branches: 50,
        statements: 60,
      },
    },
  },
```

- [ ] **Step 3: Run coverage to verify no crash**

```bash
cd frontend && npm run test:coverage 2>&1 | tail -10
```
Expected: vitest runs with coverage output. May fail thresholds (existing code not at 60% yet), but should not crash.

- [ ] **Step 4: Commit**

```bash
git add frontend/vitest.config.ts
git commit -m "chore(frontend): add vitest coverage config with v8 provider"
```

---

### Task 7: Create scripts/detect-changes.sh

**Files:**
- Create: `scripts/detect-changes.sh`

**Interfaces:**
- Consumes: `git diff --cached --name-only` (pre-commit) or `git diff origin/main...HEAD --name-only` (pre-push)
- Produces: sets `RUST_CHANGED=true` and `FRONTEND_CHANGED=true` env vars (sourced by hooks)

- [ ] **Step 1: Write script**

```bash
#!/usr/bin/env bash
# Detect changed languages from a file list on stdin.
# Source this script or use eval to set RUST_CHANGED / FRONTEND_CHANGED.
#
# Usage:
#   git diff --cached --name-only --diff-filter=ACM | ./scripts/detect-changes.sh
#   git diff origin/main...HEAD --name-only | ./scripts/detect-changes.sh
set -euo pipefail

FILES=$(cat)

RUST_CHANGED=false
FRONTEND_CHANGED=false

if echo "$FILES" | grep -q '\.rs$'; then
  RUST_CHANGED=true
fi

if echo "$FILES" | grep -q '^frontend/src/'; then
  FRONTEND_CHANGED=true
fi

# Also check for Cargo.toml / frontend/package.json changes
if echo "$FILES" | grep -q '^Cargo.toml$'; then
  RUST_CHANGED=true
fi

if echo "$FILES" | grep -q '^frontend/package.json$\|^frontend/package-lock.json$'; then
  FRONTEND_CHANGED=true
fi

echo "RUST_CHANGED=$RUST_CHANGED"
echo "FRONTEND_CHANGED=$FRONTEND_CHANGED"

# Return non-zero if nothing changed at all
if [ "$RUST_CHANGED" = "false" ] && [ "$FRONTEND_CHANGED" = "false" ]; then
  echo "NO_CHANGES=true"
else
  echo "NO_CHANGES=false"
fi
```

- [ ] **Step 2: Make executable**

```bash
chmod +x scripts/detect-changes.sh
```

- [ ] **Step 3: Test with dummy input**

```bash
echo "crates/vol-agent-server/src/main.rs" | ./scripts/detect-changes.sh
echo "frontend/src/App.tsx" | ./scripts/detect-changes.sh
echo "README.md" | ./scripts/detect-changes.sh
```
Expected: first prints RUST_CHANGED=true, second prints FRONTEND_CHANGED=true, third prints NO_CHANGES=true.

- [ ] **Step 4: Commit**

```bash
git add scripts/detect-changes.sh
git commit -m "feat: add detect-changes.sh for language-aware hook dispatch"
```

---

### Task 8: Create scripts/check-rust-fmt.sh

**Files:**
- Create: `scripts/check-rust-fmt.sh`

**Interfaces:**
- Consumes: nothing (calls cargo directly)
- Produces: exit 0 on pass, exit 1 with file diffs on fail

- [ ] **Step 1: Write script**

```bash
#!/usr/bin/env bash
# Check Rust formatting.  Exit 0 = clean, 1 = needs fmt.
set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

echo -e "${YELLOW}→ Checking Rust formatting...${NC}"

if cargo fmt --all -- --check 2>&1; then
  echo -e "${GREEN}✓ rustfmt check passed${NC}"
  exit 0
fi

echo ""
echo -e "${RED}✗ rustfmt check failed${NC}"
echo "─────────────────────────────────"
echo "  Some .rs files are not formatted."
echo "  Fix: cargo fmt --all"
echo "─────────────────────────────────"
echo ""
echo "This check must pass before committing. No bypass available."
exit 1
```

- [ ] **Step 2: Make executable**

```bash
chmod +x scripts/check-rust-fmt.sh
```

- [ ] **Step 3: Test pass case**

```bash
./scripts/check-rust-fmt.sh
```
Expected: PASS (code is already formatted from previous commit hooks).

- [ ] **Step 4: Commit**

```bash
git add scripts/check-rust-fmt.sh
git commit -m "feat: add check-rust-fmt.sh quality gate script"
```

---

### Task 9: Create scripts/check-rust-clippy.sh

**Files:**
- Create: `scripts/check-rust-clippy.sh`

**Interfaces:**
- Consumes: nothing (calls cargo directly)
- Produces: exit 0 on pass, exit 1 with warning locations on fail

- [ ] **Step 1: Write script**

```bash
#!/usr/bin/env bash
# Check Rust code with clippy.  Exit 0 = clean, 1 = warnings found.
set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

echo -e "${YELLOW}→ Checking Rust code with clippy...${NC}"

if cargo clippy --workspace 2>&1; then
  echo -e "${GREEN}✓ clippy check passed${NC}"
  exit 0
fi

echo ""
echo -e "${RED}✗ clippy check failed${NC}"
echo "─────────────────────────────────"
echo "  Fix the warnings above and commit again."
echo "  Each warning includes file:line in the output."
echo "─────────────────────────────────"
echo ""
echo "This check must pass before committing. No bypass available."
exit 1
```

- [ ] **Step 2: Make executable**

```bash
chmod +x scripts/check-rust-clippy.sh
```

- [ ] **Step 3: Test pass case**

```bash
./scripts/check-rust-clippy.sh
```
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add scripts/check-rust-clippy.sh
git commit -m "feat: add check-rust-clippy.sh quality gate script"
```

---

### Task 10: Create scripts/check-fe-format.sh

**Files:**
- Create: `scripts/check-fe-format.sh`

**Interfaces:**
- Consumes: `npm --prefix frontend run format:check` (Task 4)
- Produces: exit 0 on pass, exit 1 with unformatted file list on fail

- [ ] **Step 1: Write script**

```bash
#!/usr/bin/env bash
# Check frontend formatting with prettier.
set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

echo -e "${YELLOW}→ Checking frontend formatting (prettier)...${NC}"

if npm --prefix frontend run format:check 2>&1; then
  echo -e "${GREEN}✓ prettier check passed${NC}"
  exit 0
fi

echo ""
echo -e "${RED}✗ prettier check failed${NC}"
echo "─────────────────────────────────"
echo "  Some frontend files are not formatted."
echo "  Fix: npm --prefix frontend run format"
echo "─────────────────────────────────"
echo ""
echo "This check must pass before committing. No bypass available."
exit 1
```

- [ ] **Step 2: Make executable**

```bash
chmod +x scripts/check-fe-format.sh
```

- [ ] **Step 3: Test pass case**

```bash
./scripts/check-fe-format.sh
```
Expected: PASS (formatted in Task 5).

- [ ] **Step 4: Commit**

```bash
git add scripts/check-fe-format.sh
git commit -m "feat: add check-fe-format.sh quality gate script"
```

---

### Task 11: Create scripts/check-fe-lint.sh

**Files:**
- Create: `scripts/check-fe-lint.sh`

**Interfaces:**
- Consumes: `npm --prefix frontend run lint` (Task 4)
- Produces: exit 0 on pass, exit 1 with eslint errors on fail

- [ ] **Step 1: Write script**

```bash
#!/usr/bin/env bash
# Lint frontend code with eslint.
set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

echo -e "${YELLOW}→ Linting frontend code (eslint)...${NC}"

if npm --prefix frontend run lint 2>&1; then
  echo -e "${GREEN}✓ eslint check passed${NC}"
  exit 0
fi

echo ""
echo -e "${RED}✗ eslint check failed${NC}"
echo "─────────────────────────────────"
echo "  Each error above includes file:line and rule name."
echo "  Fix the lint errors and commit again."
echo "─────────────────────────────────"
echo ""
echo "This check must pass before committing. No bypass available."
exit 1
```

- [ ] **Step 2: Make executable**

```bash
chmod +x scripts/check-fe-lint.sh
```

- [ ] **Step 3: Test and fix any pre-existing lint errors**

```bash
./scripts/check-fe-lint.sh 2>&1 | tail -20
```
If eslint finds pre-existing issues: fix them, commit separately. If clean: proceed.

- [ ] **Step 4: Commit**

```bash
git add scripts/check-fe-lint.sh
git commit -m "feat: add check-fe-lint.sh quality gate script"
```

---

### Task 12: Create scripts/check-fe-type.sh

**Files:**
- Create: `scripts/check-fe-type.sh`

**Interfaces:**
- Consumes: `npm --prefix frontend run typecheck` (Task 4)
- Produces: exit 0 on pass, exit 1 with tsc errors on fail

- [ ] **Step 1: Write script**

```bash
#!/usr/bin/env bash
# Type-check frontend code with tsc.
set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

echo -e "${YELLOW}→ Type-checking frontend code (tsc)...${NC}"

if npm --prefix frontend run typecheck 2>&1; then
  echo -e "${GREEN}✓ tsc typecheck passed${NC}"
  exit 0
fi

echo ""
echo -e "${RED}✗ tsc typecheck failed${NC}"
echo "─────────────────────────────────"
echo "  Each error above includes file:line and error code."
echo "  Fix the type errors and commit again."
echo "─────────────────────────────────"
echo ""
echo "This check must pass before committing. No bypass available."
exit 1
```

- [ ] **Step 2: Make executable**

```bash
chmod +x scripts/check-fe-type.sh
```

- [ ] **Step 3: Test and fix any pre-existing type errors**

```bash
./scripts/check-fe-type.sh 2>&1 | tail -20
```
If tsc finds pre-existing errors: fix them, commit separately. If clean: proceed.

- [ ] **Step 4: Commit**

```bash
git add scripts/check-fe-type.sh
git commit -m "feat: add check-fe-type.sh quality gate script"
```

---

### Task 13: Create scripts/check-fe-test.sh

**Files:**
- Create: `scripts/check-fe-test.sh`

**Interfaces:**
- Consumes: `npm --prefix frontend run test:coverage` (Task 4)
- Produces: exit 0 on pass, exit 1 if tests fail or coverage below threshold

- [ ] **Step 1: Write script**

```bash
#!/usr/bin/env bash
# Run frontend tests with coverage.  For pre-push gate.
set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

echo -e "${YELLOW}→ Running frontend tests with coverage...${NC}"

if npm --prefix frontend run test:coverage 2>&1; then
  echo -e "${GREEN}✓ frontend tests passed with coverage${NC}"
  exit 0
fi

echo ""
echo -e "${RED}✗ frontend test/coverage check failed${NC}"
echo "─────────────────────────────────"
echo "  Either tests failed or coverage is below threshold."
echo "  Check the output above for details."
echo "  Fix: add tests to reach coverage targets."
echo "─────────────────────────────────"
echo ""
echo "This check must pass before pushing. No bypass available."
exit 1
```

- [ ] **Step 2: Make executable**

```bash
chmod +x scripts/check-fe-test.sh
```

- [ ] **Step 3: Test run (may need coverage threshold adjustment)**

```bash
./scripts/check-fe-test.sh 2>&1 | tail -15
```
Expected: may fail on coverage thresholds if existing tests don't reach 60%. Adjust threshold in `vitest.config.ts` to current coverage level if needed, leave a TODO to raise it.

- [ ] **Step 4: Commit**

```bash
git add scripts/check-fe-test.sh
git commit -m "feat: add check-fe-test.sh quality gate script"
```

---

### Task 14: Modify scripts/check-rust-coverage.sh to accept crate args

**Files:**
- Modify: `scripts/check-rust-coverage.sh`

**Interfaces:**
- Consumes: crate names as positional args (e.g. `check-rust-coverage.sh vol-agent-server vol-llm-agent`)
- Produces: same output format, but scoped to given crates only

- [ ] **Step 1: Read current script**

Read `scripts/check-rust-coverage.sh` (already exists — was read during brainstorming).

- [ ] **Step 2: Verify arg support already works**

The script already supports `./scripts/check-coverage.sh crate1 crate2`. Verify no changes needed.

```bash
grep -A5 'Filter to specified crates' scripts/check-rust-coverage.sh
```

- [ ] **Step 3: If needed, add usage comment at top of script**

Only if missing — add a usage line:
```bash
# Usage:
#   ./scripts/check-rust-coverage.sh                 # Check all crates
#   ./scripts/check-rust-coverage.sh crate1 crate2   # Check only specified crates
```

- [ ] **Step 4: Test with a single crate**

```bash
./scripts/check-rust-coverage.sh vol-agent-server 2>&1 | tail -10
```
Expected: runs coverage only for vol-agent-server.

- [ ] **Step 5: Commit** (only if changes were made)

```bash
git add scripts/check-rust-coverage.sh
git commit -m "chore: add usage comment to check-rust-coverage.sh"
```

---

### Task 15: Rewrite .githooks/pre-commit

**Files:**
- Modify: `.githooks/pre-commit`

**Interfaces:**
- Consumes: `scripts/detect-changes.sh`, `scripts/check-rust-fmt.sh`, `scripts/check-rust-clippy.sh`, `scripts/check-fe-format.sh`, `scripts/check-fe-lint.sh`, `scripts/check-fe-type.sh`
- Produces: exit 0 or 1

- [ ] **Step 1: Write new pre-commit hook**

```bash
#!/usr/bin/env bash
# Pre-commit quality gate: fast checks only (fmt, lint, typecheck).
# Slow checks (coverage) run at pre-push.
#
# All checks run in series.  First failure stops the chain.
# No bypass: all checks must pass before committing.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")/.." && pwd)"

# ── Detect what changed ───────────────────────────────────────────────
CHANGES=$(git diff --cached --name-only --diff-filter=ACM)
if [ -z "$CHANGES" ]; then
  exit 0
fi

RUST_CHANGED=false
FRONTEND_CHANGED=false

if echo "$CHANGES" | grep -q '\.rs$'; then
  RUST_CHANGED=true
fi
if echo "$CHANGES" | grep -q '^frontend/src/'; then
  FRONTEND_CHANGED=true
fi
if echo "$CHANGES" | grep -q '^Cargo.toml$'; then
  RUST_CHANGED=true
fi
if echo "$CHANGES" | grep -q '^frontend/package.json$\|^frontend/package-lock.json$'; then
  FRONTEND_CHANGED=true
fi

if [ "$RUST_CHANGED" = "false" ] && [ "$FRONTEND_CHANGED" = "false" ]; then
  exit 0
fi

HAS_ERROR=0

# ── Rust checks ───────────────────────────────────────────────────────
if [ "$RUST_CHANGED" = "true" ]; then
  "$SCRIPT_DIR/scripts/check-rust-fmt.sh" || HAS_ERROR=1
  if [ $HAS_ERROR -eq 1 ]; then exit 1; fi

  "$SCRIPT_DIR/scripts/check-rust-clippy.sh" || HAS_ERROR=1
  if [ $HAS_ERROR -eq 1 ]; then exit 1; fi
fi

# ── Frontend checks ───────────────────────────────────────────────────
if [ "$FRONTEND_CHANGED" = "true" ]; then
  "$SCRIPT_DIR/scripts/check-fe-format.sh" || HAS_ERROR=1
  if [ $HAS_ERROR -eq 1 ]; then exit 1; fi

  "$SCRIPT_DIR/scripts/check-fe-lint.sh" || HAS_ERROR=1
  if [ $HAS_ERROR -eq 1 ]; then exit 1; fi

  "$SCRIPT_DIR/scripts/check-fe-type.sh" || HAS_ERROR=1
  if [ $HAS_ERROR -eq 1 ]; then exit 1; fi
fi

echo ""
echo -e "\033[0;32mAll pre-commit checks passed ✓\033[0m"
exit 0
```

- [ ] **Step 2: Verify hook fires on staged Rust changes**

```bash
echo "// test" >> crates/vol-agent-server/src/main.rs
git add crates/vol-agent-server/src/main.rs
.git/hooks/pre-commit 2>&1 | tail -5
git checkout -- crates/vol-agent-server/src/main.rs
```
Expected: runs rust fmt + clippy. Pass or fail depending on content.

- [ ] **Step 3: Verify hook fires on staged frontend changes**

```bash
echo "// test" >> frontend/src/main.tsx  # or any frontend file
git add frontend/src/main.tsx
.git/hooks/pre-commit 2>&1 | tail -5
git checkout -- frontend/src/main.tsx
```
Expected: runs fe format + lint + typecheck.

- [ ] **Step 4: Commit**

```bash
git add .githooks/pre-commit
git commit -m "refactor(githooks): rewrite pre-commit for fast checks only"
```

---

### Task 16: Create .githooks/pre-push

**Files:**
- Create: `.githooks/pre-push`

**Interfaces:**
- Consumes: `scripts/check-rust-coverage.sh` (with changed crate names), `scripts/check-fe-test.sh`
- Produces: exit 0 or 1

- [ ] **Step 1: Write pre-push hook**

```bash
#!/usr/bin/env bash
# Pre-push quality gate: slow checks (coverage, integration tests).
# Fast checks run at pre-commit, so we assume those already passed.
#
# Runs only for changed crates/modules against origin/main.
# No bypass: all checks must pass before pushing.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")/.." && pwd)"

# ── Detect what changed in the push range ─────────────────────────────
REMOTE=${1:-origin}
REMOTE_URL=${2:-}

# Get the range of commits being pushed, or fall back to main..HEAD
PUSH_RANGE=""
while read -r local_ref local_sha remote_ref remote_sha; do
  if [ -n "$remote_sha" ] && [ "$remote_sha" != "0000000000000000000000000000000000000000" ]; then
    PUSH_RANGE="$remote_sha..$local_sha"
  else
    PUSH_RANGE="origin/main..HEAD"
  fi
  break
done

if [ -z "$PUSH_RANGE" ]; then
  PUSH_RANGE="origin/main..HEAD"
fi

CHANGES=$(git diff --name-only "$PUSH_RANGE" 2>/dev/null || git diff --name-only origin/main...HEAD)
if [ -z "$CHANGES" ]; then
  echo "No changes to push — skipping pre-push checks."
  exit 0
fi

RUST_CHANGED=false
FRONTEND_CHANGED=false

if echo "$CHANGES" | grep -q '\.rs$'; then
  RUST_CHANGED=true
fi
if echo "$CHANGES" | grep -q '^frontend/src/'; then
  FRONTEND_CHANGED=true
fi

if [ "$RUST_CHANGED" = "false" ] && [ "$FRONTEND_CHANGED" = "false" ]; then
  echo "No Rust or frontend changes in push range — skipping."
  exit 0
fi

HAS_ERROR=0

# ── Rust coverage (changed crates only) ────────────────────────────────
if [ "$RUST_CHANGED" = "true" ]; then
  # Extract changed crate names
  CHANGED_CRATES=$(echo "$CHANGES" \
    | grep '^crates/' \
    | sed -n 's|^crates/\([^/]*\)/.*|\1|p' \
    | sort -u \
    | tr '\n' ' ' \
    || true)

  if [ -n "$CHANGED_CRATES" ]; then
    echo "→ Changed crates: $CHANGED_CRATES"
    "$SCRIPT_DIR/scripts/check-rust-coverage.sh" $CHANGED_CRATES || HAS_ERROR=1
  else
    echo "→ No crates changed in push range — skipping Rust coverage."
  fi

  if [ $HAS_ERROR -eq 1 ]; then exit 1; fi
fi

# ── Frontend coverage ──────────────────────────────────────────────────
if [ "$FRONTEND_CHANGED" = "true" ]; then
  "$SCRIPT_DIR/scripts/check-fe-test.sh" || HAS_ERROR=1
  if [ $HAS_ERROR -eq 1 ]; then exit 1; fi
fi

echo ""
echo -e "\033[0;32mAll pre-push checks passed ✓\033[0m"
exit 0
```

- [ ] **Step 2: Make executable**

```bash
chmod +x .githooks/pre-push
```

- [ ] **Step 3: Symlink into .git/hooks**

```bash
ln -sf ../../.githooks/pre-commit .git/hooks/pre-commit
ln -sf ../../.githooks/pre-push .git/hooks/pre-push
```

- [ ] **Step 4: Test with a dry push**

```bash
.git/hooks/pre-push origin main 2>&1 | tail -10
```
Expected: detects change range and runs checks (or skips if no changes).

- [ ] **Step 5: Commit**

```bash
git add .githooks/pre-push .git/hooks/pre-commit .git/hooks/pre-push
git commit -m "feat(githooks): add pre-push hook for coverage gates"
```

---

### Task 17: Add Makefile targets for frontend quality

**Files:**
- Modify: `Makefile`

**Interfaces:**
- Consumes: npm scripts from Task 4
- Produces: `fe-fmt`, `fe-fmt-check`, `fe-lint`, `fe-type`, `fe-test` Make targets

- [ ] **Step 1: Read Makefile to find where to add targets**

Read `Makefile`, find the section after existing `web-*` targets.

- [ ] **Step 2: Add frontend quality targets**

```makefile
fe-fmt: ## Format frontend code with prettier
	npm --prefix frontend run format

fe-fmt-check: ## Check frontend formatting
	npm --prefix frontend run format:check

fe-lint: ## Lint frontend code with eslint
	npm --prefix frontend run lint

fe-type: ## Type-check frontend code with tsc
	npm --prefix frontend run typecheck

fe-test: ## Run frontend tests with coverage
	npm --prefix frontend run test:coverage
```

- [ ] **Step 3: Update .PHONY line**

Add `fe-fmt fe-fmt-check fe-lint fe-type fe-test` to the existing `.PHONY:` declaration at the top of the Makefile.

- [ ] **Step 4: Verify a target works**

```bash
make fe-fmt-check
```
Expected: runs prettier --check.

- [ ] **Step 5: Commit**

```bash
git add Makefile
git commit -m "feat(make): add frontend quality targets (fe-fmt, fe-lint, fe-type, fe-test)"
```

---

### Task 18: End-to-end verification

**Files:**
- None (verification only)

**Interfaces:**
- Consumes: all previous tasks
- Produces: confirmed working hooks, or list of issues to fix

- [ ] **Step 1: Test pre-commit with Rust change**

```bash
# Make a temporary formatting error
echo "let x =  1;" >> /tmp/test_rust_fmt.rs
cp /tmp/test_rust_fmt.rs crates/vol-core/src/lib.rs.bak
cp /tmp/test_rust_fmt.rs crates/vol-core/src/test_quality_hook.rs
git add crates/vol-core/src/test_quality_hook.rs
.git/hooks/pre-commit
# Expected: FAIL on fmt. Then:
rm crates/vol-core/src/test_quality_hook.rs
git reset HEAD crates/vol-core/src/test_quality_hook.rs
```

- [ ] **Step 2: Test pre-commit with frontend change**

```bash
# Make a prettier formatting error
echo "const x   =   1" > /tmp/test_fe.js
cp /tmp/test_fe.js frontend/src/test-quality-hook.ts
git add frontend/src/test-quality-hook.ts
.git/hooks/pre-commit
# Expected: FAIL on prettier. Then:
rm frontend/src/test-quality-hook.ts
git reset HEAD frontend/src/test-quality-hook.ts
```

- [ ] **Step 3: Test pre-push coverage gate**

```bash
# Push to a test branch (or dry-run)
# Verify pre-push fires and runs coverage
```

- [ ] **Step 4: Test no-change commit**

```bash
# Commit with only README changes — both hooks should skip
echo "test" >> README.md
git add README.md
.git/hooks/pre-commit
# Expected: exit 0 immediately (no Rust/fe changes)
git checkout -- README.md
```

- [ ] **Step 5: Run full CI to confirm nothing breaks**

```bash
# The CI already runs quality gates — confirm they still pass
```

- [ ] **Step 6: Document final state**

Note any issues found during verification and their fixes.

---

### Appendix A: Frontend coverage threshold tuning

The initial vitest coverage threshold is set to 60% (lines/functions/statements) and 50% (branches). If existing tests don't meet this, adjust downward to the current coverage level in Task 6, and file a follow-up issue to raise it.

### Appendix B: Hook installation

Git doesn't automatically use `.githooks/`. The repo's clone/setup script (or a `make setup` target) should run:

```bash
git config core.hooksPath .githooks
```

This makes git look for hooks in `.githooks/` instead of `.git/hooks/`. If this config is not set, developers need to symlink manually. Check if the repo already has this configured.

