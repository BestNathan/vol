.PHONY: help web-dev web-backend web-check web-build web-serve \
        coverage coverage-html coverage-threshold \
        fmt fmt-check check clippy clippy-strict test test-unit test-integration test-e2e audit quality quality-strict quality-full no-doc-tests

help: ## Show available commands
	@grep -E '^[a-zA-Z_-]+:.*?## ' $(MAKEFILE_LIST) | awk 'BEGIN {FS = ":.*?## "}; {printf "  %-20s %s\n", $$1, $$2}'

# ── Rust Quality Gates ──────────────────────────────────────────────────

fmt: ## Run cargo fmt on all crates
	cargo fmt --all

fmt-check: ## Check formatting (CI gate)
	cargo fmt --all -- --check

check: ## Run cargo check on entire workspace
	cargo check --workspace

clippy: ## Run clippy on entire workspace
	cargo clippy --workspace

clippy-strict: ## Run clippy with -D warnings (deny + warn = error)
	cargo clippy --workspace -- -D warnings

# ── Tests (tiered: compile < unit < integration < e2e) ──────────────

test: ## Run all tests (nextest if available, fallback to cargo test)
	cargo nextest run --workspace --no-fail-fast 2>/dev/null || cargo test --workspace --no-fail-fast

test-compile: ## Compile all tests without running (fast gate, ~3s warm)
	cargo test --workspace --no-run

test-unit: ## Run unit tests only (src/ inline #[cfg(test)])
	cargo nextest run --workspace --lib --no-fail-fast 2>/dev/null || cargo test --workspace --lib --no-fail-fast

test-integration: ## Run all tests including integration (single pass)
	cargo test --workspace --no-fail-fast

test-e2e: ## Run e2e tests (require external services, mostly #[ignore]d)
	cargo test --workspace --no-fail-fast -- --ignored

test-all: test-unit test-e2e ## Run ALL tests including e2e

audit: ## Run cargo-audit vulnerability scan (requires cargo-audit)
	cargo audit

# ── Quality gates (fast for local, full for CI) ──────────────────────

quality: fmt-check clippy test-compile no-doc-tests ## Fast quality gate (~5s warm)
	@echo "All quality gates passed"

quality-strict: fmt-check clippy-strict test-compile no-doc-tests ## Fast strict gate
	@echo "All strict quality gates passed"

quality-full: fmt-check clippy-strict test-unit no-doc-tests ## Full CI gate
	@./scripts/check-agent-boundaries.sh
	@echo "All full quality gates passed"

no-doc-tests: ## Check no active doc tests
	@./scripts/check-no-doc-tests.sh

web-dev: ## Start Vite React dev server (port 5173, WS proxy to :3001)
	npm --prefix frontend run dev

web-serve: ## Build React SPA + serve w/ cache headers (phone testing, port 8080)
	npm --prefix frontend run build
	@python3 scripts/serve-web.py frontend/dist --port 8080

web-backend: ## Start backend JSON-RPC agent service (port 3001)
	ANTHROPIC_AUTH_TOKEN=sk cargo watch -x "run -p vol-agent-server"

web-check: ## TypeScript check + Vite build
	npm --prefix frontend run build

web-build: ## Production build
	npm --prefix frontend run build

web-clippy: ## TypeScript type-check only
	cd frontend && npx tsc -b --noEmit

# ── Coverage ──
#
# Prerequisites:
#   rustup component add llvm-tools-preview
#   cargo install cargo-llvm-cov
#
# Usage:
#   make coverage PKG=vol-agent-server              # single crate
#   make coverage PKG="vol-agent-server vol-llm-agent-protocol"  # multi-crate
#   make coverage-html PKG=vol-llm-runtime           # open HTML report
#   make coverage-threshold PKG=vol-agent-server PCT=80  # gate at 80%
#
# Threshold pin values are in CLAUDE.md; update there if changed.

PKG ?= vol-agent-server
PCT ?= 80

coverage: ## Run llvm-cov summary (override PKG / PCT)
	cargo llvm-cov $(addprefix --package ,$(PKG)) --summary-only

coverage-html: ## Open llvm-cov HTML report (override PKG)
	cargo llvm-cov $(addprefix --package ,$(PKG)) --open

coverage-threshold: ## Fail if PKG line coverage < PCT (default 80)
	@LINE_COV=$$(cargo llvm-cov $(addprefix --package ,$(PKG)) --summary-only 2>&1 | grep '^TOTAL' | awk '{print $$10}' | tr -d '%'); \
	if [ "$$(echo "$$LINE_COV < $(PCT)" | bc 2>/dev/null)" = "1" ]; then \
		echo "FAIL: $(PKG) line coverage is $${LINE_COV}% (required ≥ $(PCT)%)"; \
		exit 1; \
	else \
		echo "PASS: $(PKG) line coverage is $${LINE_COV}% (≥ $(PCT)%)"; \
	fi

