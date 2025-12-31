# mmm — Changelog

## [0.1.0] — 2025-12-11

### Core
- Image compression: PNG, JPEG, WebP, GIF, BMP, TIFF, ICO
- SVG processing: minification + embedded base64 recompression (76% savings)
- JPEG optimization: DCT-aware compression (36–72% savings)
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
