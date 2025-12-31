pub mod audio;
pub mod image;
pub mod video;

use crate::config::{CompressionConfig, OutputFormat};
use crate::detection::{FileAnalysis, FileFormat};
use crate::error::{CompressionError, Result};
use crate::result::{CompressionResult, CompressionStats};

pub async fn compress(
    data: &[u8],
    analysis: &FileAnalysis,
    config: &CompressionConfig,
) -> Result<CompressionResult> {
    let start = js_sys::Date::now();
    let original_size = data.len() as u64;

    let output_format = config
        .output_format
        .or_else(|| OutputFormat::from_file_format(analysis.format));

    let compressed = match analysis.format {
        // Image formats
        FileFormat::Png => image::compress_png(data, config).await?,
        FileFormat::Jpeg => image::compress_jpeg(data, config, output_format).await?,
        // WebP/GIF: prefer FFmpeg for better compression if available
        FileFormat::Webp => {
            if audio::is_ffmpeg_available() {
                compress_webp_ffmpeg(data, analysis, config).await?
            } else {
                image::compress_webp(data, config, output_format).await?
            }
        }
        FileFormat::Gif => {
            if audio::is_ffmpeg_available() {
                compress_gif_ffmpeg(data, analysis, config).await?
            } else {
                image::compress_gif(data, analysis, config).await?
            }
        }
        FileFormat::Bmp | FileFormat::Tiff | FileFormat::Ico => {
            image::compress_generic(data, config, output_format).await?
        }
        FileFormat::Svg => image::compress_svg(data, config).await?,
        FileFormat::Avif => image::compress_avif(data, config, output_format).await?,
        FileFormat::Heic => image::compress_heic(data, config, output_format).await?,

        // Audio formats
        FileFormat::Mp3
        | FileFormat::Wav
        | FileFormat::Flac
        | FileFormat::Ogg
        | FileFormat::Aac
        | FileFormat::Opus
        | FileFormat::Ac3
        | FileFormat::Aiff
        | FileFormat::Amr
        | FileFormat::Wma => audio::compress_audio(data, analysis, config).await?,

        // Video formats
        FileFormat::Mp4
        | FileFormat::Webm
        | FileFormat::Mov
        | FileFormat::Avi
        | FileFormat::Mkv
        | FileFormat::Wmv
        | FileFormat::Flv
        | FileFormat::Mpeg => video::compress_video(data, analysis, config).await?,

        FileFormat::Unknown => {
            return Err(CompressionError::UnsupportedFormat {
                detected: "unknown".to_string(),
                fallback: None,
            });
        }
    };

    let time_ms = (js_sys::Date::now() - start) as u64;
    let compressed_size = compressed.len() as u64;

    let format_out = output_format
        .map(|f| f.extension())
        .unwrap_or(analysis.format.extension());

    let stats = CompressionStats::new(
        original_size,
        compressed_size,
        time_ms,
        analysis.format.extension(),
        format_out,
    );

    Ok(CompressionResult::new(compressed, stats))
}

/// Compress GIF using FFmpeg - convert to WebP for better compression
async fn compress_gif_ffmpeg(
    data: &[u8],
    analysis: &FileAnalysis,
    config: &CompressionConfig,
) -> Result<Vec<u8>> {
    let quality = config.quality;

    // Try converting GIF to WebP (much better compression)
    let mut args = Vec::new();
    args.push("-f".to_string());
    args.push("gif".to_string());
    args.push("-c:v".to_string());
    args.push("libwebp".to_string());
    args.push("-lossless".to_string());
    args.push("0".to_string());
    args.push("-quality".to_string());
    args.push(quality.to_string());
    args.push("-loop".to_string());
    args.push("0".to_string());
    args.push("-f".to_string());
    args.push("webp".to_string());

    if let Ok(compressed) = audio::execute_ffmpeg(data, &args).await {
        if compressed.len() < data.len() {
            return Ok(compressed);
        }
    }

    // Fallback: try optimized GIF output
    let mut args = Vec::new();
    args.push("-f".to_string());
    args.push("gif".to_string());
    args.push("-f".to_string());
    args.push("gif".to_string());

    if let Ok(compressed) = audio::execute_ffmpeg(data, &args).await {
        if compressed.len() < data.len() {
            return Ok(compressed);
        }
    }

    // Final fallback to Rust implementation
    image::compress_gif(data, analysis, config).await
}

/// Compress WebP using FFmpeg for lossy compression
async fn compress_webp_ffmpeg(
    data: &[u8],
    analysis: &FileAnalysis,
    config: &CompressionConfig,
) -> Result<Vec<u8>> {
    let quality = config.quality;
    let _ = analysis; // silence unused warning

    // Try lossy WebP with lower quality for compression
    let try_quality = if quality > 70 { 70 } else { quality };

    let mut args = Vec::new();
    args.push("-f".to_string());
    args.push("webp".to_string());
    args.push("-c:v".to_string());
    args.push("libwebp".to_string());
    args.push("-lossless".to_string());
    args.push("0".to_string());
    args.push("-quality".to_string());
    args.push(try_quality.to_string());
    args.push("-f".to_string());
    args.push("webp".to_string());

    if let Ok(compressed) = audio::execute_ffmpeg(data, &args).await {
        if compressed.len() < data.len() {
            return Ok(compressed);
        }
    }

    // Try JPEG conversion (often smaller than WebP for photos)
    let mut args = Vec::new();
    args.push("-f".to_string());
    args.push("webp".to_string());
    args.push("-c:v".to_string());
    args.push("mjpeg".to_string());
    args.push("-q:v".to_string());
    args.push("5".to_string()); // High quality JPEG
    args.push("-f".to_string());
    args.push("mjpeg".to_string());

    if let Ok(compressed) = audio::execute_ffmpeg(data, &args).await {
        if compressed.len() < data.len() {
            return Ok(compressed);
        }
    }

    // Fallback to Rust implementation
    image::compress_webp(data, config, Some(OutputFormat::Jpeg)).await
}

pub fn select_best_output(
    candidates: Vec<Vec<u8>>,
    original_size: usize,
    threshold_pct: f64,
) -> Vec<u8> {
    if candidates.is_empty() {
        return Vec::new();
    }

    let mut best = &candidates[0];
    for candidate in &candidates {
        if candidate.len() < best.len() {
            best = candidate;
        }
    }

    // Check if improvement meets threshold
    let improvement = if best.len() < original_size {
        ((original_size - best.len()) as f64 / original_size as f64) * 100.0
    } else {
        0.0
    };

    if improvement >= threshold_pct {
        best.clone()
    } else {
        // Return empty to signal no improvement
        Vec::new()
    }
}
