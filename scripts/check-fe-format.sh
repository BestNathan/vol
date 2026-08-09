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
