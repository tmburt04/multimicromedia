use crate::config::CompressionConfig;
use crate::detection::FileAnalysis;
use crate::error::{CompressionError, Result};
use image::codecs::gif::GifDecoder;
use image::{AnimationDecoder, DynamicImage, Frame, ImageReader, RgbaImage};
use std::io::Cursor;

pub async fn compress_gif(
    data: &[u8],
    analysis: &FileAnalysis,
    config: &CompressionConfig,
) -> Result<Vec<u8>> {
    let is_animated = analysis.is_animated.unwrap_or(false);
    let frame_count = analysis.frame_count.unwrap_or(1);

    if is_animated && frame_count > 1 {
        compress_animated_gif(data, config)
    } else {
        compress_static_gif(data, config)
    }
}

fn compress_static_gif(data: &[u8], config: &CompressionConfig) -> Result<Vec<u8>> {
    let original_size = data.len();
    let gif_cfg = config.get_gif_config();

    // Decode GIF
    let img = ImageReader::new(Cursor::new(data))
        .with_guessed_format()
        .map_err(|e| CompressionError::DecodeFailed {
            format: "GIF".to_string(),
            detail: e.to_string(),
        })?
        .decode()
        .map_err(|e| CompressionError::DecodeFailed {
            format: "GIF".to_string(),
            detail: e.to_string(),
        })?;

    // Apply transforms
    let img = super::apply_transforms(img, config)?;

    // Try quantized encoding for better compression
    let mut candidates = Vec::new();

    // Strategy 1: Direct re-encode
    let mut direct_output = Vec::new();
    if img
        .write_to(&mut Cursor::new(&mut direct_output), image::ImageFormat::Gif)
        .is_ok()
    {
        candidates.push(direct_output);
    }

    // Strategy 2: Quantized with reduced colors
    if let Ok(quantized) = quantize_gif_frame(&img.to_rgba8(), gif_cfg.max_colors) {
        candidates.push(quantized);
    }

    // Pick smallest result
    let best = candidates
        .into_iter()
        .filter(|c| !c.is_empty())
        .min_by_key(|c| c.len());

    match best {
        Some(output) if output.len() < original_size => Ok(output),
        _ => Ok(data.to_vec()),
    }
}

fn quantize_gif_frame(rgba: &RgbaImage, max_colors: u16) -> Result<Vec<u8>> {
    let width = rgba.width() as usize;
    let height = rgba.height() as usize;

    let mut liq = imagequant::new();
    liq.set_speed(5).map_err(|e| CompressionError::EncodeFailed {
        format: "GIF".to_string(),
        detail: format!("quantization error: {:?}", e),
    })?;
    liq.set_max_colors(max_colors as u32)
        .map_err(|e| CompressionError::EncodeFailed {
            format: "GIF".to_string(),
            detail: format!("max colors error: {:?}", e),
        })?;

    let rgba_pixels: Vec<imagequant::RGBA> = rgba
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
            format: "GIF".to_string(),
            detail: format!("image creation error: {:?}", e),
        })?;

    let mut res = liq
        .quantize(&mut img_data)
        .map_err(|e| CompressionError::EncodeFailed {
            format: "GIF".to_string(),
            detail: format!("quantization error: {:?}", e),
        })?;

    let (palette, pixels) = res.remapped(&mut img_data).map_err(|e| CompressionError::EncodeFailed {
        format: "GIF".to_string(),
        detail: format!("remap error: {:?}", e),
    })?;

    // Build GIF manually with optimized palette
    let mut output = Vec::new();
    {
        use gif::{Encoder, Frame as GifFrame, Repeat};

        let mut encoder = Encoder::new(&mut output, width as u16, height as u16, &[])
            .map_err(|e| CompressionError::EncodeFailed {
                format: "GIF".to_string(),
                detail: e.to_string(),
            })?;

        encoder.set_repeat(Repeat::Infinite).map_err(|e| CompressionError::EncodeFailed {
            format: "GIF".to_string(),
            detail: e.to_string(),
        })?;

        // Build palette (RGB triplets)
        let gif_palette: Vec<u8> = palette.iter().flat_map(|c| [c.r, c.g, c.b]).collect();

        let mut frame = GifFrame::default();
        frame.width = width as u16;
        frame.height = height as u16;
        frame.palette = Some(gif_palette);
        frame.buffer = std::borrow::Cow::Owned(pixels);

        // Handle transparency
        if let Some(transparent_idx) = palette.iter().position(|c| c.a < 128) {
            frame.transparent = Some(transparent_idx as u8);
        }

        encoder.write_frame(&frame).map_err(|e| CompressionError::EncodeFailed {
            format: "GIF".to_string(),
            detail: e.to_string(),
        })?;
    }

    Ok(output)
}

fn compress_animated_gif(data: &[u8], config: &CompressionConfig) -> Result<Vec<u8>> {
    let original_size = data.len();
    let gif_cfg = config.get_gif_config();

    // Decode all frames
    let decoder = GifDecoder::new(Cursor::new(data)).map_err(|e| CompressionError::DecodeFailed {
        format: "GIF".to_string(),
        detail: e.to_string(),
    })?;

    let frames: Vec<Frame> = decoder
        .into_frames()
        .collect_frames()
        .map_err(|e| CompressionError::DecodeFailed {
            format: "GIF".to_string(),
            detail: e.to_string(),
        })?;

    if frames.is_empty() {
        return Ok(data.to_vec());
    }

    // Use optimized encoding with quantization
    let mut output = Vec::new();
    {
        use gif::{Encoder, Frame as GifFrame, Repeat};

        let first_frame = &frames[0];
        let width = first_frame.buffer().width() as u16;
        let height = first_frame.buffer().height() as u16;

        let mut encoder = Encoder::new(&mut output, width, height, &[])
            .map_err(|e| CompressionError::EncodeFailed {
                format: "GIF".to_string(),
                detail: e.to_string(),
            })?;

        encoder.set_repeat(Repeat::Infinite).map_err(|e| CompressionError::EncodeFailed {
            format: "GIF".to_string(),
            detail: e.to_string(),
        })?;

        // Process each frame with quantization
        for frame in &frames {
            let (numerator, denominator) = frame.delay().numer_denom_ms();
            let delay_ms = (numerator as f32 / denominator as f32) as u16;
            let delay_cs = delay_ms / 10; // Convert to centiseconds

            let rgba = frame.buffer();

            // Apply transforms if needed
            let rgba = if config.resize.is_some() || config.crop.is_some() {
                let img = DynamicImage::ImageRgba8(rgba.clone());
                let transformed = super::apply_transforms(img, config)?;
                transformed.into_rgba8()
            } else {
                rgba.clone()
            };

            // Quantize frame
            if let Some((palette, pixels)) = quantize_frame_data(&rgba, gif_cfg.max_colors) {
                let gif_palette: Vec<u8> = palette.iter().flat_map(|c| [c.r, c.g, c.b]).collect();

                let mut gif_frame = GifFrame::default();
                gif_frame.width = rgba.width() as u16;
                gif_frame.height = rgba.height() as u16;
                gif_frame.delay = delay_cs;
                gif_frame.palette = Some(gif_palette);
                gif_frame.buffer = std::borrow::Cow::Owned(pixels);

                if let Some(transparent_idx) = palette.iter().position(|c| c.a < 128) {
                    gif_frame.transparent = Some(transparent_idx as u8);
                }

                let _ = encoder.write_frame(&gif_frame);
            }
        }
    }

    // Only return if smaller
    if !output.is_empty() && output.len() < original_size {
        Ok(output)
    } else {
        Ok(data.to_vec())
    }
}

fn quantize_frame_data(
    rgba: &RgbaImage,
    max_colors: u16,
) -> Option<(Vec<imagequant::RGBA>, Vec<u8>)> {
    let width = rgba.width() as usize;
    let height = rgba.height() as usize;

    let mut liq = imagequant::new();
    liq.set_speed(8).ok()?; // Faster for animation
    liq.set_max_colors(max_colors as u32).ok()?;

    let rgba_pixels: Vec<imagequant::RGBA> = rgba
        .as_raw()
        .chunks_exact(4)
        .map(|c| imagequant::RGBA {
            r: c[0],
            g: c[1],
            b: c[2],
            a: c[3],
        })
        .collect();

    let mut img_data = liq.new_image(rgba_pixels, width, height, 0.0).ok()?;
    let mut res = liq.quantize(&mut img_data).ok()?;
    let (palette, pixels) = res.remapped(&mut img_data).ok()?;

    Some((palette, pixels))
}

pub fn count_gif_frames(data: &[u8]) -> u32 {
    let mut count = 0u32;
    let mut i = 13; // Skip header

    // Skip global color table
    if data.len() > 10 {
        let flags = data[10];
        if (flags & 0x80) != 0 {
            let table_size = 3 * (1 << ((flags & 0x07) + 1));
            i += table_size as usize;
        }
    }

    while i < data.len() {
        match data.get(i) {
            Some(0x2C) => {
                // Image descriptor
                count += 1;
                i += 10;
                // Skip local color table if present
                if let Some(&local_flags) = data.get(i - 1) {
                    if (local_flags & 0x80) != 0 {
                        let table_size = 3 * (1 << ((local_flags & 0x07) + 1));
                        i += table_size as usize;
                    }
                }
                // Skip LZW min code size + sub-blocks
                i += 1;
                while i < data.len() {
                    let block_size = data.get(i).copied().unwrap_or(0) as usize;
                    if block_size == 0 {
                        i += 1;
                        break;
                    }
                    i += block_size + 1;
                }
            }
            Some(0x21) => {
                // Extension
                i += 2;
                while i < data.len() {
                    let block_size = data.get(i).copied().unwrap_or(0) as usize;
                    if block_size == 0 {
                        i += 1;
                        break;
                    }
                    i += block_size + 1;
                }
            }
            Some(0x3B) => break, // Trailer
            _ => i += 1,
        }
    }

    count
}
