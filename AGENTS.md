# Agent Instructions

## Architecture

```
src/
├── lib.rs              # WASM entry points
├── config/             # CompressionConfig schema
├── detection/          # Magic bytes, MIME mapping
├── analysis/           # Metadata extraction
├── validation/         # Config validation
├── compression/        # Format handlers
│   ├── image/          # PNG, JPEG, WebP, GIF, SVG, AVIF
│   ├── audio/          # FFmpeg bridge
│   └── video/          # FFmpeg bridge
├── chunking/           # Large file processing + CRC32
├── storage/            # OPFS/IndexedDB
├── error.rs            # CompressionError enum
└── result.rs           # CompressionResult struct
```

## Patterns

1. **No tests** — forbidden
2. **Pure Rust** — no native C deps
3. **Threshold logic** — output ≤ input always
4. **FFmpeg bridge** — A/V via external JS

## Format Extension

1. Add variant → `FileFormat` enum (`detection/mod.rs`)
2. Add magic bytes → `FileFormat::detect()`
3. Add MIME → `detection/mime.rs`
4. Create handler → `compression/{image,audio}/`
5. Wire dispatcher → `compression/mod.rs`

## Build

```bash
./scripts/build.sh web      # → pkg/mmm-js.js, mmm-js_bg.wasm
./scripts/build.sh node     # → pkg/mmm-node.js, mmm-node_bg.wasm
./scripts/serve.sh          # Testbench UI (auto-fetches FFmpeg)
```

## Gotchas

- **BigInt**: WASM u64 → JS BigInt. Use f64 for JS-exposed sizes.
- **webp crate**: Native deps fail. Use `image` crate encoder.
- **FFmpeg**: Unavailable in Node. Works in browser with bridge.
