use crate::error::{CompressionError, Result};
use std::collections::HashMap;

pub(super) struct IndexedPixels {
    pub palette: Vec<[u8; 4]>,
    pub indices: Vec<u8>,
}

/// Preserve exact colors when possible; otherwise use the MIT-licensed NeuQuant encoder.
pub(super) fn quantize_rgba(
    pixels: &[u8],
    max_colors: u16,
    sample_factor: i32,
) -> Result<IndexedPixels> {
    if pixels.is_empty() || !pixels.len().is_multiple_of(4) || !(2..=256).contains(&max_colors) {
        return Err(CompressionError::InvalidInput {
            reason: "quantization requires RGBA pixels and 2–256 colors".into(),
        });
    }
    if pixels.len() as u64 / 4 > super::memory_budget() / 32 {
        return Err(CompressionError::MemoryLimitExceeded);
    }
    let mut palette = Vec::with_capacity(max_colors as usize);
    let mut colors = HashMap::with_capacity(max_colors as usize);
    let mut indices = Vec::new();
    indices
        .try_reserve_exact(pixels.len() / 4)
        .map_err(|_| CompressionError::MemoryLimitExceeded)?;
    for pixel in pixels.chunks_exact(4) {
        let color = [pixel[0], pixel[1], pixel[2], pixel[3]];
        let index = if let Some(index) = colors.get(&color) {
            *index
        } else {
            if palette.len() == max_colors as usize {
                break;
            }
            let index = palette.len() as u8;
            palette.push(color);
            colors.insert(color, index);
            index
        };
        indices.push(index);
    }
    if indices.len() == pixels.len() / 4 {
        return Ok(IndexedPixels { palette, indices });
    }
    let sample_factor = sample_factor.clamp(1, 30).min((pixels.len() / 4) as i32);
    let quantizer = color_quant::NeuQuant::new(sample_factor, max_colors as usize, pixels);
    palette.clear();
    palette.extend(
        quantizer
            .color_map_rgba()
            .chunks_exact(4)
            .map(|c| [c[0], c[1], c[2], c[3]]),
    );
    // Quantization must not introduce transparency into an opaque source.
    if pixels.chunks_exact(4).all(|pixel| pixel[3] == 255) {
        for color in &mut palette {
            color[3] = 255;
        }
    }
    indices.clear();
    indices.extend(
        pixels
            .chunks_exact(4)
            .map(|pixel| quantizer.index_of(pixel) as u8),
    );
    Ok(IndexedPixels { palette, indices })
}
