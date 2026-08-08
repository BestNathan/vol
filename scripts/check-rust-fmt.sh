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
