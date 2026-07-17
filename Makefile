.PHONY: help web-css web-dev web-backend web-check web-build web-clippy web-serve \
        coverage coverage-html coverage-threshold \
        fmt fmt-check check clippy test audit quality

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

test: ## Run all workspace tests
	cargo test --workspace --no-fail-fast

audit: ## Run cargo-audit vulnerability scan (requires cargo-audit)
	cargo audit

quality: fmt-check clippy test ## Run all quality gates (fmt + clippy + test)
	@echo "All quality gates passed"

quality-strict: fmt-check clippy-strict test ## Run all quality gates in strict mode
	@echo "All strict quality gates passed"

web-css: ## Build Tailwind CSS in watch mode
	npx --prefix crates/vol-llm-ui @tailwindcss/cli -i crates/vol-llm-ui/assets/input.css -o crates/vol-llm-ui/assets/tailwind.css --watch=always

web-dev: ## Start Dioxus dev server w/ size-optimized WASM (port 8080)
	dx serve --package vol-llm-ui --bin vol-llm-ui-web --no-default-features --features web --addr 0.0.0.0 --port 8080

web-serve: ## Build release WASM + serve w/ cache headers (phone testing, port 8080)
	dx build --release --package vol-llm-ui --bin vol-llm-ui-web --no-default-features --features web
	@python3 scripts/serve-web.py target/dx/vol-llm-ui-web/release/web/public --port 8080

web-backend: ## Start backend JSON-RPC agent service (port 3001)
	ANTHROPIC_AUTH_TOKEN=sk cargo watch -x "run -p vol-agent-server"

web-check: ## cargo check (web only)
	cargo check -p vol-llm-ui --no-default-features --features web

web-build: ## Build WASM binary
	cargo build -p vol-llm-ui --no-default-features --features web --target wasm32-unknown-unknown

web-clippy: ## cargo clippy (web only)
	cargo clippy -p vol-llm-ui --no-default-features --features web -- -D warnings

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

