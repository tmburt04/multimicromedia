mod avif;
mod gif;
mod jpeg;
mod png;
mod svg;
mod webp;

pub use avif::*;
pub use gif::*;
pub use jpeg::*;
pub use png::*;
pub use svg::*;
pub use webp::*;

use crate::config::{CompressionConfig, OutputFormat};
use crate::error::{CompressionError, Result};
use image::{DynamicImage, ImageReader};
use std::io::Cursor;

pub async fn compress_generic(
    data: &[u8],
    config: &CompressionConfig,
    output_format: Option<OutputFormat>,
) -> Result<Vec<u8>> {
    let original_size = data.len();

    // Decode image
    let img = decode_image(data)?;

    // Apply transforms
    let img = apply_transforms(img, config)?;

    // Try multiple formats and pick the smallest
    let mut candidates = Vec::new();

    // Try original format
    let output_format = output_format.unwrap_or(OutputFormat::Png);
    if let Ok(output) = encode_image(&img, output_format, config) {
        if output.len() < original_size {
            candidates.push(output);
        }
    }

    // Try JPEG (usually best compression for photos)
    for quality in [85u8, 75, 65] {
        let mut jpeg_out = Vec::new();
        let jpeg_cfg = crate::config::JpegConfig {
            quality,
            progressive: true,
            optimize_coding: true,
            chroma_subsampling: crate::config::ChromaSubsampling::S420,
        };
        if encode_jpeg_image(&img, &jpeg_cfg, &mut jpeg_out).is_ok() {
            if jpeg_out.len() < original_size {
                candidates.push(jpeg_out);
                break; // Found a good JPEG, stop trying lower qualities
            }
        }
    }

    // Return smallest candidate
    if let Some(best) = candidates.into_iter().min_by_key(|c| c.len()) {
        return Ok(best);
    }

    Ok(data.to_vec())
}

pub async fn compress_heic(
    data: &[u8],
    config: &CompressionConfig,
    output_format: Option<OutputFormat>,
) -> Result<Vec<u8>> {
    use crate::compression::audio::{execute_ffmpeg, is_ffmpeg_available};

    let output_format = output_format.unwrap_or(OutputFormat::Jpeg);

    // HEIC requires FFmpeg for decoding (no pure Rust decoder available)
    if !is_ffmpeg_available() {
        return Err(CompressionError::FfmpegUnavailable);
    }

    let quality = config.quality;

    let args: Vec<String> = match output_format {
        OutputFormat::Jpeg => {
            let q = heic_quality_to_ffmpeg_jpeg(quality);
            vec![
                "-c:v".into(), "mjpeg".into(),
                "-q:v".into(), q.to_string(),
                "-f".into(), "mjpeg".into(),
            ]
        }
        OutputFormat::Png => {
            vec![
                "-c:v".into(), "png".into(),
                "-f".into(), "image2".into(),
            ]
        }
        OutputFormat::Webp => {
            vec![
                "-c:v".into(), "libwebp".into(),
                "-lossless".into(), "0".into(),
                "-quality".into(), quality.to_string(),
                "-f".into(), "webp".into(),
            ]
        }
        _ => {
            return Err(CompressionError::UnsupportedFormat {
                detected: format!("HEIC to {:?}", output_format),
                fallback: Some("Use JPEG, PNG, or WebP".to_string()),
            });
        }
    };

    let result = execute_ffmpeg(data, &args).await?;

    // Only return if smaller than original
    if result.len() < data.len() {
        Ok(result)
    } else {
        Ok(data.to_vec())
    }
}

/// Convert quality (1-100) to FFmpeg JPEG quality (1-31, lower = better)
fn heic_quality_to_ffmpeg_jpeg(quality: u8) -> u8 {
    let q = 32 - ((quality as u16 * 31) / 100) as u8;
    q.clamp(1, 31)
}

pub fn decode_image(data: &[u8]) -> Result<DynamicImage> {
    ImageReader::new(Cursor::new(data))
        .with_guessed_format()
        .map_err(|e| CompressionError::DecodeFailed {
            format: "image".to_string(),
            detail: e.to_string(),
        })?
        .decode()
        .map_err(|e| CompressionError::DecodeFailed {
            format: "image".to_string(),
            detail: e.to_string(),
        })
}

pub fn apply_transforms(mut img: DynamicImage, config: &CompressionConfig) -> Result<DynamicImage> {
    // Apply crop first
    if let Some(ref crop) = config.crop {
        img = img.crop_imm(crop.x, crop.y, crop.width, crop.height);
    }

    // Apply resize
    if let Some(ref resize) = config.resize {
        let (orig_w, orig_h) = (img.width(), img.height());
        let (new_w, new_h) = calculate_resize_dimensions(
            orig_w,
            orig_h,
            resize.width,
            resize.height,
            resize.preserve_aspect,
            resize.mode,
        );

        if new_w != orig_w || new_h != orig_h {
            img = img.resize_exact(new_w, new_h, image::imageops::FilterType::Lanczos3);
        }
    }

    Ok(img)
}

fn calculate_resize_dimensions(
    orig_w: u32,
    orig_h: u32,
    target_w: Option<u32>,
    target_h: Option<u32>,
    preserve_aspect: bool,
    mode: crate::config::ResizeMode,
) -> (u32, u32) {
    use crate::config::ResizeMode;

    match (target_w, target_h) {
        (Some(w), Some(h)) => {
            if !preserve_aspect || mode == ResizeMode::Exact {
                (w, h)
            } else {
                let aspect = orig_w as f64 / orig_h as f64;
                match mode {
                    ResizeMode::Fit => {
                        // Fit within bounds
                        let fit_w = w as f64;
                        let fit_h = h as f64;
                        if fit_w / fit_h > aspect {
                            ((fit_h * aspect) as u32, h)
                        } else {
                            (w, (fit_w / aspect) as u32)
                        }
                    }
                    ResizeMode::Fill | ResizeMode::Cover => {
                        // Fill/cover bounds
                        let fill_w = w as f64;
                        let fill_h = h as f64;
                        if fill_w / fill_h > aspect {
                            (w, (fill_w / aspect) as u32)
                        } else {
                            ((fill_h * aspect) as u32, h)
                        }
                    }
                    ResizeMode::Exact => (w, h),
                }
            }
        }
        (Some(w), None) => {
            if preserve_aspect {
                let aspect = orig_h as f64 / orig_w as f64;
                (w, (w as f64 * aspect) as u32)
            } else {
                (w, orig_h)
            }
        }
        (None, Some(h)) => {
            if preserve_aspect {
                let aspect = orig_w as f64 / orig_h as f64;
                ((h as f64 * aspect) as u32, h)
            } else {
                (orig_w, h)
            }
        }
        (None, None) => (orig_w, orig_h),
    }
}

pub fn encode_image(
    img: &DynamicImage,
    format: OutputFormat,
    config: &CompressionConfig,
) -> Result<Vec<u8>> {
    let mut output = Vec::new();

    match format {
        OutputFormat::Png => {
            let cfg = config.get_png_config();
            encode_png_image(img, &cfg, &mut output)?;
        }
        OutputFormat::Jpeg => {
            let cfg = config.get_jpeg_config();
            encode_jpeg_image(img, &cfg, &mut output)?;
        }
        OutputFormat::Webp => {
            let cfg = config.get_webp_config();
            encode_webp_image(img, &cfg, &mut output)?;
        }
        OutputFormat::Gif => {
            // Single frame GIF - use image crate's GIF encoder
            img.write_to(&mut Cursor::new(&mut output), image::ImageFormat::Gif)
                .map_err(|e| CompressionError::EncodeFailed {
                    format: "GIF".to_string(),
                    detail: e.to_string(),
                })?;
        }
        OutputFormat::Bmp => {
            img.write_to(&mut Cursor::new(&mut output), image::ImageFormat::Bmp)
                .map_err(|e| CompressionError::EncodeFailed {
                    format: "BMP".to_string(),
                    detail: e.to_string(),
                })?;
        }
        OutputFormat::Tiff => {
            img.write_to(&mut Cursor::new(&mut output), image::ImageFormat::Tiff)
                .map_err(|e| CompressionError::EncodeFailed {
                    format: "TIFF".to_string(),
                    detail: e.to_string(),
                })?;
        }
        OutputFormat::Ico => {
            img.write_to(&mut Cursor::new(&mut output), image::ImageFormat::Ico)
                .map_err(|e| CompressionError::EncodeFailed {
                    format: "ICO".to_string(),
                    detail: e.to_string(),
                })?;
        }
        OutputFormat::Avif => {
            // AVIF encoding would go here
            return Err(CompressionError::UnsupportedFormat {
                detected: "AVIF".to_string(),
                fallback: Some("Use WebP or JPEG".to_string()),
            });
        }
        _ => {
            return Err(CompressionError::UnsupportedFormat {
                detected: format!("{:?}", format),
                fallback: None,
            });
        }
    }

    Ok(output)
}

fn encode_png_image(
    img: &DynamicImage,
    cfg: &crate::config::PngConfig,
    output: &mut Vec<u8>,
) -> Result<()> {
    use ::png::{BitDepth, ColorType, Compression};

    let rgba = img.to_rgba8();
    let (width, height) = (rgba.width(), rgba.height());

    let compression = match cfg.compression_level {
        0..=2 => Compression::Fast,
        3..=6 => Compression::Default,
        _ => Compression::Best,
    };

    let mut encoder = ::png::Encoder::new(Cursor::new(output), width, height);
    encoder.set_color(ColorType::Rgba);
    encoder.set_depth(BitDepth::Eight);
    encoder.set_compression(compression);

    let mut writer = encoder.write_header().map_err(|e| CompressionError::EncodeFailed {
        format: "PNG".to_string(),
        detail: e.to_string(),
    })?;

    writer
        .write_image_data(rgba.as_raw())
        .map_err(|e| CompressionError::EncodeFailed {
            format: "PNG".to_string(),
            detail: e.to_string(),
        })?;

    Ok(())
}

fn encode_jpeg_image(
    img: &DynamicImage,
    cfg: &crate::config::JpegConfig,
    output: &mut Vec<u8>,
) -> Result<()> {
    let rgb = img.to_rgb8();
    let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(
        Cursor::new(output),
        cfg.quality,
    );

    encoder
        .encode(
            rgb.as_raw(),
            rgb.width(),
            rgb.height(),
            image::ExtendedColorType::Rgb8,
        )
        .map_err(|e| CompressionError::EncodeFailed {
            format: "JPEG".to_string(),
            detail: e.to_string(),
        })?;

    Ok(())
}

fn encode_webp_image(
    img: &DynamicImage,
    cfg: &crate::config::WebpConfig,
    output: &mut Vec<u8>,
) -> Result<()> {
    // Pure Rust WebP encoder only supports lossless
    // If lossy is requested, caller should use JPEG instead
    if !cfg.lossless {
        // Fall back to JPEG for lossy compression (better WASM support)
        let jpeg_cfg = crate::config::JpegConfig {
            quality: cfg.quality,
            progressive: true,
            optimize_coding: true,
            chroma_subsampling: crate::config::ChromaSubsampling::S420,
        };
        return encode_jpeg_image(img, &jpeg_cfg, output);
    }

    let encoder = image::codecs::webp::WebPEncoder::new_lossless(Cursor::new(output));
    img.write_with_encoder(encoder).map_err(|e| CompressionError::EncodeFailed {
        format: "WebP".to_string(),
        detail: e.to_string(),
    })?;

    Ok(())
}
