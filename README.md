# mmm — multimicromedia

Client-side media compression. Pure WASM. Zero server.

## Formats

| Type | Supported |
|------|-----------|
| Image | PNG, JPEG, WebP, GIF, BMP, TIFF, ICO, SVG |
| Audio | MP3, WAV, FLAC, OGG, AAC, Opus, AC3, AIFF, AMR, WMA |
| Video | MP4, WebM, MOV, AVI, MKV, WMV, FLV, MPEG |

## Install

```bash
wasm-pack build --target web --release
```

## API

```javascript
import init, { compress, analyze_file, detect_file_type } from './pkg/compression_wasm.js';

await init();

// Detect
detect_file_type(bytes);      // → "png"

// Analyze
analyze_file(bytes);          // → { format, width, height, hasAlpha, ... }

// Compress (defaults)
await compress_with_defaults(bytes);

// Compress (custom)
await compress(bytes, JSON.stringify({
  output_format: 'webp',
  quality: 85,
  resize: { width: 1920, preserve_aspect: true }
}));
```

### Builder

```javascript
import { create_config } from './pkg/compression_wasm.js';

const cfg = create_config()
  .quality(80)
  .output_format('webp')
  .resize(1200, null)
  .build();
```

## Functions

| Function | Purpose |
|----------|---------|
| `compress(data, config)` | Custom compression |
| `compress_with_defaults(data)` | Auto-optimal compression |
| `analyze_file(data)` | Metadata extraction |
| `detect_file_type(data)` | Magic byte detection |
| `validate_config(json)` | Pre-flight config check |
| `get_supported_formats()` | Format registry |
| `estimate_output_size(size, config)` | Size prediction |

## Audio/Video

Requires FFmpeg.wasm bridge:

```javascript
globalThis.ffmpegBridge = {
  isAvailable: async () => ffmpeg.loaded,
  execute: async (args, data) => { /* ... */ }
};
```

## Build Targets

| Target | Command |
|--------|---------|
| Browser | `wasm-pack build --target web` |
| Bundler | `wasm-pack build --target bundler` |
| Node | `wasm-pack build --target nodejs` |

## Benchmarks

```bash
node bench/run.mjs
```

See `BENCHMARK_REPORT.md` for results.

## License

MIT
