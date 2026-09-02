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
DEPLOY_CHANGED=false
WIKI_CHANGED=false

if echo "$FILES" | grep -q '\.rs$'; then
  RUST_CHANGED=true
fi

if echo "$FILES" | grep -q '^frontend/src/\|^frontend/tests/\|^frontend/vite.config'; then
  FRONTEND_CHANGED=true
fi

# Also check for Cargo.toml / frontend/package.json changes
if echo "$FILES" | grep -q '^Cargo.toml$'; then
  RUST_CHANGED=true
fi

if echo "$FILES" | grep -q '^frontend/package.json$\|^frontend/package-lock.json$'; then
  FRONTEND_CHANGED=true
fi

# deploy/ and k8s/ contain Kubernetes manifests — changes here trigger
# secret-scan and other deploy-related checks.
if echo "$FILES" | grep -q '^deploy/\|^k8s/'; then
  DEPLOY_CHANGED=true
fi

# docs/wiki/ changes trigger wiki-link validation in pre-commit.
if echo "$FILES" | grep -q '^docs/wiki/'; then
  WIKI_CHANGED=true
fi

echo "RUST_CHANGED=$RUST_CHANGED"
echo "FRONTEND_CHANGED=$FRONTEND_CHANGED"
echo "DEPLOY_CHANGED=$DEPLOY_CHANGED"
echo "WIKI_CHANGED=$WIKI_CHANGED"

# Use with eval: eval "$(git diff ... | ./scripts/detect-changes.sh)"
# Sets RUST_CHANGED, FRONTEND_CHANGED, DEPLOY_CHANGED, WIKI_CHANGED, NO_CHANGES.
# Always exits 0.
if [ "$RUST_CHANGED" = "false" ] && [ "$FRONTEND_CHANGED" = "false" ] && \
   [ "$DEPLOY_CHANGED" = "false" ] && [ "$WIKI_CHANGED" = "false" ]; then
  echo "NO_CHANGES=true"
else
  echo "NO_CHANGES=false"
fi
