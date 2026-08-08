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
