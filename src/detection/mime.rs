use super::FileFormat;

pub struct MimeMapping;

impl MimeMapping {
    pub fn get_all_supported() -> Vec<(&'static str, &'static str, FileFormat)> {
        vec![
            // Images
            ("image/png", "png", FileFormat::Png),
            ("image/jpeg", "jpg", FileFormat::Jpeg),
            ("image/jpeg", "jpeg", FileFormat::Jpeg),
            ("image/webp", "webp", FileFormat::Webp),
            ("image/avif", "avif", FileFormat::Avif),
            ("image/gif", "gif", FileFormat::Gif),
            ("image/bmp", "bmp", FileFormat::Bmp),
            ("image/tiff", "tiff", FileFormat::Tiff),
            ("image/tiff", "tif", FileFormat::Tiff),
            ("image/x-icon", "ico", FileFormat::Ico),
            ("image/svg+xml", "svg", FileFormat::Svg),
            ("image/heic", "heic", FileFormat::Heic),
            ("image/heif", "heif", FileFormat::Heic),
            // Audio
            ("audio/mpeg", "mp3", FileFormat::Mp3),
            ("audio/wav", "wav", FileFormat::Wav),
            ("audio/flac", "flac", FileFormat::Flac),
            ("audio/ogg", "ogg", FileFormat::Ogg),
            ("audio/aac", "aac", FileFormat::Aac),
            ("audio/mp4", "m4a", FileFormat::Aac),
            ("audio/opus", "opus", FileFormat::Opus),
            ("audio/ac3", "ac3", FileFormat::Ac3),
            ("audio/aiff", "aiff", FileFormat::Aiff),
            ("audio/amr", "amr", FileFormat::Amr),
            ("audio/x-ms-wma", "wma", FileFormat::Wma),
            // Video
            ("video/mp4", "mp4", FileFormat::Mp4),
            ("video/webm", "webm", FileFormat::Webm),
            ("video/quicktime", "mov", FileFormat::Mov),
            ("video/x-msvideo", "avi", FileFormat::Avi),
            ("video/x-matroska", "mkv", FileFormat::Mkv),
            ("video/x-ms-wmv", "wmv", FileFormat::Wmv),
            ("video/x-flv", "flv", FileFormat::Flv),
            ("video/mpeg", "mpg", FileFormat::Mpeg),
        ]
    }

    pub fn image_formats() -> Vec<FileFormat> {
        vec![
            FileFormat::Png,
            FileFormat::Jpeg,
            FileFormat::Webp,
            FileFormat::Avif,
            FileFormat::Gif,
            FileFormat::Bmp,
            FileFormat::Tiff,
            FileFormat::Ico,
            FileFormat::Svg,
            FileFormat::Heic,
        ]
    }

    pub fn audio_formats() -> Vec<FileFormat> {
        vec![
            FileFormat::Mp3,
            FileFormat::Wav,
            FileFormat::Flac,
            FileFormat::Ogg,
            FileFormat::Aac,
            FileFormat::Opus,
            FileFormat::Ac3,
            FileFormat::Aiff,
            FileFormat::Amr,
            FileFormat::Wma,
        ]
    }

    pub fn video_formats() -> Vec<FileFormat> {
        vec![
            FileFormat::Mp4,
            FileFormat::Webm,
            FileFormat::Mov,
            FileFormat::Avi,
            FileFormat::Mkv,
            FileFormat::Wmv,
            FileFormat::Flv,
            FileFormat::Mpeg,
        ]
    }
}
