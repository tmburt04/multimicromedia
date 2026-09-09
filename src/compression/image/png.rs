use crate::config::{CompressionConfig, PngConfig};
use crate::error::{CompressionError, Result};
use png::{AdaptiveFilterType, BitDepth, ColorType, Compression, Encoder, FilterType};
use std::io::Cursor;

pub async fn compress_png(data: &[u8], config: &CompressionConfig) -> Result<Vec<u8>> {
    let mut analysis =
        crate::detection::FileAnalysis::new(crate::detection::FileFormat::Png, data.len() as u64);
    crate::analysis::analyze_png(data, &mut analysis)?;
    if analysis.is_animated == Some(true) {
        return Ok(data.to_vec());
    }
    if config.crop.is_some()
        || config.resize.is_some()
        || config
            .output_format
            .is_some_and(|format| format != crate::config::OutputFormat::Png)
    {
        return super::compress_generic(data, config, config.output_format).await;
    }
    let cfg = config.get_png_config();
    let (pixels, info) = decode_png(data)?;
    let color_chunks = extra_color_chunks(data);
    let mut best = None;
    let mut consider = |mut candidate: Vec<u8>| {
        // png 0.17 decodes HDR color information but does not write it back.
        // Preserve validated original chunks before PLTE, including their CRCs.
        if !color_chunks.is_empty() {
            candidate.splice(33..33, color_chunks.iter().copied());
        }
        if !candidate.is_empty()
            && candidate.len() < data.len()
            && best
                .as_ref()
                .is_none_or(|current: &Vec<u8>| candidate.len() < current.len())
        {
            best = Some(candidate);
        }
    };

    // Decode once for both lossless strategies. Keep only the smallest encoded
    // candidate, then release all decoded pixels before optional quantization.
    {
        consider(encode_png_pixels(
            &pixels,
            &info,
            &cfg,
            config.preserve_metadata,
        )?);
        if let Some((color, packed)) = reduce_channels(&pixels, &info) {
            let mut reduced_info = info.clone();
            reduced_info.color_type = color;
            reduced_info.palette = None;
            if let Ok(encoded) =
                encode_png_pixels(&packed, &reduced_info, &cfg, config.preserve_metadata)
            {
                consider(encoded);
            }
        }
    }
    drop(pixels);
    if cfg.quantize {
        if let Ok(encoded) = compress_png_quantized(data, &cfg, &info, config.preserve_metadata) {
            consider(encoded);
        }
    }
    Ok(best.unwrap_or_else(|| data.to_vec()))
}

fn extra_color_chunks(data: &[u8]) -> Vec<u8> {
    let mut chunks = [None; 3];
    let mut offset = 8usize;
    while let Some(header) = data.get(offset..offset.saturating_add(8)) {
        let length = u32::from_be_bytes([header[0], header[1], header[2], header[3]]) as usize;
        let Some(end) = offset
            .checked_add(12)
            .and_then(|start| start.checked_add(length))
        else {
            break;
        };
        let Some(chunk) = data.get(offset..end) else {
            break;
        };
        match &header[4..8] {
            b"cICP" if length == 4 => chunks[0] = Some(chunk),
            b"mDCV" if length == 24 => chunks[1] = Some(chunk),
            b"cLLI" if length == 8 => chunks[2] = Some(chunk),
            _ => {}
        }
        offset = end;
    }
    chunks.into_iter().flatten().flatten().copied().collect()
}

fn decode_png(data: &[u8]) -> Result<(Vec<u8>, png::Info<'static>)> {
    let limit = (super::memory_budget() / 4) as usize;
    let decoder = png::Decoder::new_with_limits(Cursor::new(data), png::Limits { bytes: limit });
    let mut reader = decoder.read_info().map_err(png_decode_error)?;
    let size = reader.output_buffer_size();
    if size > limit {
        return Err(CompressionError::MemoryLimitExceeded);
    }
    let mut pixels = Vec::new();
    pixels
        .try_reserve_exact(size)
        .map_err(|_| CompressionError::MemoryLimitExceeded)?;
    pixels.resize(size, 0);
    let frame = reader.next_frame(&mut pixels).map_err(png_decode_error)?;
    pixels.truncate(frame.buffer_size());
    // Read trailing metadata and validate the remainder of the PNG stream.
    reader.finish().map_err(png_decode_error)?;
    let mut info = reader.info().clone();
    info.interlaced = false;
    info.animation_control = None;
    info.frame_control = None;
    Ok((pixels, info))
}

fn png_decode_error(error: png::DecodingError) -> CompressionError {
    if matches!(error, png::DecodingError::LimitsExceeded) {
        CompressionError::MemoryLimitExceeded
    } else {
        CompressionError::DecodeFailed {
            format: "PNG".into(),
            detail: error.to_string(),
        }
    }
}

fn encode_png_pixels(
    pixels: &[u8],
    source_info: &png::Info<'_>,
    cfg: &PngConfig,
    preserve_metadata: bool,
) -> Result<Vec<u8>> {
    let mut info = source_info.clone();
    if !preserve_metadata {
        info.exif_metadata = None;
        info.uncompressed_latin1_text.clear();
        info.compressed_latin1_text.clear();
        info.utf8_text.clear();
        info.pixel_dims = None;
    }
    // Color profiles, gamma and transparency affect displayed pixels and remain
    // necessary even when descriptive metadata is removed.
    let mut output = Vec::new();
    {
        let mut encoder = Encoder::with_info(&mut output, info).map_err(png_encode_error)?;
        configure_encoder(&mut encoder, cfg);
        let mut writer = encoder.write_header().map_err(png_encode_error)?;
        // png 0.17 exposes these fields while omitting them from Info::encode.
        // bKGD belongs after the palette and before the image data.
        if let Some(background) = &source_info.bkgd {
            writer
                .write_chunk(png::chunk::bKGD, background)
                .map_err(png_encode_error)?;
        }
        writer.write_image_data(pixels).map_err(png_encode_error)?;
        writer.finish().map_err(png_encode_error)?;
    }
    if let Some(significant_bits) = &source_info.sbit {
        // sBIT must precede PLTE, so insert it immediately after the fixed IHDR.
        let mut chunk = Vec::with_capacity(significant_bits.len() + 12);
        chunk.extend_from_slice(&(significant_bits.len() as u32).to_be_bytes());
        chunk.extend_from_slice(b"sBIT");
        chunk.extend_from_slice(significant_bits);
        chunk.extend_from_slice(&crc32fast::hash(&chunk[4..]).to_be_bytes());
        output.splice(33..33, chunk);
    }
    Ok(output)
}

fn png_encode_error(error: png::EncodingError) -> CompressionError {
    CompressionError::EncodeFailed {
        format: "PNG".into(),
        detail: error.to_string(),
    }
}

fn configure_encoder<W: std::io::Write>(encoder: &mut Encoder<'_, W>, cfg: &PngConfig) {
    encoder.set_compression(match cfg.compression_level {
        0..=2 => Compression::Fast,
        3..=6 => Compression::Default,
        _ => Compression::Best,
    });
    if cfg.compression_level >= 4 {
        encoder.set_adaptive_filter(AdaptiveFilterType::Adaptive);
    } else if cfg.compression_level >= 2 {
        encoder.set_filter(FilterType::Sub);
    }
}

fn compress_png_quantized(
    data: &[u8],
    cfg: &PngConfig,
    source_info: &png::Info<'_>,
    preserve_metadata: bool,
) -> Result<Vec<u8>> {
    if source_info.icc_profile.is_some()
        && matches!(
            source_info.color_type,
            ColorType::Grayscale | ColorType::GrayscaleAlpha
        )
    {
        return Err(CompressionError::InvalidInput {
            reason: "a grayscale color profile cannot describe an indexed RGB palette".into(),
        });
    }
    let rgba_img = super::decode_image(data)?.into_rgba8();
    let indexed = super::quantize::quantize_rgba(
        rgba_img.as_raw(),
        cfg.max_colors,
        match cfg.compression_level {
            0..=2 => 10,
            3..=5 => 5,
            _ => 1,
        },
    )?;
    let palette = indexed.palette;
    let pixels = indexed.indices;
    drop(rgba_img);

    let mut info = source_info.clone();
    info.color_type = ColorType::Indexed;
    info.bit_depth = BitDepth::Eight;
    info.sbit = None;
    info.bkgd = None;
    info.palette = Some(
        palette
            .iter()
            .flat_map(|c| [c[0], c[1], c[2]])
            .collect::<Vec<_>>()
            .into(),
    );
    let mut transparency: Vec<_> = palette.iter().map(|c| c[3]).collect();
    while transparency.last() == Some(&255) {
        transparency.pop();
    }
    info.trns = (!transparency.is_empty()).then(|| transparency.into());
    encode_png_pixels(&pixels, &info, cfg, preserve_metadata)
}

fn reduce_channels(src: &[u8], info: &png::Info<'_>) -> Option<(ColorType, Vec<u8>)> {
    // These chunks depend on channel count or individual sample values. Keep
    // their original representation rather than corrupting color semantics.
    if info.bit_depth != BitDepth::Eight
        || info.trns.is_some()
        || info.icc_profile.is_some()
        || info.sbit.is_some()
        || info.bkgd.is_some()
    {
        return None;
    }
    let (color, _, packed) = match info.color_type {
        ColorType::Rgba => analyze_and_reduce_rgba(src, info.width, info.height),
        ColorType::Rgb => analyze_and_reduce_rgb(src, info.width, info.height),
        ColorType::GrayscaleAlpha => {
            if !src.chunks_exact(2).all(|pixel| pixel[1] == 255) {
                return None;
            }
            (
                ColorType::Grayscale,
                BitDepth::Eight,
                src.chunks_exact(2).map(|pixel| pixel[0]).collect(),
            )
        }
        _ => return None,
    };
    (!packed.is_empty()).then_some((color, packed))
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
