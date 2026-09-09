mod ffmpeg;

pub use ffmpeg::*;

use crate::config::CompressionConfig;
use crate::detection::FileAnalysis;
use crate::error::{CompressionError, Result};

pub async fn compress_audio(
    data: &[u8],
    analysis: &FileAnalysis,
    config: &CompressionConfig,
) -> Result<Vec<u8>> {
    if !is_ffmpeg_available() {
        return Err(CompressionError::FfmpegUnavailable);
    }

    let defaults = crate::config::FfmpegConfig::default();
    let ffmpeg_cfg = config.ffmpeg.as_ref().unwrap_or(&defaults);

    let args = build_audio_args(analysis, config, ffmpeg_cfg)?;

    execute_ffmpeg(data, &args).await
}

pub(crate) fn build_audio_args(
    analysis: &FileAnalysis,
    config: &CompressionConfig,
    ffmpeg_cfg: &crate::config::FfmpegConfig,
) -> Result<Vec<String>> {
    let mut args = Vec::with_capacity(24 + ffmpeg_cfg.extra_flags.len());

    let copy_output = (ffmpeg_cfg.audio_codec.as_deref() == Some("copy")
        && analysis.format.is_audio())
    .then(|| crate::config::OutputFormat::from_file_format(analysis.format))
    .flatten();
    let output = config
        .output_format
        .or(copy_output)
        .unwrap_or(match analysis.format {
            crate::detection::FileFormat::Mp3 => crate::config::OutputFormat::Mp3,
            crate::detection::FileFormat::Ogg => crate::config::OutputFormat::Ogg,
            crate::detection::FileFormat::Opus | crate::detection::FileFormat::Amr => {
                crate::config::OutputFormat::Opus
            }
            _ => crate::config::OutputFormat::Aac,
        });
    let codec = match output {
        crate::config::OutputFormat::Mp3 => "libmp3lame",
        crate::config::OutputFormat::Aac => "aac",
        crate::config::OutputFormat::Opus => "libopus",
        crate::config::OutputFormat::Ogg => "libvorbis",
        crate::config::OutputFormat::Flac => "flac",
        crate::config::OutputFormat::Wav => "pcm_s16le",
        crate::config::OutputFormat::Aiff => "pcm_s16be",
        crate::config::OutputFormat::Ac3 => "ac3",
        crate::config::OutputFormat::Wma => "wmav2",
        crate::config::OutputFormat::Amr => "libopencore_amrnb",
        _ => {
            return Err(CompressionError::InvalidConfig {
                field: "output_format".into(),
                reason: "audio output requires an audio format".into(),
            })
        }
    };
    let codec = ffmpeg_cfg.audio_codec.as_deref().unwrap_or(codec);
    args.extend([
        "-map".into(),
        "0:a:0".into(),
        "-vn".into(),
        "-c:a".into(),
        codec.into(),
    ]);

    if codec == "libopencore_amrnb" {
        // AMR-NB accepts only 8 kHz mono and its small, fixed bitrate table.
        args.extend(["-ar".into(), "8000".into(), "-ac".into(), "1".into()]);
    }

    // Lossless/PCM encoders and stream copy do not use a target bitrate.
    if !matches!(codec, "copy" | "flac") && !codec.starts_with("pcm_") {
        if let Some(ref bitrate) = ffmpeg_cfg.audio_bitrate {
            args.push("-b:a".to_string());
            args.push(bitrate.clone());
        } else if codec == "libvorbis" {
            // Very low fixed bitrates can be rejected for stereo Vorbis inputs.
            args.extend([
                "-q:a".into(),
                (i32::from(config.quality.min(100)) / 10 - 2)
                    .clamp(-1, 8)
                    .to_string(),
            ]);
        } else {
            let bitrate = match codec {
                "libopencore_amrnb" => "12.2k".to_string(),
                "ac3" => format!("{}k", audio_bitrate_kbps(config.quality).max(32)),
                // WMA rejects the generic 16 kb/s default at common sample rates.
                "wmav1" | "wmav2" => format!("{}k", audio_bitrate_kbps(config.quality).max(48)),
                "libopus" if analysis.format == crate::detection::FileFormat::Amr => {
                    "6k".to_string()
                }
                _ => format!("{}k", audio_bitrate_kbps(config.quality)),
            };
            args.push("-b:a".to_string());
            args.push(bitrate);
        }
    }

    append_trim_args(&mut args, config);
    if !config.preserve_metadata {
        args.extend(["-map_metadata".into(), "-1".into()]);
        args.extend(["-map_chapters".into(), "-1".into()]);
    }
    args.extend(["-f".into(), output.extension().into()]);

    for flag in &ffmpeg_cfg.extra_flags {
        args.push(flag.clone());
    }

    Ok(args)
}

fn audio_bitrate_kbps(quality: u8) -> u16 {
    // Default lossy audio bitrate; the dispatcher returns the input if output grows.
    match quality {
        0..=20 => 16,
        21..=40 => 24,
        41..=60 => 32,
        61..=80 => 48,
        81..=95 => 64,
        _ => 96,
    }
}

/// Output-side seeking uses a duration relative to the requested start.
pub(crate) fn append_trim_args(args: &mut Vec<String>, config: &CompressionConfig) {
    if let Some(trim) = &config.trim {
        if let Some(start) = trim.start_ms {
            args.extend(["-ss".into(), milliseconds_to_seconds(start)]);
        }
        if let Some(duration) = trim.duration_ms.or_else(|| {
            trim.end_ms
                .map(|end| end.saturating_sub(trim.start_ms.unwrap_or(0)))
        }) {
            args.extend(["-t".into(), milliseconds_to_seconds(duration)]);
        }
    }
}

fn milliseconds_to_seconds(milliseconds: u64) -> String {
    format!("{}.{:03}", milliseconds / 1000, milliseconds % 1000)
}
