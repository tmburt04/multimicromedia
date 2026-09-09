use crate::config::{CompressionConfig, OutputFormat};
use crate::error::Result;

pub async fn compress_jpeg(
    data: &[u8],
    config: &CompressionConfig,
    output_format: Option<OutputFormat>,
) -> Result<Vec<u8>> {
    super::compress_generic(
        data,
        config,
        Some(output_format.unwrap_or(OutputFormat::Jpeg)),
    )
    .await
}

pub fn extract_jpeg_metadata(data: &[u8]) -> JpegMetadata {
    let mut metadata = JpegMetadata::default();

    if !data.starts_with(&[0xff, 0xd8]) {
        return metadata;
    }
    let mut offset = 2;
    while data.get(offset) == Some(&0xff) {
        while data.get(offset) == Some(&0xff) {
            offset += 1;
        }
        let Some(&marker) = data.get(offset) else {
            break;
        };
        offset += 1;
        // Entropy-coded scan data is not a sequence of metadata segments.
        if matches!(marker, 0x00 | 0xd9 | 0xda) {
            break;
        }
        if marker == 0x01 || (0xd0..=0xd8).contains(&marker) {
            continue;
        }
        let Some(length_bytes) = data.get(offset..offset.saturating_add(2)) else {
            break;
        };
        let length = u16::from_be_bytes([length_bytes[0], length_bytes[1]]) as usize;
        if length < 2 {
            break;
        }
        let Some(end) = offset.checked_add(length).filter(|&end| end <= data.len()) else {
            break;
        };
        let payload = &data[offset + 2..end];
        match marker {
            0xc0..=0xc3 | 0xc5..=0xc7 | 0xc9..=0xcb | 0xcd..=0xcf
                if payload.len() >= 6 && payload.len() >= 6 + 3 * payload[5] as usize =>
            {
                metadata.is_progressive = matches!(marker, 0xc2 | 0xc6 | 0xca | 0xce);
                metadata.bit_depth = payload[0];
                metadata.height = u16::from_be_bytes([payload[1], payload[2]]);
                metadata.width = u16::from_be_bytes([payload[3], payload[4]]);
                metadata.components = payload[5];
            }
            0xe1 if payload.starts_with(b"Exif\0\0") => metadata.has_exif = true,
            0xe0 if payload.starts_with(b"JFIF\0") => metadata.has_jfif = true,
            0xed => metadata.has_iptc = true,
            0xee => metadata.has_adobe = true,
            _ => {}
        }
        offset = end;
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

    /// Sampling factors are not extracted; this compatibility accessor returns "unknown".
    pub fn chroma_subsampling(&self) -> &'static str {
        "unknown"
    }
}
