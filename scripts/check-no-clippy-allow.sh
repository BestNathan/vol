#!/usr/bin/env bash
# Forbid #[allow(clippy::...)] in NEW production code.
# Existing allows are grandfathered; this check applies to staged changes only.
# Clippy warnings must be fixed at the root cause, not suppressed with allow.
set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

echo -e "${YELLOW}→ Checking for new #[allow(clippy::...)] in staged code...${NC}"

SCRIPT_DIR="$(cd "$(dirname "$(readlink -f "$0")")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$REPO_ROOT"

# Check lines ADDED in Rust files that contain #[allow(clippy::...)].
# Pre-commit: check staged diff. Pre-push: check commits vs origin/main.
# CI (no staged changes, no git history): check all source files.
if git rev-parse --verify HEAD >/dev/null 2>&1 && \
   ! git diff --cached --quiet 2>/dev/null; then
  # Staged changes exist — pre-commit mode
  DIFF=$(git diff --cached -U0 -- '*.rs')
elif git merge-base origin/main HEAD >/dev/null 2>&1; then
  # Pre-push / CI mode: check commits against origin/main
  DIFF=$(git diff origin/main...HEAD -U0 -- '*.rs')
else
  # Fallback: CI shallow clone, check everything (trust existing code is clean)
  echo -e "${YELLOW}  (CI fallback: checking all source files)${NC}"
  DIFF=""
  VIOLATIONS=$(grep -rn '#\[allow(clippy::' crates/ --include='*.rs' --exclude-dir=tests --exclude-dir=target 2>/dev/null || true)
  if [ -z "$VIOLATIONS" ]; then
    echo -e "${GREEN}✓ No clippy allow annotations found${NC}"
    exit 0
  fi
  # In CI fallback, all existing allows are errors
  echo ""
  echo -e "${RED}✗ #[allow(clippy::...)] found in source${NC}"
  echo "──────────────────────────────────────────────────────────────────"
  echo "$VIOLATIONS"
  echo "──────────────────────────────────────────────────────────────────"
  echo ""
  echo "  Do NOT silence warnings with #[allow(clippy::...)]."
  echo "  Fix the root cause instead of suppressing the lint."
  echo ""
  echo "This check must pass before committing. No bypass available."
  exit 1
fi

VIOLATIONS=$(echo "$DIFF" \
  | grep -E '^\+.*#\[allow\(clippy::' \
  | grep -v '^+++' \
  || true)

if [ -z "$VIOLATIONS" ]; then
  echo -e "${GREEN}✓ No new clippy allow annotations${NC}"
  exit 0
fi

echo ""
echo -e "${RED}✗ New #[allow(clippy::...)] found in staged changes${NC}"
echo "──────────────────────────────────────────────────────────────────"
echo "$VIOLATIONS"
echo "──────────────────────────────────────────────────────────────────"
echo ""
echo "  Do NOT silence warnings with #[allow(clippy::...)]."
echo "  Fix the root cause instead of suppressing the lint."
echo "  • Replace as-casts with try_from() / from()"
echo "  • Use proper error handling instead of expect/panic"
echo "  • Refactor to avoid the clippy warning entirely"
echo ""
echo "This check must pass before committing. No bypass available."
exit 1
