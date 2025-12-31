# mmm — Contributing

## Rules

1. **No tests** — no unit tests, mocks, fixtures
2. **No dead code** — remove unused imports/functions
3. **Pattern fidelity** — match existing architecture
4. **Pure Rust** — must compile to WASM

## Style

- `thiserror` for errors
- `serde` for serialization
- `_` prefix for unused vars
- Cohesive, focused modules

## PR Checklist

1. `wasm-pack build --target web --release`
2. Zero compiler warnings
3. `node bench/run.mjs` passes
4. Atomic commits

## New Format Checklist

- [ ] Magic bytes in `detection/mod.rs`
- [ ] MIME in `detection/mime.rs`
- [ ] Analyzer in `analysis/`
- [ ] Handler in `compression/`
- [ ] Dispatcher in `compression/mod.rs`
