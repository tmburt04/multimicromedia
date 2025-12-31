use crate::analysis::svg::{detect_embedded_images, replace_embedded_images, EmbeddedImage};
use crate::config::{CompressionConfig, SvgConfig};
use crate::error::{CompressionError, Result};
use base64::Engine;

pub async fn compress_svg(data: &[u8], config: &CompressionConfig) -> Result<Vec<u8>> {
    let cfg = config.get_svg_config();

    // Convert to string
    let svg_text = std::str::from_utf8(data).map_err(|_| CompressionError::DecodeFailed {
        format: "SVG".to_string(),
        detail: "invalid UTF-8".to_string(),
    })?;

    // Parse and validate
    let _doc = roxmltree::Document::parse(svg_text).map_err(|e| CompressionError::DecodeFailed {
        format: "SVG".to_string(),
        detail: e.to_string(),
    })?;

    let mut output = svg_text.to_string();

    // Compress embedded images if enabled
    if cfg.compress_embedded_images {
        let embedded = detect_embedded_images(data)?;
        if !embedded.is_empty() {
            output = compress_embedded_images(&output, data, &embedded, config).await?;
        }
    }

    // Minify if enabled
    if cfg.minify {
        output = minify_svg(&output, &cfg)?;
    }

    Ok(output.into_bytes())
}

async fn compress_embedded_images(
    svg_text: &str,
    svg_bytes: &[u8],
    embedded: &[EmbeddedImage],
    config: &CompressionConfig,
) -> Result<String> {
    let mut replacements: Vec<(usize, usize, String)> = Vec::new();
    let engine = base64::engine::general_purpose::STANDARD;

    for img in embedded {
        // Compress the embedded image based on its format
        let compressed = match img.format.as_str() {
            "png" => compress_embedded_png(&img.data, config).await?,
            "jpeg" | "jpg" => compress_embedded_jpeg(&img.data, config).await?,
            "webp" => compress_embedded_webp(&img.data, config).await?,
            "gif" => compress_embedded_gif(&img.data, config).await?,
            _ => img.data.clone(),
        };

        // Only use compressed version if it's smaller
        if compressed.len() < img.data.len() {
            // Detect output format (might have changed during compression)
            let output_format = detect_image_format(&compressed);
            let mime = match output_format {
                "png" => "image/png",
                "jpeg" | "jpg" => "image/jpeg",
                "webp" => "image/webp",
                "gif" => "image/gif",
                _ => "image/png",
            };

            let new_data_uri = format!("data:{};base64,{}", mime, engine.encode(&compressed));
            replacements.push((img.offset, img.length, new_data_uri));
        }
    }

    if replacements.is_empty() {
        return Ok(svg_text.to_string());
    }

    // Apply replacements
    let result = replace_embedded_images(svg_bytes, &replacements);
    String::from_utf8(result).map_err(|_| CompressionError::EncodeFailed {
        format: "SVG".to_string(),
        detail: "invalid UTF-8 after replacement".to_string(),
    })
}

async fn compress_embedded_png(data: &[u8], config: &CompressionConfig) -> Result<Vec<u8>> {
    super::compress_png(data, config).await
}

async fn compress_embedded_jpeg(data: &[u8], config: &CompressionConfig) -> Result<Vec<u8>> {
    super::compress_jpeg(data, config, Some(crate::config::OutputFormat::Jpeg)).await
}

async fn compress_embedded_webp(data: &[u8], config: &CompressionConfig) -> Result<Vec<u8>> {
    super::compress_webp(data, config, Some(crate::config::OutputFormat::Webp)).await
}

async fn compress_embedded_gif(data: &[u8], config: &CompressionConfig) -> Result<Vec<u8>> {
    let analysis = crate::analysis::analyze_file(data)?;
    super::compress_gif(data, &analysis, config).await
}

fn detect_image_format(data: &[u8]) -> &'static str {
    if data.starts_with(&[0x89, 0x50, 0x4E, 0x47]) {
        "png"
    } else if data.starts_with(&[0xFF, 0xD8, 0xFF]) {
        "jpeg"
    } else if data.starts_with(b"RIFF") && data.len() >= 12 && &data[8..12] == b"WEBP" {
        "webp"
    } else if data.starts_with(b"GIF8") {
        "gif"
    } else {
        "unknown"
    }
}

fn minify_svg(svg_text: &str, cfg: &SvgConfig) -> Result<String> {
    let mut output = String::with_capacity(svg_text.len());
    let mut in_tag = false;
    let mut in_string = false;
    let mut string_char = ' ';
    let mut last_char = ' ';
    let mut in_comment = false;

    let chars: Vec<char> = svg_text.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        let c = chars[i];

        // Handle comments
        if cfg.remove_comments {
            if !in_string && !in_comment {
                if c == '<' && i + 3 < chars.len() && &svg_text[i..i + 4] == "<!--" {
                    in_comment = true;
                    i += 4;
                    continue;
                }
            }
            if in_comment {
                if c == '-' && i + 2 < chars.len() && &svg_text[i..i + 3] == "-->" {
                    in_comment = false;
                    i += 3;
                    continue;
                }
                i += 1;
                continue;
            }
        }

        // Track string state
        if in_tag && !in_string && (c == '"' || c == '\'') {
            in_string = true;
            string_char = c;
            output.push(c);
            i += 1;
            continue;
        }
        if in_string && c == string_char && last_char != '\\' {
            in_string = false;
            output.push(c);
            last_char = c;
            i += 1;
            continue;
        }

        // Track tag state
        if !in_string {
            if c == '<' {
                in_tag = true;
            } else if c == '>' {
                in_tag = false;
            }
        }

        // Minify whitespace
        if !in_string {
            if c.is_whitespace() {
                // Collapse multiple whitespace to single space
                if !last_char.is_whitespace() && last_char != '>' && last_char != '\0' {
                    // Check if next non-whitespace is '<'
                    let mut next_non_ws = i + 1;
                    while next_non_ws < chars.len() && chars[next_non_ws].is_whitespace() {
                        next_non_ws += 1;
                    }
                    if next_non_ws < chars.len() && chars[next_non_ws] == '<' {
                        // Skip whitespace before tag
                        i = next_non_ws;
                        continue;
                    }
                    output.push(' ');
                    last_char = ' ';
                }
                i += 1;
                continue;
            }
        }

        output.push(c);
        last_char = c;
        i += 1;
    }

    // Round numeric precision if configured
    if cfg.precision < 10 {
        output = reduce_precision(&output, cfg.precision);
    }

    Ok(output)
}

fn reduce_precision(svg: &str, precision: u8) -> String {
    
    let mut result = String::with_capacity(svg.len());
    let mut chars = svg.chars().peekable();
    let precision = precision as usize;

    while let Some(c) = chars.next() {
        if c.is_ascii_digit() || c == '-' || c == '.' {
            // Collect the number
            let mut num_str = String::new();
            num_str.push(c);
            
            while let Some(&next) = chars.peek() {
                if next.is_ascii_digit() || next == '.' || next == 'e' || next == 'E' || next == '-' || next == '+' {
                    num_str.push(chars.next().unwrap());
                } else {
                    break;
                }
            }

            // Try to parse and round
            if let Ok(num) = num_str.parse::<f64>() {
                let factor = 10f64.powi(precision as i32);
                let rounded = (num * factor).round() / factor;
                
                // Format without trailing zeros
                let formatted = if rounded.fract() == 0.0 {
                    format!("{}", rounded as i64)
                } else {
                    let s = format!("{:.prec$}", rounded, prec = precision);
                    s.trim_end_matches('0').trim_end_matches('.').to_string()
                };
                
                result.push_str(&formatted);
            } else {
                result.push_str(&num_str);
            }
        } else {
            result.push(c);
        }
    }

    result
}

pub fn estimate_svg_compression(data: &[u8]) -> CompressionEstimate {
    let embedded = detect_embedded_images(data).unwrap_or_default();
    let text_size = data.len() - embedded.iter().map(|e| e.length).sum::<usize>();
    
    let embedded_savings: usize = embedded.iter()
        .map(|e| {
            // Estimate ~20-40% savings on embedded images
            (e.data.len() as f64 * 0.3) as usize
        })
        .sum();
    
    // Estimate ~10-30% text minification savings
    let text_savings = (text_size as f64 * 0.2) as usize;

    CompressionEstimate {
        original_size: data.len(),
        estimated_size: data.len().saturating_sub(embedded_savings + text_savings),
        embedded_image_count: embedded.len(),
        embedded_image_bytes: embedded.iter().map(|e| e.data.len()).sum(),
    }
}

#[derive(Debug)]
pub struct CompressionEstimate {
    pub original_size: usize,
    pub estimated_size: usize,
    pub embedded_image_count: usize,
    pub embedded_image_bytes: usize,
}
