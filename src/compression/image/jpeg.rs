use crate::config::{CompressionConfig, JpegConfig, OutputFormat};
use crate::error::{CompressionError, Result};
use image::{DynamicImage, ImageReader};
use std::io::Cursor;

pub async fn compress_jpeg(
    data: &[u8],
    config: &CompressionConfig,
    output_format: Option<OutputFormat>,
) -> Result<Vec<u8>> {
    let cfg = config.get_jpeg_config();
    let output_format = output_format.unwrap_or(OutputFormat::Jpeg);
    let original_size = data.len();

    // If output is still JPEG and no transforms, try lossless optimization first
    if output_format == OutputFormat::Jpeg && config.crop.is_none() && config.resize.is_none() {
        if let Ok(optimized) = optimize_jpeg_lossless(data, &cfg) {
            if optimized.len() < original_size {
                return Ok(optimized);
            }
        }
    }

    // Decode and re-encode
    let img = decode_jpeg(data)?;
    let img = super::apply_transforms(img, config)?;

    let output = match output_format {
        OutputFormat::Jpeg => encode_jpeg(&img, &cfg)?,
        OutputFormat::Png => {
            let png_cfg = config.get_png_config();
            let mut out = Vec::new();
            super::encode_png_image(&img, &png_cfg, &mut out)?;
            out
        }
        OutputFormat::Webp => {
            let webp_cfg = config.get_webp_config();
            let mut out = Vec::new();
            super::encode_webp_image(&img, &webp_cfg, &mut out)?;
            out
        }
        _ => {
            return Err(CompressionError::UnsupportedFormat {
                detected: format!("JPEG to {:?}", output_format),
                fallback: Some("Use JPEG, PNG, or WebP output".to_string()),
            });
        }
    };

    // Only return compressed output if it's actually smaller
    if output.len() < original_size {
        return Ok(output);
    }

    // Try progressively lower quality until we achieve compression
    let img = decode_jpeg(data)?;
    for quality in [70u8, 60, 50, 40] {
        let lower_cfg = JpegConfig {
            quality,
            progressive: true,
            optimize_coding: true,
            chroma_subsampling: cfg.chroma_subsampling,
        };
        if let Ok(smaller) = encode_jpeg(&img, &lower_cfg) {
            if smaller.len() < original_size {
                return Ok(smaller);
            }
        }
    }

    // Last resort: return original
    Ok(data.to_vec())
}

fn decode_jpeg(data: &[u8]) -> Result<DynamicImage> {
    // First try zune-jpeg for speed
    let _decoder = zune_jpeg::JpegDecoder::new(data);
    
    // Fall back to image crate for compatibility
    ImageReader::new(Cursor::new(data))
        .with_guessed_format()
        .map_err(|e| CompressionError::DecodeFailed {
            format: "JPEG".to_string(),
            detail: e.to_string(),
        })?
        .decode()
        .map_err(|e| CompressionError::DecodeFailed {
            format: "JPEG".to_string(),
            detail: e.to_string(),
        })
}

fn encode_jpeg(img: &DynamicImage, cfg: &JpegConfig) -> Result<Vec<u8>> {
    let rgb = img.to_rgb8();
    let (width, height) = (rgb.width(), rgb.height());

    let mut output = Vec::new();
    let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(
        Cursor::new(&mut output),
        cfg.quality,
    );

    encoder
        .encode(rgb.as_raw(), width, height, image::ExtendedColorType::Rgb8)
        .map_err(|e| CompressionError::EncodeFailed {
            format: "JPEG".to_string(),
            detail: e.to_string(),
        })?;

    Ok(output)
}

fn optimize_jpeg_lossless(data: &[u8], _cfg: &JpegConfig) -> Result<Vec<u8>> {
    // Lossless JPEG optimization would use jpegtran-style techniques:
    // 1. Optimize Huffman tables
    // 2. Remove unnecessary metadata (if not preserving)
    // 3. Convert to progressive (if requested)
    //
    // For now, we just validate and return as-is since full jpegtran
    // requires native C bindings which are complex in WASM
    
    // Validate JPEG structure
    if !data.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return Err(CompressionError::DecodeFailed {
            format: "JPEG".to_string(),
            detail: "invalid JPEG header".to_string(),
        });
    }

    // Find and validate end marker
    let mut found_eoi = false;
    for i in (0..data.len().saturating_sub(1)).rev() {
        if data[i] == 0xFF && data[i + 1] == 0xD9 {
            found_eoi = true;
            break;
        }
    }

    if !found_eoi {
        return Err(CompressionError::DecodeFailed {
            format: "JPEG".to_string(),
            detail: "missing EOI marker".to_string(),
        });
    }

    // Return original data - actual lossless optimization would go here
    Ok(data.to_vec())
}

pub fn extract_jpeg_metadata(data: &[u8]) -> JpegMetadata {
    let mut metadata = JpegMetadata::default();

    let mut i = 2; // Skip SOI
    while i < data.len().saturating_sub(4) {
        if data[i] != 0xFF {
            break;
        }

        let marker = data[i + 1];
        
        // Skip markers without length
        if marker == 0x00 || marker == 0x01 || (0xD0..=0xD9).contains(&marker) {
            i += 2;
            continue;
        }

        if i + 4 > data.len() {
            break;
        }

        let length = u16::from_be_bytes([data[i + 2], data[i + 3]]) as usize;

        match marker {
            0xC0 => {
                // SOF0 - Baseline DCT
                metadata.is_progressive = false;
                if i + 9 < data.len() {
                    metadata.bit_depth = data[i + 4];
                    metadata.height = u16::from_be_bytes([data[i + 5], data[i + 6]]);
                    metadata.width = u16::from_be_bytes([data[i + 7], data[i + 8]]);
                    metadata.components = data[i + 9];
                }
            }
            0xC2 => {
                // SOF2 - Progressive DCT
                metadata.is_progressive = true;
                if i + 9 < data.len() {
                    metadata.bit_depth = data[i + 4];
                    metadata.height = u16::from_be_bytes([data[i + 5], data[i + 6]]);
                    metadata.width = u16::from_be_bytes([data[i + 7], data[i + 8]]);
                    metadata.components = data[i + 9];
                }
            }
            0xE1 => {
                // APP1 - EXIF
                if i + 6 + 4 < data.len() && &data[i + 4..i + 8] == b"Exif" {
                    metadata.has_exif = true;
                }
            }
            0xE0 => {
                // APP0 - JFIF
                if i + 6 + 4 < data.len() && &data[i + 4..i + 8] == b"JFIF" {
                    metadata.has_jfif = true;
                }
            }
            0xED => {
                // APP13 - IPTC
                metadata.has_iptc = true;
            }
            0xEE => {
                // APP14 - Adobe
                metadata.has_adobe = true;
            }
            _ => {}
        }

        i += 2 + length;
    }

    metadata
}

#[derive(Debug, Default)]
pub struct JpegMetadata {
    pub width: u16,
    pub height: u16,
    pub bit_depth: u8,
    pub components: u8,
    pub is_progressive: bool,
    pub has_exif: bool,
    pub has_jfif: bool,
    pub has_iptc: bool,
    pub has_adobe: bool,
}

impl JpegMetadata {
    pub fn is_cmyk(&self) -> bool {
        self.components == 4
    }

    pub fn chroma_subsampling(&self) -> &'static str {
        // Would need to parse SOF components to determine this
        "unknown"
    }
}
