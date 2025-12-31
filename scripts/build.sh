#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"

cd "$PROJECT_DIR"

TARGET="${1:-web}"
PROFILE="${2:-release}"

echo "==> Building compression-wasm for target: $TARGET ($PROFILE)"

case "$TARGET" in
    web)
        if [[ "$PROFILE" == "release" ]]; then
            wasm-pack build --target web --release
        else
            wasm-pack build --target web --dev
        fi
        ;;
    nodejs|node)
        if [[ "$PROFILE" == "release" ]]; then
            wasm-pack build --target nodejs --release
        else
            wasm-pack build --target nodejs --dev
        fi
        ;;
    bundler)
        if [[ "$PROFILE" == "release" ]]; then
            wasm-pack build --target bundler --release
        else
            wasm-pack build --target bundler --dev
        fi
        ;;
    all)
        echo "==> Building for web..."
        wasm-pack build --target web --release --out-dir pkg/web
        echo "==> Building for nodejs..."
        wasm-pack build --target nodejs --release --out-dir pkg/nodejs
        echo "==> Building for bundler..."
        wasm-pack build --target bundler --release --out-dir pkg/bundler
        ;;
    *)
        echo "Usage: $0 [web|nodejs|bundler|all] [release|dev]"
        echo "  web      - Browser ES modules (default)"
        echo "  nodejs   - Node.js CommonJS"
        echo "  bundler  - For webpack/rollup"
        echo "  all      - Build all targets"
        exit 1
        ;;
esac

echo "==> Build complete: pkg/"
