#!/bin/bash
# scripts/run-dev.sh - Local development startup script
# 本地开发启动脚本
#
# Usage:
#   ./scripts/run-dev.sh [dev|prod]
#
# Options:
#   dev  - Run with dev config (default)
#   prod - Run with production config (requires .env with real credentials)

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
cd "$PROJECT_ROOT"

# Default to dev mode
MODE="${1:-dev}"

echo "=== Vol Monitor - $MODE mode ==="

# Load environment variables
if [ -f ".env" ]; then
    echo "Loading environment from .env..."
    export $(grep -v '^#' .env | xargs)
else
    echo "Warning: .env file not found!"
    echo "Please copy .env.example to .env and fill in your credentials."
    if [ "$MODE" == "dev" ]; then
        echo "For dev mode, you can run without Feishu credentials (stdout only)."
    fi
    exit 1
fi

# Select config file
if [ "$MODE" == "dev" ]; then
    CONFIG_FILE="config.dev.toml"
    echo "Using development configuration: $CONFIG_FILE"
elif [ "$MODE" == "prod" ]; then
    CONFIG_FILE="config.prod.toml"
    echo "Using production configuration: $CONFIG_FILE"
else
    echo "Unknown mode: $MODE"
    echo "Usage: $0 [dev|prod]"
    exit 1
fi

# Check if config file exists
if [ ! -f "$CONFIG_FILE" ]; then
    echo "Error: Config file not found: $CONFIG_FILE"
    exit 1
fi

# Build if needed
echo "Building..."
cargo build --release

# Run with configuration
echo "Starting vol-monitor..."
RUST_LOG="${RUST_LOG:-info}" \
./target/release/vol-monitor --config "$CONFIG_FILE"
