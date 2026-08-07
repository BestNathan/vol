#!/usr/bin/env bash
# ensure-config.sh — SessionStart hook
# Runs base project configuration checks so every session starts with the
# correct setup (githooks, etc.).

PROJECT_DIR="${CLAUDE_PROJECT_DIR:-.}"

SCRIPT="$PROJECT_DIR/scripts/ensure-githooks.sh"
if [ -x "$SCRIPT" ]; then
  bash "$SCRIPT" >&2
fi

# Always succeed — configuration warnings shouldn't block the session.
exit 0
