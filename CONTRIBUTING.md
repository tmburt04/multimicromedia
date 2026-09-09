# Contributing

## Rules

1. **No tests** — no unit tests or mocks. The explicitly requested benchmark generates ignored media under `target/`; do not commit fixtures.
2. **No dead code** — remove unused private code; retain public compatibility APIs deliberately
3. **Pattern fidelity** — match existing architecture
4. **Pure Rust** — no native C dependencies in the crate; audio/video use an external JS bridge

## Style

- `thiserror` for errors
- `serde` for serialization
- Shared format/validation/bridge helpers instead of divergent copies
- Cohesive, focused modules

## PR Checklist

1. `cargo fmt --all -- --check` and `cargo clippy --lib --target wasm32-unknown-unknown -- -D warnings`
2. `node scripts/build.mjs all`
3. `node --check` for changed JS modules; `bash -n` for changed shell scripts
4. Describe what was verified and any unverified runtime behavior

For measurement, build all targets, then run `npm run benchmark -- --repeats=3 --label=current`. See README for FFmpeg setup and report limitations.

## npm releases

Keep the versions in `Cargo.toml`, `Cargo.lock`, `package.json` and `package-lock.json` synchronized, and record compatibility changes in `CHANGELOG.md`. Commit both lockfiles so release builds resolve the reviewed dependency versions.

Run the development checks above, then prepare the public artifact:

```bash
npm run build:all -- release -- --locked
npm pack
npm publish ./multimicromedia-<version>.tgz --access public
```

Before publishing, install the tarball in a separate consumer directory and verify its imports and declarations. The prepack script checks build versions and timestamps and assembles the WASM targets, bridge modules and third-party notices. Consumers install prebuilt files and do not run Rust or packaging scripts. Publish the inspected tarball after authenticating with npm; keep credentials outside the repository.

## New Format Checklist

- [ ] Magic bytes in `detection/mod.rs`
- [ ] MIME in `detection/mime.rs`
- [ ] Analyzer in `analysis/`
- [ ] Handler in `compression/`
- [ ] Dispatcher in `compression/mod.rs`
