# multimicromedia

Client-side media compression via WebAssembly. Compression runs locally; audio/video codecs are supplied by an external FFmpeg bridge.

[Demo](https://notschmee.com/widgets/image-compression)

## Install from npm

```bash
npm install multimicromedia
```

Published packages include prebuilt WASM and TypeScript declarations. Raster compression does not require Rust, a build step, or FFmpeg.

Node (ES modules or CommonJS):

```javascript
import { readFile, writeFile } from 'node:fs/promises';
import { compress } from 'multimicromedia';
// CommonJS: const { compress } = require('multimicromedia');
const result = await compress(await readFile('input.png'), JSON.stringify({ quality: 80 }));
const extension = result.format_out;
await writeFile(`output.${extension}`, result.into_data());
```

Browser:

```javascript
import init, { compress } from 'multimicromedia/web';
await init();
const result = await compress(new Uint8Array(await file.arrayBuffer()), '{}');
const bytes = result.into_data();
```

The browser initializer loads the WASM file next to its module. If your host or bundler relocates assets, pass an explicit URL with `init({ module_or_path: wasmUrl })`. The `multimicromedia/wasm` export identifies that asset. `multimicromedia/bundler` provides synchronous generated bindings for bundlers configured to import WASM; `multimicromedia/node` explicitly selects Node bindings. The root export selects Node automatically in Node and the browser initializer elsewhere.

For audio/video in Node, install FFmpeg separately or set `FFMPEG_PATH` to its executable, then call `await initialize()` from `multimicromedia/ffmpeg/node`. Optional `npm install ffmpeg-static` supplies a binary. Browser consumers can import `initialize` from `multimicromedia/ffmpeg/browser` and pass a loaded FFmpeg.wasm instance. Importing either bridge installs the global bridge used by WASM.

## Build from source

Builds require Rust, `wasm-pack`, and Node.js. The optional browser server script also requires Bash and Python.

```bash
rustup target add wasm32-unknown-unknown
cargo install wasm-pack --locked
node scripts/build.mjs web
```

| Target | Command | Output |
| --- | --- | --- |
| Browser | `npm run build:web` | `pkg/mmm-js.js` and WASM |
| Node | `npm run build:node` | `pkg/mmm-node.js` and WASM |
| Bundler | `node scripts/build.mjs bundler` | `pkg/mmm-js.js` and WASM |
| All | `npm run build:all` | Separate `pkg/web`, `pkg/nodejs`, `pkg/bundler` packages |

The build script's optional second argument is `release` (default) or `dev`; subsequent arguments are passed to wasm-pack. Individual targets replace the package metadata in `pkg/`; use `all` to keep targets separate. `build.sh` is a compatibility wrapper around the Node script.

To run the browser bench:

```bash
npm ci
./scripts/serve.sh       # http://localhost:8080
```

The server builds browser WASM if missing and copies FFmpeg assets from `node_modules` when absent. Rebuild after Rust changes. Optional sample files belong in the ignored `testdata/` directory; local files can also be selected. Batch stop aborts pending fetches but waits for an active encode to finish.

## Usage

```javascript
import init, { compress, compress_with_defaults, analyze_file } from 'multimicromedia/web';

await init();
// bytes is a Uint8Array containing the input file.
const analysis = analyze_file(bytes); // { format, mime, size, width?, height?, ... }
const result = await compress(bytes, JSON.stringify({
  output_format: 'jpeg',
  quality: 85,
  resize: { width: 1920, preserve_aspect: true }
}));
const format = result.format_out;
const outputBytes = result.into_data(); // Consumes the result; do not call free() afterward.

// Default configuration:
const defaultResult = await compress_with_defaults(bytes);
const defaultBytes = defaultResult.into_data();
```

Read statistics before `into_data()`. Alternatively, `result.data` copies the buffer and leaves the result alive; release it with `result.free()` in a `finally` block. Consuming the result avoids a Rust buffer clone; WASM-to-JavaScript transfer still copies bytes.

The builder produces the same JSON configuration:

```javascript
import { create_config } from 'multimicromedia/web';
const config = create_config().quality(80).output_format('jpeg').resize(1200, null).build();
```

| API | Returns |
| --- | --- |
| `compress(bytes, configJson)` / `compress_with_defaults(bytes)` | Promise of an owned `CompressionResult` |
| `analyze_file(bytes)` | Metadata object; unavailable fields are omitted |
| `detect_file_type(bytes)` / `detect_file_mime(bytes)` | Detected extension / MIME string |
| `validate_config(json)` / `validate_config_for_file(json, bytes)` | JSON string containing `valid`, `errors`, `warnings` |
| `get_supported_formats()` | Input format registry, not an encoder availability check |
| `get_config_schema(bytes?)` | Form field descriptions; file-specific validation still applies |
| `get_default_config()` | Default configuration JSON |
| `estimate_output_size(size, json)` | Heuristic JavaScript Number, not a measured prediction |
| `is_ffmpeg_available()` | Whether the installed bridge reports availability |

## Formats and behavior

- **Raster:** PNG, JPEG, WebP, GIF, BMP, TIFF and ICO. TIFF/ICO compression handles one selected image. Untransformed PNG optimization can reduce channels and optionally quantize colors; PNG quantization settings do not apply to the generic transform/conversion path.
- **Animation:** GIF frames are processed incrementally with timing and loop settings. Animated PNG/WebP are returned unchanged; their transforms and format conversions are rejected. Animated GIF conversion and detected AVIF/HEIC image sequences are rejected to avoid discarding frames.
- **AVIF/HEIC still images:** require an FFmpeg decoder. Default output is WebP for AVIF and JPEG for HEIC. AVIF/HEIC encoding is unavailable. Ambiguous HEIF dimensions may be omitted.
- **SVG:** removes parsed comments when both `svg.minify` and `svg.remove_comments` are enabled, and can recompress supported base64 image data URIs. Other text, IDs, CSS and whitespace are preserved. SVG rasterization and transforms are unavailable.
- **Audio:** MP3, WAV, FLAC, Ogg, AAC, Opus, AC3, AIFF, AMR and WMA through FFmpeg. Auto output is MP3 for MP3, Ogg for Ogg, Opus for Opus/AMR, otherwise AAC.
- **Video:** MP4, WebM, MOV, AVI, MKV, WMV, FLV and MPEG through FFmpeg. Auto output is WebM for WebM, otherwise MP4. Video can also be converted to audio.

The public `compress` APIs never return more bytes than the input. If an encode grows the file, they return the original bytes, so a requested transform or conversion may not be applied. `result.unchanged` reports byte-for-byte equality with the input; read it before consuming the result. Use `result.format_out` for the returned file's extension. Failures return errors rather than an original-file fallback in every case.

Configuration objects reject unknown fields, including nested typos. This is a compatibility change for callers that previously attached extra properties. Configuration errors expose `code`, `message`, `recoverable`, and `field`. File-aware preflight shares audio/video argument planning with execution. The supplied bridges validate output options; initialized Node bridges also check the installed encoder inventory. Preflight does not inspect stream/container compatibility or confirm that hardware codecs can access a device.

With no explicit output format, audio stream copy retains the source audio container; video retains its source container when both audio and video codecs are `copy`. Explicit output formats still take precedence and must support the copied codecs. Image resize preflight rejects requests whose minimum workspace already exceeds the allocation limit; decoding may reveal a higher pixel cost and reject additional requests.

Nonzero video trim starts require re-encoding; preflight rejects them with video stream copy because the requested start may have no independently decodable keyframe. The supplied bridges decode the first video/audio frame of stream-copy outputs before returning them, within the remaining execution timeout. This catches muxers that report success for unusable output; it adds remux overhead and requires a decoder. It is not a full-file integrity check.

Metadata preservation is best effort. Untransformed PNG optimization retains supported color/HDR, text and EXIF fields; unknown ancillary chunks and metadata across transforms/conversions may be lost. WebP encoding is always lossless. JPEG quality affects encoding, but progressive/Huffman/subsampling settings are compatibility fields.

Other compatibility fields currently unused by encoding are `input_hint`, `png.interlaced`, `webp.quality/lossless/method`, `avif.*`, `gif.lossy/optimize_frames` and `svg.precision`. `gif.max_colors` applies only to the static GIF quantization candidate. `chunk_size_mb` does not enable automatic streaming. Accepted settings are not all implemented; validation warnings are not exhaustive.

## FFmpeg bridge

Importing the WASM library does not load FFmpeg. Import `initialize` from `multimicromedia/ffmpeg/browser` and call `await initialize(ffmpeg, limits)` with a loaded FFmpeg.wasm instance. For Node, import it from `multimicromedia/ffmpeg/node` and call `await initialize(limits)`; it tries `FFMPEG_PATH`, then `ffmpeg-static`, then `ffmpeg` on PATH. The source implementations are in `bench/`. Available codecs depend on the FFmpeg build.

A custom bridge must provide `globalThis.__ffmpeg__.isAvailable()` and `execute(args, bytes)`. `execute` returns a Promise resolving to `{ data: Uint8Array }` or `{ error: string }`. Arguments describe output options; input probing is automatic.

An optional synchronous `validateArgs(args)` hook returns `null` or a diagnostic string. File-aware WASM preflight uses it when present; custom bridges without the hook keep execution-only validation. Node inventories encoder names and codec aliases during initialization. If inventory loading fails, execution remains the authority.

Both supplied bridges serialize jobs and accept these optional defaults:

```javascript
const limits = {
  timeoutMs: 600_000,
  maxQueuedJobs: 16,
  maxQueuedBytes: 512 * 1024 * 1024,
  maxOutputBytes: 512 * 1024 * 1024
};
```

Values must be positive safe integers; `timeoutMs` cannot exceed 2,147,483,647. Queue limits include active jobs. Output size is checked after reading in the browser and before reading in Node; it does not cap FFmpeg's internal memory or filesystem. `ffmpeg.extra_flags` accepts supported output options; input paths, extra outputs and global options are controlled by the bridge. Filter expressions are trusted application configuration.

## Benchmarks

```bash
npm ci
npm run build:all
npm run benchmark -- --repeats=3 --label=current
```

The runner generates short synthetic images, animation, SVG, audio and video under `target/benchmarks/media`, then measures conversions, quality levels, transforms, trims, PNG settings, stream copy and invalid configurations. Install FFmpeg or set `FFMPEG_PATH` to an executable if `ffmpeg-static` is unavailable. Generators unsupported by that FFmpeg build are listed in the report.

For comparisons, add `--reuse-media` to reuse the generated files byte-for-byte; missing files fail the run rather than silently reducing coverage. Without this flag, regenerating container IDs can change fixture hashes. Use the same FFmpeg executable for both runs.

Add `--extended` for tiny and 16-bit images, stereo/low-rate audio, silent/portrait video, combined operations and explicit remuxes. Add `--verify-decode` to independently decode unique returned outputs with native FFmpeg outside the compression timer (SVG is excluded). Run once without `--reuse-media` to generate the expanded fixtures.

Raw JSON observations and a detailed Markdown report are saved to `target/benchmarks/<label>.{json,md}`. Reports break down input families, operations, requested formats, defaults, quality settings, slow cases and failures. Individual repetition timings, sizes and statuses are retained in JSON. Errors include deliberately invalid configurations. These are sequential Node/native FFmpeg measurements, not browser timings, visual-quality scores, peak-memory measurements or exhaustive format coverage. Generated fixtures are ignored by Git; no test suite is added.

Regenerate a report or compare matching configurations without recompressing:

```bash
node scripts/benchmark-report.mjs target/benchmarks/current.json target/benchmarks/previous.json
```

## Memory and storage

Compression holds complete input/output buffers. Raster decoding and transform workspace have allocation guards, not a process-wide memory limit. The separate Rust chunking/storage utilities are not used automatically by `compress`; arbitrary media chunks cannot be independently encoded and concatenated.

Storage honors its configured in-memory fallback ceiling and transfers OPFS data in 4 MiB chunks. Cancellation cleanup is best effort and may not finish after page/process termination. `StreamingProcessor::try_finalize()` rejects incomplete input; `finalize()` returns accumulated bytes without that check.

See [AUDIT.md](https://github.com/tmburt04/multimicromedia/blob/main/AUDIT.md) for verification scope and [CONTRIBUTING.md](https://github.com/tmburt04/multimicromedia/blob/main/CONTRIBUTING.md) for development and release checks.

## License

MIT

Sponsored by [notschmee](https://notschmee.com).
