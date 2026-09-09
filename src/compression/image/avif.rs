use crate::compression::audio::{execute_ffmpeg, is_ffmpeg_available};
use crate::config::{CompressionConfig, OutputFormat};
use crate::error::{CompressionError, Result};

pub async fn compress_avif(
    data: &[u8],
    config: &CompressionConfig,
    output_format: Option<OutputFormat>,
) -> Result<Vec<u8>> {
    transcode_heif(data, config, output_format.unwrap_or(OutputFormat::Webp)).await
}

pub(super) async fn transcode_heif(
    data: &[u8],
    config: &CompressionConfig,
    output: OutputFormat,
) -> Result<Vec<u8>> {
    if output == OutputFormat::Avif {
        return Err(CompressionError::UnsupportedFormat {
            detected: "AVIF output".into(),
            fallback: Some("PNG, JPEG, or WebP".into()),
        });
    }
    if !is_ffmpeg_available() {
        return Err(CompressionError::FfmpegUnavailable);
    }
    let args = ["-frames:v", "1", "-c:v", "png", "-f", "image2"].map(String::from);
    let decoded = execute_ffmpeg(data, &args).await?;
    let decoded_image = super::decode_image(&decoded)?;
    drop(decoded);
    let image = super::apply_transforms(decoded_image, config)?;
    let encoded = super::encode_image(&image, output, config)?;
    Ok(if encoded.len() <= data.len() {
        encoded
    } else {
        data.to_vec()
    })
}

/// Analyze AVIF file structure (ISOBMFF container)
pub fn analyze_avif(data: &[u8]) -> IsobmffInfo {
    analyze_isobmff(data, &[b"avif", b"avis", b"mif1"])
}

/// Analyze HEIC file structure (ISOBMFF container)
pub fn analyze_heic(data: &[u8]) -> IsobmffInfo {
    analyze_isobmff(
        data,
        &[
            b"heic", b"heix", b"hevc", b"hevx", b"mif1", b"msf1", b"heif",
        ],
    )
}

/// Common ISOBMFF (ISO Base Media File Format) analysis for AVIF/HEIC
fn analyze_isobmff(data: &[u8], valid_brands: &[&[u8; 4]]) -> IsobmffInfo {
    let mut info = IsobmffInfo::default();
    for brand in crate::detection::isobmff_brands(data) {
        info.is_valid |= valid_brands.iter().any(|valid| brand == *valid);
        info.is_sequence |= matches!(brand, b"avis" | b"msf1" | b"hevc" | b"hevx");
    }
    if !info.is_valid {
        return info;
    }
    let mut properties = ImageProperties::default();
    let mut remaining_boxes = 16_384;
    if !read_property_boxes(data, 0, &mut remaining_boxes, &mut properties) {
        info.is_valid = false;
        return info;
    }
    // HEIF can contain tiles, thumbnails and auxiliary images. Without resolving
    // item-property associations, differing extents cannot identify the canvas.
    if !properties.ambiguous_dimensions {
        if let Some((width, height)) = properties.dimensions {
            info.width = width;
            info.height = height;
        }
    }
    info.bit_depth = properties.bit_depth;
    info.channels = properties.channels;
    info.has_alpha = properties.channels >= 4;
    info.is_hdr = properties.hdr == Some(true);
    info
}

#[derive(Default)]
struct ImageProperties {
    dimensions: Option<(u32, u32)>,
    ambiguous_dimensions: bool,
    bit_depth: u8,
    channels: u8,
    hdr: Option<bool>,
}

fn read_property_boxes(
    mut bytes: &[u8],
    depth: u8,
    remaining: &mut usize,
    properties: &mut ImageProperties,
) -> bool {
    if depth > 16 {
        return false;
    }
    while !bytes.is_empty() {
        if bytes.len() < 8 || *remaining == 0 {
            return false;
        }
        *remaining -= 1;
        let short_size = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        let (box_size, header_size) = match short_size {
            0 => (bytes.len(), 8),
            1 => {
                let Some(large_size) = bytes.get(8..16) else {
                    return false;
                };
                let size = u64::from_be_bytes(large_size.try_into().expect("eight-byte slice"));
                let Ok(size) = usize::try_from(size) else {
                    return false;
                };
                (size, 16)
            }
            size => (size as usize, 8),
        };
        if box_size < header_size || box_size > bytes.len() {
            return false;
        }
        let kind = &bytes[4..8];
        let payload = &bytes[header_size..box_size];
        match kind {
            b"meta" => {
                let Some(children) = payload.get(4..) else {
                    return false;
                };
                if !read_property_boxes(children, depth + 1, remaining, properties) {
                    return false;
                }
            }
            b"iprp" | b"ipco"
                if !read_property_boxes(payload, depth + 1, remaining, properties) =>
            {
                return false;
            }
            b"ispe" if payload.len() >= 12 && payload[0] == 0 => {
                let width = u32::from_be_bytes([payload[4], payload[5], payload[6], payload[7]]);
                let height = u32::from_be_bytes([payload[8], payload[9], payload[10], payload[11]]);
                if width != 0 && height != 0 {
                    match properties.dimensions {
                        Some(previous) if previous != (width, height) => {
                            properties.ambiguous_dimensions = true
                        }
                        None => properties.dimensions = Some((width, height)),
                        _ => {}
                    }
                }
            }
            b"pixi" if payload.len() >= 5 && payload[0] == 0 => {
                let channels = payload[4];
                let Some(depths) = payload.get(5..5 + channels as usize) else {
                    return false;
                };
                properties.channels = properties.channels.max(channels);
                properties.bit_depth = depths.iter().copied().fold(properties.bit_depth, u8::max);
            }
            b"colr" if payload.len() >= 11 && &payload[..4] == b"nclx" => {
                let primaries = u16::from_be_bytes([payload[4], payload[5]]);
                let transfer = u16::from_be_bytes([payload[6], payload[7]]);
                let hdr = primaries == 9 && matches!(transfer, 16 | 18);
                properties.hdr = Some(properties.hdr.unwrap_or(true) && hdr);
            }
            _ => {}
        }
        bytes = &bytes[box_size..];
    }
    true
}

#[derive(Debug, Default)]
pub struct IsobmffInfo {
    pub width: u32,
    pub height: u32,
    pub bit_depth: u8,
    pub channels: u8,
    pub is_valid: bool,
    pub is_sequence: bool,
    pub has_alpha: bool,
    pub is_hdr: bool,
}

/// Legacy alias for compatibility
pub type AvifInfo = IsobmffInfo;
