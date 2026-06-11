#!/usr/bin/env bash
set -euo pipefail

if cargo tree -p vol-llm-agent-protocol | grep -q 'vol-agent-server'; then
  echo "boundary violation: vol-llm-agent-protocol depends on vol-agent-server" >&2
  exit 1
fi

if cargo tree -p vol-llm-runtime | grep -q 'vol-agent-server'; then
  echo "boundary violation: vol-llm-runtime depends on vol-agent-server" >&2
  exit 1
fi

echo "agent boundary checks passed"
