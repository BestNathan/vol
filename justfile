# ── Help ─────────────────────────────────────────────────────────────────

_default:
    @just --list

# Show available commands
help:
    @just --list

# ── Quality gates (fast checks only) ────────────────────────────────────

# Fast quality gate: fmt + clippy + test compile + no-doc-tests (~5s warm)
quality:
    just fmt-check
    just clippy
    just test-compile
    just no-doc-tests

# Strict quality gate: fmt + strict clippy + test compile
quality-strict:
    just fmt-check
    just clippy-strict
    just test-compile
    just no-doc-tests

# Full CI gate: strict clippy + unit tests + no-doc-tests + boundary check
quality-full:
    just fmt-check
    just clippy-strict
    just test-unit
    just no-doc-tests
    @./scripts/check-agent-boundaries.sh

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

# ── Tests (nextest-powered) ─────────────────────────────────────────────

# Run all tests (nextest, parallel)
test *ARGS:
    cargo nextest run --workspace --no-fail-fast {{ARGS}}

# Compile all tests without running (~3s warm)
test-compile:
    cargo test --workspace --no-run

# Run only unit tests (src/ inline #[cfg(test)])
test-unit *ARGS:
    cargo nextest run --workspace --lib --no-fail-fast {{ARGS}}

# Run all tests including integration
test-all *ARGS:
    cargo nextest run --workspace --no-fail-fast {{ARGS}}

# Run tests for specific crate (e.g. `just test-crate vol-llm-tool`)
test-crate CRATE *ARGS:
    cargo nextest run -p {{CRATE}} --no-fail-fast {{ARGS}}

# Run e2e tests (mostly #[ignore]d, require external services)
test-e2e:
    cargo test --workspace --no-fail-fast -- --ignored

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

# Run llvm-cov summary for a crate (e.g. `just cover vol-llm-tool`)
cover CRATE:
    cargo llvm-cov --package {{CRATE}} --summary-only

# Run llvm-cov for all tool crates
cover-tools:
    cargo llvm-cov -p vol-llm-tool -p vol-llm-sandbox -p vol-llm-tools-builtin --summary-only

# Open HTML coverage report for a crate
cover-html CRATE:
    cargo llvm-cov --package {{CRATE}} --open

# Run llvm-cov with coverage threshold (e.g. `just cover-gate vol-llm-tool 80`)
cover-gate CRATE PCT="80":
    @LINE_COV=$$(cargo llvm-cov --package {{CRATE}} --summary-only 2>&1 | grep '^TOTAL' | awk '{print $$4}' | tr -d '%'); \
    if [ "$$(echo "$$LINE_COV < {{PCT}}" | bc 2>/dev/null)" = "1" ]; then \
        echo "FAIL: {{CRATE}} line coverage is $${LINE_COV}% (required ≥ {{PCT}}%)"; \
        exit 1; \
    else \
        echo "PASS: {{CRATE}} line coverage is $${LINE_COV}% (≥ {{PCT}}%)"; \
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

# Run frontend tests
fe-test:
    npm --prefix frontend run test:coverage

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
