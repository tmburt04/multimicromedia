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
    Mpeg,
}

impl OutputFormat {
    pub fn extension(&self) -> &'static str {
        match self {
            OutputFormat::Png => "png",
            OutputFormat::Jpeg => "jpg",
            OutputFormat::Webp => "webp",
            OutputFormat::Avif => "avif",
            OutputFormat::Gif => "gif",
            OutputFormat::Bmp => "bmp",
            OutputFormat::Tiff => "tiff",
            OutputFormat::Ico => "ico",
            OutputFormat::Mp3 => "mp3",
            OutputFormat::Wav => "wav",
            OutputFormat::Flac => "flac",
            OutputFormat::Ogg => "ogg",
            OutputFormat::Aac => "aac",
            OutputFormat::Opus => "opus",
            OutputFormat::Ac3 => "ac3",
            OutputFormat::Aiff => "aiff",
            OutputFormat::Amr => "amr",
            OutputFormat::Wma => "wma",
            OutputFormat::Mp4 => "mp4",
            OutputFormat::Webm => "webm",
            OutputFormat::Mov => "mov",
            OutputFormat::Avi => "avi",
            OutputFormat::Mkv => "mkv",
            OutputFormat::Wmv => "wmv",
            OutputFormat::Flv => "flv",
            OutputFormat::Mpeg => "mpg",
        }
    }

    pub fn mime_type(&self) -> &'static str {
        match self {
            OutputFormat::Png => "image/png",
            OutputFormat::Jpeg => "image/jpeg",
            OutputFormat::Webp => "image/webp",
            OutputFormat::Avif => "image/avif",
            OutputFormat::Gif => "image/gif",
            OutputFormat::Bmp => "image/bmp",
            OutputFormat::Tiff => "image/tiff",
            OutputFormat::Ico => "image/x-icon",
            OutputFormat::Mp3 => "audio/mpeg",
            OutputFormat::Wav => "audio/wav",
            OutputFormat::Flac => "audio/flac",
            OutputFormat::Ogg => "audio/ogg",
            OutputFormat::Aac => "audio/aac",
            OutputFormat::Opus => "audio/opus",
            OutputFormat::Ac3 => "audio/ac3",
            OutputFormat::Aiff => "audio/aiff",
            OutputFormat::Amr => "audio/amr",
            OutputFormat::Wma => "audio/x-ms-wma",
            OutputFormat::Mp4 => "video/mp4",
            OutputFormat::Webm => "video/webm",
            OutputFormat::Mov => "video/quicktime",
            OutputFormat::Avi => "video/x-msvideo",
            OutputFormat::Mkv => "video/x-matroska",
            OutputFormat::Wmv => "video/x-ms-wmv",
            OutputFormat::Flv => "video/x-flv",
            OutputFormat::Mpeg => "video/mpeg",
        }
    }

    pub fn is_image(&self) -> bool {
        matches!(
            self,
            OutputFormat::Png
                | OutputFormat::Jpeg
                | OutputFormat::Webp
                | OutputFormat::Avif
                | OutputFormat::Gif
                | OutputFormat::Bmp
                | OutputFormat::Tiff
                | OutputFormat::Ico
        )
    }

    pub fn is_audio(&self) -> bool {
        matches!(
            self,
            OutputFormat::Mp3
                | OutputFormat::Wav
                | OutputFormat::Flac
                | OutputFormat::Ogg
                | OutputFormat::Aac
                | OutputFormat::Opus
                | OutputFormat::Ac3
                | OutputFormat::Aiff
                | OutputFormat::Amr
                | OutputFormat::Wma
        )
    }

    pub fn is_video(&self) -> bool {
        matches!(
            self,
            OutputFormat::Mp4
                | OutputFormat::Webm
                | OutputFormat::Mov
                | OutputFormat::Avi
                | OutputFormat::Mkv
                | OutputFormat::Wmv
                | OutputFormat::Flv
                | OutputFormat::Mpeg
        )
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
pub struct CropConfig {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrimConfig {
    pub start_ms: Option<u64>,
    pub end_ms: Option<u64>,
    pub duration_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JpegConfig {
    #[serde(default = "default_jpeg_quality")]
    pub quality: u8,
    #[serde(default)]
    pub progressive: bool,
    #[serde(default)]
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
pub struct SvgConfig {
    #[serde(default = "default_true")]
    pub minify: bool,
    #[serde(default = "default_true")]
    pub compress_embedded_images: bool,
    #[serde(default = "default_svg_precision")]
    pub precision: u8,
    #[serde(default)]
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
pub struct CompressionConfig {
    // Input hint (optional MIME type)
    #[serde(default)]
    pub input_hint: Option<String>,

    // Output configuration
    #[serde(default)]
    pub output_format: Option<OutputFormat>,

    // Quality: 1-100, where 100 is best quality
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
    pub chunk_size_mb: u32,
    #[serde(default = "default_true")]
    pub preserve_metadata: bool,
}

fn default_quality() -> u8 {
    80
}

fn default_chunk_size() -> u32 {
    64
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
            chunk_size_mb: 64,
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

        // Validate quality range
        if self.quality == 0 || self.quality > 100 {
            result.add_error("quality", "must be between 1 and 100");
        }

        // Validate chunk size
        if self.chunk_size_mb == 0 {
            result.add_error("chunk_size_mb", "must be greater than 0");
        }

        // Validate resize config
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

        // Validate crop config
        if let Some(ref crop) = self.crop {
            if crop.width == 0 || crop.height == 0 {
                result.add_error("crop", "width and height must be greater than 0");
            }
        }

        // Validate trim config
        if let Some(ref trim) = self.trim {
            if let (Some(start), Some(end)) = (trim.start_ms, trim.end_ms) {
                if start >= end {
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

        // Validate format-specific configs
        if let Some(ref jpeg) = self.jpeg {
            if jpeg.quality == 0 || jpeg.quality > 100 {
                result.add_error("jpeg.quality", "must be between 1 and 100");
            }
        }

        if let Some(ref png) = self.png {
            if png.compression_level > 9 {
                result.add_error("png.compression_level", "must be between 0 and 9");
            }
            if png.max_colors == 0 || png.max_colors > 256 {
                result.add_error("png.max_colors", "must be between 1 and 256");
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
            if gif.max_colors == 0 || gif.max_colors > 256 {
                result.add_error("gif.max_colors", "must be between 1 and 256");
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
        }

        // Validate format compatibility
        if let (Some(input), Some(output)) = (input_format, self.output_format) {
            let input_is_image = matches!(
                input,
                FileFormat::Png
                    | FileFormat::Jpeg
                    | FileFormat::Webp
                    | FileFormat::Avif
                    | FileFormat::Gif
                    | FileFormat::Bmp
                    | FileFormat::Tiff
                    | FileFormat::Ico
                    | FileFormat::Svg
                    | FileFormat::Heic
            );
            let input_is_audio = matches!(
                input,
                FileFormat::Mp3
                    | FileFormat::Wav
                    | FileFormat::Flac
                    | FileFormat::Ogg
                    | FileFormat::Aac
                    | FileFormat::Opus
            );
            let input_is_video = matches!(
                input,
                FileFormat::Mp4
                    | FileFormat::Webm
                    | FileFormat::Mov
                    | FileFormat::Avi
                    | FileFormat::Mkv
            );

            if input_is_image && !output.is_image() {
                result.add_error(
                    "output_format",
                    "cannot convert image to non-image format",
                );
            }
            if input_is_audio && !output.is_audio() && !output.is_video() {
                result.add_error(
                    "output_format",
                    "cannot convert audio to image format",
                );
            }
            if input_is_video && !output.is_video() {
                // Video to audio is allowed (extract audio)
                if !output.is_audio() {
                    result.add_error(
                        "output_format",
                        "video can only be converted to video or audio formats",
                    );
                }
            }

            // Trim only valid for audio/video
            if self.trim.is_some() && input_is_image {
                result.add_warning("trim is ignored for image formats");
            }

            // Resize/crop only valid for images/video
            if (self.resize.is_some() || self.crop.is_some()) && input_is_audio {
                result.add_warning("resize/crop is ignored for audio formats");
            }
        }

        result
    }

    pub fn get_jpeg_config(&self) -> JpegConfig {
        self.jpeg.clone().unwrap_or_else(|| {
            let mut cfg = JpegConfig::default();
            cfg.quality = self.quality;
            cfg
        })
    }

    pub fn get_png_config(&self) -> PngConfig {
        self.png.clone().unwrap_or_else(|| {
            let mut cfg = PngConfig::default();
            cfg.compression_level = quality_to_png_compression(self.quality);
            cfg
        })
    }

    pub fn get_webp_config(&self) -> WebpConfig {
        self.webp.clone().unwrap_or_else(|| {
            let mut cfg = WebpConfig::default();
            cfg.quality = self.quality;
            cfg
        })
    }

    pub fn get_avif_config(&self) -> AvifConfig {
        self.avif.clone().unwrap_or_else(|| {
            let mut cfg = AvifConfig::default();
            cfg.quality = self.quality;
            cfg
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
    // Higher quality = lower compression, but for PNG it's lossless so we invert
    // quality 100 -> compression 9 (best compression)
    // quality 1 -> compression 1 (fastest)
    ((quality as u16 * 9) / 100).clamp(1, 9) as u8
}
