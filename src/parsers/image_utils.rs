use crate::parsers::ImageFormat;

/// Check if a file name is an image file
pub fn is_image_file(name: &str) -> bool {
    let lower = name.to_lowercase();
    lower.ends_with(".jpg")
        || lower.ends_with(".jpeg")
        || lower.ends_with(".png")
        || lower.ends_with(".webp")
        || lower.ends_with(".gif")
        || lower.ends_with(".bmp")
        || lower.ends_with(".svg")
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
    } else {
        // Other formats are not supported
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
        fn test_no_extension() {
            assert_eq!(get_image_format("noextension"), None);
        }

        #[test]
        fn test_empty_string() {
            assert_eq!(get_image_format(""), None);
        }
    }
}
