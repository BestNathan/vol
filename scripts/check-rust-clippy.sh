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
