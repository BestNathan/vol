#!/bin/bash
# Rebuild and serve the vol-llm-ui web application.
# Usage: ./scripts/rebuild-web.sh
set -e

PROJECT_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
WASM_DIR="$PROJECT_ROOT/target/wasm32-unknown-unknown/wasm-dev"
DIST_DIR="$WASM_DIR/dist"

echo "=== Setting up dist directory ==="
rm -rf "$DIST_DIR"
mkdir -p "$DIST_DIR/wasm"

echo "=== Generating Tailwind CSS ==="
npx @tailwindcss/cli \
    -i "$PROJECT_ROOT/crates/vol-llm-ui/src/web/input.css" \
    -o "$DIST_DIR/tailwind.css" \
    --minify

echo "=== Building vol-llm-ui-web (wasm32) ==="
cargo build \
    --target wasm32-unknown-unknown \
    --package vol-llm-ui \
    --bin vol-llm-ui-web \
    --no-default-features \
    --features web \
    --quiet

echo "=== Processing WASM with wasm-bindgen ==="
wasm-bindgen \
    --out-dir "$WASM_DIR/wasm" \
    --target web \
    "$WASM_DIR/vol-llm-ui-web.wasm" \
    --quiet

cp -r "$WASM_DIR/wasm/"* "$DIST_DIR/wasm/"

cat > "$DIST_DIR/index.html" << 'HTML'
<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <title>vol-llm-ui</title>
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <link rel="stylesheet" href="tailwind.css">
</head>
<body>
<script type="module">
import init from './wasm/vol-llm-ui-web.js';
init('./wasm/vol-llm-ui-web_bg.wasm');
</script>
</body>
</html>
HTML

echo "=== Done! ==="
echo "Dist directory: $DIST_DIR"
echo ""
echo "To serve: basic-http-server --addr 0.0.0.0:8080 $DIST_DIR"
