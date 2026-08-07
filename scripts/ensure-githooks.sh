#!/usr/bin/env bash
# Ensure the project's githooks and base configuration are correctly set up.
# Safe to run repeatedly — idempotent.
set -euo pipefail

PROJECT_DIR="${CLAUDE_PROJECT_DIR:-$(git rev-parse --show-toplevel 2>/dev/null || echo "$PWD")}"
cd "$PROJECT_DIR"

HAS_CHANGE=0

# ── core.hooksPath ──────────────────────────────────────────────────────
CURRENT_HOOKS=$(git config core.hooksPath 2>/dev/null || echo "")
EXPECTED_HOOKS=".githooks"

if [ "$CURRENT_HOOKS" != "$EXPECTED_HOOKS" ]; then
  echo "→ Setting core.hooksPath to .githooks (was: ${CURRENT_HOOKS:-unset})"
  git config core.hooksPath "$EXPECTED_HOOKS"
  HAS_CHANGE=1
fi

# ── pre-commit hook exists and is executable ────────────────────────────
HOOK_SCRIPT="$PROJECT_DIR/.githooks/pre-commit"
if [ ! -x "$HOOK_SCRIPT" ]; then
  echo "→ Making pre-commit hook executable"
  chmod +x "$HOOK_SCRIPT"
  HAS_CHANGE=1
fi

# ── Report ──────────────────────────────────────────────────────────────
if [ "$HAS_CHANGE" -eq 1 ]; then
  echo "✓ Git hooks configured"
else
  echo "✓ Git hooks already correctly configured"
fi
