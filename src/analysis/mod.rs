mod image;
pub mod svg;

pub use self::svg::*;
pub use image::*;

use crate::detection::{FileAnalysis, FileFormat};
use crate::error::Result;

pub fn analyze_file(data: &[u8]) -> Result<FileAnalysis> {
    let format = FileFormat::detect(data);
    let size = data.len() as u64;

    let mut analysis = FileAnalysis::new(format, size);

    match format {
        FileFormat::Png => analyze_png(data, &mut analysis)?,
        FileFormat::Jpeg => analyze_jpeg(data, &mut analysis)?,
        FileFormat::Webp => analyze_webp(data, &mut analysis)?,
        FileFormat::Gif => analyze_gif(data, &mut analysis)?,
        FileFormat::Bmp => analyze_bmp(data, &mut analysis)?,
        FileFormat::Svg => analyze_svg(data, &mut analysis)?,
        FileFormat::Avif | FileFormat::Heic => analyze_heif_family(data, &mut analysis)?,
        FileFormat::Tiff => analyze_tiff(data, &mut analysis)?,
        FileFormat::Ico => analyze_ico(data, &mut analysis)?,
        _ => {
            // Audio/video formats - basic analysis only
            // Detailed analysis would require FFmpeg
        }
    }

    Ok(analysis)
}
