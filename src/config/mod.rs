pub mod defaults;

pub use defaults::*;

use crate::detection::FileFormat;
use crate::error::{CompressionError, Result};
use crate::result::ValidationResult;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OutputFormat {
    // Images
    Png,
    #[serde(alias = "jpg")]
    Jpeg,
    Webp,
    Avif,
    Gif,
    Bmp,
    Tiff,
    Ico,
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
    #[serde(alias = "mpg")]
    Mpeg,
}

impl OutputFormat {
    pub fn file_format(self) -> FileFormat {
        match self {
            Self::Png => FileFormat::Png,
            Self::Jpeg => FileFormat::Jpeg,
            Self::Webp => FileFormat::Webp,
            Self::Avif => FileFormat::Avif,
            Self::Gif => FileFormat::Gif,
            Self::Bmp => FileFormat::Bmp,
            Self::Tiff => FileFormat::Tiff,
            Self::Ico => FileFormat::Ico,
            Self::Mp3 => FileFormat::Mp3,
            Self::Wav => FileFormat::Wav,
            Self::Flac => FileFormat::Flac,
            Self::Ogg => FileFormat::Ogg,
            Self::Aac => FileFormat::Aac,
            Self::Opus => FileFormat::Opus,
            Self::Ac3 => FileFormat::Ac3,
            Self::Aiff => FileFormat::Aiff,
            Self::Amr => FileFormat::Amr,
            Self::Wma => FileFormat::Wma,
            Self::Mp4 => FileFormat::Mp4,
            Self::Webm => FileFormat::Webm,
            Self::Mov => FileFormat::Mov,
            Self::Avi => FileFormat::Avi,
            Self::Mkv => FileFormat::Mkv,
            Self::Wmv => FileFormat::Wmv,
            Self::Flv => FileFormat::Flv,
            Self::Mpeg => FileFormat::Mpeg,
        }
    }
    pub fn extension(&self) -> &'static str {
        self.file_format().extension()
    }
    pub fn mime_type(&self) -> &'static str {
        self.file_format().mime_type()
    }
    pub fn is_image(&self) -> bool {
        self.file_format().is_image()
    }
    pub fn is_audio(&self) -> bool {
        self.file_format().is_audio()
    }
    pub fn is_video(&self) -> bool {
        self.file_format().is_video()
    }

    pub fn from_file_format(format: FileFormat) -> Option<Self> {
        match format {
            FileFormat::Png => Some(OutputFormat::Png),
            FileFormat::Jpeg => Some(OutputFormat::Jpeg),
            FileFormat::Webp => Some(OutputFormat::Webp),
            FileFormat::Avif => Some(OutputFormat::Avif),
            FileFormat::Gif => Some(OutputFormat::Gif),
            FileFormat::Bmp => Some(OutputFormat::Bmp),
            FileFormat::Tiff => Some(OutputFormat::Tiff),
            FileFormat::Ico => Some(OutputFormat::Ico),
            FileFormat::Svg => None,
            FileFormat::Heic => Some(OutputFormat::Jpeg),
            FileFormat::Mp3 => Some(OutputFormat::Mp3),
            FileFormat::Wav => Some(OutputFormat::Wav),
            FileFormat::Flac => Some(OutputFormat::Flac),
            FileFormat::Ogg => Some(OutputFormat::Ogg),
            FileFormat::Aac => Some(OutputFormat::Aac),
            FileFormat::Opus => Some(OutputFormat::Opus),
            FileFormat::Ac3 => Some(OutputFormat::Ac3),
            FileFormat::Aiff => Some(OutputFormat::Aiff),
            FileFormat::Amr => Some(OutputFormat::Amr),
            FileFormat::Wma => Some(OutputFormat::Wma),
            FileFormat::Mp4 => Some(OutputFormat::Mp4),
            FileFormat::Webm => Some(OutputFormat::Webm),
            FileFormat::Mov => Some(OutputFormat::Mov),
            FileFormat::Avi => Some(OutputFormat::Avi),
            FileFormat::Mkv => Some(OutputFormat::Mkv),
            FileFormat::Wmv => Some(OutputFormat::Wmv),
            FileFormat::Flv => Some(OutputFormat::Flv),
            FileFormat::Mpeg => Some(OutputFormat::Mpeg),
            FileFormat::Unknown => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResizeConfig {
    pub width: Option<u32>,
    pub height: Option<u32>,
    #[serde(default)]
    pub mode: ResizeMode,
    #[serde(default = "default_true")]
    pub preserve_aspect: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ResizeMode {
    #[default]
    Fit,
    Fill,
    Exact,
    Cover,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CropConfig {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrimConfig {
    pub start_ms: Option<u64>,
    pub end_ms: Option<u64>,
    pub duration_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Only `quality` is applied; the remaining fields are retained for compatibility.
#[serde(deny_unknown_fields)]
pub struct JpegConfig {
    #[serde(default = "default_jpeg_quality")]
    pub quality: u8,
    #[serde(default)]
    pub progressive: bool,
    #[serde(default = "default_true")]
    pub optimize_coding: bool,
    #[serde(default)]
    pub chroma_subsampling: ChromaSubsampling,
}

fn default_jpeg_quality() -> u8 {
    85
}

impl Default for JpegConfig {
    fn default() -> Self {
        Self {
            quality: 85,
            progressive: false,
            optimize_coding: true,
            chroma_subsampling: ChromaSubsampling::Auto,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChromaSubsampling {
    #[default]
    Auto,
    #[serde(rename = "4:4:4")]
    S444,
    #[serde(rename = "4:2:2")]
    S422,
    #[serde(rename = "4:2:0")]
    S420,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Quantization/color limits apply to still PNG output, including transforms and conversions.
/// `interlaced` is retained for compatibility; output is non-interlaced.
#[serde(deny_unknown_fields)]
pub struct PngConfig {
    #[serde(default = "default_png_compression")]
    pub compression_level: u8,
    #[serde(default)]
    pub quantize: bool,
    #[serde(default = "default_png_colors")]
    pub max_colors: u16,
    #[serde(default)]
    pub interlaced: bool,
}

fn default_png_compression() -> u8 {
    6
}

fn default_png_colors() -> u16 {
    256
}

impl Default for PngConfig {
    fn default() -> Self {
        Self {
            compression_level: 6,
            quantize: false,
            max_colors: 256,
            interlaced: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Compatibility settings; the current encoder always uses lossless WebP.
#[serde(deny_unknown_fields)]
pub struct WebpConfig {
    #[serde(default = "default_webp_quality")]
    pub quality: u8,
    #[serde(default)]
    pub lossless: bool,
    #[serde(default = "default_webp_method")]
    pub method: u8,
}

fn default_webp_quality() -> u8 {
    80
}

fn default_webp_method() -> u8 {
    4
}

impl Default for WebpConfig {
    fn default() -> Self {
        Self {
            quality: 80,
            lossless: false,
            method: 4,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Compatibility settings; AVIF encoding is not implemented.
#[serde(deny_unknown_fields)]
pub struct AvifConfig {
    #[serde(default = "default_avif_quality")]
    pub quality: u8,
    #[serde(default = "default_avif_speed")]
    pub speed: u8,
}

fn default_avif_quality() -> u8 {
    70
}

fn default_avif_speed() -> u8 {
    6
}

impl Default for AvifConfig {
    fn default() -> Self {
        Self {
            quality: 70,
            speed: 6,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// `max_colors` affects the static quantization candidate, not every output.
/// `lossy` and `optimize_frames` are retained but unused.
#[serde(deny_unknown_fields)]
pub struct GifConfig {
    #[serde(default = "default_gif_colors")]
    pub max_colors: u16,
    #[serde(default)]
    pub lossy: u8,
    #[serde(default = "default_true")]
    pub optimize_frames: bool,
}

fn default_gif_colors() -> u16 {
    256
}

impl Default for GifConfig {
    fn default() -> Self {
        Self {
            max_colors: 256,
            lossy: 0,
            optimize_frames: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Comment removal requires both `minify` and `remove_comments`.
/// `precision` is retained but unused; numeric text is preserved.
#[serde(deny_unknown_fields)]
pub struct SvgConfig {
    #[serde(default = "default_true")]
    pub minify: bool,
    #[serde(default = "default_true")]
    pub compress_embedded_images: bool,
    #[serde(default = "default_svg_precision")]
    pub precision: u8,
    #[serde(default = "default_true")]
    pub remove_comments: bool,
}

fn default_svg_precision() -> u8 {
    3
}

impl Default for SvgConfig {
    fn default() -> Self {
        Self {
            minify: true,
            compress_embedded_images: true,
            precision: 3,
            remove_comments: true,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FfmpegConfig {
    pub video_codec: Option<String>,
    pub audio_codec: Option<String>,
    pub video_bitrate: Option<String>,
    pub audio_bitrate: Option<String>,
    pub crf: Option<u8>,
    pub preset: Option<String>,
    #[serde(default)]
    pub extra_flags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompressionConfig {
    /// Compatibility field; format detection currently uses file bytes only.
    #[serde(default)]
    pub input_hint: Option<String>,

    // Output configuration
    #[serde(default)]
    pub output_format: Option<OutputFormat>,

    // Quality/compression effort in 1-100; interpretation depends on the encoder.
    #[serde(default = "default_quality")]
    pub quality: u8,

    // Transforms
    #[serde(default)]
    pub resize: Option<ResizeConfig>,
    #[serde(default)]
    pub crop: Option<CropConfig>,
    #[serde(default)]
    pub trim: Option<TrimConfig>,

    // Format-specific overrides
    #[serde(default)]
    pub jpeg: Option<JpegConfig>,
    #[serde(default)]
    pub png: Option<PngConfig>,
    #[serde(default)]
    pub webp: Option<WebpConfig>,
    #[serde(default)]
    pub avif: Option<AvifConfig>,
    #[serde(default)]
    pub gif: Option<GifConfig>,
    #[serde(default)]
    pub svg: Option<SvgConfig>,
    #[serde(default)]
    pub ffmpeg: Option<FfmpegConfig>,

    // Processing options
    #[serde(default = "default_chunk_size")]
    /// Utility chunk size; `compress` does not automatically chunk media.
    pub chunk_size_mb: u32,
    #[serde(default = "default_true")]
    pub preserve_metadata: bool,
}

fn default_quality() -> u8 {
    80
}

fn default_chunk_size() -> u32 {
    DEFAULT_CHUNK_SIZE_MB
}

impl Default for CompressionConfig {
    fn default() -> Self {
        Self {
            input_hint: None,
            output_format: None,
            quality: 80,
            resize: None,
            crop: None,
            trim: None,
            jpeg: None,
            png: None,
            webp: None,
            avif: None,
            gif: None,
            svg: None,
            ffmpeg: None,
            chunk_size_mb: DEFAULT_CHUNK_SIZE_MB,
            preserve_metadata: true,
        }
    }
}

impl CompressionConfig {
    pub fn from_json(json: &str) -> Result<Self> {
        serde_json::from_str(json).map_err(|e| CompressionError::InvalidConfig {
            field: "json".to_string(),
            reason: e.to_string(),
        })
    }

    pub fn validate(&self, input_format: Option<FileFormat>) -> ValidationResult {
        let mut result = ValidationResult::ok();

        if self.input_hint.is_some() {
            result.add_warning("input_hint is ignored; the input format is detected from bytes");
        }
        if self.webp.is_some() {
            result.add_warning(
                "webp settings are compatibility fields; the Rust encoder is always lossless",
            );
        }
        if self.avif.is_some() {
            result.add_warning(
                "avif settings are compatibility fields; AVIF encoding is unavailable",
            );
        }
        if self.png.as_ref().is_some_and(|png| png.interlaced) {
            result.add_warning("png.interlaced is a compatibility field and is ignored");
        }
        if self.jpeg.as_ref().is_some_and(|jpeg| jpeg.progressive) {
            result.add_warning("jpeg.progressive is a compatibility field and is ignored");
        }
        if self.ffmpeg.as_ref().is_some_and(|ffmpeg| {
            ffmpeg.audio_codec.as_deref() == Some("copy")
                || ffmpeg.video_codec.as_deref() == Some("copy")
        }) {
            result.add_warning("stream copy requires codecs supported by the output container; preflight does not probe stream codecs");
        }

        if self.quality == 0 || self.quality > 100 {
            result.add_error("quality", "must be between 1 and 100");
        }

        if self.chunk_size_mb == 0 {
            result.add_error("chunk_size_mb", "must be greater than 0");
        }

        if let Some(ref resize) = self.resize {
            if resize.width.is_none() && resize.height.is_none() {
                result.add_error("resize", "must specify at least width or height");
            }
            if let Some(w) = resize.width {
                if w == 0 {
                    result.add_error("resize.width", "must be greater than 0");
                }
            }
            if let Some(h) = resize.height {
                if h == 0 {
                    result.add_error("resize.height", "must be greater than 0");
                }
            }
        }

        if let Some(ref crop) = self.crop {
            if crop.width == 0 || crop.height == 0 {
                result.add_error("crop", "width and height must be greater than 0");
            }
        }

        if let Some(ref trim) = self.trim {
            if trim.duration_ms == Some(0) {
                result.add_error("trim.duration_ms", "must be greater than 0");
            }
            if trim.end_ms.is_some() && trim.duration_ms.is_some() {
                result.add_error("trim", "specify end_ms or duration_ms, not both");
            }
            if let Some(end) = trim.end_ms {
                if trim.start_ms.unwrap_or(0) >= end {
                    result.add_error("trim", "start_ms must be less than end_ms");
                }
            }
            if trim.start_ms.is_none() && trim.end_ms.is_none() && trim.duration_ms.is_none() {
                result.add_error(
                    "trim",
                    "must specify at least one of start_ms, end_ms, or duration_ms",
                );
            }
        }

        if let Some(ref jpeg) = self.jpeg {
            if jpeg.quality == 0 || jpeg.quality > 100 {
                result.add_error("jpeg.quality", "must be between 1 and 100");
            }
        }

        if let Some(ref png) = self.png {
            if png.compression_level > 9 {
                result.add_error("png.compression_level", "must be between 0 and 9");
            }
            if png.max_colors < 2 || png.max_colors > 256 {
                result.add_error("png.max_colors", "must be between 2 and 256");
            }
        }

        if let Some(ref webp) = self.webp {
            if webp.quality > 100 {
                result.add_error("webp.quality", "must be between 0 and 100");
            }
            if webp.method > 6 {
                result.add_error("webp.method", "must be between 0 and 6");
            }
        }

        if let Some(ref avif) = self.avif {
            if avif.quality > 100 {
                result.add_error("avif.quality", "must be between 0 and 100");
            }
            if avif.speed > 10 {
                result.add_error("avif.speed", "must be between 0 and 10");
            }
        }

        if let Some(ref gif) = self.gif {
            if gif.max_colors < 2 || gif.max_colors > 256 {
                result.add_error("gif.max_colors", "must be between 2 and 256");
            }
        }

        if let Some(ref svg) = self.svg {
            if svg.precision > 10 {
                result.add_error("svg.precision", "must be between 0 and 10");
            }
        }

        if let Some(ref ffmpeg) = self.ffmpeg {
            if let Some(crf) = ffmpeg.crf {
                if crf > 63 {
                    result.add_error("ffmpeg.crf", "must be between 0 and 63");
                }
            }
            for (field, value) in [
                ("ffmpeg.video_codec", &ffmpeg.video_codec),
                ("ffmpeg.audio_codec", &ffmpeg.audio_codec),
                ("ffmpeg.video_bitrate", &ffmpeg.video_bitrate),
                ("ffmpeg.audio_bitrate", &ffmpeg.audio_bitrate),
                ("ffmpeg.preset", &ffmpeg.preset),
            ] {
                if value
                    .as_ref()
                    .is_some_and(|value| value.trim().is_empty() || value.contains('\0'))
                {
                    result.add_error(field, "must be nonempty and contain no NUL characters");
                }
            }
            if ffmpeg
                .extra_flags
                .iter()
                .any(|flag| flag.is_empty() || flag.contains('\0'))
            {
                result.add_error(
                    "ffmpeg.extra_flags",
                    "arguments must be nonempty and contain no NUL characters",
                );
            }
        }

        if let (Some(input), Some(output)) = (input_format, self.output_format) {
            let conversion =
                crate::validation::validate_format_conversion(input, output.file_format());
            if !conversion.valid {
                result.valid = false;
            }
            result.errors.extend(conversion.errors);
            result.warnings.extend(conversion.warnings);
        }
        if let Some(input) = input_format {
            if self.trim.is_some() && input.is_image() {
                result.add_warning("trim is ignored for image formats");
            }
            if (self.resize.is_some() || self.crop.is_some()) && input.is_audio() {
                result.add_warning("resize/crop is ignored for audio formats");
            }
        }

        result
    }

    pub fn get_jpeg_config(&self) -> JpegConfig {
        self.jpeg.clone().unwrap_or_else(|| JpegConfig {
            quality: self.quality,
            ..Default::default()
        })
    }

    pub fn get_png_config(&self) -> PngConfig {
        self.png.clone().unwrap_or_else(|| PngConfig {
            compression_level: quality_to_png_compression(self.quality),
            ..Default::default()
        })
    }

    pub fn get_webp_config(&self) -> WebpConfig {
        self.webp.clone().unwrap_or_else(|| WebpConfig {
            quality: self.quality,
            ..Default::default()
        })
    }

    pub fn get_avif_config(&self) -> AvifConfig {
        self.avif.clone().unwrap_or_else(|| AvifConfig {
            quality: self.quality,
            ..Default::default()
        })
    }

    pub fn get_gif_config(&self) -> GifConfig {
        self.gif.clone().unwrap_or_default()
    }

    pub fn get_svg_config(&self) -> SvgConfig {
        self.svg.clone().unwrap_or_default()
    }

    pub fn get_ffmpeg_config(&self) -> FfmpegConfig {
        self.ffmpeg.clone().unwrap_or_default()
    }
}

fn quality_to_png_compression(quality: u8) -> u8 {
    // Map quality to lossless compression effort: 1 -> level 1, 100 -> level 9.
    ((quality as u16 * 9) / 100).clamp(1, 9) as u8
}
