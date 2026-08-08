#!/usr/bin/env bash
# Detect changed languages from a file list on stdin.
# Source this script or use eval to set RUST_CHANGED / FRONTEND_CHANGED.
#
# Usage:
#   git diff --cached --name-only --diff-filter=ACM | ./scripts/detect-changes.sh
#   git diff origin/main...HEAD --name-only | ./scripts/detect-changes.sh
set -euo pipefail

FILES=$(cat)

RUST_CHANGED=false
FRONTEND_CHANGED=false

if echo "$FILES" | grep -q '\.rs$'; then
  RUST_CHANGED=true
fi

if echo "$FILES" | grep -q '^frontend/src/'; then
  FRONTEND_CHANGED=true
fi

# Also check for Cargo.toml / frontend/package.json changes
if echo "$FILES" | grep -q '^Cargo.toml$'; then
  RUST_CHANGED=true
fi

if echo "$FILES" | grep -q '^frontend/package.json$\|^frontend/package-lock.json$'; then
  FRONTEND_CHANGED=true
fi

echo "RUST_CHANGED=$RUST_CHANGED"
echo "FRONTEND_CHANGED=$FRONTEND_CHANGED"

# Return non-zero if nothing changed at all
if [ "$RUST_CHANGED" = "false" ] && [ "$FRONTEND_CHANGED" = "false" ]; then
  echo "NO_CHANGES=true"
else
  echo "NO_CHANGES=false"
fi
