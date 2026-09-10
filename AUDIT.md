# Verification scope

The current changes have been reviewed statically and checked with:

- `cargo fmt --all -- --check`
- `cargo clippy --locked --offline --lib -- -D warnings`
- `cargo clippy --locked --offline --lib --target wasm32-unknown-unknown -- -D warnings`
- Release WASM packaging for browser, Node and bundler
- The 0.2.0 npm tarball was installed in a clean consumer directory: Node ESM/CommonJS, browser initialization, bundler imports, TypeScript declarations and optional Node FFmpeg execution
- `node --check` for JavaScript modules and generated bindings
- Bash syntax checks for build/serve scripts
- `git diff --check`

Packaging in this workspace uses the matching cached wasm-bindgen executable on PATH with `wasm-pack --mode no-install`. The Node build entry point was executed for all targets, including the root browser package. Shell-script execution was not verified in the sandbox because of its PATH/cache restrictions.

No test suite was added or run, following AGENTS.md. At the user's explicit request, `scripts/benchmark.mjs` generates ignored synthetic media and measures real Node/WASM/native FFmpeg conversions. Reports are under `target/benchmarks/`; see README for reproduction. Browser FFmpeg.wasm performance, storage quota failures and cancellation timing have not been exercised. Neither build checks nor the sequential synthetic benchmark establish a process-wide memory bound or visual quality.

For 0.2.1, the focused PNG benchmark measured 112 configurations and independently decoded each output. Untransformed lossless output matched source samples, including 16-bit inputs and transparent RGB/RGBA palettes. Browser verification in headless Chrome exercised file upload, PNG output selection and both PNG modes: an already losslessly compressed synthetic texture remained unchanged in lossless mode and shrank by 74% with color reduction. This is a measured example, not a guarantee for arbitrary PNGs. Generated reports and browser captures are under `target/png-patch/`.

The complete 0.2.1 matrix also ran 1,967 configurations with three repetitions: 1,727 completed, 1,211 returned smaller files and 488 returned unchanged input. There were no new errors versus 0.2.0, no oversized outputs and no changed outputs rejected by independent decoding. The 240 errors include deliberately invalid configurations and incompatible codec/container requests. Reports are `target/benchmarks/png_patch_0_2_1.{json,md}`.

The expanded matrix includes independent native FFmpeg decoding of unique returned outputs. This exposed unusable stream-copy outputs and a grayscale-alpha TIFF encoding failure that API-success counts alone missed. The supplied bridges now check initial remux decodability, and preflight can consult their output-option rules and Node encoder inventory. Browser bridge changes were syntax-checked, not exercised in a browser.

[README.md](README.md) documents supported behavior, compatibility fields and remaining limitations. [CHANGELOG.md](CHANGELOG.md) records the changes. Public Rust helpers remain available; absence of internal callers alone is not evidence that an API is unused downstream.
