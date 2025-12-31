use crate::detection::FileAnalysis;
use crate::error::{CompressionError, Result};
use base64::Engine;
use regex::bytes::Regex;

const BASE64_DATA_URI_PATTERN: &str = r#"data:image/(png|jpeg|jpg|webp|gif|avif);base64,([A-Za-z0-9+/=]+)"#;

pub fn analyze_svg(data: &[u8], analysis: &mut FileAnalysis) -> Result<()> {
    let text = std::str::from_utf8(data).map_err(|_| CompressionError::DecodeFailed {
        format: "SVG".to_string(),
        detail: "invalid UTF-8".to_string(),
    })?;

    // Parse SVG to get dimensions
    let doc = roxmltree::Document::parse(text).map_err(|e| CompressionError::DecodeFailed {
        format: "SVG".to_string(),
        detail: e.to_string(),
    })?;

    let root = doc.root_element();

    // Try to get dimensions from viewBox or width/height attributes
    if let Some(viewbox) = root.attribute("viewBox") {
        let parts: Vec<&str> = viewbox.split_whitespace().collect();
        if parts.len() >= 4 {
            if let (Ok(w), Ok(h)) = (parts[2].parse::<f64>(), parts[3].parse::<f64>()) {
                analysis.width = Some(w as u32);
                analysis.height = Some(h as u32);
            }
        }
    }

    if analysis.width.is_none() {
        if let Some(width) = root.attribute("width") {
            if let Ok(w) = parse_svg_dimension(width) {
                analysis.width = Some(w);
            }
        }
    }

    if analysis.height.is_none() {
        if let Some(height) = root.attribute("height") {
            if let Ok(h) = parse_svg_dimension(height) {
                analysis.height = Some(h);
            }
        }
    }

    // Count embedded images
    let embedded_info = detect_embedded_images(data)?;
    analysis.has_embedded_images = !embedded_info.is_empty();
    analysis.embedded_image_count = embedded_info.len() as u32;

    analysis.has_alpha = Some(true);
    analysis.is_animated = Some(false); // Could check for SMIL animations

    Ok(())
}

fn parse_svg_dimension(value: &str) -> std::result::Result<u32, ()> {
    // Strip units (px, em, %, etc.)
    let numeric: String = value.chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect();
    numeric.parse::<f64>().map(|v| v as u32).map_err(|_| ())
}

#[derive(Debug, Clone)]
pub struct EmbeddedImage {
    pub format: String,
    pub data: Vec<u8>,
    pub offset: usize,
    pub length: usize,
    pub original_data_uri: String,
}

pub fn detect_embedded_images(data: &[u8]) -> Result<Vec<EmbeddedImage>> {
    // Use regex to find all data URIs
    // Note: regex crate doesn't support bytes by default, so we use regex::bytes
    let re = Regex::new(BASE64_DATA_URI_PATTERN).map_err(|e| CompressionError::InvalidInput {
        reason: format!("regex error: {}", e),
    })?;

    let mut images = Vec::new();
    let engine = base64::engine::general_purpose::STANDARD;

    for cap in re.captures_iter(data) {
        if let (Some(format_match), Some(b64_match)) = (cap.get(1), cap.get(2)) {
            let format = String::from_utf8_lossy(format_match.as_bytes()).to_string();
            let b64_str = std::str::from_utf8(b64_match.as_bytes()).unwrap_or("");
            
            // Decode base64
            if let Ok(decoded) = engine.decode(b64_str) {
                let full_match = cap.get(0).unwrap();
                images.push(EmbeddedImage {
                    format,
                    data: decoded,
                    offset: full_match.start(),
                    length: full_match.len(),
                    original_data_uri: String::from_utf8_lossy(full_match.as_bytes()).to_string(),
                });
            }
        }
    }

    Ok(images)
}

pub fn replace_embedded_images(svg_data: &[u8], replacements: &[(usize, usize, String)]) -> Vec<u8> {
    // Sort replacements by offset (descending) to avoid offset shifts
    let mut sorted: Vec<_> = replacements.iter().collect();
    sorted.sort_by(|a, b| b.0.cmp(&a.0));

    let mut result = svg_data.to_vec();

    for (offset, length, new_data_uri) in sorted {
        let end = offset + length;
        if end <= result.len() {
            result.splice(*offset..end, new_data_uri.bytes());
        }
    }

    result
}
