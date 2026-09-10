mod avif;
mod gif;
mod jpeg;
mod png;
mod quantize;
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
    let img = apply_transforms(decode_image(data)?, config)?;
    let output = encode_image(&img, output_format.unwrap_or(OutputFormat::Png), config)?;
    Ok(if output.len() <= data.len() {
        output
    } else {
        data.to_vec()
    })
}

pub async fn compress_heic(
    data: &[u8],
    config: &CompressionConfig,
    output_format: Option<OutputFormat>,
) -> Result<Vec<u8>> {
    avif::transcode_heif(data, config, output_format.unwrap_or(OutputFormat::Jpeg)).await
}

pub fn decode_image(data: &[u8]) -> Result<DynamicImage> {
    let mut reader = ImageReader::new(Cursor::new(data))
        .with_guessed_format()
        .map_err(|e| CompressionError::DecodeFailed {
            format: "image".into(),
            detail: e.to_string(),
        })?;
    reader.limits(decode_limits());
    reader.decode().map_err(|error| match error {
        image::ImageError::Limits(_) => CompressionError::MemoryLimitExceeded,
        _ => CompressionError::DecodeFailed {
            format: "image".into(),
            detail: error.to_string(),
        },
    })
}

pub(super) fn decode_limits() -> image::Limits {
    let mut limits = image::Limits::default();
    // Leave room for the source, encoder workspace and transformed pixel buffers.
    limits.max_alloc = Some(memory_budget() / 4);
    limits
}

pub(super) const fn memory_budget() -> u64 {
    crate::config::MAX_WASM_MEMORY_MB as u64 * 1024 * 1024
}

pub fn apply_transforms(mut img: DynamicImage, config: &CompressionConfig) -> Result<DynamicImage> {
    if img.width() == 0 || img.height() == 0 {
        return Err(CompressionError::InvalidInput {
            reason: "image dimensions must be greater than zero".into(),
        });
    }
    // Validate here too: Rust callers can invoke handlers without the WASM entry point.
    if let Some(crop) = &config.crop {
        if crop.width == 0
            || crop.height == 0
            || u64::from(crop.x) + u64::from(crop.width) > u64::from(img.width())
            || u64::from(crop.y) + u64::from(crop.height) > u64::from(img.height())
        {
            return Err(CompressionError::InvalidConfig {
                field: "crop".into(),
                reason: "region exceeds image bounds or is empty".into(),
            });
        }
    }
    if let Some(resize) = &config.resize {
        if resize.width == Some(0) || resize.height == Some(0) {
            return Err(CompressionError::InvalidConfig {
                field: "resize".into(),
                reason: "dimensions must be greater than zero".into(),
            });
        }
    }
    if let Some(ref crop) = config.crop {
        if crop.x != 0 || crop.y != 0 || crop.width != img.width() || crop.height != img.height() {
            img = img.crop_imm(crop.x, crop.y, crop.width, crop.height);
        }
    }

    if let Some(ref resize) = config.resize {
        let (orig_w, orig_h) = (img.width(), img.height());
        let (new_w, new_h) = calculate_resize_dimensions(
            orig_w,
            orig_h,
            resize.width,
            resize.height,
            resize.preserve_aspect,
            resize.mode,
        )?;

        let (new_w, new_h) = (new_w.max(1), new_h.max(1));
        if new_w != orig_w || new_h != orig_h {
            validate_resize_workspace(orig_w, orig_h, new_w, new_h, img.color().bytes_per_pixel())?;
            img = img.resize_exact(new_w, new_h, image::imageops::FilterType::Lanczos3);
        }
        if resize.preserve_aspect
            && matches!(
                resize.mode,
                crate::config::ResizeMode::Fill | crate::config::ResizeMode::Cover
            )
        {
            if let (Some(w), Some(h)) = (resize.width, resize.height) {
                if new_w != w || new_h != h {
                    img = img.crop_imm((new_w - w) / 2, (new_h - h) / 2, w, h);
                }
            }
        }
    }

    Ok(img)
}

/// Shared allocation guard; preflight uses a one-byte lower bound until decoding.
pub(crate) fn validate_resize_workspace(
    orig_w: u32,
    orig_h: u32,
    new_w: u32,
    new_h: u32,
    bytes_per_pixel: u8,
) -> Result<()> {
    if (orig_w, orig_h) == (new_w, new_h) {
        return Ok(());
    }
    // The resampler holds source, destination and an RGBA f32 intermediate.
    let pixels =
        (u64::from(orig_w) * u64::from(orig_h)).checked_add(u64::from(new_w) * u64::from(new_h));
    let working_bytes = pixels
        .and_then(|pixels| pixels.checked_mul(u64::from(bytes_per_pixel)))
        .zip((u64::from(orig_w) * u64::from(new_h)).checked_mul(16))
        .and_then(|(pixels, intermediate)| pixels.checked_add(intermediate));
    if working_bytes.is_none_or(|bytes| bytes > memory_budget() / 2) {
        return Err(CompressionError::MemoryLimitExceeded);
    }
    Ok(())
}

pub(crate) fn calculate_resize_dimensions(
    orig_w: u32,
    orig_h: u32,
    target_w: Option<u32>,
    target_h: Option<u32>,
    preserve_aspect: bool,
    mode: crate::config::ResizeMode,
) -> Result<(u32, u32)> {
    use crate::config::ResizeMode;

    let scaled = |numerator: u64, denominator: u32, round_up: bool| -> Result<u32> {
        let value = if round_up {
            numerator.div_ceil(u64::from(denominator))
        } else {
            numerator / u64::from(denominator)
        };
        u32::try_from(value.max(1)).map_err(|_| CompressionError::MemoryLimitExceeded)
    };
    Ok(match (target_w, target_h) {
        (Some(w), Some(h)) => {
            if !preserve_aspect || mode == ResizeMode::Exact {
                (w, h)
            } else {
                let wider = u64::from(w) * u64::from(orig_h) > u64::from(h) * u64::from(orig_w);
                match mode {
                    ResizeMode::Fit => {
                        if wider {
                            (scaled(u64::from(h) * u64::from(orig_w), orig_h, false)?, h)
                        } else {
                            (w, scaled(u64::from(w) * u64::from(orig_h), orig_w, false)?)
                        }
                    }
                    ResizeMode::Fill | ResizeMode::Cover => {
                        if wider {
                            (w, scaled(u64::from(w) * u64::from(orig_h), orig_w, true)?)
                        } else {
                            (scaled(u64::from(h) * u64::from(orig_w), orig_h, true)?, h)
                        }
                    }
                    ResizeMode::Exact => (w, h),
                }
            }
        }
        (Some(w), None) => {
            if preserve_aspect {
                (w, scaled(u64::from(w) * u64::from(orig_h), orig_w, false)?)
            } else {
                (w, orig_h)
            }
        }
        (None, Some(h)) => {
            if preserve_aspect {
                (scaled(u64::from(h) * u64::from(orig_w), orig_h, false)?, h)
            } else {
                (orig_w, h)
            }
        }
        (None, None) => (orig_w, orig_h),
    })
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
            output = png::optimize_png(&output, config)?;
        }
        OutputFormat::Jpeg => {
            let cfg = config.get_jpeg_config();
            encode_jpeg_image(img, &cfg, &mut output)?;
        }
        OutputFormat::Webp => {
            encode_webp_image(img, &mut output)?;
        }
        OutputFormat::Gif => {
            // Single frame GIF - use image crate's GIF encoder
            image::DynamicImage::ImageRgba8(img.to_rgba8())
                .write_to(&mut Cursor::new(&mut output), image::ImageFormat::Gif)
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
            // TIFF's encoder accepts RGBA but not grayscale+alpha; retain depth and alpha.
            let converted = match img {
                DynamicImage::ImageLumaA8(_) => Some(DynamicImage::ImageRgba8(img.to_rgba8())),
                DynamicImage::ImageLumaA16(_) => Some(DynamicImage::ImageRgba16(img.to_rgba16())),
                _ => None,
            };
            converted
                .as_ref()
                .unwrap_or(img)
                .write_to(&mut Cursor::new(&mut output), image::ImageFormat::Tiff)
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
    use image::codecs::png::{CompressionType, FilterType, PngEncoder};

    let compression = match cfg.compression_level {
        0 => CompressionType::Uncompressed,
        level => CompressionType::Level(level.min(9)),
    };
    let filter = match cfg.compression_level {
        0..=1 => FilterType::NoFilter,
        2..=3 => FilterType::Sub,
        _ => FilterType::Adaptive,
    };
    // DynamicImage forwards its existing channels and depth, preserving 16-bit
    // PNGs and avoiding an unconditional full-sized RGBA conversion.
    img.write_with_encoder(PngEncoder::new_with_quality(output, compression, filter))
        .map_err(|e| CompressionError::EncodeFailed {
            format: "PNG".into(),
            detail: e.to_string(),
        })
}

fn encode_jpeg_image(
    img: &DynamicImage,
    cfg: &crate::config::JpegConfig,
    output: &mut Vec<u8>,
) -> Result<()> {
    let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(output, cfg.quality);
    // encode_image reads pixels directly and also preserves grayscale encoding.
    let encoded = match img {
        DynamicImage::ImageLuma8(buffer) => encoder.encode_image(buffer),
        DynamicImage::ImageLumaA8(buffer) => encoder.encode_image(buffer),
        DynamicImage::ImageRgb8(buffer) => encoder.encode_image(buffer),
        DynamicImage::ImageRgba8(buffer) => encoder.encode_image(buffer),
        _ => encoder.encode_image(img),
    };
    encoded.map_err(|e| CompressionError::EncodeFailed {
        format: "JPEG".into(),
        detail: e.to_string(),
    })
}

fn encode_webp_image(img: &DynamicImage, output: &mut Vec<u8>) -> Result<()> {
    // DynamicImage borrows supported 8-bit grayscale/RGB(A) buffers and the
    // encoder's compatibility hook converts 16-bit/float input only when needed.
    // The pure Rust encoder supports lossless WebP.
    let encoder = image::codecs::webp::WebPEncoder::new_lossless(Cursor::new(output));
    img.write_with_encoder(encoder)
        .map_err(|e| CompressionError::EncodeFailed {
            format: "WebP".to_string(),
            detail: e.to_string(),
        })?;

    Ok(())
}
