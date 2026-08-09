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
