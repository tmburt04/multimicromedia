use crate::config::{CompressionConfig, PngConfig};
use crate::error::{CompressionError, Result};
use png::{AdaptiveFilterType, BitDepth, ColorType, Compression, Encoder, FilterType};
use std::io::Cursor;

pub async fn compress_png(data: &[u8], config: &CompressionConfig) -> Result<Vec<u8>> {
    let cfg = config.get_png_config();
    let original_size = data.len();

    // Try multiple strategies and pick the best
    let mut candidates = Vec::new();

    // Strategy 1: Direct re-encode with optimal settings
    if let Ok(result) = compress_png_direct(data, &cfg) {
        if !result.is_empty() {
            candidates.push(result);
        }
    }

    // Strategy 2: Channel reduction (lossless)
    if let Ok(result) = compress_png_channel_reduced(data, &cfg) {
        if !result.is_empty() {
            candidates.push(result);
        }
    }

    // Strategy 3: Quantization - try for larger files or when explicitly enabled
    // This is lossy but can dramatically reduce file size
    let should_quantize = cfg.quantize || original_size > 50_000; // 50KB threshold
    if should_quantize {
        if let Ok(result) = compress_png_quantized(data, &cfg) {
            if !result.is_empty() {
                candidates.push(result);
            }
        }
    }

    // Select best result (smallest)
    let best = candidates
        .into_iter()
        .min_by_key(|c| c.len())
        .unwrap_or_else(|| data.to_vec());

    // Return best if it's smaller than original
    if best.len() < original_size {
        return Ok(best);
    }

    // Fallback: try converting to JPEG for better compression
    if let Ok(img) = decode_png(data) {
        for quality in [80u8, 70, 60] {
            let mut jpeg_out = Vec::new();
            let jpeg_cfg = crate::config::JpegConfig {
                quality,
                progressive: true,
                optimize_coding: true,
                chroma_subsampling: crate::config::ChromaSubsampling::S420,
            };
            if super::encode_jpeg_image(&img, &jpeg_cfg, &mut jpeg_out).is_ok() {
                if jpeg_out.len() < original_size {
                    return Ok(jpeg_out);
                }
            }
        }
    }

    Ok(data.to_vec())
}

fn decode_png(data: &[u8]) -> Result<image::DynamicImage> {
    image::ImageReader::new(Cursor::new(data))
        .with_guessed_format()
        .map_err(|e| CompressionError::DecodeFailed {
            format: "PNG".to_string(),
            detail: e.to_string(),
        })?
        .decode()
        .map_err(|e| CompressionError::DecodeFailed {
            format: "PNG".to_string(),
            detail: e.to_string(),
        })
}

fn compress_png_direct(data: &[u8], cfg: &PngConfig) -> Result<Vec<u8>> {
    let decoder = png::Decoder::new(Cursor::new(data));
    let mut reader = decoder.read_info().map_err(|e| CompressionError::DecodeFailed {
        format: "PNG".to_string(),
        detail: e.to_string(),
    })?;

    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).map_err(|e| CompressionError::DecodeFailed {
        format: "PNG".to_string(),
        detail: e.to_string(),
    })?;

    let width = info.width;
    let height = info.height;
    let color_type = info.color_type;
    let bit_depth = info.bit_depth;

    let compression = match cfg.compression_level {
        0..=2 => Compression::Fast,
        3..=6 => Compression::Default,
        _ => Compression::Best,
    };

    let mut output = Vec::new();
    {
        let mut encoder = Encoder::new(Cursor::new(&mut output), width, height);
        encoder.set_color(color_type);
        encoder.set_depth(bit_depth);
        encoder.set_compression(compression);

        if cfg.compression_level >= 4 {
            encoder.set_adaptive_filter(AdaptiveFilterType::Adaptive);
        } else if cfg.compression_level >= 2 {
            encoder.set_filter(FilterType::Sub);
        }

        let mut writer = encoder.write_header().map_err(|e| CompressionError::EncodeFailed {
            format: "PNG".to_string(),
            detail: e.to_string(),
        })?;

        writer
            .write_image_data(&buf[..info.buffer_size()])
            .map_err(|e| CompressionError::EncodeFailed {
                format: "PNG".to_string(),
                detail: e.to_string(),
            })?;
    }

    Ok(output)
}

fn compress_png_quantized(data: &[u8], cfg: &PngConfig) -> Result<Vec<u8>> {
    use image::ImageReader;

    let img = ImageReader::new(Cursor::new(data))
        .with_guessed_format()
        .map_err(|e| CompressionError::DecodeFailed {
            format: "PNG".to_string(),
            detail: e.to_string(),
        })?
        .decode()
        .map_err(|e| CompressionError::DecodeFailed {
            format: "PNG".to_string(),
            detail: e.to_string(),
        })?;

    let rgba_img = img.to_rgba8();
    let width = rgba_img.width() as usize;
    let height = rgba_img.height() as usize;

    let mut liq = imagequant::new();
    liq.set_speed(match cfg.compression_level {
        0..=2 => 10,
        3..=5 => 5,
        _ => 1,
    })
    .map_err(|e| CompressionError::EncodeFailed {
        format: "PNG".to_string(),
        detail: format!("quantization speed error: {:?}", e),
    })?;

    liq.set_quality(0, 100)
        .map_err(|e| CompressionError::EncodeFailed {
            format: "PNG".to_string(),
            detail: format!("quantization quality error: {:?}", e),
        })?;

    let rgba_pixels: Vec<imagequant::RGBA> = rgba_img
        .as_raw()
        .chunks_exact(4)
        .map(|c| imagequant::RGBA {
            r: c[0],
            g: c[1],
            b: c[2],
            a: c[3],
        })
        .collect();

    let mut img_data = liq
        .new_image(rgba_pixels, width, height, 0.0)
        .map_err(|e| CompressionError::EncodeFailed {
            format: "PNG".to_string(),
            detail: format!("image creation error: {:?}", e),
        })?;

    let mut res = liq.quantize(&mut img_data).map_err(|e| CompressionError::EncodeFailed {
        format: "PNG".to_string(),
        detail: format!("quantization error: {:?}", e),
    })?;

    let (palette, pixels) = res.remapped(&mut img_data).map_err(|e| CompressionError::EncodeFailed {
        format: "PNG".to_string(),
        detail: format!("remap error: {:?}", e),
    })?;

    let compression = match cfg.compression_level {
        0..=2 => Compression::Fast,
        3..=6 => Compression::Default,
        _ => Compression::Best,
    };

    let mut output = Vec::new();
    {
        let mut encoder = Encoder::new(Cursor::new(&mut output), width as u32, height as u32);
        encoder.set_color(ColorType::Indexed);
        encoder.set_depth(BitDepth::Eight);
        encoder.set_compression(compression);

        if cfg.compression_level >= 4 {
            encoder.set_adaptive_filter(AdaptiveFilterType::Adaptive);
        }

        let palette_data: Vec<u8> = palette.iter().flat_map(|c| [c.r, c.g, c.b]).collect();
        encoder.set_palette(palette_data);

        let alpha_palette: Vec<u8> = palette.iter().map(|c| c.a).collect();
        encoder.set_trns(alpha_palette);

        let mut writer = encoder.write_header().map_err(|e| CompressionError::EncodeFailed {
            format: "PNG".to_string(),
            detail: e.to_string(),
        })?;

        writer
            .write_image_data(&pixels)
            .map_err(|e| CompressionError::EncodeFailed {
                format: "PNG".to_string(),
                detail: e.to_string(),
            })?;
    }

    Ok(output)
}

fn compress_png_channel_reduced(data: &[u8], cfg: &PngConfig) -> Result<Vec<u8>> {
    let decoder = png::Decoder::new(Cursor::new(data));
    let mut reader = decoder.read_info().map_err(|e| CompressionError::DecodeFailed {
        format: "PNG".to_string(),
        detail: e.to_string(),
    })?;

    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).map_err(|e| CompressionError::DecodeFailed {
        format: "PNG".to_string(),
        detail: e.to_string(),
    })?;

    let width = info.width;
    let height = info.height;
    let src_color_type = info.color_type;
    let src_bit_depth = info.bit_depth;
    let src = &buf[..info.buffer_size()];

    // Only attempt reduction for 8-bit images
    if src_bit_depth != BitDepth::Eight {
        return Err(CompressionError::InvalidInput {
            reason: "channel reduction only for 8-bit".to_string(),
        });
    }

    let (target_color, target_depth, packed) = match src_color_type {
        ColorType::Rgba => analyze_and_reduce_rgba(src, width, height),
        ColorType::Rgb => analyze_and_reduce_rgb(src, width, height),
        _ => return Err(CompressionError::InvalidInput {
            reason: "no reduction possible".to_string(),
        }),
    };

    if packed.is_empty() {
        return Err(CompressionError::InvalidInput {
            reason: "no reduction achieved".to_string(),
        });
    }

    let compression = match cfg.compression_level {
        0..=2 => Compression::Fast,
        3..=6 => Compression::Default,
        _ => Compression::Best,
    };

    let mut output = Vec::new();
    {
        let mut encoder = Encoder::new(Cursor::new(&mut output), width, height);
        encoder.set_color(target_color);
        encoder.set_depth(target_depth);
        encoder.set_compression(compression);

        if cfg.compression_level >= 4 {
            encoder.set_adaptive_filter(AdaptiveFilterType::Adaptive);
        } else if cfg.compression_level >= 2 {
            encoder.set_filter(FilterType::Sub);
        }

        let mut writer = encoder.write_header().map_err(|e| CompressionError::EncodeFailed {
            format: "PNG".to_string(),
            detail: e.to_string(),
        })?;

        writer
            .write_image_data(&packed)
            .map_err(|e| CompressionError::EncodeFailed {
                format: "PNG".to_string(),
                detail: e.to_string(),
            })?;
    }

    Ok(output)
}

fn analyze_and_reduce_rgba(src: &[u8], width: u32, height: u32) -> (ColorType, BitDepth, Vec<u8>) {
    let mut all_opaque = true;
    let mut grayscale = true;

    for px in src.chunks_exact(4) {
        if px[3] != 255 {
            all_opaque = false;
        }
        if !(px[0] == px[1] && px[1] == px[2]) {
            grayscale = false;
        }
        if !grayscale && !all_opaque {
            break;
        }
    }

    let size = (width as usize) * (height as usize);

    if grayscale && all_opaque {
        // RGBA -> Grayscale
        let mut packed = Vec::with_capacity(size);
        for px in src.chunks_exact(4) {
            packed.push(px[0]);
        }
        (ColorType::Grayscale, BitDepth::Eight, packed)
    } else if grayscale {
        // RGBA -> GrayscaleAlpha
        let mut packed = Vec::with_capacity(size * 2);
        for px in src.chunks_exact(4) {
            packed.extend_from_slice(&[px[0], px[3]]);
        }
        (ColorType::GrayscaleAlpha, BitDepth::Eight, packed)
    } else if all_opaque {
        // RGBA -> RGB
        let mut packed = Vec::with_capacity(size * 3);
        for px in src.chunks_exact(4) {
            packed.extend_from_slice(&[px[0], px[1], px[2]]);
        }
        (ColorType::Rgb, BitDepth::Eight, packed)
    } else {
        // No reduction possible
        (ColorType::Rgba, BitDepth::Eight, Vec::new())
    }
}

fn analyze_and_reduce_rgb(src: &[u8], width: u32, height: u32) -> (ColorType, BitDepth, Vec<u8>) {
    let mut grayscale = true;

    for px in src.chunks_exact(3) {
        if !(px[0] == px[1] && px[1] == px[2]) {
            grayscale = false;
            break;
        }
    }

    if grayscale {
        let size = (width as usize) * (height as usize);
        let mut packed = Vec::with_capacity(size);
        for px in src.chunks_exact(3) {
            packed.push(px[0]);
        }
        (ColorType::Grayscale, BitDepth::Eight, packed)
    } else {
        (ColorType::Rgb, BitDepth::Eight, Vec::new())
    }
}
