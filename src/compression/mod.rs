pub mod audio;
pub mod image;
pub mod video;

use crate::config::{CompressionConfig, OutputFormat};
use crate::detection::{FileAnalysis, FileFormat};
use crate::error::{CompressionError, Result};
use crate::result::{CompressionResult, CompressionStats};

pub async fn compress(
    data: &[u8],
    analysis: &FileAnalysis,
    config: &CompressionConfig,
) -> Result<CompressionResult> {
    if data.is_empty() {
        return Err(CompressionError::InvalidInput {
            reason: "empty file".into(),
        });
    }
    let validation = crate::validation::validate_config_for_input(config, analysis);
    if !validation.valid {
        return Err(CompressionError::InvalidConfig {
            field: validation
                .errors
                .first()
                .map(|error| error.field.clone())
                .unwrap_or_else(|| "config".into()),
            reason: validation
                .errors
                .iter()
                .map(|error| format!("{}: {}", error.field, error.message))
                .collect::<Vec<_>>()
                .join("; "),
        });
    }
    let start = js_sys::Date::now();
    let original_size = data.len() as u64;

    let output_format = config.output_format.or_else(|| {
        if analysis.format == FileFormat::Avif {
            Some(OutputFormat::Webp)
        } else {
            OutputFormat::from_file_format(analysis.format)
        }
    });

    let compressed = match analysis.format {
        // Image formats
        FileFormat::Png => image::compress_png(data, config).await?,
        FileFormat::Jpeg => image::compress_jpeg(data, config, output_format).await?,
        FileFormat::Webp => image::compress_webp(data, config, output_format).await?,
        FileFormat::Gif => image::compress_gif(data, analysis, config).await?,
        FileFormat::Bmp | FileFormat::Tiff | FileFormat::Ico => {
            image::compress_generic(data, config, output_format).await?
        }
        FileFormat::Svg => image::compress_svg(data, config).await?,
        FileFormat::Avif => image::compress_avif(data, config, output_format).await?,
        FileFormat::Heic => image::compress_heic(data, config, output_format).await?,

        // Audio formats
        FileFormat::Mp3
        | FileFormat::Wav
        | FileFormat::Flac
        | FileFormat::Ogg
        | FileFormat::Aac
        | FileFormat::Opus
        | FileFormat::Ac3
        | FileFormat::Aiff
        | FileFormat::Amr
        | FileFormat::Wma => audio::compress_audio(data, analysis, config).await?,

        // Video formats
        FileFormat::Mp4
        | FileFormat::Webm
        | FileFormat::Mov
        | FileFormat::Avi
        | FileFormat::Mkv
        | FileFormat::Wmv
        | FileFormat::Flv
        | FileFormat::Mpeg => video::compress_video(data, analysis, config).await?,

        FileFormat::Unknown => {
            return Err(CompressionError::UnsupportedFormat {
                detected: "unknown".to_string(),
                fallback: None,
            });
        }
    };

    // Enforce the size contract for every handler, including external bridges.
    if compressed.is_empty() {
        return Err(CompressionError::EncodeFailed {
            format: analysis.format.extension().into(),
            detail: "encoder returned empty output".into(),
        });
    }
    let compressed = if compressed.len() > data.len() {
        data.to_vec()
    } else {
        compressed
    };
    let time_ms = (js_sys::Date::now() - start) as u64;
    let compressed_size = compressed.len() as u64;

    let format_out = FileFormat::detect(&compressed).extension();

    let stats = CompressionStats::new(
        original_size,
        compressed_size,
        time_ms,
        analysis.format.extension(),
        format_out,
    );

    let unchanged = compressed == data;
    Ok(CompressionResult::new(compressed, stats, unchanged))
}

pub fn select_best_output(
    candidates: Vec<Vec<u8>>,
    original_size: usize,
    threshold_pct: f64,
) -> Vec<u8> {
    candidates
        .into_iter()
        .filter(|candidate| !candidate.is_empty() && candidate.len() <= original_size)
        .min_by_key(Vec::len)
        .filter(|best| {
            original_size > 0
                && (original_size - best.len()) as f64 * 100.0 / original_size as f64
                    >= threshold_pct
        })
        .unwrap_or_default()
}
