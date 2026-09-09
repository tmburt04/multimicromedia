use super::{AvifConfig, GifConfig, JpegConfig, PngConfig, SvgConfig, WebpConfig};

/// Budget used by image allocation guards, not a process-wide memory cap.
pub const MAX_WASM_MEMORY_MB: u32 = 256;
pub const DEFAULT_CHUNK_SIZE_MB: u32 = 64;
/// Legacy advisory threshold; the main dispatcher only enforces output <= input.
pub const COMPRESSION_THRESHOLD_PCT: f64 = 1.0;

/// Suggested settings; fields unsupported by current encoders remain inactive.
pub struct FormatDefaults;

impl FormatDefaults {
    pub fn jpeg_web() -> JpegConfig {
        JpegConfig {
            quality: 82,
            progressive: true,
            optimize_coding: true,
            chroma_subsampling: super::ChromaSubsampling::S420,
        }
    }

    pub fn jpeg_print() -> JpegConfig {
        JpegConfig {
            quality: 95,
            progressive: false,
            optimize_coding: true,
            chroma_subsampling: super::ChromaSubsampling::S444,
        }
    }

    pub fn png_web() -> PngConfig {
        PngConfig {
            compression_level: 6,
            quantize: true,
            max_colors: 256,
            interlaced: false,
        }
    }

    pub fn png_lossless() -> PngConfig {
        PngConfig {
            compression_level: 9,
            quantize: false,
            max_colors: 256,
            interlaced: false,
        }
    }

    pub fn webp_web() -> WebpConfig {
        WebpConfig {
            quality: 80,
            lossless: false,
            method: 4,
        }
    }

    pub fn webp_lossless() -> WebpConfig {
        WebpConfig {
            quality: 100,
            lossless: true,
            method: 6,
        }
    }

    pub fn avif_web() -> AvifConfig {
        AvifConfig {
            quality: 65,
            speed: 6,
        }
    }

    pub fn avif_quality() -> AvifConfig {
        AvifConfig {
            quality: 80,
            speed: 4,
        }
    }

    pub fn gif_optimized() -> GifConfig {
        GifConfig {
            max_colors: 256,
            lossy: 20,
            optimize_frames: true,
        }
    }

    pub fn svg_minified() -> SvgConfig {
        SvgConfig {
            minify: true,
            compress_embedded_images: true,
            precision: 2,
            remove_comments: true,
        }
    }
}
