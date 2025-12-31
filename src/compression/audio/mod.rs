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
    // Audio compression requires FFmpeg
    // Check if FFmpeg is available
    if !is_ffmpeg_available() {
        return Err(CompressionError::FfmpegUnavailable);
    }

    let ffmpeg_cfg = config.get_ffmpeg_config();

    // Build FFmpeg arguments
    let args = build_audio_args(analysis, config, &ffmpeg_cfg)?;

    // Execute FFmpeg
    execute_ffmpeg(data, &args).await
}

fn build_audio_args(
    analysis: &FileAnalysis,
    config: &CompressionConfig,
    ffmpeg_cfg: &crate::config::FfmpegConfig,
) -> Result<Vec<String>> {
    let mut args = Vec::new();

    // Input format hint
    args.push("-f".to_string());
    args.push(analysis.format.extension().to_string());

    // Audio codec - convert to lossy format for compression
    if let Some(ref codec) = ffmpeg_cfg.audio_codec {
        args.push("-c:a".to_string());
        args.push(codec.clone());
    } else {
        // Select codec based on output format or input format
        // Prefer lossy codecs for better compression
        let codec = match config.output_format {
            Some(crate::config::OutputFormat::Mp3) => "libmp3lame",
            Some(crate::config::OutputFormat::Aac) => "aac",
            Some(crate::config::OutputFormat::Opus) => "libopus",
            Some(crate::config::OutputFormat::Flac) => "flac",
            Some(crate::config::OutputFormat::Ogg) => "libvorbis",
            Some(crate::config::OutputFormat::Wav) => "aac", // Convert WAV to AAC for compression
            _ => {
                // No output format - use lossy codec for maximum compression
                match analysis.format {
                    crate::detection::FileFormat::Mp3 => "libmp3lame",
                    crate::detection::FileFormat::Aac => "aac",
                    crate::detection::FileFormat::Ogg => "libvorbis",
                    crate::detection::FileFormat::Opus => "libopus",
                    crate::detection::FileFormat::Flac => "aac", // Convert FLAC to AAC
                    crate::detection::FileFormat::Wav => "aac",  // Convert WAV to AAC
                    crate::detection::FileFormat::Wma => "aac",  // Convert WMA to AAC
                    crate::detection::FileFormat::Aiff => "aac", // Convert AIFF to AAC
                    crate::detection::FileFormat::Ac3 => "aac",  // Convert AC3 to AAC
                    crate::detection::FileFormat::Amr => "libopus", // Opus is better at low bitrates
                    _ => "aac",
                }
            }
        };
        args.push("-c:a".to_string());
        args.push(codec.to_string());
    }

    // Audio bitrate - use very aggressive bitrate for already-compressed formats
    if let Some(ref bitrate) = ffmpeg_cfg.audio_bitrate {
        args.push("-b:a".to_string());
        args.push(bitrate.clone());
    } else {
        // For very small files or already-compressed formats, use minimum viable bitrate
        let bitrate = match analysis.format {
            crate::detection::FileFormat::Amr => "6k".to_string(), // Opus minimum for speech
            _ => quality_to_audio_bitrate(config.quality),
        };
        args.push("-b:a".to_string());
        args.push(bitrate);
    }

    // Trimming
    if let Some(ref trim) = config.trim {
        if let Some(start) = trim.start_ms {
            args.push("-ss".to_string());
            args.push(format!("{:.3}", start as f64 / 1000.0));
        }
        if let Some(end) = trim.end_ms {
            args.push("-to".to_string());
            args.push(format!("{:.3}", end as f64 / 1000.0));
        } else if let Some(duration) = trim.duration_ms {
            args.push("-t".to_string());
            args.push(format!("{:.3}", duration as f64 / 1000.0));
        }
    }

    // Output format - use appropriate container for codec
    let output_ext = if config.output_format.is_some() {
        config.output_format.map(|f| f.extension()).unwrap()
    } else {
        // Use m4a for AAC codec, ogg for Opus, otherwise match input format
        match analysis.format {
            crate::detection::FileFormat::Amr => "ogg", // Opus uses ogg container
            crate::detection::FileFormat::Wav
            | crate::detection::FileFormat::Aiff
            | crate::detection::FileFormat::Flac
            | crate::detection::FileFormat::Wma
            | crate::detection::FileFormat::Ac3 => "m4a",
            _ => analysis.format.extension(),
        }
    };
    args.push("-f".to_string());
    // Map extensions to FFmpeg formats
    let ffmpeg_format = match output_ext {
        "m4a" => "ipod",
        "ogg" => "ogg",
        other => other,
    };
    args.push(ffmpeg_format.to_string());

    // Extra flags
    for flag in &ffmpeg_cfg.extra_flags {
        args.push(flag.clone());
    }

    Ok(args)
}

fn quality_to_audio_bitrate(quality: u8) -> String {
    // Map quality 1-100 to bitrate - use very low bitrates for aggressive compression
    // This ensures even already-compressed audio gets reduced
    let bitrate = match quality {
        0..=20 => 16,
        21..=40 => 24,
        41..=60 => 32,
        61..=80 => 48,
        81..=95 => 64,
        _ => 96,
    };
    format!("{}k", bitrate)
}
