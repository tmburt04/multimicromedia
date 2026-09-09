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
    if config.output_format.is_some_and(|format| format.is_audio()) {
        return crate::compression::audio::compress_audio(data, analysis, config).await;
    }
    if !is_ffmpeg_available() {
        return Err(CompressionError::FfmpegUnavailable);
    }

    let defaults = crate::config::FfmpegConfig::default();
    let ffmpeg_cfg = config.ffmpeg.as_ref().unwrap_or(&defaults);

    let args = build_video_args(analysis, config, ffmpeg_cfg)?;

    crate::compression::audio::execute_ffmpeg(data, &args).await
}

pub(crate) fn build_video_args(
    analysis: &FileAnalysis,
    config: &CompressionConfig,
    ffmpeg_cfg: &crate::config::FfmpegConfig,
) -> Result<Vec<String>> {
    let mut args = Vec::with_capacity(48 + ffmpeg_cfg.extra_flags.len());

    // A full remux should retain its container unless the caller requests another.
    let copy_output = (ffmpeg_cfg.video_codec.as_deref() == Some("copy")
        && ffmpeg_cfg.audio_codec.as_deref() == Some("copy"))
    .then(|| crate::config::OutputFormat::from_file_format(analysis.format))
    .flatten();
    let output = config.output_format.or(copy_output).unwrap_or(
        if analysis.format == crate::detection::FileFormat::Webm {
            crate::config::OutputFormat::Webm
        } else {
            crate::config::OutputFormat::Mp4
        },
    );

    if !output.is_video() {
        return Err(CompressionError::InvalidConfig {
            field: "output_format".into(),
            reason: "video output requires a video format".into(),
        });
    }
    let (default_video, default_audio) = match output {
        crate::config::OutputFormat::Webm => ("libvpx-vp9", "libopus"),
        crate::config::OutputFormat::Avi => ("mpeg4", "libmp3lame"),
        crate::config::OutputFormat::Wmv => ("wmv2", "wmav2"),
        crate::config::OutputFormat::Flv => ("flv", "libmp3lame"),
        crate::config::OutputFormat::Mpeg => ("mpeg2video", "mp2"),
        _ => ("libx264", "aac"),
    };
    let codec = ffmpeg_cfg.video_codec.as_deref().unwrap_or(default_video);
    let is_vp9 = codec == "libvpx-vp9";
    let is_av1 = codec == "libaom-av1";
    let is_x26x = matches!(codec, "libx264" | "libx265");
    let stream_copy = codec == "copy";
    if stream_copy
        && config
            .trim
            .as_ref()
            .and_then(|trim| trim.start_ms)
            .is_some_and(|start| start > 0)
    {
        return Err(CompressionError::InvalidConfig {
            field: "trim.start_ms".into(),
            reason: "nonzero video trim starts require re-encoding; stream copy cannot guarantee a decodable keyframe at the requested start".into(),
        });
    }
    if stream_copy && (config.resize.is_some() || config.crop.is_some()) {
        return Err(CompressionError::InvalidConfig {
            field: "ffmpeg.video_codec".into(),
            reason: "stream copy cannot apply resize or crop filters".into(),
        });
    }
    if is_x26x && ffmpeg_cfg.crf.is_some_and(|crf| crf > 51) {
        return Err(CompressionError::InvalidConfig {
            field: "ffmpeg.crf".into(),
            reason: "H.264/H.265 CRF must be between 0 and 51".into(),
        });
    }
    if ffmpeg_cfg.crf.is_some()
        && matches!(
            codec,
            "copy" | "mpeg4" | "wmv2" | "flv" | "flv1" | "mpeg1video" | "mpeg2video"
        )
    {
        return Err(CompressionError::InvalidConfig {
            field: "ffmpeg.crf".into(),
            reason: format!("codec {codec} does not support CRF; use video_bitrate or quality"),
        });
    }
    args.extend([
        "-map".into(),
        "0:V:0?".into(),
        "-map".into(),
        "0:a:0?".into(),
        "-c:v".into(),
        codec.into(),
    ]);

    if !stream_copy {
        // Pixel format for compatibility
        args.push("-pix_fmt".to_string());
        args.push("yuv420p".to_string());

        if let Some(crf) = ffmpeg_cfg.crf {
            args.push("-crf".to_string());
            args.push(crf.to_string());
        } else if let Some(ref bitrate) = ffmpeg_cfg.video_bitrate {
            args.push("-b:v".to_string());
            args.push(bitrate.clone());
        } else if is_vp9 || is_x26x || is_av1 {
            let crf = quality_to_crf(config.quality);
            args.push("-crf".to_string());
            args.push(crf.to_string());
        } else {
            args.extend([
                "-q:v".into(),
                (32 - u32::from(config.quality.min(100)) * 31 / 100)
                    .clamp(1, 31)
                    .to_string(),
            ]);
        }

        if ffmpeg_cfg.crf.is_some() {
            if let Some(bitrate) = &ffmpeg_cfg.video_bitrate {
                args.extend(["-b:v".into(), bitrate.clone()]);
            }
        }
        if (is_vp9 || is_av1) && ffmpeg_cfg.video_bitrate.is_none() {
            // libvpx otherwise constrains CRF output to its default target bitrate.
            args.extend(["-b:v".into(), "0".into()]);
        }

        // Speed controls depend on the selected encoder.
        if is_vp9 {
            args.push("-deadline".to_string());
            args.push("realtime".to_string());
            args.push("-cpu-used".to_string());
            args.push("5".to_string());
        } else if is_av1 {
            args.extend(["-cpu-used".into(), "6".into()]);
        } else if is_x26x {
            args.extend([
                "-preset".into(),
                ffmpeg_cfg.preset.as_deref().unwrap_or("fast").into(),
            ]);
        }
    }

    let audio_codec = ffmpeg_cfg.audio_codec.as_deref().unwrap_or(default_audio);
    args.extend(["-c:a".into(), audio_codec.into()]);

    // Lossless/PCM encoders and stream copy do not use a target bitrate.
    if !matches!(audio_codec, "copy" | "flac") && !audio_codec.starts_with("pcm_") {
        args.extend([
            "-b:a".into(),
            ffmpeg_cfg.audio_bitrate.as_deref().unwrap_or("64k").into(),
        ]);
    }

    let mut filters = Vec::new();
    if let Some(crop) = &config.crop {
        filters.push(format!(
            "crop={}:{}:{}:{}",
            crop.width, crop.height, crop.x, crop.y
        ));
    }
    if let Some(resize) = &config.resize {
        let scale = match (resize.width, resize.height) {
            (Some(w), Some(h))
                if resize.preserve_aspect && resize.mode != crate::config::ResizeMode::Exact =>
            {
                if matches!(
                    resize.mode,
                    crate::config::ResizeMode::Fill | crate::config::ResizeMode::Cover
                ) {
                    format!("scale={w}:{h}:force_original_aspect_ratio=increase,crop={w}:{h}")
                } else {
                    format!(
                        "scale={w}:{h}:force_original_aspect_ratio=decrease:force_divisible_by=2"
                    )
                }
            }
            (Some(w), Some(h)) => format!("scale={w}:{h}"),
            (Some(w), None) if resize.preserve_aspect => format!("scale={w}:-2"),
            (None, Some(h)) if resize.preserve_aspect => format!("scale=-2:{h}"),
            (Some(w), None) => format!("scale={w}:ih"),
            (None, Some(h)) => format!("scale=iw:{h}"),
            _ => String::new(),
        };
        if !scale.is_empty() {
            filters.push(scale);
            if !resize.preserve_aspect
                || (resize.mode == crate::config::ResizeMode::Exact
                    && resize.width.is_some()
                    && resize.height.is_some())
            {
                // scale otherwise changes sample aspect ratio to retain the
                // original display proportions, undoing an explicit stretch.
                filters.push("setsar=1".into());
            }
        }
    }
    if !stream_copy {
        // Also covers odd crop and exact-resize sizes required by yuv420p.
        filters.push("pad=ceil(iw/2)*2:ceil(ih/2)*2".into());
    }
    if !filters.is_empty() {
        args.extend(["-vf".into(), filters.join(",")]);
    }
    crate::compression::audio::append_trim_args(&mut args, config);
    if !config.preserve_metadata {
        args.extend(["-map_metadata".into(), "-1".into()]);
        args.extend(["-map_chapters".into(), "-1".into()]);
    }

    let output_ext = output.extension();
    args.push("-f".to_string());
    args.push(output_ext.to_string());

    // Move the MP4/MOV index ahead of media data for progressive playback.
    if output_ext == "mp4" || output_ext == "mov" {
        args.push("-movflags".to_string());
        args.push("+faststart".to_string());
    }

    for flag in &ffmpeg_cfg.extra_flags {
        args.push(flag.clone());
    }

    Ok(args)
}

fn quality_to_crf(quality: u8) -> u8 {
    // Map quality 1-100 to CRF (lower CRF = higher quality)

    match quality {
        0..=20 => 38,
        21..=40 => 34,
        41..=60 => 30,
        61..=80 => 26,
        81..=95 => 23,
        _ => 20,
    }
}
