mod ffmpeg;

pub use ffmpeg::*;

use crate::compression::audio::is_ffmpeg_available;
use crate::config::CompressionConfig;
use crate::detection::FileAnalysis;
use crate::error::{CompressionError, Result};

pub async fn compress_video(
    data: &[u8],
    analysis: &FileAnalysis,
    config: &CompressionConfig,
) -> Result<Vec<u8>> {
    // Video compression requires FFmpeg
    if !is_ffmpeg_available() {
        return Err(CompressionError::FfmpegUnavailable);
    }

    let ffmpeg_cfg = config.get_ffmpeg_config();

    // Build FFmpeg arguments
    let args = build_video_args(analysis, config, &ffmpeg_cfg)?;

    // Execute FFmpeg
    crate::compression::audio::execute_ffmpeg(data, &args).await
}

fn build_video_args(
    analysis: &FileAnalysis,
    config: &CompressionConfig,
    ffmpeg_cfg: &crate::config::FfmpegConfig,
) -> Result<Vec<String>> {
    let mut args = Vec::new();

    // Input format hint
    args.push("-f".to_string());
    args.push(analysis.format.extension().to_string());

    // Video codec - use H.264 for maximum compression (best ratio)
    let is_vp9 = if let Some(ref codec) = ffmpeg_cfg.video_codec {
        args.push("-c:v".to_string());
        args.push(codec.clone());
        codec == "libvpx-vp9"
    } else {
        // Use libx264 for most formats (best compression)
        // Only use VP9 for WebM (required for container compatibility)
        let codec = match config.output_format {
            Some(crate::config::OutputFormat::Webm) => "libvpx-vp9",
            _ => {
                match analysis.format {
                    crate::detection::FileFormat::Webm => "libvpx-vp9",
                    // All other formats: use H.264 for best compression
                    _ => "libx264",
                }
            }
        };
        args.push("-c:v".to_string());
        args.push(codec.to_string());
        codec == "libvpx-vp9"
    };

    // Pixel format for compatibility
    args.push("-pix_fmt".to_string());
    args.push("yuv420p".to_string());

    // Video bitrate or CRF
    if let Some(crf) = ffmpeg_cfg.crf {
        args.push("-crf".to_string());
        args.push(crf.to_string());
    } else if let Some(ref bitrate) = ffmpeg_cfg.video_bitrate {
        args.push("-b:v".to_string());
        args.push(bitrate.clone());
    } else {
        // Default CRF based on quality
        let crf = quality_to_crf(config.quality);
        args.push("-crf".to_string());
        args.push(crf.to_string());
    }

    // Preset (x264 only) or deadline (VP9)
    if is_vp9 {
        args.push("-deadline".to_string());
        args.push("realtime".to_string());
        args.push("-cpu-used".to_string());
        args.push("5".to_string());
    } else if let Some(ref preset) = ffmpeg_cfg.preset {
        args.push("-preset".to_string());
        args.push(preset.clone());
    } else {
        args.push("-preset".to_string());
        args.push("fast".to_string());
    }

    // Audio codec - use AAC for MP4, Opus for WebM
    if let Some(ref codec) = ffmpeg_cfg.audio_codec {
        args.push("-c:a".to_string());
        args.push(codec.clone());
    } else {
        let codec = if is_vp9 { "libopus" } else { "aac" };
        args.push("-c:a".to_string());
        args.push(codec.to_string());
    }

    // Audio bitrate - use lower for better compression
    if let Some(ref bitrate) = ffmpeg_cfg.audio_bitrate {
        args.push("-b:a".to_string());
        args.push(bitrate.clone());
    } else {
        args.push("-b:a".to_string());
        args.push("64k".to_string());
    }

    // Resize
    if let Some(ref resize) = config.resize {
        let scale = match (resize.width, resize.height) {
            (Some(w), Some(h)) => format!("scale={}:{}", w, h),
            (Some(w), None) => format!("scale={}:-2", w),
            (None, Some(h)) => format!("scale=-2:{}", h),
            (None, None) => String::new(),
        };
        if !scale.is_empty() {
            args.push("-vf".to_string());
            args.push(scale);
        }
    }

    // Crop
    if let Some(ref crop) = config.crop {
        let crop_filter = format!(
            "crop={}:{}:{}:{}",
            crop.width, crop.height, crop.x, crop.y
        );
        // If we already have a video filter, chain it
        if let Some(pos) = args.iter().position(|a| a == "-vf") {
            let existing = args[pos + 1].clone();
            args[pos + 1] = format!("{},{}", existing, crop_filter);
        } else {
            args.push("-vf".to_string());
            args.push(crop_filter);
        }
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

    // Output format - use MP4 for H.264 codec (best compatibility)
    let output_ext = if config.output_format.is_some() {
        config.output_format.map(|f| f.extension()).unwrap()
    } else {
        match analysis.format {
            crate::detection::FileFormat::Webm => "webm",
            // All H.264 encoded video goes to MP4
            _ => "mp4",
        }
    };
    args.push("-f".to_string());
    args.push(output_ext.to_string());

    // Avoid buffering issues (for MP4/MOV containers)
    if output_ext == "mp4" || output_ext == "mov" {
        args.push("-movflags".to_string());
        args.push("+faststart".to_string());
    }

    // Extra flags
    for flag in &ffmpeg_cfg.extra_flags {
        args.push(flag.clone());
    }

    Ok(args)
}

fn quality_to_crf(quality: u8) -> u8 {
    // Map quality 1-100 to CRF (lower CRF = higher quality)
    // CRF range: 0 (lossless) to 51 (worst)
    // Use higher CRF for more aggressive compression
    let crf = match quality {
        0..=20 => 38,
        21..=40 => 34,
        41..=60 => 30,
        61..=80 => 26,
        81..=95 => 23,
        _ => 20,
    };
    crf
}
