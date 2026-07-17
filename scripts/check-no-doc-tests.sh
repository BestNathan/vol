#!/usr/bin/env bash
# Check that no crate has active doc tests (they should be proper tests).
# Doc tests are forbidden — write #[cfg(test)] unit tests or tests/ instead.
set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
NC='\033[0m'

# Find doc comment code blocks that are active Rust tests.
# Active means: ``` or ```rust (without ignore/no_run/text/json/toml/bash attributes).
# We search for /// ``` or //! ``` that opens a Rust code block.
VIOLATIONS=$(grep -rn '/// ```$\|/// ```rust$\|//! ```$\|//! ```rust$\|/// ```rust,no_run\|//! ```rust,no_run\|/// ```no_run\|//! ```no_run' crates/ --include="*.rs" | grep -v target/ || true)

if [ -n "$VIOLATIONS" ]; then
    COUNT=$(echo "$VIOLATIONS" | wc -l | tr -d ' ')
    echo -e "${RED}✗ Found ${COUNT} active doc test(s) — doc tests are forbidden${NC}"
    echo -e "${RED}  Convert them to #[cfg(test)] unit tests or tests/ integration tests${NC}"
    echo -e "${RED}  Or use \`\`\`text for documentation-only code blocks${NC}"
    echo "$VIOLATIONS"
    exit 1
fi

echo -e "${GREEN}✓ No active doc tests${NC}"
exit 0
