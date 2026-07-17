#!/usr/bin/env bash
# Per-crate coverage threshold check for Rust workspace using cargo-llvm-cov
# Exit 0 if all crates meet their thresholds, exit 1 otherwise
#
# Usage:
#   ./scripts/check-coverage.sh                 # Check all crates
#   ./scripts/check-coverage.sh crate1 crate2   # Check only specified crates

set -euo pipefail

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

# ── Coverage thresholds per crate (line coverage percentage) ────────────
# Core agent/server crates: 80%+ target (per CLAUDE.md)
# UI/TUI crates: lower threshold (rendering code not easily testable)
# CLI tools: lower threshold
# Excluded from coverage: main.rs, app.rs, health.rs (per CLAUDE.md)
declare -A THRESHOLDS=(
    # Core infrastructure
    ["vol-core"]=80
    ["vol-config"]=80
    ["vol-eventbus"]=80
    ["vol-tracing"]=80
    ["vol-observability"]=80

    # Data / protocol
    ["vol-datasource"]=80
    ["vol-deribit"]=80
    ["vol-alert"]=80
    ["vol-notification"]=80
    ["vol-rules"]=80
    ["vol-engine"]=80
    ["vol-tdengine"]=80

    # LLM core
    ["vol-llm-core"]=80
    ["vol-llm-provider"]=80
    ["vol-llm-tool"]=80
    ["vol-llm-agent"]=80
    ["vol-llm-agents"]=80

    # Agent infrastructure
    ["vol-llm-agent-protocol"]=80
    ["vol-llm-runtime"]=80
    ["vol-llm-task"]=80
    ["vol-llm-skill"]=80
    ["vol-llm-context"]=80
    ["vol-llm-memory"]=80
    ["vol-llm-mcp"]=80
    ["vol-llm-sandbox"]=80
    ["vol-llm-wiki"]=80
    ["vol-llm-observability"]=80
    ["vol-session"]=80
    ["vol-agent-server"]=80

    # Tools
    ["vol-llm-tools-builtin"]=80
    ["vol-llm-yaml-agent"]=80
    ["md-frontmatter"]=80
    ["vol-mcp-servers"]=80

    # UI / TUI — lower threshold (rendering code)
    ["vol-llm-ui"]=40
    ["vol-llm-tui"]=40

    # CLI tools — lower threshold
    ["vol-llm-cli-tool"]=40

    # Special agents
    ["ppt-agent"]=40
    ["vol-llm-tdengine"]=40
    ["vol-monitor"]=40
)

# Filter to specified crates if arguments provided
if [ $# -gt 0 ]; then
    declare -A FILTERED
    for arg in "$@"; do
        if [ -n "${THRESHOLDS[$arg]+x}" ]; then
            FILTERED[$arg]=${THRESHOLDS[$arg]}
        fi
    done
    # Replace THRESHOLDS with filtered version
    unset THRESHOLDS
    declare -A THRESHOLDS
    for key in "${!FILTERED[@]}"; do
        THRESHOLDS[$key]=${FILTERED[$key]}
    done
    if [ ${#THRESHOLDS[@]} -eq 0 ]; then
        echo -e "${YELLOW}→ No matching crates to check, skipping coverage${NC}"
        exit 0
    fi
fi

echo -e "${YELLOW}→ Checking Rust test coverage by crate...${NC}"
echo ""

# Build list of -p flags for llvm-cov to only test specified crates
PACKAGE_FLAGS=""
for crate in "${!THRESHOLDS[@]}"; do
    PACKAGE_FLAGS="$PACKAGE_FLAGS -p $crate"
done

# Run llvm-cov with JSON output on just the target crates (faster)
JSON=$(cargo llvm-cov $PACKAGE_FLAGS --json 2>/dev/null)

if [ -z "$JSON" ]; then
    echo -e "${RED}✗ cargo llvm-cov not installed or failed${NC}"
    echo -e "${RED}  Install: cargo install cargo-llvm-cov${NC}"
    echo -e "${RED}  Then: rustup component add llvm-tools-preview${NC}"
    exit 1
fi

HAS_ERROR=0

# Parse coverage per crate using jq
for crate in "${!THRESHOLDS[@]}"; do
    threshold=${THRESHOLDS[$crate]}

    # Sum lines/covered across all files in this crate (excluding main.rs, app.rs, health.rs)
    result=$(echo "$JSON" | jq -r --arg crate "$crate" '
        [.data[0].files[]
         | select(.filename | contains("/crates/" + $crate + "/"))
         | select(.filename | (endswith("/main.rs") or endswith("/app.rs") or endswith("/health.rs")) | not)]
        | {
            covered: (map(.summary.lines.covered) | add // 0),
            count: (map(.summary.lines.count) | add // 0)
          }
        | if .count > 0 then "\(.covered) \(.count)" else "0 0" end
    ')

    covered=$(echo "$result" | awk '{print $1}')
    count=$(echo "$result" | awk '{print $2}')

    if [ "$count" -eq 0 ]; then
        echo -e "${YELLOW}  ${crate}: no coverage data${NC}"
        continue
    fi

    coverage=$((covered * 100 / count))

    if [ $coverage -ge $threshold ]; then
        echo -e "${GREEN}  ✓ ${crate}: ${coverage}% (≥ ${threshold}%)  [${covered}/${count} lines]${NC}"
    else
        echo -e "${RED}  ✗ ${crate}: ${coverage}% (< ${threshold}%)  [${covered}/${count} lines]${NC}"
        HAS_ERROR=1
    fi
done

echo ""

if [ $HAS_ERROR -eq 1 ]; then
    echo -e "${RED}✗ Coverage check failed — some crates below threshold${NC}"
    exit 1
else
    echo -e "${GREEN}✓ All crates meet coverage thresholds${NC}"
    exit 0
fi
