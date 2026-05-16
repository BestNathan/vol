.PHONY: help web-css web-dev web-backend web-check web-build web-clippy

help: ## Show available commands
	@grep -E '^[a-zA-Z_-]+:.*?## ' $(MAKEFILE_LIST) | awk 'BEGIN {FS = ":.*?## "}; {printf "  %-12s %s\n", $$1, $$2}'

web-css: ## Build Tailwind CSS
	npx @tailwindcss/cli -i crates/vol-llm-ui/assets/input.css -o crates/vol-llm-ui/assets/tailwind.css

web-dev: ## Start Dioxus dev server (port 8080)
	dx serve --package vol-llm-ui --bin vol-llm-ui-web --no-default-features --features web --addr 0.0.0.0 --port 8080

web-backend: ## Start backend JSON-RPC agent service
	ANTHROPIC_AUTH_TOKEN=sk cargo run --example jsonrpc_agent_service -p vol-llm-agent-channel

web-check: ## cargo check (web only)
	cargo check -p vol-llm-ui --no-default-features --features web

web-build: ## Build WASM binary
	cargo build -p vol-llm-ui --no-default-features --features web --target wasm32-unknown-unknown

web-clippy: ## cargo clippy (web only)
	cargo clippy -p vol-llm-ui --no-default-features --features web -- -D warnings
