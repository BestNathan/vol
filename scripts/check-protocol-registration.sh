#!/usr/bin/env bash
# Check that all protocol operations are registered in the codec.
# See CLAUDE.md "New protocol operation → register in codec" guardrail.
set -euo pipefail
cd "$(dirname "$0")/.."
python3 scripts/check-protocol-registration.py
