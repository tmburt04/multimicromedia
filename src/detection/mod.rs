mod mime;

pub use mime::*;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FileFormat {
    // Images
    Png,
    Jpeg,
    Webp,
    Avif,
    Gif,
    Bmp,
    Tiff,
    Ico,
    Svg,
    Heic,
    // Audio
    Mp3,
    Wav,
    Flac,
    Ogg,
    Aac,
    Opus,
    Ac3,
    Aiff,
    Amr,
    Wma,
    // Video
    Mp4,
    Webm,
    Mov,
    Avi,
    Mkv,
    Wmv,
    Flv,
    Mpeg,
    // Unknown
    Unknown,
}

impl FileFormat {
    pub fn detect(data: &[u8]) -> Self {
        if data.len() < 2 {
            return Self::Unknown;
        }

        // PNG: 89 50 4E 47 0D 0A 1A 0A
        if data.starts_with(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]) {
            return Self::Png;
        }

        // JPEG: FF D8 FF
        if data.starts_with(&[0xFF, 0xD8, 0xFF]) {
            return Self::Jpeg;
        }

        // WebP: RIFF....WEBP
        if data.starts_with(b"RIFF") && data.len() >= 12 && &data[8..12] == b"WEBP" {
            return Self::Webp;
        }

        // GIF: GIF87a or GIF89a
        if data.starts_with(b"GIF87a") || data.starts_with(b"GIF89a") {
            return Self::Gif;
        }

        // BMP: BM
        if data.starts_with(b"BM") {
            return Self::Bmp;
        }

        // TIFF: II (little-endian) or MM (big-endian)
        if (data.starts_with(&[0x49, 0x49, 0x2A, 0x00]))
            || (data.starts_with(&[0x4D, 0x4D, 0x00, 0x2A]))
        {
            return Self::Tiff;
        }

        // ICO: 00 00 01 00
        if data.starts_with(&[0x00, 0x00, 0x01, 0x00]) {
            return Self::Ico;
        }

        // Check both the major and compatible brands; mif1 alone is not AVIF.
        if let Some(format) = detect_isobmff(data) {
            return format;
        }

        // SVG root following an optional XML prolog
        if is_likely_svg(data) {
            return Self::Svg;
        }

        // AAC: ADTS sync word or ADIF
        // Must check BEFORE MP3 since ADTS 0xFFF matches less-strict MP3 pattern
        // ADTS: 0xFF 0xF0-0xF9 (sync word 0xFFF, layer=0)
        if data.starts_with(b"ADIF") || (data[0] == 0xFF && (data[1] & 0xF6) == 0xF0) {
            return Self::Aac;
        }

        // MP3: ID3 tag or sync word
        // Sync: 0xFF followed by 0xE0+ with layer bits != 00
        if data.starts_with(b"ID3")
            || (data[0] == 0xFF && (data[1] & 0xE0) == 0xE0 && (data[1] & 0x06) != 0x00)
        {
            return Self::Mp3;
        }

        // WAV: RIFF....WAVE
        if data.starts_with(b"RIFF") && data.len() >= 12 && &data[8..12] == b"WAVE" {
            return Self::Wav;
        }

        // FLAC: fLaC
        if data.starts_with(b"fLaC") {
            return Self::Flac;
        }

        // OGG: OggS
        if data.starts_with(b"OggS") {
            // Could be Vorbis or Opus - check further
            if let Some(&segments) = data.get(26) {
                let packet_start = 27 + usize::from(segments);
                if segments > 0
                    && data.get(5).is_some_and(|flags| flags & 1 == 0)
                    && data.get(27).is_some_and(|length| *length >= 8)
                    && data.get(packet_start..packet_start + 8) == Some(b"OpusHead")
                {
                    return Self::Opus;
                }
            }
            return Self::Ogg;
        }

        // WebM: EBML header with webm doctype
        if data.starts_with(&[0x1A, 0x45, 0xDF, 0xA3]) {
            if ebml_doctype(data) == Some(b"webm") {
                return Self::Webm;
            }
            // Default to MKV for EBML without webm doctype
            return Self::Mkv;
        }

        // AVI: RIFF....AVI
        if data.starts_with(b"RIFF") && data.len() >= 12 && &data[8..12] == b"AVI " {
            return Self::Avi;
        }

        // AIFF/AIFC: FORM container
        if data.starts_with(b"FORM")
            && data.len() >= 12
            && matches!(&data[8..12], b"AIFF" | b"AIFC")
        {
            return Self::Aiff;
        }

        // AC3: Dolby Digital sync word 0x0B77
        if data.starts_with(&[0x0B, 0x77]) {
            return Self::Ac3;
        }

        // AMR: #!AMR
        if data.starts_with(b"#!AMR") {
            return Self::Amr;
        }

        // ASF/WMV/WMA: ASF header GUID
        if data.len() >= 16
            && data.starts_with(&[
                0x30, 0x26, 0xB2, 0x75, 0x8E, 0x66, 0xCF, 0x11, 0xA6, 0xD9, 0x00, 0xAA, 0x00, 0x62,
                0xCE, 0x6C,
            ])
        {
            return detect_asf(data);
        }

        // FLV: FLV + version
        if data.starts_with(b"FLV") && data.len() >= 4 {
            return Self::Flv;
        }

        // MPEG-1/2: Pack start code 0x000001BA or video start code 0x000001B3
        if data.starts_with(&[0, 0, 1, 0xBA]) || data.starts_with(&[0, 0, 1, 0xB3]) {
            return Self::Mpeg;
        }

        Self::Unknown
    }

    pub fn from_extension(ext: &str) -> Self {
        match ext
            .trim()
            .trim_start_matches('.')
            .to_ascii_lowercase()
            .as_str()
        {
            "png" => Self::Png,
            "jpg" | "jpeg" | "jpe" | "jfif" => Self::Jpeg,
            "webp" => Self::Webp,
            "avif" => Self::Avif,
            "gif" => Self::Gif,
            "bmp" | "dib" => Self::Bmp,
            "tif" | "tiff" => Self::Tiff,
            "ico" => Self::Ico,
            "svg" | "svgz" => Self::Svg,
            "heic" | "heif" => Self::Heic,
            "mp3" => Self::Mp3,
            "wav" | "wave" => Self::Wav,
            "flac" => Self::Flac,
            "ogg" | "oga" => Self::Ogg,
            "aac" | "m4a" => Self::Aac,
            "opus" => Self::Opus,
            "ac3" => Self::Ac3,
            "aif" | "aiff" | "aifc" => Self::Aiff,
            "amr" => Self::Amr,
            "wma" => Self::Wma,
            "mp4" | "m4v" => Self::Mp4,
            "webm" => Self::Webm,
            "mov" | "qt" => Self::Mov,
            "avi" => Self::Avi,
            "mkv" => Self::Mkv,
            "wmv" | "asf" => Self::Wmv,
            "flv" | "f4v" => Self::Flv,
            "mpg" | "mpeg" | "mpe" | "m2v" | "mpv" => Self::Mpeg,
            _ => Self::Unknown,
        }
    }

    pub fn from_mime(mime: &str) -> Self {
        match mime
            .split(';')
            .next()
            .unwrap_or_default()
            .trim()
            .to_ascii_lowercase()
            .as_str()
        {
            "image/png" => Self::Png,
            "image/jpeg" | "image/jpg" => Self::Jpeg,
            "image/webp" => Self::Webp,
            "image/avif" => Self::Avif,
            "image/gif" => Self::Gif,
            "image/bmp" | "image/x-bmp" => Self::Bmp,
            "image/tiff" => Self::Tiff,
            "image/x-icon" | "image/vnd.microsoft.icon" => Self::Ico,
            "image/svg+xml" => Self::Svg,
            "image/heic" | "image/heif" => Self::Heic,
            "audio/mpeg" | "audio/mp3" => Self::Mp3,
            "audio/wav" | "audio/wave" | "audio/x-wav" => Self::Wav,
            "audio/flac" | "audio/x-flac" => Self::Flac,
            "audio/ogg" | "audio/vorbis" => Self::Ogg,
            "audio/aac" | "audio/x-aac" | "audio/mp4" => Self::Aac,
            "audio/opus" => Self::Opus,
            "audio/ac3" | "audio/vnd.dolby.dd-raw" => Self::Ac3,
            "audio/aiff" | "audio/x-aiff" => Self::Aiff,
            "audio/amr" => Self::Amr,
            "audio/x-ms-wma" => Self::Wma,
            "video/mp4" => Self::Mp4,
            "video/webm" => Self::Webm,
            "video/quicktime" => Self::Mov,
            "video/x-msvideo" | "video/avi" => Self::Avi,
            "video/x-matroska" => Self::Mkv,
            "video/x-ms-wmv" | "video/x-ms-asf" => Self::Wmv,
            "video/x-flv" => Self::Flv,
            "video/mpeg" => Self::Mpeg,
            _ => Self::Unknown,
        }
    }

    pub fn extension(&self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Jpeg => "jpg",
            Self::Webp => "webp",
            Self::Avif => "avif",
            Self::Gif => "gif",
            Self::Bmp => "bmp",
            Self::Tiff => "tiff",
            Self::Ico => "ico",
            Self::Svg => "svg",
            Self::Heic => "heic",
            Self::Mp3 => "mp3",
            Self::Wav => "wav",
            Self::Flac => "flac",
            Self::Ogg => "ogg",
            Self::Aac => "aac",
            Self::Opus => "opus",
            Self::Ac3 => "ac3",
            Self::Aiff => "aiff",
            Self::Amr => "amr",
            Self::Wma => "wma",
            Self::Mp4 => "mp4",
            Self::Webm => "webm",
            Self::Mov => "mov",
            Self::Avi => "avi",
            Self::Mkv => "mkv",
            Self::Wmv => "wmv",
            Self::Flv => "flv",
            Self::Mpeg => "mpg",
            Self::Unknown => "bin",
        }
    }

    pub fn mime_type(&self) -> &'static str {
        match self {
            Self::Png => "image/png",
            Self::Jpeg => "image/jpeg",
            Self::Webp => "image/webp",
            Self::Avif => "image/avif",
            Self::Gif => "image/gif",
            Self::Bmp => "image/bmp",
            Self::Tiff => "image/tiff",
            Self::Ico => "image/x-icon",
            Self::Svg => "image/svg+xml",
            Self::Heic => "image/heic",
            Self::Mp3 => "audio/mpeg",
            Self::Wav => "audio/wav",
            Self::Flac => "audio/flac",
            Self::Ogg => "audio/ogg",
            Self::Aac => "audio/aac",
            Self::Opus => "audio/opus",
            Self::Ac3 => "audio/ac3",
            Self::Aiff => "audio/aiff",
            Self::Amr => "audio/amr",
            Self::Wma => "audio/x-ms-wma",
            Self::Mp4 => "video/mp4",
            Self::Webm => "video/webm",
            Self::Mov => "video/quicktime",
            Self::Avi => "video/x-msvideo",
            Self::Mkv => "video/x-matroska",
            Self::Wmv => "video/x-ms-wmv",
            Self::Flv => "video/x-flv",
            Self::Mpeg => "video/mpeg",
            Self::Unknown => "application/octet-stream",
        }
    }

    pub fn is_image(&self) -> bool {
        matches!(
            self,
            Self::Png
                | Self::Jpeg
                | Self::Webp
                | Self::Avif
                | Self::Gif
                | Self::Bmp
                | Self::Tiff
                | Self::Ico
                | Self::Svg
                | Self::Heic
        )
    }

    pub fn is_audio(&self) -> bool {
        matches!(
            self,
            Self::Mp3
                | Self::Wav
                | Self::Flac
                | Self::Ogg
                | Self::Aac
                | Self::Opus
                | Self::Ac3
                | Self::Aiff
                | Self::Amr
                | Self::Wma
        )
    }

    pub fn is_video(&self) -> bool {
        matches!(
            self,
            Self::Mp4
                | Self::Webm
                | Self::Mov
                | Self::Avi
                | Self::Mkv
                | Self::Wmv
                | Self::Flv
                | Self::Mpeg
        )
    }

    pub fn requires_ffmpeg(&self) -> bool {
        self.is_audio() || self.is_video() || matches!(self, Self::Avif | Self::Heic)
    }

    /// Legacy format-level hint; inspect FileAnalysis for detected animation.
    pub fn is_animated(&self) -> bool {
        matches!(self, Self::Gif | Self::Webp | Self::Avif)
    }
}

/// Iterate only the declared FileTypeBox, excluding the minor-version field.
pub(crate) fn isobmff_brands(data: &[u8]) -> impl Iterator<Item = &[u8]> {
    let payload = (|| {
        if data.get(4..8)? != b"ftyp" {
            return None;
        }
        let size = u32::from_be_bytes(data.get(..4)?.try_into().ok()?);
        let (header, end) = match size {
            0 => (8, data.len()),
            1 => (
                16,
                usize::try_from(u64::from_be_bytes(data.get(8..16)?.try_into().ok()?)).ok()?,
            ),
            size => (8, size as usize),
        };
        if end < header + 8 || end > data.len() {
            return None;
        }
        data.get(header..end)
    })()
    .unwrap_or_default();
    payload
        .get(..4)
        .into_iter()
        .chain(payload.get(8..).unwrap_or_default().chunks_exact(4))
}

fn detect_isobmff(data: &[u8]) -> Option<FileFormat> {
    let mut fallback = None;
    for brand in isobmff_brands(data) {
        match brand {
            b"avif" | b"avis" => return Some(FileFormat::Avif),
            b"heic" | b"heix" | b"hevc" | b"hevx" => fallback = Some(FileFormat::Heic),
            b"mif1" | b"msf1" => {
                fallback.get_or_insert(FileFormat::Heic);
            }
            b"qt  " if fallback.is_none() || fallback == Some(FileFormat::Mp4) => {
                fallback = Some(FileFormat::Mov);
            }
            b"M4A " | b"M4B " if fallback.is_none() || fallback == Some(FileFormat::Mp4) => {
                fallback = Some(FileFormat::Aac);
            }
            b"isom" | b"mp41" | b"mp42" | b"M4V " | b"mp71" | b"avc1" | b"iso2" | b"iso3"
            | b"iso4" | b"iso5" | b"iso6" | b"iso7" | b"iso8" | b"iso9" => {
                fallback.get_or_insert(FileFormat::Mp4);
            }
            _ => {}
        }
    }
    fallback
}

fn ebml_integer(data: &[u8], keep_marker: bool) -> Option<(u64, usize)> {
    let first = *data.first()?;
    if first == 0 {
        return None;
    }
    let length = first.leading_zeros() as usize + 1;
    let bytes = data.get(..length)?;
    let mut value = u64::from(if keep_marker {
        first
    } else {
        first & (0xffu16 >> length) as u8
    });
    for byte in &bytes[1..] {
        value = (value << 8) | u64::from(*byte);
    }
    Some((value, length))
}

fn ebml_doctype(data: &[u8]) -> Option<&[u8]> {
    let (size, length) = ebml_integer(data.get(4..)?, false)?;
    let start = 4 + length;
    let end = start.checked_add(usize::try_from(size).ok()?)?;
    let mut header = data.get(start..end)?;
    while !header.is_empty() {
        let (id, id_length) = ebml_integer(header, true)?;
        let (size, size_length) = ebml_integer(header.get(id_length..)?, false)?;
        let offset = id_length + size_length;
        let end = offset.checked_add(usize::try_from(size).ok()?)?;
        let value = header.get(offset..end)?;
        if id == 0x4282 {
            return Some(value);
        }
        header = header.get(end..)?;
    }
    None
}

fn detect_asf(data: &[u8]) -> FileFormat {
    // ASF objects have a 16-byte GUID and 8-byte length. Skip their payloads.
    let Some(header_size) = data
        .get(16..24)
        .and_then(|v| v.try_into().ok())
        .map(u64::from_le_bytes)
    else {
        return FileFormat::Wmv;
    };
    let limit = usize::try_from(header_size)
        .unwrap_or(data.len())
        .min(data.len());
    let mut offset = 30usize;
    let mut audio = false;
    while offset <= limit.saturating_sub(24) {
        let object = &data[offset..limit];
        let length = u64::from_le_bytes(object[16..24].try_into().unwrap());
        let Ok(length) = usize::try_from(length) else {
            break;
        };
        if length < 24 || length > object.len() {
            break;
        }
        // Stream Properties Object GUID, then its Stream Type GUID.
        if object[..16]
            == [
                0x91, 0x07, 0xdc, 0xb7, 0xb7, 0xa9, 0xcf, 0x11, 0x8e, 0xe6, 0, 0xc0, 0x0c, 0x20,
                0x53, 0x65,
            ]
            && length >= 40
        {
            match &object[24..40] {
                [0xc0, 0xef, 0x19, 0xbc, 0x4d, 0x5b, 0xcf, 0x11, 0xa8, 0xfd, 0, 0x80, 0x5f, 0x5c, 0x44, 0x2b] => {
                    return FileFormat::Wmv
                }
                [0x40, 0x9e, 0x69, 0xf8, 0x4d, 0x5b, 0xcf, 0x11, 0xa8, 0xfd, 0, 0x80, 0x5f, 0x5c, 0x44, 0x2b] => {
                    audio = true
                }
                _ => {}
            }
        }
        offset += length;
    }
    if audio {
        FileFormat::Wma
    } else {
        FileFormat::Wmv
    }
}

fn is_likely_svg(data: &[u8]) -> bool {
    // Sniff only the XML prolog/root name, without allocating a DOM for MIME detection.
    let mut remaining = data.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(data);
    loop {
        remaining = trim_leading_whitespace(remaining);
        let (prefix, terminator): (&[u8], &[u8]) = if remaining.starts_with(b"<?") {
            (b"<?", b"?>")
        } else if remaining.starts_with(b"<!--") {
            (b"<!--", b"-->")
        } else if remaining.starts_with(b"<!DOCTYPE") {
            let mut quote = None;
            let mut depth = 0u32;
            let mut end = None;
            for (i, &byte) in remaining.iter().enumerate().skip(9) {
                if let Some(delimiter) = quote {
                    if byte == delimiter {
                        quote = None;
                    }
                } else {
                    match byte {
                        b'\'' | b'"' => quote = Some(byte),
                        b'[' => depth = depth.saturating_add(1),
                        b']' => depth = depth.saturating_sub(1),
                        b'>' if depth == 0 => {
                            end = Some(i + 1);
                            break;
                        }
                        _ => {}
                    }
                }
            }
            let Some(end) = end else {
                return false;
            };
            remaining = &remaining[end..];
            continue;
        } else {
            let Some(name) = remaining.strip_prefix(b"<") else {
                return false;
            };
            let Some(end) = name
                .iter()
                .position(|b| b.is_ascii_whitespace() || matches!(b, b'>' | b'/'))
            else {
                return false;
            };
            return name[..end].rsplit(|b| *b == b':').next() == Some(b"svg");
        };
        let Some(end) = remaining[prefix.len()..]
            .windows(terminator.len())
            .position(|w| w == terminator)
        else {
            return false;
        };
        remaining = &remaining[prefix.len() + end + terminator.len()..];
    }
}

fn trim_leading_whitespace(data: &[u8]) -> &[u8] {
    let mut start = 0;
    while start < data.len()
        && (data[start] == b' '
            || data[start] == b'\t'
            || data[start] == b'\n'
            || data[start] == b'\r')
    {
        start += 1;
    }
    &data[start..]
}

#[derive(Debug, Clone)]
pub struct FileAnalysis {
    pub format: FileFormat,
    pub size: u64,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub has_alpha: Option<bool>,
    pub is_animated: Option<bool>,
    pub frame_count: Option<u32>,
    pub duration_ms: Option<u64>,
    pub bit_depth: Option<u8>,
    pub color_type: Option<String>,
    pub has_embedded_images: bool,
    pub embedded_image_count: u32,
}

impl FileAnalysis {
    pub fn new(format: FileFormat, size: u64) -> Self {
        Self {
            format,
            size,
            width: None,
            height: None,
            has_alpha: None,
            is_animated: None,
            frame_count: None,
            duration_ms: None,
            bit_depth: None,
            color_type: None,
            has_embedded_images: false,
            embedded_image_count: 0,
        }
    }
}
