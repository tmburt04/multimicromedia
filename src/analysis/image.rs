use crate::detection::FileAnalysis;
use crate::error::{CompressionError, Result};
use std::io::Cursor;

pub fn analyze_png(data: &[u8], analysis: &mut FileAnalysis) -> Result<()> {
    let decoder = png::Decoder::new(Cursor::new(data));
    let reader = decoder.read_info().map_err(|e| CompressionError::DecodeFailed {
        format: "PNG".to_string(),
        detail: e.to_string(),
    })?;

    let info = reader.info();
    analysis.width = Some(info.width);
    analysis.height = Some(info.height);
    analysis.bit_depth = Some(info.bit_depth as u8);
    analysis.has_alpha = Some(matches!(
        info.color_type,
        png::ColorType::Rgba | png::ColorType::GrayscaleAlpha
    ));
    analysis.color_type = Some(format!("{:?}", info.color_type));
    analysis.is_animated = Some(info.animation_control.is_some());

    if let Some(anim) = info.animation_control {
        analysis.frame_count = Some(anim.num_frames);
    }

    Ok(())
}

pub fn analyze_jpeg(data: &[u8], analysis: &mut FileAnalysis) -> Result<()> {
    // Use zune-jpeg for fast header parsing
    let mut decoder = zune_jpeg::JpegDecoder::new(data);
    
    if decoder.decode_headers().is_err() {
        // Fallback to image crate
        return analyze_jpeg_fallback(data, analysis);
    }

    let info = decoder.info().ok_or_else(|| CompressionError::DecodeFailed {
        format: "JPEG".to_string(),
        detail: "failed to read header".to_string(),
    })?;

    analysis.width = Some(info.width as u32);
    analysis.height = Some(info.height as u32);
    analysis.has_alpha = Some(false); // JPEG doesn't support alpha
    analysis.bit_depth = Some(8);

    // Detect progressive
    // zune-jpeg doesn't expose this directly, so we check markers
    analysis.is_animated = Some(false);

    Ok(())
}

fn analyze_jpeg_fallback(data: &[u8], analysis: &mut FileAnalysis) -> Result<()> {
    use image::ImageReader;

    let reader = ImageReader::new(Cursor::new(data))
        .with_guessed_format()
        .map_err(|e| CompressionError::DecodeFailed {
            format: "JPEG".to_string(),
            detail: e.to_string(),
        })?;

    let dims = reader.into_dimensions().map_err(|e| CompressionError::DecodeFailed {
        format: "JPEG".to_string(),
        detail: e.to_string(),
    })?;

    analysis.width = Some(dims.0);
    analysis.height = Some(dims.1);
    analysis.has_alpha = Some(false);
    analysis.bit_depth = Some(8);
    analysis.is_animated = Some(false);

    Ok(())
}

pub fn analyze_webp(data: &[u8], analysis: &mut FileAnalysis) -> Result<()> {
    // Parse WebP header manually for speed
    if data.len() < 30 {
        return Err(CompressionError::DecodeFailed {
            format: "WebP".to_string(),
            detail: "file too small".to_string(),
        });
    }

    // Check for VP8X (extended) chunk
    let chunk_type = &data[12..16];
    
    match chunk_type {
        b"VP8X" => {
            // Extended WebP
            if data.len() < 30 {
                return Err(CompressionError::DecodeFailed {
                    format: "WebP".to_string(),
                    detail: "VP8X chunk too small".to_string(),
                });
            }
            let flags = data[20];
            analysis.has_alpha = Some((flags & 0x10) != 0);
            analysis.is_animated = Some((flags & 0x02) != 0);
            
            // Canvas dimensions (24-bit each)
            let width = u32::from_le_bytes([data[24], data[25], data[26], 0]) + 1;
            let height = u32::from_le_bytes([data[27], data[28], data[29], 0]) + 1;
            analysis.width = Some(width);
            analysis.height = Some(height);
        }
        b"VP8 " => {
            // Lossy WebP
            if data.len() < 30 {
                return Err(CompressionError::DecodeFailed {
                    format: "WebP".to_string(),
                    detail: "VP8 chunk too small".to_string(),
                });
            }
            // Skip to frame header
            let frame_start = 20;
            if data.len() > frame_start + 10 {
                let width = u16::from_le_bytes([data[frame_start + 6], data[frame_start + 7]]) & 0x3FFF;
                let height = u16::from_le_bytes([data[frame_start + 8], data[frame_start + 9]]) & 0x3FFF;
                analysis.width = Some(width as u32);
                analysis.height = Some(height as u32);
            }
            analysis.has_alpha = Some(false);
            analysis.is_animated = Some(false);
        }
        b"VP8L" => {
            // Lossless WebP
            if data.len() < 25 {
                return Err(CompressionError::DecodeFailed {
                    format: "WebP".to_string(),
                    detail: "VP8L chunk too small".to_string(),
                });
            }
            // Parse VP8L header
            let b = u32::from_le_bytes([data[21], data[22], data[23], data[24]]);
            let width = (b & 0x3FFF) + 1;
            let height = ((b >> 14) & 0x3FFF) + 1;
            let has_alpha = ((b >> 28) & 1) != 0;
            
            analysis.width = Some(width);
            analysis.height = Some(height);
            analysis.has_alpha = Some(has_alpha);
            analysis.is_animated = Some(false);
        }
        _ => {
            // Unknown chunk, try image crate fallback
            return analyze_webp_fallback(data, analysis);
        }
    }

    analysis.bit_depth = Some(8);

    Ok(())
}

fn analyze_webp_fallback(data: &[u8], analysis: &mut FileAnalysis) -> Result<()> {
    use image::ImageReader;

    let reader = ImageReader::new(Cursor::new(data))
        .with_guessed_format()
        .map_err(|e| CompressionError::DecodeFailed {
            format: "WebP".to_string(),
            detail: e.to_string(),
        })?;

    let dims = reader.into_dimensions().map_err(|e| CompressionError::DecodeFailed {
        format: "WebP".to_string(),
        detail: e.to_string(),
    })?;

    analysis.width = Some(dims.0);
    analysis.height = Some(dims.1);
    analysis.bit_depth = Some(8);

    Ok(())
}

pub fn analyze_gif(data: &[u8], analysis: &mut FileAnalysis) -> Result<()> {
    if data.len() < 13 {
        return Err(CompressionError::DecodeFailed {
            format: "GIF".to_string(),
            detail: "file too small".to_string(),
        });
    }

    // Parse logical screen descriptor
    let width = u16::from_le_bytes([data[6], data[7]]);
    let height = u16::from_le_bytes([data[8], data[9]]);

    analysis.width = Some(width as u32);
    analysis.height = Some(height as u32);
    analysis.bit_depth = Some(8);

    // Count frames by scanning for image descriptors
    let mut frame_count = 0u32;
    let mut i = 13; // Skip header
    
    // Skip global color table if present
    let flags = data[10];
    if (flags & 0x80) != 0 {
        let color_table_size = 3 * (1 << ((flags & 0x07) + 1));
        i += color_table_size as usize;
    }

    while i < data.len() {
        match data.get(i) {
            Some(0x2C) => {
                // Image descriptor
                frame_count += 1;
                i += 10; // Skip image descriptor
                
                // Skip local color table if present
                if i < data.len() {
                    let local_flags = data.get(i - 1).copied().unwrap_or(0);
                    if (local_flags & 0x80) != 0 {
                        let local_table_size = 3 * (1 << ((local_flags & 0x07) + 1));
                        i += local_table_size as usize;
                    }
                }
                
                // Skip LZW minimum code size
                i += 1;
                
                // Skip sub-blocks
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
                // Extension block
                i += 2; // Skip extension type
                // Skip sub-blocks
                while i < data.len() {
                    let block_size = data.get(i).copied().unwrap_or(0) as usize;
                    if block_size == 0 {
                        i += 1;
                        break;
                    }
                    i += block_size + 1;
                }
            }
            Some(0x3B) => {
                // Trailer
                break;
            }
            _ => {
                i += 1;
            }
        }
    }

    analysis.frame_count = Some(frame_count);
    analysis.is_animated = Some(frame_count > 1);
    analysis.has_alpha = Some(true); // GIF supports transparency

    Ok(())
}

pub fn analyze_bmp(data: &[u8], analysis: &mut FileAnalysis) -> Result<()> {
    if data.len() < 26 {
        return Err(CompressionError::DecodeFailed {
            format: "BMP".to_string(),
            detail: "file too small".to_string(),
        });
    }

    // BITMAPINFOHEADER starts at offset 14
    let width = i32::from_le_bytes([data[18], data[19], data[20], data[21]]);
    let height = i32::from_le_bytes([data[22], data[23], data[24], data[25]]).abs();
    let bit_depth = u16::from_le_bytes([data[28], data[29]]);

    analysis.width = Some(width as u32);
    analysis.height = Some(height as u32);
    analysis.bit_depth = Some(bit_depth as u8);
    analysis.has_alpha = Some(bit_depth == 32);
    analysis.is_animated = Some(false);

    Ok(())
}

pub fn analyze_heif_family(data: &[u8], analysis: &mut FileAnalysis) -> Result<()> {
    // Basic HEIF/AVIF parsing - just get dimensions from ispe box
    // This is a simplified parser; full parsing would require a proper ISOBMFF parser
    
    if data.len() < 12 {
        return Err(CompressionError::DecodeFailed {
            format: "HEIF".to_string(),
            detail: "file too small".to_string(),
        });
    }

    // Search for 'ispe' box (image spatial extent)
    for i in 0..data.len().saturating_sub(12) {
        if &data[i..i + 4] == b"ispe" && i >= 4 {
            // ispe box: 4 bytes size, 4 bytes type, 1 byte version, 3 bytes flags, 4 bytes width, 4 bytes height
            if i + 12 <= data.len() {
                let width = u32::from_be_bytes([data[i + 4], data[i + 5], data[i + 6], data[i + 7]]);
                let height = u32::from_be_bytes([data[i + 8], data[i + 9], data[i + 10], data[i + 11]]);
                analysis.width = Some(width);
                analysis.height = Some(height);
                break;
            }
        }
    }

    analysis.bit_depth = Some(8); // Typically 8 or 10
    analysis.is_animated = Some(false);

    Ok(())
}

pub fn analyze_tiff(data: &[u8], analysis: &mut FileAnalysis) -> Result<()> {
    if data.len() < 8 {
        return Err(CompressionError::DecodeFailed {
            format: "TIFF".to_string(),
            detail: "file too small".to_string(),
        });
    }

    let is_little_endian = data[0] == b'I';

    let read_u16 = |offset: usize| -> u16 {
        if offset + 1 >= data.len() {
            return 0;
        }
        if is_little_endian {
            u16::from_le_bytes([data[offset], data[offset + 1]])
        } else {
            u16::from_be_bytes([data[offset], data[offset + 1]])
        }
    };

    let read_u32 = |offset: usize| -> u32 {
        if offset + 3 >= data.len() {
            return 0;
        }
        if is_little_endian {
            u32::from_le_bytes([data[offset], data[offset + 1], data[offset + 2], data[offset + 3]])
        } else {
            u32::from_be_bytes([data[offset], data[offset + 1], data[offset + 2], data[offset + 3]])
        }
    };

    // Read tag value based on type field
    // Type 3 = SHORT (u16), Type 4 = LONG (u32)
    let read_tag_value = |entry_offset: usize| -> u32 {
        let tag_type = read_u16(entry_offset + 2);
        let value_offset = entry_offset + 8;
        match tag_type {
            3 => read_u16(value_offset) as u32, // SHORT
            4 => read_u32(value_offset),        // LONG
            _ => read_u32(value_offset),        // Default to LONG
        }
    };

    // Get IFD offset
    let ifd_offset = read_u32(4) as usize;
    if ifd_offset + 2 > data.len() {
        return Ok(());
    }

    let num_entries = read_u16(ifd_offset) as usize;

    for i in 0..num_entries {
        let entry_offset = ifd_offset + 2 + i * 12;
        if entry_offset + 12 > data.len() {
            break;
        }

        let tag = read_u16(entry_offset);

        match tag {
            256 => {
                // ImageWidth
                analysis.width = Some(read_tag_value(entry_offset));
            }
            257 => {
                // ImageLength (height)
                analysis.height = Some(read_tag_value(entry_offset));
            }
            258 => {
                // BitsPerSample
                let tag_type = read_u16(entry_offset + 2);
                let value_offset = entry_offset + 8;
                let bits = if tag_type == 3 {
                    read_u16(value_offset) as u8
                } else {
                    8 // Default
                };
                analysis.bit_depth = Some(bits);
            }
            _ => {}
        }
    }

    analysis.is_animated = Some(false);

    Ok(())
}

pub fn analyze_ico(data: &[u8], analysis: &mut FileAnalysis) -> Result<()> {
    if data.len() < 6 {
        return Err(CompressionError::DecodeFailed {
            format: "ICO".to_string(),
            detail: "file too small".to_string(),
        });
    }

    let num_images = u16::from_le_bytes([data[4], data[5]]);
    analysis.frame_count = Some(num_images as u32);

    // Get dimensions of first (largest) image
    if data.len() >= 22 {
        let width = if data[6] == 0 { 256 } else { data[6] as u32 };
        let height = if data[7] == 0 { 256 } else { data[7] as u32 };
        analysis.width = Some(width);
        analysis.height = Some(height);
        analysis.bit_depth = Some(data[8]);
    }

    analysis.has_alpha = Some(true);
    analysis.is_animated = Some(false);

    Ok(())
}
