use crate::compression::audio::{execute_ffmpeg, is_ffmpeg_available};
use crate::config::{CompressionConfig, OutputFormat};
use crate::error::{CompressionError, Result};
use image::{DynamicImage, ImageReader};
use std::io::Cursor;

pub async fn compress_avif(
    data: &[u8],
    config: &CompressionConfig,
    output_format: Option<OutputFormat>,
) -> Result<Vec<u8>> {
    let output_format = output_format.unwrap_or(OutputFormat::Webp);

    // AVIF encoding is not supported - only decoding/transcoding
    if output_format == OutputFormat::Avif {
        return Err(CompressionError::UnsupportedFormat {
            detected: "AVIF output".to_string(),
            fallback: Some("Use WebP for similar compression with better WASM support".to_string()),
        });
    }

    // Try FFmpeg first (more reliable for AVIF decoding in WASM)
    if is_ffmpeg_available() {
        if let Ok(result) = compress_avif_ffmpeg(data, config, output_format).await {
            if result.len() < data.len() {
                return Ok(result);
            }
        }
    }

    // Fallback to image crate (may not work in all WASM environments)
    let img = decode_avif(data)?;
    let img = super::apply_transforms(img, config)?;

    match output_format {
        OutputFormat::Png => {
            let cfg = config.get_png_config();
            let mut output = Vec::new();
            super::encode_png_image(&img, &cfg, &mut output)?;
            Ok(output)
        }
        OutputFormat::Jpeg => {
            let cfg = config.get_jpeg_config();
            let mut output = Vec::new();
            super::encode_jpeg_image(&img, &cfg, &mut output)?;
            Ok(output)
        }
        OutputFormat::Webp => {
            let cfg = config.get_webp_config();
            let mut output = Vec::new();
            super::encode_webp_image(&img, &cfg, &mut output)?;
            Ok(output)
        }
        _ => Err(CompressionError::UnsupportedFormat {
            detected: format!("AVIF to {:?}", output_format),
            fallback: Some("Use PNG, JPEG, or WebP".to_string()),
        }),
    }
}

async fn compress_avif_ffmpeg(
    data: &[u8],
    config: &CompressionConfig,
    output_format: OutputFormat,
) -> Result<Vec<u8>> {
    let quality = config.quality;

    let args: Vec<String> = match output_format {
        OutputFormat::Jpeg => {
            let q = quality_to_ffmpeg_jpeg(quality);
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
        OutputFormat::Webp | _ => {
            vec![
                "-c:v".into(), "libwebp".into(),
                "-lossless".into(), "0".into(),
                "-quality".into(), quality.to_string(),
                "-f".into(), "webp".into(),
            ]
        }
    };

    execute_ffmpeg(data, &args).await
}

/// Convert quality (1-100) to FFmpeg JPEG quality (1-31, lower = better)
fn quality_to_ffmpeg_jpeg(quality: u8) -> u8 {
    // quality 100 -> q 1 (best)
    // quality 1 -> q 31 (worst)
    let q = 32 - ((quality as u16 * 31) / 100) as u8;
    q.clamp(1, 31)
}

fn decode_avif(data: &[u8]) -> Result<DynamicImage> {
    ImageReader::new(Cursor::new(data))
        .with_guessed_format()
        .map_err(|e| CompressionError::DecodeFailed {
            format: "AVIF".to_string(),
            detail: e.to_string(),
        })?
        .decode()
        .map_err(|e| CompressionError::DecodeFailed {
            format: "AVIF".to_string(),
            detail: format!("AVIF decoding failed (try enabling FFmpeg): {}", e),
        })
}

/// Analyze AVIF file structure (ISOBMFF container)
pub fn analyze_avif(data: &[u8]) -> IsobmffInfo {
    analyze_isobmff(data, &[b"avif", b"avis", b"mif1"])
}

/// Analyze HEIC file structure (ISOBMFF container)
pub fn analyze_heic(data: &[u8]) -> IsobmffInfo {
    analyze_isobmff(data, &[b"heic", b"heix", b"mif1", b"heif"])
}

/// Common ISOBMFF (ISO Base Media File Format) analysis for AVIF/HEIC
fn analyze_isobmff(data: &[u8], valid_brands: &[&[u8; 4]]) -> IsobmffInfo {
    let mut info = IsobmffInfo::default();

    if data.len() < 12 {
        return info;
    }

    // Verify ftyp box
    if &data[4..8] != b"ftyp" {
        return info;
    }

    let brand = &data[8..12];
    info.is_valid = valid_brands.iter().any(|b| brand == *b);
    info.is_sequence = brand == b"avis" || brand == b"hevc";

    // Look for ispe box (image spatial extent)
    for i in 0..data.len().saturating_sub(20) {
        if &data[i..i + 4] == b"ispe" {
            if i + 12 <= data.len() {
                info.width = u32::from_be_bytes([data[i + 4], data[i + 5], data[i + 6], data[i + 7]]);
                info.height = u32::from_be_bytes([data[i + 8], data[i + 9], data[i + 10], data[i + 11]]);
            }
            break;
        }
    }

    // Look for pixi box (pixel information)
    for i in 0..data.len().saturating_sub(12) {
        if &data[i..i + 4] == b"pixi" {
            if i + 8 < data.len() {
                info.bit_depth = data[i + 7];
                info.channels = data[i + 6];
                info.has_alpha = info.channels >= 4;
            }
            break;
        }
    }

    // Look for colr box (color information) for HDR detection
    for i in 0..data.len().saturating_sub(16) {
        if &data[i..i + 4] == b"colr" {
            // Check color primaries for HDR (BT.2020 = 9, Display P3 = 12)
            if i + 12 < data.len() && &data[i + 4..i + 8] == b"nclx" {
                let primaries = u16::from_be_bytes([data[i + 8], data[i + 9]]);
                info.is_hdr = primaries == 9 || primaries == 12;
            }
            break;
        }
    }

    info
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
