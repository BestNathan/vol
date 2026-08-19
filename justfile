# ── Help ─────────────────────────────────────────────────────────────────

_default:
    @just --list

# Show available commands
help:
    @just --list

# NOTE: no umbrella "quality-*" recipes — each scenario (pre-commit hook,
# pre-push hook, CI) composes its own list of atomic recipes below.

# ── Formatting ──────────────────────────────────────────────────────────

# Format all Rust code
fmt:
    cargo fmt --all

# Check formatting (CI gate)
fmt-check:
    cargo fmt --all -- --check

# ── Linting ─────────────────────────────────────────────────────────────

# Run clippy on workspace (warnings allowed)
clippy:
    cargo clippy --workspace

# Strict clippy (deny warnings)
clippy-strict:
    cargo clippy --workspace -- -D warnings

# ── Type checking ───────────────────────────────────────────────────────

# Check compilation (no codegen, fast)
check:
    cargo check --workspace

# ── Tests (nextest-powered, fallback to cargo test) ────────────────────

# Run all non-e2e tests: unit + integration (nextest, fallback to cargo test)
test *ARGS:
    cargo nextest run --workspace --no-fail-fast {{ARGS}} 2>/dev/null || cargo test --workspace --no-fail-fast {{ARGS}}

# Compile all tests without running (~3s warm)
test-compile:
    cargo test --workspace --no-run

# Run only unit tests (src/ inline #[cfg(test)])
test-unit *ARGS:
    cargo nextest run --workspace --lib --no-fail-fast {{ARGS}} 2>/dev/null || cargo test --workspace --lib --no-fail-fast {{ARGS}}

# Run unit tests for specific crates (pre-push tier: changed crates only)
test-unit-crates *CRATES:
    #!/usr/bin/env bash
    set -euo pipefail
    flags=()
    for c in {{CRATES}}; do flags+=(-p "$c"); done
    cargo nextest run "${flags[@]}" --lib --no-fail-fast 2>/dev/null || cargo test "${flags[@]}" --lib --no-fail-fast

# Run only integration tests (tests/ dirs, excludes unit)
# NOTE: nextest `--tests` means ALL targets — use the kind(test) filter instead.
# cargo test has no kind filter, so the fallback runs the full non-ignored suite.
test-integration *ARGS:
    cargo nextest run --workspace -E 'kind(test)' --no-fail-fast {{ARGS}} 2>/dev/null || cargo test --workspace --no-fail-fast {{ARGS}}

# Run tests for specific crate (e.g. `just test-crate vol-llm-tool`)
test-crate CRATE *ARGS:
    cargo nextest run -p {{CRATE}} --no-fail-fast {{ARGS}} 2>/dev/null || cargo test -p {{CRATE}} --no-fail-fast {{ARGS}}

# Run e2e tests (all #[ignore = "e2e: ..."], require external services).
# Missing env/services degrade to clean skips via in-test guards.
test-e2e *ARGS:
    cargo test --workspace --no-fail-fast -- --ignored {{ARGS}}

# Run e2e tests for a single crate (e.g. `just test-e2e-crate vol-llm-sandbox`)
test-e2e-crate CRATE *ARGS:
    cargo test -p {{CRATE}} --no-fail-fast -- --ignored {{ARGS}}

# Run tests with slow timeout (for heavy crates like vol-llm-agent)
test-slow *ARGS:
    cargo nextest run --workspace --no-fail-fast --profile slow {{ARGS}}

# ── Tool & sandbox specific tests ────────────────────────────────────────

# Run all tool-related tests (tool + sandbox + builtins)
test-tools *ARGS:
    cargo nextest run -p vol-llm-tool -p vol-llm-sandbox \
        -p vol-llm-tools-builtin -p vol-llm-skill \
        -p vol-llm-tools-builtin-bash -p vol-llm-tools-builtin-read \
        -p vol-llm-tools-builtin-write -p vol-llm-tools-builtin-edit \
        -p vol-llm-tools-builtin-glob -p vol-llm-tools-builtin-grep \
        --no-fail-fast {{ARGS}}

# Run sandbox tests only
test-sandbox *ARGS:
    cargo nextest run -p vol-llm-sandbox --no-fail-fast {{ARGS}}

# Run sandbox tests with SSH feature
test-sandbox-ssh:
    cargo nextest run -p vol-llm-sandbox --features ssh --no-fail-fast

# Run sandbox tests with Wasm feature
test-sandbox-wasm:
    cargo nextest run -p vol-llm-sandbox --features wasm --no-fail-fast

# ── Coverage ────────────────────────────────────────────────────────────

# Prerequisites:
#   rustup component add llvm-tools-preview
#   cargo install cargo-llvm-cov
#
# Usage:
#   just cover vol-agent-server              # single crate
#   just cover-multi vol-agent-server vol-llm-agent-protocol  # multi-crate
#   just cover-html vol-llm-runtime           # open HTML report
#   just cover-gate vol-agent-server 80       # gate at 80%
#   just cover-tools                          # all tool crates
#
# Threshold pin values are in CLAUDE.md; update there if changed.

# Run llvm-cov summary for a single crate
cover CRATE:
    cargo llvm-cov --package {{CRATE}} --summary-only

# Run llvm-cov summary for multiple crates
cover-multi *CRATES:
    @for crate in {{CRATES}}; do \
        echo "=== $crate ==="; \
        cargo llvm-cov --package $crate --summary-only; \
    done

# Run llvm-cov for all tool crates
cover-tools:
    cargo llvm-cov -p vol-llm-tool -p vol-llm-sandbox -p vol-llm-tools-builtin --summary-only

# Open HTML coverage report for a crate
cover-html CRATE:
    cargo llvm-cov --package {{CRATE}} --open

# Run llvm-cov with coverage threshold (single crate)
cover-gate CRATE PCT="80":
    @LINE_COV=$(cargo llvm-cov --package {{CRATE}} --summary-only 2>&1 | grep '^TOTAL' | awk '{print $4}' | tr -d '%'); \
    if [ "$(echo "$LINE_COV < {{PCT}}" | bc 2>/dev/null)" = "1" ]; then \
        echo "FAIL: {{CRATE}} line coverage is ${LINE_COV}% (required ≥ {{PCT}}%)"; \
        exit 1; \
    else \
        echo "PASS: {{CRATE}} line coverage is ${LINE_COV}% (≥ {{PCT}}%)"; \
    fi

# Run llvm-cov with coverage threshold (multi-crate)
cover-gate-multi PCT *CRATES:
    @CRATE_LIST="{{CRATES}}"; \
    FAILED=""; \
    for crate in $CRATE_LIST; do \
        LINE_COV=$(cargo llvm-cov --package $crate --summary-only 2>&1 | grep '^TOTAL' | awk '{print $4}' | tr -d '%'); \
        if [ "$(echo "$LINE_COV < {{PCT}}" | bc 2>/dev/null)" = "1" ]; then \
            echo "FAIL: $crate line coverage is ${LINE_COV}% (required ≥ {{PCT}}%)"; \
            FAILED="$FAILED $crate"; \
        else \
            echo "PASS: $crate line coverage is ${LINE_COV}% (≥ {{PCT}}%)"; \
        fi; \
    done; \
    if [ -n "$FAILED" ]; then \
        echo "Coverage check failed for:$FAILED"; \
        exit 1; \
    fi

# ── Quality: no-doc-tests ───────────────────────────────────────────────

# Check no active doc tests
no-doc-tests:
    @./scripts/check-no-doc-tests.sh

# Check no new clippy allow annotations
no-clippy-allow:
    @./scripts/check-no-clippy-allow.sh

# ── Web dev (React frontend) ────────────────────────────────────────────

# Start Vite React dev server (port 5173, WS proxy to :3001)
web-dev:
    npm --prefix frontend run dev

# Start backend JSON-RPC agent service (port 3001)
web-backend:
    ANTHROPIC_AUTH_TOKEN=sk cargo watch -x "run -p vol-agent-server"

# Build React SPA + serve (port 8080)
web-serve:
    npm --prefix frontend run build
    @python3 scripts/serve-web.py frontend/dist --port 8080

# TypeScript check + Vite build
web-check:
    npm --prefix frontend run build

# TypeScript type-check only (no build)
web-clippy:
    cd frontend && npx tsc -b --noEmit

# Production build
web-build:
    npm --prefix frontend run build

# ── Frontend quality ────────────────────────────────────────────────────

# Format frontend code
fe-fmt:
    npm --prefix frontend run format

# Check frontend formatting
fe-fmt-check:
    npm --prefix frontend run format:check

# Lint frontend code
fe-lint:
    npm --prefix frontend run lint

# Type-check frontend code
fe-type:
    npm --prefix frontend run typecheck

# Run frontend tests (both vitest projects: unit + integration, with coverage)
fe-test:
    npm --prefix frontend run test:coverage

# Run frontend unit tests (tests/unit/, node environment)
fe-test-unit:
    npm --prefix frontend run test:unit

# Run frontend integration tests (tests/integration/, jsdom + testing-library)
fe-test-integration:
    npm --prefix frontend run test:integration

# Run frontend Playwright e2e tests (self-contained: mock backend, no external services)
fe-e2e:
    npm --prefix frontend run test:e2e

# ── Docker ──────────────────────────────────────────────────────────────

# Build agent-server Docker image
docker-agent:
    docker build -f dockers/vol-agent-server.Dockerfile -t vol-agent-server .

# Build vol-monitor Docker image
docker-monitor:
    docker build -f dockers/vol-monitor.cross.Dockerfile -t vol-monitor .

# ── Audit ───────────────────────────────────────────────────────────────

# Run cargo-audit vulnerability scan
audit:
    cargo audit
