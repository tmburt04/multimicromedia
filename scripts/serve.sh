#!/bin/bash
# Testbench server with auto-setup
# Usage: ./scripts/serve.sh [port]

set -e

PORT="${1:-8080}"
ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
BENCH_DIR="$ROOT_DIR/bench"
FFMPEG_DIR="$BENCH_DIR/ffmpeg"

cd "$ROOT_DIR"

# Build WASM if missing
if [ ! -f "pkg/mmm-js.js" ] && [ ! -f "pkg/compression_wasm.js" ]; then
    echo "⚠️  WASM module not found. Building..."
    if command -v wasm-pack &> /dev/null; then
        wasm-pack build --target web --release --out-name mmm-js
    else
        echo "❌ wasm-pack not installed. Run: cargo install wasm-pack"
        exit 1
    fi
fi

# Create symlinks
cd "$BENCH_DIR"

if [ ! -e "testdata" ]; then
    ln -s ../testdata testdata
    echo "✓ Linked: testdata"
fi

if [ ! -e "pkg" ]; then
    ln -s ../pkg pkg
    echo "✓ Linked: pkg"
fi

# Copy FFmpeg from node_modules
FFMPEG_CORE_SRC="$ROOT_DIR/node_modules/@ffmpeg/core/dist/esm"
FFMPEG_WRAPPER_SRC="$ROOT_DIR/node_modules/@ffmpeg/ffmpeg/dist/esm"

if [ ! -f "$FFMPEG_DIR/ffmpeg-core.wasm" ]; then
    if [ -d "$FFMPEG_CORE_SRC" ]; then
        mkdir -p "$FFMPEG_DIR"
        cp "$FFMPEG_CORE_SRC/ffmpeg-core.js" "$FFMPEG_DIR/"
        cp "$FFMPEG_CORE_SRC/ffmpeg-core.wasm" "$FFMPEG_DIR/"
        SIZE=$(du -h "$FFMPEG_DIR/ffmpeg-core.wasm" | cut -f1)
        echo "✓ Copied FFmpeg core ($SIZE)"
    else
        echo "❌ @ffmpeg/core not found. Run: npm install"
        exit 1
    fi
fi

if [ ! -f "$FFMPEG_DIR/index.js" ]; then
    if [ -d "$FFMPEG_WRAPPER_SRC" ]; then
        cp "$FFMPEG_WRAPPER_SRC"/*.js "$FFMPEG_DIR/"
        echo "✓ Copied FFmpeg wrapper"
    else
        echo "❌ @ffmpeg/ffmpeg not found. Run: npm install"
        exit 1
    fi
fi

echo ""
echo "🚀 Testbench"
echo ""
echo "   http://localhost:$PORT/"
echo ""
echo "   Ctrl+C to stop"
echo ""

# Serve
if command -v python3 &> /dev/null; then
    python3 -m http.server "$PORT"
elif command -v python &> /dev/null; then
    python -m http.server "$PORT"
elif command -v npx &> /dev/null; then
    npx serve -l "$PORT"
elif command -v php &> /dev/null; then
    php -S "localhost:$PORT"
else
    echo "❌ No HTTP server found. Install python3, node, or php."
    exit 1
fi
