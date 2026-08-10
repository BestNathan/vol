#!/usr/bin/env bash
# Validate that the session's working directory exists.
# If CWD was deleted (stale worktree, temp dir cleanup, etc.), the harness
# tools (Read, Glob, Write, Edit) will all fail with "No such file or
# directory". This hook detects that early and prints clear guidance.
set -euo pipefail

CWD="${CLAUDE_PROJECT_DIR:-$(pwd 2>/dev/null || true)}"

if [ ! -d "$CWD" ]; then
  cat >&2 <<EOF

╔══════════════════════════════════════════════════════════════╗
║                    ⚠️  WORKING DIRECTORY GONE                 ║
╠══════════════════════════════════════════════════════════════╣
║ The session working directory no longer exists:              ║
║   $CWD
║                                                              ║
║ This causes Read, Write, Edit, Glob, and Bash to fail.       ║
║ Common causes:                                               ║
║   1. A git worktree was removed while the session was open   ║
║   2. A temporary directory was cleaned up                    ║
║   3. A Kubernetes mount was rotated                          ║
║                                                              ║
║ Fix: Restart the session from the repo root:                 ║
║   cd /root/vol && claude                                     ║
╚══════════════════════════════════════════════════════════════╝

EOF
  # Signal the harness that CWD is invalid (non-zero exit).
  # The harness should prevent tools from using a dead CWD.
  exit 1
fi
