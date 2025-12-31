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
        if data.len() < 12 {
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

        // AVIF: ftyp box with avif/avis/mif1 brand
        if data.len() >= 12 && &data[4..8] == b"ftyp" {
            let brand = &data[8..12];
            if brand == b"avif" || brand == b"avis" || brand == b"mif1" {
                return Self::Avif;
            }
            // HEIC: ftyp heic/heix/mif1
            if brand == b"heic" || brand == b"heix" {
                return Self::Heic;
            }
            // MP4/MOV detection
            if brand == b"isom"
                || brand == b"mp41"
                || brand == b"mp42"
                || brand == b"M4V "
                || brand == b"M4A "
                || brand == b"mp71"
                || brand == b"avc1"
                || brand == b"iso2"
                || brand == b"iso5"
                || brand == b"iso6"
            {
                return Self::Mp4;
            }
            if brand == b"qt  " {
                return Self::Mov;
            }
        }

        // SVG: text-based, check for <?xml or <svg
        if is_likely_svg(data) {
            return Self::Svg;
        }

        // AAC: ADTS sync word or ADIF
        // Must check BEFORE MP3 since ADTS 0xFFF matches less-strict MP3 pattern
        // ADTS: 0xFF 0xF0-0xF9 (sync word 0xFFF, layer=0)
        if data.starts_with(b"ADIF")
            || (data[0] == 0xFF && (data[1] & 0xF6) == 0xF0)
        {
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
            if data.len() >= 35 {
                if &data[28..35] == b"OpusHea" {
                    return Self::Opus;
                }
            }
            return Self::Ogg;
        }

        // WebM: EBML header with webm doctype
        if data.starts_with(&[0x1A, 0x45, 0xDF, 0xA3]) {
            // Check for webm doctype in the header
            if data.len() >= 31 {
                for i in 0..(data.len().saturating_sub(4)) {
                    if &data[i..i + 4] == b"webm" {
                        return Self::Webm;
                    }
                    if &data[i..i + 8] == b"matroska" {
                        return Self::Mkv;
                    }
                }
            }
            // Default to MKV for EBML without webm doctype
            return Self::Mkv;
        }

        // AVI: RIFF....AVI
        if data.starts_with(b"RIFF") && data.len() >= 12 && &data[8..12] == b"AVI " {
            return Self::Avi;
        }

        // AIFF: FORM....AIFF
        if data.starts_with(b"FORM") && data.len() >= 12 && &data[8..12] == b"AIFF" {
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
            // ASF container - could be WMV or WMA
            // Check for video stream GUID to distinguish
            // For simplicity, check file size and extension hint
            // Default to WMV for video, but we'll check content type GUIDs
            for i in 0..(data.len().saturating_sub(16)) {
                // Video Media GUID
                if &data[i..i + 16]
                    == &[
                        0xC0, 0xEF, 0x19, 0xBC, 0x4D, 0x5B, 0xCF, 0x11, 0xA8, 0xFD, 0x00, 0x80,
                        0x5F, 0x5C, 0x44, 0x2B,
                    ]
                {
                    return Self::Wmv;
                }
                // Audio Media GUID
                if &data[i..i + 16]
                    == &[
                        0x40, 0x9E, 0x69, 0xF8, 0x4D, 0x5B, 0xCF, 0x11, 0xA8, 0xFD, 0x00, 0x80,
                        0x5F, 0x5C, 0x44, 0x2B,
                    ]
                {
                    return Self::Wma;
                }
            }
            // Default to WMV if we can't determine
            return Self::Wmv;
        }

        // FLV: FLV + version
        if data.starts_with(b"FLV") && data.len() >= 4 {
            return Self::Flv;
        }

        // MPEG-1/2: Pack start code 0x000001BA or video start code 0x000001B3
        if data.len() >= 4 {
            if (data[0] == 0x00 && data[1] == 0x00 && data[2] == 0x01 && data[3] == 0xBA)
                || (data[0] == 0x00 && data[1] == 0x00 && data[2] == 0x01 && data[3] == 0xB3)
            {
                return Self::Mpeg;
            }
        }

        Self::Unknown
    }

    pub fn from_extension(ext: &str) -> Self {
        match ext.to_lowercase().as_str() {
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
        match mime.to_lowercase().as_str() {
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
        self.is_audio() || self.is_video() || *self == Self::Gif
    }

    pub fn is_animated(&self) -> bool {
        matches!(self, Self::Gif | Self::Webp | Self::Avif)
    }
}

fn is_likely_svg(data: &[u8]) -> bool {
    // Check for BOM or whitespace at start
    let trimmed = trim_leading_whitespace(data);
    if trimmed.len() < 4 {
        return false;
    }

    // Check for <?xml or <svg or <!DOCTYPE svg
    if trimmed.starts_with(b"<?xml")
        || trimmed.starts_with(b"<svg")
        || trimmed.starts_with(b"<!DOCTYPE svg")
        || trimmed.starts_with(b"<!doctype svg")
    {
        return true;
    }

    // Check for UTF-8 BOM followed by SVG content
    if trimmed.starts_with(&[0xEF, 0xBB, 0xBF]) {
        let after_bom = &trimmed[3..];
        return is_likely_svg(after_bom);
    }

    false
}

fn trim_leading_whitespace(data: &[u8]) -> &[u8] {
    let mut start = 0;
    while start < data.len() && (data[start] == b' ' || data[start] == b'\t' || data[start] == b'\n' || data[start] == b'\r') {
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
