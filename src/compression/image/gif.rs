use crate::config::CompressionConfig;
use crate::detection::FileAnalysis;
use crate::error::{CompressionError, Result};
use image::codecs::gif::GifDecoder;
use image::{AnimationDecoder, DynamicImage, Frame, ImageDecoder, RgbaImage};
use std::io::{self, Cursor, Write};

pub async fn compress_gif(
    data: &[u8],
    analysis: &FileAnalysis,
    config: &CompressionConfig,
) -> Result<Vec<u8>> {
    if config
        .output_format
        .is_some_and(|format| format != crate::config::OutputFormat::Gif)
    {
        return super::compress_generic(data, config, config.output_format).await;
    }
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

    let img = super::apply_transforms(super::decode_image(data)?, config)?;
    let mut best = None;
    let mut direct_output = Vec::new();
    let direct_result = img.write_to(
        &mut Cursor::new(&mut direct_output),
        image::ImageFormat::Gif,
    );
    if direct_result.is_ok() && !direct_output.is_empty() && direct_output.len() < original_size {
        best = Some(direct_output);
    } else {
        drop(direct_output);
    }

    // Reuse the decoded RGBA allocation when available and retain just one candidate.
    let rgba = img.into_rgba8();
    match quantize_gif_frame(&rgba, gif_cfg.max_colors) {
        Ok(output)
            if output.len() < original_size
                && best
                    .as_ref()
                    .is_none_or(|current: &Vec<u8>| output.len() < current.len()) =>
        {
            best = Some(output);
        }
        Err(error) if direct_result.is_err() => return Err(error),
        _ => {}
    }
    Ok(best.unwrap_or_else(|| data.to_vec()))
}

fn quantize_gif_frame(rgba: &RgbaImage, max_colors: u16) -> Result<Vec<u8>> {
    let width = rgba.width() as usize;
    let height = rgba.height() as usize;
    if width > u16::MAX as usize || height > u16::MAX as usize {
        return Err(CompressionError::EncodeFailed {
            format: "GIF".into(),
            detail: "dimensions exceed the GIF limit of 65535 pixels".into(),
        });
    }

    let indexed = super::quantize::quantize_rgba(rgba.as_raw(), max_colors, 5)?;
    let palette = indexed.palette;
    let mut pixels = indexed.indices;
    let transparent = rgba
        .as_raw()
        .chunks_exact(4)
        .any(|pixel| pixel[3] < 128)
        .then(|| {
            palette
                .iter()
                .enumerate()
                .min_by_key(|(_, color)| color[3])
                .unwrap()
                .0
        });
    if let Some(transparent) = transparent {
        for (index, pixel) in pixels.iter_mut().zip(rgba.as_raw().chunks_exact(4)) {
            if pixel[3] < 128 {
                *index = transparent as u8;
            } else if *index as usize == transparent {
                // GIF has only one transparent index; opaque pixels must use another entry.
                *index = palette
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| *i != transparent)
                    .min_by_key(|(_, color)| {
                        (0..3)
                            .map(|c| (i32::from(color[c]) - i32::from(pixel[c])).pow(2))
                            .sum::<i32>()
                    })
                    .map(|(i, _)| i as u8)
                    .unwrap_or(*index);
            }
        }
    }

    let mut output = Vec::new();
    {
        use gif::{Encoder, Frame as GifFrame};

        let mut encoder =
            Encoder::new(&mut output, width as u16, height as u16, &[]).map_err(|e| {
                CompressionError::EncodeFailed {
                    format: "GIF".to_string(),
                    detail: e.to_string(),
                }
            })?;

        let gif_palette: Vec<u8> = palette.iter().flat_map(|c| [c[0], c[1], c[2]]).collect();

        let mut frame = GifFrame {
            width: width as u16,
            height: height as u16,
            palette: Some(gif_palette),
            buffer: std::borrow::Cow::Owned(pixels),
            ..Default::default()
        };

        frame.transparent = transparent.map(|index| index as u8);

        encoder
            .write_frame(&frame)
            .map_err(|e| CompressionError::EncodeFailed {
                format: "GIF".to_string(),
                detail: e.to_string(),
            })?;
    }

    Ok(output)
}

fn compress_animated_gif(data: &[u8], config: &CompressionConfig) -> Result<Vec<u8>> {
    let decode_error = |e: image::ImageError| CompressionError::DecodeFailed {
        format: "GIF".into(),
        detail: e.to_string(),
    };
    let encode_error = |e: image::ImageError| CompressionError::EncodeFailed {
        format: "GIF".into(),
        detail: e.to_string(),
    };
    // Read the loop extension separately; the image decoder supplies composited frames.
    let header = gif::DecodeOptions::new()
        .read_info(Cursor::new(data))
        .map_err(|e| CompressionError::DecodeFailed {
            format: "GIF".into(),
            detail: e.to_string(),
        })?;
    let repeat = match header.repeat() {
        gif::Repeat::Infinite => image::codecs::gif::Repeat::Infinite,
        gif::Repeat::Finite(n) => image::codecs::gif::Repeat::Finite(n),
    };
    drop(header);
    let mut decoder = GifDecoder::new(Cursor::new(data)).map_err(decode_error)?;
    decoder
        .set_limits(super::decode_limits())
        .map_err(decode_error)?;
    let mut output = AnimationOutput {
        bytes: Vec::new(),
        limit: data.len().min((super::memory_budget() / 4) as usize),
        exceeded: false,
    };
    let encoded = (|| -> Result<()> {
        let mut encoder = image::codecs::gif::GifEncoder::new(&mut output);
        encoder.set_repeat(repeat).map_err(encode_error)?;
        // Stream frames instead of retaining the entire decoded animation in WASM memory.
        for frame in decoder.into_frames() {
            let frame = frame.map_err(decode_error)?;
            let delay = frame.delay();
            let img =
                super::apply_transforms(DynamicImage::ImageRgba8(frame.into_buffer()), config)?;
            encoder
                .encode_frame(Frame::from_parts(img.into_rgba8(), 0, 0, delay))
                .map_err(encode_error)?;
        }
        Ok(())
    })();
    if output.exceeded {
        return Ok(data.to_vec());
    }
    encoded?;
    Ok(if output.bytes.len() < data.len() {
        output.bytes
    } else {
        data.to_vec()
    })
}

// Stop encoding when an animation can no longer satisfy the size contract.
// This also caps memory for long animations whose composited frames grow greatly.
struct AnimationOutput {
    bytes: Vec<u8>,
    limit: usize,
    exceeded: bool,
}

impl Write for AnimationOutput {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > self.limit.saturating_sub(self.bytes.len()) {
            self.exceeded = true;
            return Err(io::Error::other("animation exceeds output size budget"));
        }
        self.bytes
            .try_reserve(bytes.len())
            .map_err(|error| io::Error::new(io::ErrorKind::OutOfMemory, error))?;
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub fn count_gif_frames(data: &[u8]) -> u32 {
    let mut analysis = FileAnalysis::new(crate::detection::FileFormat::Gif, data.len() as u64);
    crate::analysis::analyze_gif(data, &mut analysis).ok();
    analysis.frame_count.unwrap_or(0)
}
