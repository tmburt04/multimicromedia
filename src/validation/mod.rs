use crate::config::CompressionConfig;
use crate::detection::{FileAnalysis, FileFormat};
use crate::result::ValidationResult;

pub fn validate_config_for_input(
    config: &CompressionConfig,
    analysis: &FileAnalysis,
) -> ValidationResult {
    let mut result = config.validate(Some(analysis.format));

    if analysis.size == 0 || analysis.format == FileFormat::Unknown {
        result.add_error("input", "input is empty or its media format is unknown");
    }
    if analysis.format.is_image() && analysis.format != FileFormat::Svg {
        if let (Some(resize), Some(width), Some(height)) =
            (&config.resize, analysis.width, analysis.height)
        {
            let (width, height) = config
                .crop
                .as_ref()
                .map_or((width, height), |crop| (crop.width, crop.height));
            if width > 0 && height > 0 {
                let workspace = crate::compression::image::calculate_resize_dimensions(
                    width,
                    height,
                    resize.width,
                    resize.height,
                    resize.preserve_aspect,
                    resize.mode,
                )
                .and_then(|(w, h)| {
                    crate::compression::image::validate_resize_workspace(
                        width,
                        height,
                        w.max(1),
                        h.max(1),
                        1,
                    )
                });
                if workspace.is_err() {
                    result.add_error(
                        "resize",
                        "resize exceeds the image workspace limit even at one byte per pixel",
                    );
                }
            }
        }
    }
    // Share argument planning and ask the installed bridge about its own capabilities.
    if result.valid && (analysis.format.is_audio() || analysis.format.is_video()) {
        let defaults = crate::config::FfmpegConfig::default();
        let ffmpeg = config.ffmpeg.as_ref().unwrap_or(&defaults);
        let plan = if analysis.format.is_audio()
            || config.output_format.is_some_and(|format| format.is_audio())
        {
            crate::compression::audio::build_audio_args(analysis, config, ffmpeg)
        } else {
            crate::compression::video::build_video_args(analysis, config, ffmpeg)
        };
        if let Err(crate::error::CompressionError::InvalidConfig { field, reason }) =
            plan.and_then(|args| crate::compression::audio::validate_ffmpeg_args(&args))
        {
            result.add_error(&field, &reason);
        }
    }

    if analysis.is_animated == Some(true)
        && matches!(analysis.format, FileFormat::Avif | FileFormat::Heic)
    {
        result.add_error(
            "input",
            "AVIF/HEIC image sequences cannot be transcoded without losing frames",
        );
    }

    if analysis.format == FileFormat::Svg
        && (config.output_format.is_some() || config.resize.is_some() || config.crop.is_some())
    {
        result.add_error(
            "output_format",
            "SVG rasterization and transforms are not supported",
        );
    }
    if analysis.is_animated == Some(true)
        && matches!(
            analysis.format,
            FileFormat::Png | FileFormat::Webp | FileFormat::Gif
        )
    {
        if config
            .output_format
            .is_some_and(|f| f.extension() != analysis.format.extension())
        {
            result.add_error(
                "output_format",
                "animated images cannot be converted without losing frames",
            );
        }
        if matches!(analysis.format, FileFormat::Png | FileFormat::Webp)
            && (config.crop.is_some() || config.resize.is_some())
        {
            result.add_error("resize", "animated PNG/WebP transforms are not supported");
        }
    }
    if config.preserve_metadata && analysis.format.is_image() && analysis.format != FileFormat::Svg
    {
        result.add_warning("raster re-encoding may discard metadata; preservation is best effort");
    }
    if !config.get_webp_config().lossless
        && config.output_format == Some(crate::config::OutputFormat::Webp)
    {
        result.add_warning("the pure Rust WebP encoder uses lossless encoding");
    }
    if let Some(ref crop) = config.crop {
        if let (Some(w), Some(h)) = (analysis.width, analysis.height) {
            if u64::from(crop.x) + u64::from(crop.width) > u64::from(w) {
                result.add_error(
                    "crop",
                    &format!(
                        "crop region exceeds image width ({} > {})",
                        u64::from(crop.x) + u64::from(crop.width),
                        w
                    ),
                );
            }
            if u64::from(crop.y) + u64::from(crop.height) > u64::from(h) {
                result.add_error(
                    "crop",
                    &format!(
                        "crop region exceeds image height ({} > {})",
                        u64::from(crop.y) + u64::from(crop.height),
                        h
                    ),
                );
            }
        }
    }

    if let Some(ref trim) = config.trim {
        if let Some(duration) = analysis.duration_ms {
            if let Some(start) = trim.start_ms {
                if start >= duration {
                    result.add_error(
                        "trim.start_ms",
                        &format!(
                            "start time ({}) exceeds media duration ({})",
                            start, duration
                        ),
                    );
                }
            }
            if let Some(end) = trim.end_ms {
                if end > duration {
                    result.add_warning(&format!(
                        "trim.end_ms ({}) exceeds duration ({}), will be clamped",
                        end, duration
                    ));
                }
            }
        }
    }

    if analysis.is_animated == Some(true) {
        if let Some(frames) = analysis.frame_count {
            if frames > 1 && analysis.format == FileFormat::Gif {
                result.add_warning(&format!(
                    "animated GIF with {} frames - processing may be slow",
                    frames
                ));
            }
        }
    }

    let size_mb = analysis.size / (1024 * 1024);
    if size_mb > 100 {
        result.add_warning(&format!(
            "large file ({} MB) - compression currently buffers the complete file",
            size_mb
        ));
    }

    if analysis.has_embedded_images && analysis.embedded_image_count > 0 {
        let svg_config = config.get_svg_config();
        if svg_config.compress_embedded_images {
            result.add_warning(&format!(
                "SVG contains {} embedded images that will be compressed",
                analysis.embedded_image_count
            ));
        }
    }

    result
}

pub fn validate_format_conversion(from: FileFormat, to: FileFormat) -> ValidationResult {
    let mut result = ValidationResult::ok();

    if from == FileFormat::Unknown || to == FileFormat::Unknown {
        result.add_error("output_format", "unknown input or output format");
    } else if from == FileFormat::Svg {
        if to != FileFormat::Svg {
            result.add_error("output_format", "SVG rasterization is not supported");
        }
    } else if to == FileFormat::Svg || to == FileFormat::Heic || to == FileFormat::Avif {
        result.add_error(
            "output_format",
            "no encoder is available for this output format",
        );
    } else if (from.is_image() && !to.is_image())
        || (from.is_audio() && !to.is_audio())
        || (from.is_video() && to.is_image())
    {
        result.add_error(
            "output_format",
            &format!("cannot convert {} to {}", from.extension(), to.extension()),
        );
    }
    result
}
