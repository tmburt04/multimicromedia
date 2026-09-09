pub mod analysis;
pub mod chunking;
pub mod compression;
pub mod config;
pub mod detection;
pub mod error;
pub mod result;
pub mod storage;
pub mod validation;

use config::CompressionConfig;
use detection::{FileFormat, MimeMapping};
use error::CompressionError;
use result::CompressionResult;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn init_panic_hook() {
    console_error_panic_hook::set_once();
}

#[wasm_bindgen]
pub fn detect_file_type(data: &[u8]) -> String {
    let format = FileFormat::detect(data);
    format.extension().to_string()
}

#[wasm_bindgen]
pub fn detect_file_mime(data: &[u8]) -> String {
    let format = FileFormat::detect(data);
    format.mime_type().to_string()
}

#[wasm_bindgen]
pub fn validate_config(config_json: &str) -> Result<JsValue, JsValue> {
    init_panic_hook();

    let config = CompressionConfig::from_json(config_json)?;
    let result = config.validate(None);

    serde_json::to_string(&result)
        .map(|s| s.into())
        .map_err(|e| {
            CompressionError::InvalidConfig {
                field: "json".to_string(),
                reason: e.to_string(),
            }
            .into()
        })
}

#[wasm_bindgen]
pub fn validate_config_for_file(config_json: &str, data: &[u8]) -> Result<JsValue, JsValue> {
    init_panic_hook();

    let config = CompressionConfig::from_json(config_json)?;
    let analysis = analysis::analyze_file(data)?;
    let result = validation::validate_config_for_input(&config, &analysis);

    serde_json::to_string(&result)
        .map(|s| s.into())
        .map_err(|e| {
            CompressionError::InvalidConfig {
                field: "json".to_string(),
                reason: e.to_string(),
            }
            .into()
        })
}

#[wasm_bindgen]
pub async fn compress(data: &[u8], config_json: &str) -> Result<CompressionResult, JsValue> {
    init_panic_hook();

    let config = CompressionConfig::from_json(config_json)?;

    // Reject invalid settings before scanning headers or an animated file's frames.
    if let Some(error) = config.validate(None).errors.into_iter().next() {
        return Err(CompressionError::InvalidConfig {
            field: error.field,
            reason: error.message,
        }
        .into());
    }

    let analysis = analysis::analyze_file(data)?;

    compression::compress(data, &analysis, &config)
        .await
        .map_err(|e| e.into())
}

#[wasm_bindgen]
pub async fn compress_with_defaults(data: &[u8]) -> Result<CompressionResult, JsValue> {
    init_panic_hook();

    let config = CompressionConfig::default();
    let analysis = analysis::analyze_file(data)?;

    compression::compress(data, &analysis, &config)
        .await
        .map_err(|e| e.into())
}

#[wasm_bindgen]
pub fn analyze_file(data: &[u8]) -> Result<JsValue, JsValue> {
    init_panic_hook();

    let analysis = analysis::analyze_file(data)?;

    let obj = js_sys::Object::new();
    let _ = js_sys::Reflect::set(&obj, &"format".into(), &analysis.format.extension().into());
    let _ = js_sys::Reflect::set(&obj, &"mime".into(), &analysis.format.mime_type().into());
    let _ = js_sys::Reflect::set(&obj, &"size".into(), &(analysis.size as f64).into());

    if let Some(w) = analysis.width {
        let _ = js_sys::Reflect::set(&obj, &"width".into(), &(w as f64).into());
    }
    if let Some(h) = analysis.height {
        let _ = js_sys::Reflect::set(&obj, &"height".into(), &(h as f64).into());
    }
    if let Some(alpha) = analysis.has_alpha {
        let _ = js_sys::Reflect::set(&obj, &"hasAlpha".into(), &alpha.into());
    }
    if let Some(animated) = analysis.is_animated {
        let _ = js_sys::Reflect::set(&obj, &"isAnimated".into(), &animated.into());
    }
    if let Some(frames) = analysis.frame_count {
        let _ = js_sys::Reflect::set(&obj, &"frameCount".into(), &(frames as f64).into());
    }
    if let Some(duration) = analysis.duration_ms {
        let _ = js_sys::Reflect::set(&obj, &"durationMs".into(), &(duration as f64).into());
    }
    if let Some(depth) = analysis.bit_depth {
        let _ = js_sys::Reflect::set(&obj, &"bitDepth".into(), &(depth as f64).into());
    }
    if analysis.has_embedded_images {
        let _ = js_sys::Reflect::set(&obj, &"hasEmbeddedImages".into(), &true.into());
        let _ = js_sys::Reflect::set(
            &obj,
            &"embeddedImageCount".into(),
            &(analysis.embedded_image_count as f64).into(),
        );
    }

    Ok(obj.into())
}

#[wasm_bindgen]
pub fn get_supported_formats() -> JsValue {
    let obj = js_sys::Object::new();

    // Image formats
    let images = js_sys::Array::new();
    for format in MimeMapping::image_formats() {
        let fmt = js_sys::Object::new();
        let _ = js_sys::Reflect::set(&fmt, &"extension".into(), &format.extension().into());
        let _ = js_sys::Reflect::set(&fmt, &"mime".into(), &format.mime_type().into());
        images.push(&fmt);
    }
    let _ = js_sys::Reflect::set(&obj, &"images".into(), &images);

    // Audio formats
    let audio = js_sys::Array::new();
    for format in MimeMapping::audio_formats() {
        let fmt = js_sys::Object::new();
        let _ = js_sys::Reflect::set(&fmt, &"extension".into(), &format.extension().into());
        let _ = js_sys::Reflect::set(&fmt, &"mime".into(), &format.mime_type().into());
        audio.push(&fmt);
    }
    let _ = js_sys::Reflect::set(&obj, &"audio".into(), &audio);

    // Video formats
    let video = js_sys::Array::new();
    for format in MimeMapping::video_formats() {
        let fmt = js_sys::Object::new();
        let _ = js_sys::Reflect::set(&fmt, &"extension".into(), &format.extension().into());
        let _ = js_sys::Reflect::set(&fmt, &"mime".into(), &format.mime_type().into());
        video.push(&fmt);
    }
    let _ = js_sys::Reflect::set(&obj, &"video".into(), &video);

    obj.into()
}

#[wasm_bindgen]
pub fn estimate_output_size(input_size: f64, config_json: &str) -> f64 {
    if !input_size.is_finite() || input_size < 0.0 {
        return 0.0;
    }
    let config = match CompressionConfig::from_json(config_json) {
        Ok(c) => c,
        Err(_) => return input_size,
    };

    // Legacy heuristic; coefficients do not model current encoder behavior or availability.
    let quality = config.quality as f64 / 100.0;
    let base_ratio = match config.output_format {
        Some(config::OutputFormat::Png) => 0.9,
        Some(config::OutputFormat::Jpeg) => 0.3 + (quality * 0.5),
        Some(config::OutputFormat::Webp) => 0.2 + (quality * 0.4),
        Some(config::OutputFormat::Avif) => 0.15 + (quality * 0.35),
        Some(config::OutputFormat::Gif) => 0.8,
        Some(config::OutputFormat::Mp3) => 0.1 + (quality * 0.15),
        Some(config::OutputFormat::Aac) => 0.08 + (quality * 0.12),
        Some(config::OutputFormat::Opus) => 0.05 + (quality * 0.1),
        Some(config::OutputFormat::Mp4) => 0.3 + (quality * 0.4),
        Some(config::OutputFormat::Webm) => 0.25 + (quality * 0.35),
        _ => 0.7,
    };

    (input_size * base_ratio).floor().min(input_size)
}

#[wasm_bindgen]
pub fn get_default_config() -> String {
    serde_json::to_string_pretty(&CompressionConfig::default()).unwrap_or_default()
}

#[wasm_bindgen]
pub fn create_config() -> CompressionConfigBuilder {
    CompressionConfigBuilder::new()
}

#[wasm_bindgen]
pub struct CompressionConfigBuilder {
    config: CompressionConfig,
}

#[wasm_bindgen]
impl CompressionConfigBuilder {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            config: CompressionConfig::default(),
        }
    }

    #[wasm_bindgen]
    pub fn quality(mut self, quality: u8) -> Self {
        self.config.quality = quality.clamp(1, 100);
        self
    }

    #[wasm_bindgen]
    pub fn output_format(mut self, format: &str) -> Result<Self, JsValue> {
        let detected = FileFormat::from_extension(format);
        if matches!(
            detected,
            FileFormat::Heic | FileFormat::Svg | FileFormat::Unknown
        ) {
            return Err(CompressionError::InvalidConfig {
                field: "output_format".into(),
                reason: format!("unsupported output format '{format}'"),
            }
            .into());
        }
        self.config.output_format = Some(
            config::OutputFormat::from_file_format(detected).ok_or_else(|| {
                CompressionError::InvalidConfig {
                    field: "output_format".into(),
                    reason: format!("unsupported format '{format}'"),
                }
            })?,
        );
        Ok(self)
    }

    #[wasm_bindgen]
    pub fn resize(mut self, width: Option<u32>, height: Option<u32>) -> Self {
        if width.is_some() || height.is_some() {
            self.config.resize = Some(config::ResizeConfig {
                width,
                height,
                mode: config::ResizeMode::Fit,
                preserve_aspect: true,
            });
        }
        self
    }

    #[wasm_bindgen]
    pub fn crop(mut self, x: u32, y: u32, width: u32, height: u32) -> Self {
        self.config.crop = Some(config::CropConfig {
            x,
            y,
            width,
            height,
        });
        self
    }

    #[wasm_bindgen]
    pub fn trim(mut self, start_ms: Option<u64>, end_ms: Option<u64>) -> Self {
        self.config.trim = Some(config::TrimConfig {
            start_ms,
            end_ms,
            duration_ms: None,
        });
        self
    }

    #[wasm_bindgen]
    pub fn preserve_metadata(mut self, preserve: bool) -> Self {
        self.config.preserve_metadata = preserve;
        self
    }

    #[wasm_bindgen]
    pub fn build(self) -> String {
        serde_json::to_string(&self.config).unwrap_or_default()
    }
}

impl Default for CompressionConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[wasm_bindgen]
pub fn is_ffmpeg_available() -> bool {
    compression::audio::is_ffmpeg_available()
}

#[wasm_bindgen]
pub fn get_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// Returns config schema for UI form generation.
/// Optionally pass file data to get format-specific options.
#[wasm_bindgen]
pub fn get_config_schema(data: Option<Vec<u8>>) -> JsValue {
    let format = data
        .as_ref()
        .map(|d| FileFormat::detect(d))
        .unwrap_or(FileFormat::Unknown);

    let is_image = format.is_image() || format == FileFormat::Unknown;
    let is_audio = format.is_audio() || format == FileFormat::Unknown;
    let is_video = format.is_video() || format == FileFormat::Unknown;

    let schema = js_sys::Array::new();

    // Quality - applies to all
    let quality = build_field(
        "quality",
        "number",
        "Quality",
        "Compression quality (1-100, higher = better quality)",
        &serde_json::json!(80),
        Some(&serde_json::json!({"min": 1, "max": 100})),
        &["image", "audio", "video"],
    );
    schema.push(&quality);

    // Output format
    let formats = get_output_formats(format);
    let output_format = build_field(
        "output_format",
        "select",
        "Output Format",
        "Convert to a different format",
        &serde_json::json!(null),
        Some(&serde_json::json!({"options": formats})),
        &["image", "audio", "video"],
    );
    schema.push(&output_format);

    // Raster/video transforms
    if (is_image || is_video) && format != FileFormat::Svg {
        // Resize
        let resize_width = build_field(
            "resize.width",
            "number",
            "Resize Width",
            "Target width in pixels",
            &serde_json::json!(null),
            Some(&serde_json::json!({"min": 1, "max": 16384})),
            &["image", "video"],
        );
        schema.push(&resize_width);

        let resize_height = build_field(
            "resize.height",
            "number",
            "Resize Height",
            "Target height in pixels",
            &serde_json::json!(null),
            Some(&serde_json::json!({"min": 1, "max": 16384})),
            &["image", "video"],
        );
        schema.push(&resize_height);

        let resize_mode = build_field(
            "resize.mode",
            "select",
            "Resize Mode",
            "How to handle aspect ratio",
            &serde_json::json!("fit"),
            Some(&serde_json::json!({"options": ["fit", "fill", "exact", "cover"]})),
            &["image", "video"],
        );
        schema.push(&resize_mode);
    }

    if is_image {
        // PNG options
        let png_quantize = build_field(
            "png.quantize",
            "boolean",
            "Quantize PNG",
            "Reduce colors for smaller files (lossy)",
            &serde_json::json!(false),
            None,
            &["image"],
        );
        schema.push(&png_quantize);

        let png_compression = build_field(
            "png.compression_level",
            "number",
            "PNG Compression",
            "Compression level (0-9, higher = smaller)",
            &serde_json::json!(6),
            Some(&serde_json::json!({"min": 0, "max": 9})),
            &["image"],
        );
        schema.push(&png_compression);

        // SVG options
        let svg_minify = build_field(
            "svg.minify",
            "boolean",
            "Minify SVG",
            "Remove comments while preserving meaningful text and whitespace",
            &serde_json::json!(true),
            None,
            &["image"],
        );
        schema.push(&svg_minify);
    }

    // Audio/Video trimming
    if is_audio || is_video {
        let trim_start = build_field(
            "trim.start_ms",
            "number",
            "Trim Start (ms)",
            "Start time in milliseconds",
            &serde_json::json!(null),
            Some(&serde_json::json!({"min": 0})),
            &["audio", "video"],
        );
        schema.push(&trim_start);

        let trim_end = build_field(
            "trim.end_ms",
            "number",
            "Trim End (ms)",
            "End time in milliseconds",
            &serde_json::json!(null),
            Some(&serde_json::json!({"min": 0})),
            &["audio", "video"],
        );
        schema.push(&trim_end);
    }

    // FFmpeg options
    if is_audio || is_video {
        let audio_bitrate = build_field(
            "ffmpeg.audio_bitrate",
            "select",
            "Audio Bitrate",
            "Target audio bitrate",
            &serde_json::json!(null),
            Some(&serde_json::json!({"options": ["64k", "96k", "128k", "192k", "256k", "320k"]})),
            &["audio", "video"],
        );
        schema.push(&audio_bitrate);
    }

    if is_video {
        let video_bitrate = build_field(
            "ffmpeg.video_bitrate",
            "text",
            "Video Bitrate",
            "Target video bitrate (e.g., 1M, 2M, 5M)",
            &serde_json::json!(null),
            None,
            &["video"],
        );
        schema.push(&video_bitrate);

        let crf = build_field(
            "ffmpeg.crf",
            "number",
            "CRF (Quality)",
            "Constant Rate Factor (0-51, lower = better)",
            &serde_json::json!(null),
            Some(&serde_json::json!({"min": 0, "max": 51})),
            &["video"],
        );
        schema.push(&crf);

        let preset = build_field(
            "ffmpeg.preset",
            "select",
            "Encoding Speed",
            "Faster = larger file, slower = smaller file",
            &serde_json::json!("fast"),
            Some(
                &serde_json::json!({"options": ["ultrafast", "superfast", "veryfast", "faster", "fast", "medium", "slow", "slower", "veryslow"]}),
            ),
            &["video"],
        );
        schema.push(&preset);
    }

    // Metadata
    let preserve_metadata = build_field(
        "preserve_metadata",
        "boolean",
        "Preserve Metadata",
        "Preserve metadata where supported by the selected encoder",
        &serde_json::json!(true),
        None,
        &["image", "audio", "video"],
    );
    schema.push(&preserve_metadata);

    schema.into()
}

fn build_field(
    name: &str,
    field_type: &str,
    label: &str,
    description: &str,
    default: &serde_json::Value,
    constraints: Option<&serde_json::Value>,
    applies_to: &[&str],
) -> JsValue {
    let obj = js_sys::Object::new();
    let _ = js_sys::Reflect::set(&obj, &"name".into(), &name.into());
    let _ = js_sys::Reflect::set(&obj, &"type".into(), &field_type.into());
    let _ = js_sys::Reflect::set(&obj, &"label".into(), &label.into());
    let _ = js_sys::Reflect::set(&obj, &"description".into(), &description.into());

    let default_val = match default {
        serde_json::Value::Null => JsValue::NULL,
        serde_json::Value::Bool(b) => JsValue::from(*b),
        serde_json::Value::Number(n) => JsValue::from(n.as_f64().unwrap_or(0.0)),
        serde_json::Value::String(s) => JsValue::from(s.as_str()),
        _ => JsValue::NULL,
    };
    let _ = js_sys::Reflect::set(&obj, &"default".into(), &default_val);

    if let Some(c) = constraints {
        if let Some(min) = c.get("min") {
            let _ = js_sys::Reflect::set(
                &obj,
                &"min".into(),
                &JsValue::from(min.as_f64().unwrap_or(0.0)),
            );
        }
        if let Some(max) = c.get("max") {
            let _ = js_sys::Reflect::set(
                &obj,
                &"max".into(),
                &JsValue::from(max.as_f64().unwrap_or(100.0)),
            );
        }
        if let Some(options) = c.get("options") {
            let arr = js_sys::Array::new();
            if let Some(opts) = options.as_array() {
                for opt in opts {
                    if let Some(s) = opt.as_str() {
                        arr.push(&s.into());
                    }
                }
            }
            let _ = js_sys::Reflect::set(&obj, &"options".into(), &arr);
        }
    }

    let applies = js_sys::Array::new();
    for a in applies_to {
        applies.push(&(*a).into());
    }
    let _ = js_sys::Reflect::set(&obj, &"appliesTo".into(), &applies);

    obj.into()
}

fn get_output_formats(input: FileFormat) -> Vec<&'static str> {
    MimeMapping::image_formats()
        .into_iter()
        .chain(MimeMapping::audio_formats())
        .chain(MimeMapping::video_formats())
        .filter(|format| {
            !matches!(
                format,
                FileFormat::Svg | FileFormat::Avif | FileFormat::Heic
            )
        })
        .filter(|format| {
            input == FileFormat::Unknown
                || validation::validate_format_conversion(input, *format).valid
        })
        .map(|format| format.extension())
        .collect()
}
