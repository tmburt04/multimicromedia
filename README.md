# multimicromedia

Client-side media compression via WebAssembly. Zero server dependency.

> **Live demo:** [notschmee.com/widgets/image-compression](https://notschmee.com/widgets/image-compression)

## Formats

| Type | Supported |
|------|-----------|
| Image | PNG, JPEG, WebP, GIF, BMP, TIFF, ICO, SVG |
| Audio | MP3, WAV, FLAC, OGG, AAC, Opus, AC3, AIFF, AMR, WMA |
| Video | MP4, WebM, MOV, AVI, MKV, WMV, FLV, MPEG |

## Install

```bash
./scripts/build.sh web
```

## Usage

```javascript
import init, { compress, analyze_file, detect_file_type } from './pkg/mmm-js.js';

await init();

// Detect format
detect_file_type(bytes);      // → "png"

// Analyze metadata
analyze_file(bytes);          // → { format, width, height, hasAlpha, ... }

// Compress with defaults
await compress_with_defaults(bytes);

// Compress with config
await compress(bytes, JSON.stringify({
  output_format: 'webp',
  quality: 85,
  resize: { width: 1920, preserve_aspect: true }
}));
```

### Config Builder

```javascript
import { create_config } from './pkg/mmm-js.js';

const cfg = create_config()
  .quality(80)
  .output_format('webp')
  .resize(1200, null)
  .build();
```

## API

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

FFmpeg WASM (~22MB) is auto-downloaded on first run for A/V support.

```javascript
// Bridge interface (auto-configured)
globalThis.__ffmpeg__ = {
  isAvailable: () => boolean,
  execute: (args, data) => Promise<{data, error}>
};
```

## Build Targets

| Target | Command | Artifacts |
|--------|---------|-----------|
| Browser | `./scripts/build.sh web` | `mmm-js.js`, `mmm-js_bg.wasm` |
| Node | `./scripts/build.sh node` | `mmm-node.js`, `mmm-node_bg.wasm` |
| Bundler | `./scripts/build.sh bundler` | `mmm-js.js`, `mmm-js_bg.wasm` |

## Testbench

```bash
./scripts/serve.sh        # http://localhost:8080
```

Auto-downloads FFmpeg WASM on first run. No manual setup required.

## License

MIT

---

<sub>Sponsored by [notschmee](https://notschmee.com)</sub>
