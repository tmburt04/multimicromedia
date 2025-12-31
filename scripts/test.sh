#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"

cd "$PROJECT_DIR"

# Ensure WASM is built for Node.js
if [[ ! -f "pkg/compression_wasm.js" ]]; then
    echo "==> WASM not built. Building for nodejs..."
    wasm-pack build --target nodejs --release
fi

# Check for testdata directory
if [[ ! -d "testdata" ]]; then
    echo "Error: testdata/ directory not found."
    echo "Create testdata/ and add sample files to benchmark."
    exit 1
fi

echo "==> Running benchmarks..."
echo "    Note: Audio/video require FFmpeg bridge (see js/ffmpeg_bridge.js)"
echo ""
node bench/run.mjs

echo ""
echo "==> Report generated: BENCHMARK_REPORT.md"
