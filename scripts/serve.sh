#!/bin/bash
# Serve the compression testbench UI
# Usage: ./scripts/serve.sh [port]

set -e

PORT="${1:-8080}"
ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
BENCH_DIR="$ROOT_DIR/bench"

cd "$ROOT_DIR"

# Check if WASM is built
if [ ! -f "pkg/compression_wasm.js" ]; then
    echo "⚠️  WASM module not found. Building..."
    if command -v wasm-pack &> /dev/null; then
        wasm-pack build --target web --release
    else
        echo "❌ wasm-pack not installed. Run: cargo install wasm-pack"
        exit 1
    fi
fi

# Create symlinks in bench/ for testdata and pkg
cd "$BENCH_DIR"

if [ ! -e "testdata" ]; then
    ln -s ../testdata testdata
    echo "✓ Created symlink: bench/testdata → ../testdata"
fi

if [ ! -e "pkg" ]; then
    ln -s ../pkg pkg
    echo "✓ Created symlink: bench/pkg → ../pkg"
fi

echo ""
echo "🚀 Compression Testbench"
echo ""
echo "   Open: http://localhost:$PORT/"
echo ""
echo "   Press Ctrl+C to stop"
echo ""

# Try Python first (most common)
if command -v python3 &> /dev/null; then
    python3 -m http.server "$PORT"
elif command -v python &> /dev/null; then
    python -m http.server "$PORT"
# Fallback to npx serve
elif command -v npx &> /dev/null; then
    npx serve -l "$PORT"
# Fallback to php
elif command -v php &> /dev/null; then
    php -S "localhost:$PORT"
else
    echo "❌ No suitable HTTP server found."
    echo "   Install one of: python3, node/npx, or php"
    exit 1
fi


