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
