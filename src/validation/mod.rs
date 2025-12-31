use crate::config::CompressionConfig;
use crate::detection::{FileAnalysis, FileFormat};
use crate::result::ValidationResult;

pub fn validate_config_for_input(
    config: &CompressionConfig,
    analysis: &FileAnalysis,
) -> ValidationResult {
    let mut result = config.validate(Some(analysis.format));

    // Additional validation based on file analysis
    if let Some(ref crop) = config.crop {
        if let (Some(w), Some(h)) = (analysis.width, analysis.height) {
            if crop.x + crop.width > w {
                result.add_error(
                    "crop",
                    &format!("crop region exceeds image width ({} > {})", crop.x + crop.width, w),
                );
            }
            if crop.y + crop.height > h {
                result.add_error(
                    "crop",
                    &format!("crop region exceeds image height ({} > {})", crop.y + crop.height, h),
                );
            }
        }
    }

    // Validate trim against duration
    if let Some(ref trim) = config.trim {
        if let Some(duration) = analysis.duration_ms {
            if let Some(start) = trim.start_ms {
                if start >= duration {
                    result.add_error(
                        "trim.start_ms",
                        &format!("start time ({}) exceeds media duration ({})", start, duration),
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

    // Warn about animated content
    if analysis.is_animated == Some(true) {
        if let Some(frames) = analysis.frame_count {
            if frames > 1 {
                if analysis.format == FileFormat::Gif {
                    result.add_warning(&format!(
                        "animated GIF with {} frames - processing may be slow",
                        frames
                    ));
                }
            }
        }
    }

    // Warn about large files
    let size_mb = analysis.size / (1024 * 1024);
    if size_mb > 100 {
        result.add_warning(&format!(
            "large file ({} MB) - will use chunked processing",
            size_mb
        ));
    }

    // Warn about embedded images in SVG
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

    // Image to image
    if from.is_image() && !to.is_image() {
        result.add_error(
            "output_format",
            &format!("cannot convert {} to {}", from.extension(), to.extension()),
        );
        return result;
    }

    // Audio to non-audio/video
    if from.is_audio() && !(to.is_audio() || to.is_video()) {
        result.add_error(
            "output_format",
            "audio can only be converted to audio or video formats",
        );
        return result;
    }

    // Video to image (not supported)
    if from.is_video() && to.is_image() {
        result.add_error(
            "output_format",
            "video to image conversion not supported (use frame extraction)",
        );
        return result;
    }

    // SVG special case - can only output as SVG or rasterize to image
    if from == FileFormat::Svg {
        match to {
            FileFormat::Svg => {}
            f if f.is_image() => {
                result.add_warning("SVG will be rasterized to output format");
            }
            _ => {
                result.add_error(
                    "output_format",
                    "SVG can only be output as SVG or rasterized to image formats",
                );
            }
        }
    }

    // HEIC requires conversion (no native encode support)
    if from == FileFormat::Heic {
        result.add_warning("HEIC input will be converted to JPEG for output");
    }

    result
}
