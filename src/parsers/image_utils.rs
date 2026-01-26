use crate::parsers::ImageFormat;
use jxl_oxide::JxlImage;
use resvg::usvg::{Options, Tree};
use std::io::Cursor;

/// Result of detecting image format from bytes
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImageFormatDetection {
    /// A supported image format was detected
    Supported(ImageFormat),
    /// An unsupported image format was detected (includes MIME type for logging)
    Unsupported(String),
    /// Could not determine the format from the bytes
    Unknown,
}

/// Detect image format from raw bytes using magic byte detection
///
/// Uses the `infer` crate to detect the format from file signatures.
/// Returns the detected format, an unsupported format indicator with MIME type,
/// or unknown if detection failed.
///
/// Note: SVG and JXL formats cannot be detected by magic bytes alone and should
/// be handled by extension-based detection in [`get_image_format`].
pub fn detect_image_format_from_bytes(data: &[u8]) -> ImageFormatDetection {
    match infer::get(data) {
        Some(kind) => match kind.mime_type() {
            "image/jpeg" => ImageFormatDetection::Supported(ImageFormat::JPEG),
            "image/png" => ImageFormatDetection::Supported(ImageFormat::PNG),
            "image/webp" => ImageFormatDetection::Supported(ImageFormat::WEBP),
            "image/gif" => ImageFormatDetection::Supported(ImageFormat::GIF),
            "image/bmp" => ImageFormatDetection::Supported(ImageFormat::BMP),
            "image/avif" => ImageFormatDetection::Supported(ImageFormat::AVIF),
            // JXL has magic bytes but infer may not support it yet
            "image/jxl" => ImageFormatDetection::Supported(ImageFormat::JXL),
            // Any other detected type is unsupported
            mime => {
                tracing::debug!(
                    detected_mime = %mime,
                    "Unsupported image format detected"
                );
                ImageFormatDetection::Unsupported(mime.to_string())
            }
        },
        None => {
            // Try JXL detection manually since infer might not support it
            // JXL has two possible signatures:
            // - Naked codestream: 0xFF 0x0A
            // - ISOBMFF container: 0x00 0x00 0x00 0x0C 0x4A 0x58 0x4C 0x20
            if data.len() >= 2 && data[0] == 0xFF && data[1] == 0x0A {
                return ImageFormatDetection::Supported(ImageFormat::JXL);
            }
            if data.len() >= 12
                && data[0..4] == [0x00, 0x00, 0x00, 0x0C]
                && data[4..8] == [0x4A, 0x58, 0x4C, 0x20]
            {
                return ImageFormatDetection::Supported(ImageFormat::JXL);
            }
            ImageFormatDetection::Unknown
        }
    }
}

/// Detect image format from bytes, returning only supported formats
///
/// Convenience wrapper that returns `Some(ImageFormat)` for supported formats
/// and `None` otherwise. Use [`detect_image_format_from_bytes`] if you need
/// to distinguish between unsupported and unknown formats for logging.
#[allow(dead_code)]
pub fn get_image_format_from_bytes(data: &[u8]) -> Option<ImageFormat> {
    match detect_image_format_from_bytes(data) {
        ImageFormatDetection::Supported(format) => Some(format),
        _ => None,
    }
}

/// Detect image format from bytes with logging for unsupported formats
///
/// This function detects the image format from magic bytes and logs warnings
/// for unsupported formats. Use this when processing images from archives
/// where you want visibility into unsupported content.
///
/// # Arguments
/// * `data` - The raw image bytes
/// * `filename` - The filename (used for logging context)
///
/// # Returns
/// `Some(ImageFormat)` if a supported format was detected, `None` otherwise
#[allow(dead_code)]
pub fn detect_image_format_with_logging(data: &[u8], filename: &str) -> Option<ImageFormat> {
    match detect_image_format_from_bytes(data) {
        ImageFormatDetection::Supported(format) => Some(format),
        ImageFormatDetection::Unsupported(mime) => {
            tracing::warn!(
                filename = %filename,
                detected_mime = %mime,
                "Unsupported image format detected"
            );
            None
        }
        ImageFormatDetection::Unknown => {
            tracing::debug!(
                filename = %filename,
                "Could not detect image format from magic bytes"
            );
            None
        }
    }
}

/// Get image format with magic byte verification
///
/// First tries to detect format from extension, then verifies with magic bytes.
/// Logs warnings if there's a mismatch between extension and actual content.
///
/// # Arguments
/// * `filename` - The filename to check extension
/// * `data` - The raw image bytes for magic byte detection
///
/// # Returns
/// The detected `ImageFormat`, preferring magic byte detection over extension
pub fn get_verified_image_format(filename: &str, data: &[u8]) -> Option<ImageFormat> {
    let extension_format = get_image_format(filename);
    let magic_format = detect_image_format_from_bytes(data);

    match (&extension_format, &magic_format) {
        // Both agree on a supported format
        (Some(ext_fmt), ImageFormatDetection::Supported(magic_fmt)) if ext_fmt == magic_fmt => {
            Some(*magic_fmt)
        }
        // Extension says one thing, magic bytes say another supported format
        (Some(ext_fmt), ImageFormatDetection::Supported(magic_fmt)) => {
            tracing::warn!(
                filename = %filename,
                extension_format = ?ext_fmt,
                detected_format = ?magic_fmt,
                "Image format mismatch: extension doesn't match content"
            );
            // Trust magic bytes over extension
            Some(*magic_fmt)
        }
        // Magic bytes detected supported format, no extension match
        (None, ImageFormatDetection::Supported(magic_fmt)) => Some(*magic_fmt),
        // Extension matches but magic bytes show unsupported format
        (Some(ext_fmt), ImageFormatDetection::Unsupported(mime)) => {
            tracing::warn!(
                filename = %filename,
                extension_format = ?ext_fmt,
                detected_mime = %mime,
                "Extension suggests supported format but content is different"
            );
            // Fall back to extension for SVG/JXL which may not be detected by infer
            if matches!(ext_fmt, ImageFormat::SVG | ImageFormat::JXL) {
                Some(*ext_fmt)
            } else {
                None
            }
        }
        // Extension matches but magic bytes unknown (common for SVG/JXL)
        (Some(ext_fmt), ImageFormatDetection::Unknown) => {
            // SVG and JXL may not be detected by infer, trust extension
            if matches!(ext_fmt, ImageFormat::SVG | ImageFormat::JXL) {
                tracing::debug!(
                    filename = %filename,
                    extension_format = ?ext_fmt,
                    "Using extension-based format (magic bytes not recognized)"
                );
            }
            Some(*ext_fmt)
        }
        // No extension match, unsupported format detected
        (None, ImageFormatDetection::Unsupported(mime)) => {
            tracing::warn!(
                filename = %filename,
                detected_mime = %mime,
                "Unsupported image format detected"
            );
            None
        }
        // No extension match, unknown format
        (None, ImageFormatDetection::Unknown) => {
            tracing::debug!(
                filename = %filename,
                "Could not determine image format from extension or content"
            );
            None
        }
    }
}

/// Get dimensions from SVG data using resvg
///
/// SVG files require special handling since they are vector graphics.
/// This function parses the SVG and returns the viewBox/size dimensions.
pub fn get_svg_dimensions(svg_data: &[u8]) -> Option<(u32, u32)> {
    let tree = Tree::from_data(svg_data, &Options::default()).ok()?;
    let size = tree.size();
    let width = size.width() as u32;
    let height = size.height() as u32;
    if width > 0 && height > 0 {
        Some((width, height))
    } else {
        None
    }
}

/// Get dimensions from JXL (JPEG XL) data using jxl-oxide
///
/// JPEG XL is a modern image format that requires special handling
/// since the `image` crate doesn't support it natively.
pub fn get_jxl_dimensions(jxl_data: &[u8]) -> Option<(u32, u32)> {
    let image = JxlImage::builder().read(Cursor::new(jxl_data)).ok()?;
    let width = image.width();
    let height = image.height();
    if width > 0 && height > 0 {
        Some((width, height))
    } else {
        None
    }
}

/// Check if a file name is an image file
///
/// Includes SVG files which are rendered to raster format using resvg.
/// Includes JXL (JPEG XL) files which are decoded using jxl-oxide.
pub fn is_image_file(name: &str) -> bool {
    let lower = name.to_lowercase();
    lower.ends_with(".jpg")
        || lower.ends_with(".jpeg")
        || lower.ends_with(".png")
        || lower.ends_with(".webp")
        || lower.ends_with(".gif")
        || lower.ends_with(".bmp")
        || lower.ends_with(".svg")
        || lower.ends_with(".jxl")
}

/// Determine image format from file extension
pub fn get_image_format(name: &str) -> Option<ImageFormat> {
    let lower = name.to_lowercase();
    if lower.ends_with(".jpg") || lower.ends_with(".jpeg") {
        Some(ImageFormat::JPEG)
    } else if lower.ends_with(".png") {
        Some(ImageFormat::PNG)
    } else if lower.ends_with(".webp") {
        Some(ImageFormat::WEBP)
    } else if lower.ends_with(".gif") {
        Some(ImageFormat::GIF)
    } else if lower.ends_with(".bmp") {
        Some(ImageFormat::BMP)
    } else if lower.ends_with(".svg") {
        Some(ImageFormat::SVG)
    } else if lower.ends_with(".jxl") {
        Some(ImageFormat::JXL)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod is_image_file {
        use super::*;

        #[test]
        fn test_jpg_lowercase() {
            assert!(is_image_file("image.jpg"));
        }

        #[test]
        fn test_jpg_uppercase() {
            assert!(is_image_file("image.JPG"));
        }

        #[test]
        fn test_jpeg_lowercase() {
            assert!(is_image_file("photo.jpeg"));
        }

        #[test]
        fn test_jpeg_uppercase() {
            assert!(is_image_file("photo.JPEG"));
        }

        #[test]
        fn test_png() {
            assert!(is_image_file("graphic.png"));
            assert!(is_image_file("graphic.PNG"));
        }

        #[test]
        fn test_webp() {
            assert!(is_image_file("modern.webp"));
            assert!(is_image_file("modern.WEBP"));
        }

        #[test]
        fn test_gif() {
            assert!(is_image_file("animation.gif"));
            assert!(is_image_file("animation.GIF"));
        }

        #[test]
        fn test_bmp() {
            assert!(is_image_file("bitmap.bmp"));
            assert!(is_image_file("bitmap.BMP"));
        }

        #[test]
        fn test_svg() {
            assert!(is_image_file("vector.svg"));
            assert!(is_image_file("vector.SVG"));
        }

        #[test]
        fn test_jxl() {
            assert!(is_image_file("photo.jxl"));
            assert!(is_image_file("photo.JXL"));
        }

        #[test]
        fn test_mixed_case() {
            assert!(is_image_file("Image.JpG"));
            assert!(is_image_file("Photo.PnG"));
        }

        #[test]
        fn test_with_path() {
            assert!(is_image_file("path/to/image.jpg"));
            assert!(is_image_file("/absolute/path/image.png"));
        }

        #[test]
        fn test_non_image_files() {
            assert!(!is_image_file("document.txt"));
            assert!(!is_image_file("archive.zip"));
            assert!(!is_image_file("data.json"));
            assert!(!is_image_file("ComicInfo.xml"));
        }

        #[test]
        fn test_no_extension() {
            assert!(!is_image_file("noextension"));
        }

        #[test]
        fn test_empty_string() {
            assert!(!is_image_file(""));
        }
    }

    mod get_image_format {
        use super::*;

        #[test]
        fn test_jpg_format() {
            assert_eq!(get_image_format("image.jpg"), Some(ImageFormat::JPEG));
            assert_eq!(get_image_format("image.JPG"), Some(ImageFormat::JPEG));
        }

        #[test]
        fn test_jpeg_format() {
            assert_eq!(get_image_format("photo.jpeg"), Some(ImageFormat::JPEG));
            assert_eq!(get_image_format("photo.JPEG"), Some(ImageFormat::JPEG));
        }

        #[test]
        fn test_png_format() {
            assert_eq!(get_image_format("graphic.png"), Some(ImageFormat::PNG));
            assert_eq!(get_image_format("graphic.PNG"), Some(ImageFormat::PNG));
        }

        #[test]
        fn test_webp_format() {
            assert_eq!(get_image_format("modern.webp"), Some(ImageFormat::WEBP));
        }

        #[test]
        fn test_gif_format() {
            assert_eq!(get_image_format("animation.gif"), Some(ImageFormat::GIF));
        }

        #[test]
        fn test_bmp_format() {
            assert_eq!(get_image_format("bitmap.bmp"), Some(ImageFormat::BMP));
        }

        #[test]
        fn test_mixed_case() {
            assert_eq!(get_image_format("Image.JpG"), Some(ImageFormat::JPEG));
        }

        #[test]
        fn test_with_path() {
            assert_eq!(
                get_image_format("path/to/image.jpg"),
                Some(ImageFormat::JPEG)
            );
        }

        #[test]
        fn test_unsupported_format() {
            assert_eq!(get_image_format("document.txt"), None);
            assert_eq!(get_image_format("archive.zip"), None);
            assert_eq!(get_image_format("video.mp4"), None);
        }

        #[test]
        fn test_svg_format() {
            assert_eq!(get_image_format("vector.svg"), Some(ImageFormat::SVG));
            assert_eq!(get_image_format("vector.SVG"), Some(ImageFormat::SVG));
        }

        #[test]
        fn test_jxl_format() {
            assert_eq!(get_image_format("photo.jxl"), Some(ImageFormat::JXL));
            assert_eq!(get_image_format("photo.JXL"), Some(ImageFormat::JXL));
        }

        #[test]
        fn test_no_extension() {
            assert_eq!(get_image_format("noextension"), None);
        }

        #[test]
        fn test_empty_string() {
            assert_eq!(get_image_format(""), None);
        }
    }

    mod detect_image_format_from_bytes {
        use super::*;

        #[test]
        fn test_jpeg_magic_bytes() {
            // JPEG starts with FF D8 FF
            let jpeg_data = [0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10];
            assert_eq!(
                detect_image_format_from_bytes(&jpeg_data),
                ImageFormatDetection::Supported(ImageFormat::JPEG)
            );
        }

        #[test]
        fn test_png_magic_bytes() {
            // PNG signature: 89 50 4E 47 0D 0A 1A 0A
            let png_data = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
            assert_eq!(
                detect_image_format_from_bytes(&png_data),
                ImageFormatDetection::Supported(ImageFormat::PNG)
            );
        }

        #[test]
        fn test_gif_magic_bytes() {
            // GIF89a
            let gif_data = b"GIF89a\x00\x00\x00\x00";
            assert_eq!(
                detect_image_format_from_bytes(gif_data),
                ImageFormatDetection::Supported(ImageFormat::GIF)
            );
        }

        #[test]
        fn test_webp_magic_bytes() {
            // RIFF....WEBP
            let webp_data = b"RIFF\x00\x00\x00\x00WEBP";
            assert_eq!(
                detect_image_format_from_bytes(webp_data),
                ImageFormatDetection::Supported(ImageFormat::WEBP)
            );
        }

        #[test]
        fn test_bmp_magic_bytes() {
            // BMP starts with BM
            let bmp_data = b"BM\x00\x00\x00\x00\x00\x00\x00\x00\x36\x00\x00\x00";
            assert_eq!(
                detect_image_format_from_bytes(bmp_data),
                ImageFormatDetection::Supported(ImageFormat::BMP)
            );
        }

        #[test]
        fn test_jxl_naked_codestream() {
            // JXL naked codestream: FF 0A
            let jxl_data = [0xFF, 0x0A, 0x00, 0x00];
            assert_eq!(
                detect_image_format_from_bytes(&jxl_data),
                ImageFormatDetection::Supported(ImageFormat::JXL)
            );
        }

        #[test]
        fn test_jxl_isobmff_container() {
            // JXL ISOBMFF container: 00 00 00 0C 4A 58 4C 20 (JXL )
            let jxl_data = [
                0x00, 0x00, 0x00, 0x0C, 0x4A, 0x58, 0x4C, 0x20, 0x0D, 0x0A, 0x87, 0x0A,
            ];
            assert_eq!(
                detect_image_format_from_bytes(&jxl_data),
                ImageFormatDetection::Supported(ImageFormat::JXL)
            );
        }

        #[test]
        fn test_unsupported_format() {
            // PDF magic bytes - not an image format we support
            let pdf_data = b"%PDF-1.4";
            let result = detect_image_format_from_bytes(pdf_data);
            assert!(matches!(result, ImageFormatDetection::Unsupported(_)));
            if let ImageFormatDetection::Unsupported(mime) = result {
                assert_eq!(mime, "application/pdf");
            }
        }

        #[test]
        fn test_unknown_format() {
            // Random bytes that don't match any known format
            let unknown_data = [0x12, 0x34, 0x56, 0x78];
            assert_eq!(
                detect_image_format_from_bytes(&unknown_data),
                ImageFormatDetection::Unknown
            );
        }

        #[test]
        fn test_empty_data() {
            assert_eq!(
                detect_image_format_from_bytes(&[]),
                ImageFormatDetection::Unknown
            );
        }

        #[test]
        fn test_too_short_data() {
            // Single byte - too short for any detection
            assert_eq!(
                detect_image_format_from_bytes(&[0xFF]),
                ImageFormatDetection::Unknown
            );
        }
    }

    mod get_image_format_from_bytes {
        use super::*;

        #[test]
        fn test_returns_some_for_supported() {
            let jpeg_data = [0xFF, 0xD8, 0xFF, 0xE0];
            assert_eq!(
                get_image_format_from_bytes(&jpeg_data),
                Some(ImageFormat::JPEG)
            );
        }

        #[test]
        fn test_returns_none_for_unsupported() {
            let pdf_data = b"%PDF-1.4";
            assert_eq!(get_image_format_from_bytes(pdf_data), None);
        }

        #[test]
        fn test_returns_none_for_unknown() {
            let unknown_data = [0x12, 0x34, 0x56, 0x78];
            assert_eq!(get_image_format_from_bytes(&unknown_data), None);
        }
    }

    mod get_svg_dimensions {
        use super::*;

        #[test]
        fn test_valid_svg() {
            let svg_data =
                br#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="200"></svg>"#;
            assert_eq!(get_svg_dimensions(svg_data), Some((100, 200)));
        }

        #[test]
        fn test_svg_with_viewbox() {
            let svg_data =
                br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 300 400"></svg>"#;
            assert_eq!(get_svg_dimensions(svg_data), Some((300, 400)));
        }

        #[test]
        fn test_invalid_svg() {
            let invalid_data = b"not an svg";
            assert_eq!(get_svg_dimensions(invalid_data), None);
        }

        #[test]
        fn test_empty_data() {
            assert_eq!(get_svg_dimensions(&[]), None);
        }
    }

    mod get_jxl_dimensions {
        use super::*;

        #[test]
        fn test_invalid_jxl() {
            let invalid_data = b"not a jxl file";
            assert_eq!(get_jxl_dimensions(invalid_data), None);
        }

        #[test]
        fn test_empty_data() {
            assert_eq!(get_jxl_dimensions(&[]), None);
        }
    }
}
