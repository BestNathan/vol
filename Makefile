.PHONY: help web-css web-dev web-backend web-check web-build web-clippy web-serve

help: ## Show available commands
	@grep -E '^[a-zA-Z_-]+:.*?## ' $(MAKEFILE_LIST) | awk 'BEGIN {FS = ":.*?## "}; {printf "  %-14s %s\n", $$1, $$2}'

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
