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
RUST_DIFF_ARGS="-- '*.rs'"
if git rev-parse --verify HEAD >/dev/null 2>&1 && \
   git diff --cached --quiet 2>/dev/null; then
  # Nothing staged — we're in pre-push; check committed changes vs main
  DIFF=$(git diff origin/main...HEAD -U0 -- '*.rs' 2>/dev/null || git diff HEAD~1 -U0 -- '*.rs')
else
  DIFF=$(git diff --cached -U0 -- '*.rs')
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
