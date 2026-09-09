use crate::detection::FileAnalysis;
use crate::error::{CompressionError, Result};
use image::{ImageDecoder, ImageFormat, ImageReader};
use std::io::Cursor;

pub fn analyze_png(data: &[u8], analysis: &mut FileAnalysis) -> Result<()> {
    let decoder = png::Decoder::new(Cursor::new(data));
    let reader = decoder
        .read_info()
        .map_err(|e| CompressionError::DecodeFailed {
            format: "PNG".to_string(),
            detail: e.to_string(),
        })?;

    let info = reader.info();
    analysis.width = Some(info.width);
    analysis.height = Some(info.height);
    analysis.bit_depth = Some(info.bit_depth as u8);
    analysis.has_alpha = Some(
        matches!(
            info.color_type,
            png::ColorType::Rgba | png::ColorType::GrayscaleAlpha
        ) || info.trns.is_some(),
    );
    analysis.color_type = Some(format!("{:?}", info.color_type));
    analysis.is_animated = Some(info.animation_control.is_some());

    if let Some(anim) = info.animation_control {
        analysis.frame_count = Some(anim.num_frames);
    }

    Ok(())
}

/// Inspect decoder headers without allocating the decoded pixel buffer. Using the
/// same decoder as compression keeps crop validation consistent with its output.
fn analyze_raster(data: &[u8], analysis: &mut FileAnalysis, format: ImageFormat) -> Result<()> {
    let decoder = ImageReader::with_format(Cursor::new(data), format)
        .into_decoder()
        .map_err(|error| CompressionError::DecodeFailed {
            format: analysis.format.extension().into(),
            detail: error.to_string(),
        })?;
    record_decoder_info(&decoder, analysis);
    analysis.is_animated = Some(false);
    Ok(())
}

fn record_decoder_info(decoder: &dyn ImageDecoder, analysis: &mut FileAnalysis) {
    let (width, height) = decoder.dimensions();
    let color = decoder.original_color_type();
    analysis.width = Some(width);
    analysis.height = Some(height);
    analysis.has_alpha = Some(decoder.color_type().has_alpha());
    analysis.bit_depth = Some((color.bits_per_pixel() / u16::from(color.channel_count())) as u8);
    analysis.color_type = Some(format!("{color:?}"));
}

pub fn analyze_jpeg(data: &[u8], analysis: &mut FileAnalysis) -> Result<()> {
    analyze_raster(data, analysis, ImageFormat::Jpeg)
}

pub fn analyze_webp(data: &[u8], analysis: &mut FileAnalysis) -> Result<()> {
    let decoder = image::codecs::webp::WebPDecoder::new(Cursor::new(data)).map_err(|error| {
        CompressionError::DecodeFailed {
            format: "WebP".into(),
            detail: error.to_string(),
        }
    })?;
    record_decoder_info(&decoder, analysis);
    analysis.is_animated = Some(decoder.has_animation());
    Ok(())
}

pub fn analyze_gif(data: &[u8], analysis: &mut FileAnalysis) -> Result<()> {
    let mut options = gif::DecodeOptions::new();
    options.skip_frame_decoding(true);
    let mut decoder =
        options
            .read_info(Cursor::new(data))
            .map_err(|e| CompressionError::DecodeFailed {
                format: "GIF".into(),
                detail: e.to_string(),
            })?;
    analysis.width = Some(decoder.width() as u32);
    analysis.height = Some(decoder.height() as u32);
    analysis.bit_depth = Some(8);
    let mut frames = 0u32;
    let mut duration = 0u64;
    let mut transparent = false;
    while let Some(frame) =
        decoder
            .read_next_frame()
            .map_err(|e| CompressionError::DecodeFailed {
                format: "GIF".into(),
                detail: e.to_string(),
            })?
    {
        frames = frames.saturating_add(1);
        duration += u64::from(frame.delay) * 10;
        transparent |= frame.transparent.is_some();
    }
    if frames == 0 {
        return Err(CompressionError::DecodeFailed {
            format: "GIF".into(),
            detail: "no image frames".into(),
        });
    }
    analysis.frame_count = Some(frames);
    analysis.is_animated = Some(frames > 1);
    analysis.has_alpha = Some(transparent);
    analysis.duration_ms = Some(duration);
    Ok(())
}

pub fn analyze_bmp(data: &[u8], analysis: &mut FileAnalysis) -> Result<()> {
    analyze_raster(data, analysis, ImageFormat::Bmp)
}

pub fn analyze_heif_family(data: &[u8], analysis: &mut FileAnalysis) -> Result<()> {
    // Box properties are inspected without resolving primary-item associations.

    if data.len() < 12 {
        return Err(CompressionError::DecodeFailed {
            format: "HEIF".to_string(),
            detail: "file too small".to_string(),
        });
    }

    let info = if analysis.format == crate::detection::FileFormat::Heic {
        crate::compression::image::analyze_heic(data)
    } else {
        crate::compression::image::analyze_avif(data)
    };
    if !info.is_valid {
        return Err(CompressionError::DecodeFailed {
            format: analysis.format.extension().into(),
            detail: "invalid HEIF container structure".into(),
        });
    }
    if info.width > 0 && info.height > 0 {
        analysis.width = Some(info.width);
        analysis.height = Some(info.height);
    }

    analysis.bit_depth = (info.bit_depth > 0).then_some(info.bit_depth);
    analysis.is_animated = Some(info.is_sequence);

    Ok(())
}

pub fn analyze_tiff(data: &[u8], analysis: &mut FileAnalysis) -> Result<()> {
    analyze_raster(data, analysis, ImageFormat::Tiff)
}

pub fn analyze_ico(data: &[u8], analysis: &mut FileAnalysis) -> Result<()> {
    analyze_raster(data, analysis, ImageFormat::Ico)?;
    // The decoder validates the directory and selects the same entry compression uses.
    analysis.frame_count = Some(u32::from(u16::from_le_bytes([data[4], data[5]])));
    Ok(())
}
