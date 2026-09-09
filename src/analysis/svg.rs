use crate::detection::FileAnalysis;
use crate::error::{CompressionError, Result};
use base64::Engine;
use regex::bytes::Regex;
use std::sync::LazyLock;

const BASE64_DATA_URI_PATTERN: &str =
    r#"data:image/(png|jpeg|jpg|webp|gif|avif);base64,([A-Za-z0-9+/=]+)"#;

static EMBEDDED_IMAGE_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(BASE64_DATA_URI_PATTERN).expect("static data URI regex is valid"));

pub fn analyze_svg(data: &[u8], analysis: &mut FileAnalysis) -> Result<()> {
    let text = std::str::from_utf8(data).map_err(|_| CompressionError::DecodeFailed {
        format: "SVG".to_string(),
        detail: "invalid UTF-8".to_string(),
    })?;

    let doc = roxmltree::Document::parse(text).map_err(|e| CompressionError::DecodeFailed {
        format: "SVG".to_string(),
        detail: e.to_string(),
    })?;

    let root = doc.root_element();

    if root.tag_name().name() != "svg" {
        return Err(CompressionError::DecodeFailed {
            format: "SVG".into(),
            detail: "root element must be svg".into(),
        });
    }
    // Explicit pixel dimensions describe the viewport; viewBox is a fallback.
    analysis.width = root
        .attribute("width")
        .and_then(|v| parse_svg_dimension(v).ok());
    analysis.height = root
        .attribute("height")
        .and_then(|v| parse_svg_dimension(v).ok());
    if let Some(viewbox) = root.attribute("viewBox") {
        let mut parts = viewbox
            .split(|c: char| c.is_ascii_whitespace() || c == ',')
            .filter(|s| !s.is_empty())
            .map(str::parse::<f64>);
        if let (Some(Ok(x)), Some(Ok(y)), Some(Ok(w)), Some(Ok(h)), None) = (
            parts.next(),
            parts.next(),
            parts.next(),
            parts.next(),
            parts.next(),
        ) {
            if x.is_finite() && y.is_finite() {
                analysis.width = analysis.width.or_else(|| pixel_dimension(w).ok());
                analysis.height = analysis.height.or_else(|| pixel_dimension(h).ok());
            }
        }
    }

    // Metadata analysis counts matches without allocating decoded copies of every image.
    analysis.embedded_image_count = EMBEDDED_IMAGE_REGEX.find_iter(data).count() as u32;
    analysis.has_embedded_images = analysis.embedded_image_count > 0;

    analysis.has_alpha = Some(true);
    analysis.is_animated = Some(false); // SMIL/CSS animation is not analyzed.

    Ok(())
}

fn parse_svg_dimension(value: &str) -> std::result::Result<u32, ()> {
    // Relative units need a viewport or font context; do not report them as pixels.
    let value = value.trim();
    let number = value
        .strip_suffix("px")
        .unwrap_or(value)
        .parse::<f64>()
        .map_err(|_| ())?;
    pixel_dimension(number)
}

fn pixel_dimension(number: f64) -> std::result::Result<u32, ()> {
    if number.is_finite() && number > 0.0 && number <= f64::from(u32::MAX) {
        Ok(number.ceil() as u32)
    } else {
        Err(())
    }
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
    Ok(iter_embedded_images(data).collect())
}

/// Decode only the image being processed instead of retaining every raster at once.
pub(crate) fn iter_embedded_images(data: &[u8]) -> impl Iterator<Item = EmbeddedImage> + '_ {
    EMBEDDED_IMAGE_REGEX.captures_iter(data).filter_map(|cap| {
        let format = cap.get(1)?;
        let encoded = cap.get(2)?;
        let full_match = cap.get(0)?;
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(encoded.as_bytes())
            .ok()?;
        Some(EmbeddedImage {
            format: String::from_utf8_lossy(format.as_bytes()).into_owned(),
            data: decoded,
            offset: full_match.start(),
            length: full_match.len(),
            original_data_uri: String::from_utf8_lossy(full_match.as_bytes()).into_owned(),
        })
    })
}

pub fn replace_embedded_images(
    svg_data: &[u8],
    replacements: &[(usize, usize, String)],
) -> Vec<u8> {
    let mut sorted: Vec<_> = replacements.iter().collect();
    sorted.sort_by_key(|replacement| replacement.0);
    let mut result = Vec::with_capacity(svg_data.len());
    let mut cursor = 0;
    for (offset, length, new_data_uri) in sorted {
        let Some(end) = offset.checked_add(*length) else {
            continue;
        };
        if *offset < cursor || end > svg_data.len() {
            continue;
        }
        result.extend_from_slice(&svg_data[cursor..*offset]);
        result.extend_from_slice(new_data_uri.as_bytes());
        cursor = end;
    }
    result.extend_from_slice(&svg_data[cursor..]);
    result
}
