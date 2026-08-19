#!/usr/bin/env bash
# CI coverage report (report-only, no threshold gate).
#
# Runs `cargo llvm-cov` over the core LLM crates and prints the summary.
# No pass/fail threshold: this produces a report for humans. The coverage
# GATE lives in local dev (`just cover-gate <crate> 80`, per CLAUDE.md).
#
# Package override for quick local checks:
#   COV_PACKAGES="vol-llm-sandbox" ./scripts/ci-coverage-report.sh

set -euo pipefail

# Core LLM crates (previously inlined in quality.yml's coverage job).
DEFAULT_PACKAGES="vol-agent-server vol-llm-agent-protocol vol-llm-runtime vol-llm-sandbox vol-llm-task vol-session vol-llm-agent vol-llm-provider vol-llm-tool vol-llm-skill vol-llm-mcp"

PACKAGES="${COV_PACKAGES:-$DEFAULT_PACKAGES}"

flags=()
for p in $PACKAGES; do
    flags+=(--package "$p")
done

# Workspace-relative so the workflow can upload it as an artifact.
cargo llvm-cov "${flags[@]}" --summary-only | tee "${COV_OUTPUT:-target/coverage-summary.txt}"
