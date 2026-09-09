use crate::config::{CompressionConfig, OutputFormat};
use crate::error::Result;

pub async fn compress_webp(
    data: &[u8],
    config: &CompressionConfig,
    output_format: Option<OutputFormat>,
) -> Result<Vec<u8>> {
    if analyze_webp(data).is_animated {
        return Ok(data.to_vec());
    }
    super::compress_generic(
        data,
        config,
        Some(output_format.unwrap_or(OutputFormat::Webp)),
    )
    .await
}

pub fn analyze_webp(data: &[u8]) -> WebpInfo {
    let mut info = WebpInfo::default();

    if data.len() < 20 {
        return info;
    }

    // Check chunk type at offset 12
    let chunk_type = &data[12..16];

    match chunk_type {
        b"VP8X" => {
            info.is_extended = true;
            if data.len() >= 30 {
                let flags = data[20];
                info.has_alpha = (flags & 0x10) != 0;
                info.is_animated = (flags & 0x02) != 0;
                info.has_exif = (flags & 0x08) != 0;
                info.has_xmp = (flags & 0x04) != 0;
                info.has_icc = (flags & 0x20) != 0;

                info.canvas_width = u32::from_le_bytes([data[24], data[25], data[26], 0]) + 1;
                info.canvas_height = u32::from_le_bytes([data[27], data[28], data[29], 0]) + 1;
            }
        }
        b"VP8 " => {
            info.is_lossy = true;
            // Parse VP8 bitstream header
            if data.len() >= 30 {
                let frame_start = 20;
                if data.len() >= frame_start + 10 {
                    // Check frame tag
                    let tag = u32::from_le_bytes([
                        data[frame_start],
                        data[frame_start + 1],
                        data[frame_start + 2],
                        0,
                    ]);
                    info.is_keyframe = (tag & 1) == 0;

                    if info.is_keyframe {
                        // Skip frame tag (3 bytes) and sync code (3 bytes)
                        let dim_offset = frame_start + 6;
                        if data.len() >= dim_offset + 4 {
                            info.canvas_width =
                                u16::from_le_bytes([data[dim_offset], data[dim_offset + 1]]) as u32
                                    & 0x3FFF;
                            info.canvas_height =
                                u16::from_le_bytes([data[dim_offset + 2], data[dim_offset + 3]])
                                    as u32
                                    & 0x3FFF;
                        }
                    }
                }
            }
        }
        b"VP8L" => {
            info.is_lossless = true;
            // Parse VP8L header
            if data.len() >= 25 {
                let sig = data[20];
                if sig == 0x2F {
                    let b = u32::from_le_bytes([data[21], data[22], data[23], data[24]]);
                    info.canvas_width = (b & 0x3FFF) + 1;
                    info.canvas_height = ((b >> 14) & 0x3FFF) + 1;
                    info.has_alpha = ((b >> 28) & 1) != 0;
                }
            }
        }
        _ => {}
    }

    info
}

#[derive(Debug, Default)]
pub struct WebpInfo {
    pub canvas_width: u32,
    pub canvas_height: u32,
    pub is_extended: bool,
    pub is_lossy: bool,
    pub is_lossless: bool,
    pub is_animated: bool,
    pub is_keyframe: bool,
    pub has_alpha: bool,
    pub has_exif: bool,
    pub has_xmp: bool,
    pub has_icc: bool,
}
