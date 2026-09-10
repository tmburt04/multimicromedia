# Changelog

## [0.2.1] — 2026-09-09

- Fix PNG quantization being ignored after resize, crop and conversion to PNG. All still PNG encoding paths now consider the configured palette candidate.
- Improve lossless RGB/RGBA PNGs with exact palettes, and pack small palettes into 1-, 2- or 4-bit rows. Compare unfiltered and adaptive-filter palette encodings, retaining the smaller result.
- Add a visible PNG compression mode to the browser bench. Its default is explicitly labeled lossy color reduction (up to 256 colors); users can select lossless pixel preservation. The library API remains lossless by default. The quality slider controls PNG encoding effort, not color count.
- Add a reproducible PNG benchmark with independent decoding and exact lossless pixel comparisons, including transparency, odd-width packed rows and 16-bit inputs. The output-size ceiling remains in force; PNG compression cannot guarantee savings on every source.
- Verify 112 focused configurations and the full 1,967-configuration matrix: no lossless sample changes in the focused run, no new execution errors, no oversized returns and no changed outputs rejected by decoding. In Chrome, one already losslessly compressed synthetic PNG shrank from 682 KB to 177 KB (74%) with color reduction; lossless mode left it unchanged.

## [0.2.0] — 2026-09-09

### Compatibility and migration

- Unknown configuration fields, including nested typos, now return `E_INVALID_CONFIG`. Remove extra properties before passing configuration JSON.
- Nonzero video trim starts require re-encoding; video stream copy cannot guarantee an independently decodable keyframe at the requested timestamp. Remove the copy codec override when trimming from a nonzero start.
- Audio stream copy retains its source container when no output format is specified. Video does the same when both codecs are copy. Explicit output formats still take precedence.
- The supplied FFmpeg bridges check the initial decodability of stream-copy output before returning it. Remuxing now requires a decoder and adds a short decode step within the execution timeout.
- Unsupported animation conversions and animated PNG/WebP transforms are rejected. AVIF/HEIC encoding and SVG rasterization remain unavailable.

### Developer experience

- Publish as `multimicromedia` with prebuilt Node, browser and bundler entry points, TypeScript declarations, optional FFmpeg bridge exports and bundled dependency notices. Raster-only installs have no npm runtime dependencies.
- Add cross-platform Node build commands; retain the Bash build wrapper.
- Add configuration error fields, ignored-setting warnings, `CompressionResult.unchanged`, and the consuming `into_data()` buffer API. Size estimates use JavaScript Numbers.
- Share audio/video argument planning between preflight and execution. An optional bridge validation hook checks output options; Node also inventories installed encoder names and aliases.
- Support `FFMPEG_PATH`, concise FFmpeg diagnostics, and bounded, serialized jobs with isolated input files.
- Add a reproducible benchmark runner, fixture reuse, independent output decoding, individual repetition observations, and reports by input family, operation, target format, quality and failure reason.

### Correctness, performance and cleanup

- Replace GPL-licensed `imagequant` with MIT-licensed `color_quant`. Preserve exact palettes when possible, avoid a second RGBA copy and normalize GIF transparency to one index. Quantized pixels and output sizes may differ from earlier benchmarks; perceptual equivalence is not claimed.
- Enforce output ≤ input across handlers and report the format of returned bytes. A requested conversion or transform may return the original when encoding grows the file.
- Fix grayscale-alpha TIFF and 16-bit grayscale GIF conversions, default WMA bitrates, and codec-specific FFmpeg options.
- Reduce raster and embedded-image copies, guard image workspace, retain supported PNG metadata, and process GIF frames incrementally.
- Harden format detection, container parsing, crop/resize validation, storage cleanup, OPFS transfers, quota reporting and chunk assembly.
- Prevent stale bench results and conflicting operations. Remove unused dependencies and redundant private helpers; document inactive compatibility settings and runtime limitations.

### Verification

- Expanded benchmark: 1,967 configurations across 40 generated inputs, with three repetitions per configuration. Independent decoding exposed six changed outputs that could not be decoded; after fixes, none of the changed outputs returned by that matrix failed decoding. All successful outputs respected the size ceiling.
- Native/WASM Clippy, formatting, JavaScript syntax checks and release packaging for browser, Node and bundler passed. No test suite was added or run.
- Repeat the complete matrix on the npm release candidate: 1,727 completed, 1,209 smaller, 490 unchanged, no oversized returns and no changed outputs rejected by independent decoding. Verify the packed artifact from a clean consumer installation, including all entry points, TypeScript declarations and Node audio compression.
- Measurements use short synthetic media and native FFmpeg on Windows. They do not establish browser performance, perceptual quality or peak memory behavior.

## [0.1.0] — 2025-12-11

### Core
- Image compression: PNG, JPEG, WebP, GIF, BMP, TIFF, ICO
- SVG processing: comment removal and embedded base64 recompression
- JPEG decode/re-encode compression
- Audio/Video: FFmpeg.wasm bridge (MP3, WAV, FLAC, OGG, AAC, MP4, WebM, MOV, AVI, MKV)

### Features
- Magic byte detection + MIME mapping
- File analysis: dimensions, animation, frame count
- Config validation with error hints
- Transforms: resize, crop, trim
- Chunked processing with CRC32 integrity
- Storage: OPFS primary, IndexedDB fallback
- Builder API for config construction

### Architecture
- Pure Rust → WASM (no native deps)
- Threshold logic: output ≤ input guaranteed
- Structured errors with recovery hints
- Async compression pipeline
