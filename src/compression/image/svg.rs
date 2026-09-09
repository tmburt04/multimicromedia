use crate::analysis::svg::iter_embedded_images;
use crate::config::{CompressionConfig, OutputFormat};
use crate::error::{CompressionError, Result};
use base64::Engine;

pub async fn compress_svg(data: &[u8], config: &CompressionConfig) -> Result<Vec<u8>> {
    let cfg = config.get_svg_config();

    let svg_text = std::str::from_utf8(data).map_err(|_| CompressionError::DecodeFailed {
        format: "SVG".to_string(),
        detail: "invalid UTF-8".to_string(),
    })?;

    // Release the XML tree before allocating decoded embedded image buffers.
    {
        let doc =
            roxmltree::Document::parse(svg_text).map_err(|e| CompressionError::DecodeFailed {
                format: "SVG".into(),
                detail: e.to_string(),
            })?;
        if doc.root_element().tag_name().name() != "svg" {
            return Err(CompressionError::DecodeFailed {
                format: "SVG".into(),
                detail: "root element must be svg".into(),
            });
        }
    }
    let mut output = if cfg.compress_embedded_images {
        compress_embedded_images(svg_text, config).await
    } else {
        svg_text.to_owned()
    };

    if cfg.minify && cfg.remove_comments {
        output = remove_comments(&output)?;
    }

    Ok(if output.len() <= data.len() {
        output.into_bytes()
    } else {
        data.to_vec()
    })
}

async fn compress_embedded_images(svg_text: &str, config: &CompressionConfig) -> String {
    let engine = base64::engine::general_purpose::STANDARD;
    let mut embedded_config = config.clone();
    embedded_config.crop = None;
    embedded_config.resize = None;
    embedded_config.output_format = None;
    let config = &embedded_config;
    let mut output = String::with_capacity(svg_text.len());
    let mut offset = 0;

    // Decode, recompress and replace one embedded image at a time. No collection
    // of decoded images or replacement strings survives across iterations.
    for img in iter_embedded_images(svg_text.as_bytes()) {
        let compressed = match img.format.as_str() {
            "png" => super::compress_png(&img.data, config).await,
            "jpeg" | "jpg" => {
                super::compress_jpeg(&img.data, config, Some(OutputFormat::Jpeg)).await
            }
            "webp" => super::compress_webp(&img.data, config, Some(OutputFormat::Webp)).await,
            "gif" => compress_embedded_gif(&img.data, config).await,
            _ => continue,
        };
        let Ok(compressed) = compressed else { continue };
        if compressed.len() >= img.data.len() {
            continue;
        }
        let mime = crate::detection::FileFormat::detect(&compressed).mime_type();
        let prefix = format!("data:{mime};base64,");
        let Some(encoded_len) = base64::encoded_len(compressed.len(), true) else {
            continue;
        };
        if encoded_len.saturating_add(prefix.len()) >= img.length {
            continue;
        }
        output.push_str(&svg_text[offset..img.offset]);
        output.push_str(&prefix);
        engine.encode_string(&compressed, &mut output);
        offset = img.offset + img.length;
    }
    output.push_str(&svg_text[offset..]);
    output
}

async fn compress_embedded_gif(data: &[u8], config: &CompressionConfig) -> Result<Vec<u8>> {
    let analysis = crate::analysis::analyze_file(data)?;
    super::compress_gif(data, &analysis, config).await
}

fn remove_comments(svg_text: &str) -> Result<String> {
    let doc = roxmltree::Document::parse(svg_text).map_err(|e| CompressionError::DecodeFailed {
        format: "SVG".into(),
        detail: e.to_string(),
    })?;
    // XML text whitespace, IDs, URLs, CSS and data URIs are semantically significant.
    // Remove only parsed comments; source ranges are UTF-8 byte offsets.
    let mut output = String::with_capacity(svg_text.len());
    let mut offset = 0;
    for node in doc.descendants().filter(|node| node.is_comment()) {
        let range = node.range();
        output.push_str(&svg_text[offset..range.start]);
        offset = range.end;
    }
    output.push_str(&svg_text[offset..]);
    Ok(output)
}

/// Heuristic only; does not run the encoder or account for actual comment lengths.
pub fn estimate_svg_compression(data: &[u8]) -> CompressionEstimate {
    let mut text_size = data.len();
    let mut embedded_image_count = 0;
    let mut embedded_image_bytes = 0usize;
    let mut embedded_savings = 0usize;
    for image in iter_embedded_images(data) {
        text_size = text_size.saturating_sub(image.length);
        embedded_image_count += 1;
        embedded_image_bytes = embedded_image_bytes.saturating_add(image.data.len());
        embedded_savings =
            embedded_savings.saturating_add((image.data.len() as f64 * 0.3) as usize);
    }
    let text_savings = (text_size as f64 * 0.2) as usize;
    CompressionEstimate {
        original_size: data.len(),
        estimated_size: data
            .len()
            .saturating_sub(embedded_savings.saturating_add(text_savings)),
        embedded_image_count,
        embedded_image_bytes,
    }
}

#[derive(Debug)]
pub struct CompressionEstimate {
    pub original_size: usize,
    pub estimated_size: usize,
    pub embedded_image_count: usize,
    pub embedded_image_bytes: usize,
}
