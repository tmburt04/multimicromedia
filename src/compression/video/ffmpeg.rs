use crate::detection::FileFormat;

pub fn get_recommended_codec(format: FileFormat, quality: u8) -> VideoCodecRecommendation {
    match format {
        FileFormat::Mp4 => {
            if quality >= 90 {
                VideoCodecRecommendation {
                    video_codec: "libx265".to_string(),
                    audio_codec: "aac".to_string(),
                    container: "mp4".to_string(),
                    preset: "slow".to_string(),
                    notes: "H.265/HEVC option for high-quality encoding".to_string(),
                }
            } else {
                VideoCodecRecommendation {
                    video_codec: "libx264".to_string(),
                    audio_codec: "aac".to_string(),
                    container: "mp4".to_string(),
                    preset: "medium".to_string(),
                    notes: "H.264/AVC for broad compatibility".to_string(),
                }
            }
        }
        FileFormat::Webm => {
            if quality >= 80 {
                VideoCodecRecommendation {
                    video_codec: "libaom-av1".to_string(),
                    audio_codec: "libopus".to_string(),
                    container: "webm".to_string(),
                    preset: "medium".to_string(),
                    notes: "AV1 option; encoder availability and speed depend on the FFmpeg build"
                        .to_string(),
                }
            } else {
                VideoCodecRecommendation {
                    video_codec: "libvpx-vp9".to_string(),
                    audio_codec: "libopus".to_string(),
                    container: "webm".to_string(),
                    preset: "medium".to_string(),
                    notes: "VP9 for good compression with faster encoding".to_string(),
                }
            }
        }
        FileFormat::Mov => VideoCodecRecommendation {
            video_codec: "libx264".to_string(),
            audio_codec: "aac".to_string(),
            container: "mov".to_string(),
            preset: "medium".to_string(),
            notes: "QuickTime compatible H.264".to_string(),
        },
        FileFormat::Avi => VideoCodecRecommendation {
            video_codec: "mpeg4".to_string(),
            audio_codec: "libmp3lame".to_string(),
            container: "avi".to_string(),
            preset: "medium".to_string(),
            notes: "Legacy AVI format with MPEG-4".to_string(),
        },
        FileFormat::Mkv => VideoCodecRecommendation {
            video_codec: "libx264".to_string(),
            audio_codec: "aac".to_string(),
            container: "mkv".to_string(),
            preset: "medium".to_string(),
            notes: "Matroska with H.264".to_string(),
        },
        _ => VideoCodecRecommendation {
            video_codec: "libx264".to_string(),
            audio_codec: "aac".to_string(),
            container: "mp4".to_string(),
            preset: "medium".to_string(),
            notes: "Default fallback to H.264/AAC/MP4".to_string(),
        },
    }
}

#[derive(Debug, Clone)]
pub struct VideoCodecRecommendation {
    pub video_codec: String,
    pub audio_codec: String,
    pub container: String,
    pub preset: String,
    pub notes: String,
}

pub fn estimate_video_output_size(
    input_size: u64,
    _duration_ms: u64,
    target_crf: u8,
    original_crf: Option<u8>,
) -> u64 {
    // Rough estimation based on CRF change
    // Model a factor of two per six CRF units; this is not a size guarantee.
    let crf_diff = if let Some(orig) = original_crf {
        target_crf as i32 - orig as i32
    } else {
        // Use 23 when the original CRF is unknown.
        target_crf as i32 - 23
    };

    let ratio = 2.0_f64.powf(-(crf_diff as f64) / 6.0);
    ((input_size as f64) * ratio) as u64
}

pub fn get_video_filter_chain(
    resize: Option<(u32, u32)>,
    crop: Option<(u32, u32, u32, u32)>,
    fps: Option<f32>,
) -> String {
    let mut filters: Vec<String> = Vec::new();

    // Crop first (before resize to work with original coordinates)
    if let Some((w, h, x, y)) = crop {
        filters.push(format!("crop={}:{}:{}:{}", w, h, x, y));
    }

    // Scale
    if let Some((w, h)) = resize {
        // A zero dimension requests an inferred, even dimension via -2.
        let scale = if w == 0 && h > 0 {
            format!("scale=-2:{}", h)
        } else if h == 0 && w > 0 {
            format!("scale={}:-2", w)
        } else {
            format!("scale={}:{}", w, h)
        };
        filters.push(scale);
    }

    // FPS
    if let Some(target_fps) = fps {
        filters.push(format!("fps={}", target_fps));
    }

    filters.join(",")
}
