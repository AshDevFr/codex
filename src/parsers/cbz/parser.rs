use crate::parsers::{parse_comic_info, BookMetadata, FileFormat, ImageFormat, PageInfo};
use crate::parsers::traits::FormatParser;
use crate::utils::{hash_file, CodexError, Result};
use chrono::{DateTime, Utc};
use image::GenericImageView;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use zip::ZipArchive;

pub struct CbzParser;

impl CbzParser {
    pub fn new() -> Self {
        Self
    }

    /// Check if a file name is an image
    fn is_image_file(name: &str) -> bool {
        let lower = name.to_lowercase();
        lower.ends_with(".jpg")
            || lower.ends_with(".jpeg")
            || lower.ends_with(".png")
            || lower.ends_with(".webp")
            || lower.ends_with(".gif")
            || lower.ends_with(".bmp")
    }

    /// Determine image format from file extension
    fn get_image_format(name: &str) -> Option<ImageFormat> {
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
        } else {
            None
        }
    }
}

impl FormatParser for CbzParser {
    fn can_parse<P: AsRef<Path>>(&self, path: P) -> bool {
        path.as_ref()
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase() == "cbz")
            .unwrap_or(false)
    }

    fn parse<P: AsRef<Path>>(&self, path: P) -> Result<BookMetadata> {
        let path = path.as_ref();

        // Get file metadata
        let metadata = std::fs::metadata(path)?;
        let file_size = metadata.len();
        let modified_at: DateTime<Utc> = metadata.modified()?.into();

        // Compute file hash
        let file_hash = hash_file(path)?;

        // Open ZIP archive
        let file = File::open(path)?;
        let mut archive = ZipArchive::new(file)?;

        // Look for ComicInfo.xml
        let mut comic_info = None;
        if let Ok(mut comic_info_file) = archive.by_name("ComicInfo.xml") {
            let mut xml_content = String::new();
            comic_info_file.read_to_string(&mut xml_content)?;
            if let Ok(info) = parse_comic_info(&xml_content) {
                comic_info = Some(info);
            }
        }

        // Collect and sort image files
        let mut image_entries: Vec<(usize, String)> = Vec::new();
        for i in 0..archive.len() {
            let file = archive.by_index(i)?;
            let name = file.name().to_string();

            // Skip directories and non-image files
            if file.is_dir() || !Self::is_image_file(&name) {
                continue;
            }

            image_entries.push((i, name));
        }

        // Sort by name (this gives us page order)
        image_entries.sort_by(|a, b| a.1.cmp(&b.1));

        // Process each page
        let mut pages = Vec::new();
        for (page_num, (idx, name)) in image_entries.iter().enumerate() {
            let mut file = archive.by_index(*idx)?;
            let file_size = file.size();

            // Read image data
            let mut image_data = Vec::new();
            file.read_to_end(&mut image_data)?;

            // Get image dimensions
            let img = image::load_from_memory(&image_data)?;
            let (width, height) = img.dimensions();

            let format = Self::get_image_format(name)
                .ok_or_else(|| CodexError::UnsupportedFormat(name.clone()))?;

            pages.push(PageInfo {
                page_number: page_num + 1,
                file_name: name.clone(),
                format,
                width,
                height,
                file_size,
            });
        }

        let page_count = pages.len();

        Ok(BookMetadata {
            file_path: path.to_string_lossy().to_string(),
            format: FileFormat::CBZ,
            file_size,
            file_hash,
            modified_at,
            page_count,
            pages,
            comic_info,
            isbns: Vec::new(), // TODO: Implement barcode detection
        })
    }
}

impl Default for CbzParser {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod is_image_file {
        use super::*;

        #[test]
        fn test_jpg_lowercase() {
            assert!(CbzParser::is_image_file("image.jpg"));
        }

        #[test]
        fn test_jpg_uppercase() {
            assert!(CbzParser::is_image_file("image.JPG"));
        }

        #[test]
        fn test_jpeg_lowercase() {
            assert!(CbzParser::is_image_file("photo.jpeg"));
        }

        #[test]
        fn test_jpeg_uppercase() {
            assert!(CbzParser::is_image_file("photo.JPEG"));
        }

        #[test]
        fn test_png() {
            assert!(CbzParser::is_image_file("graphic.png"));
            assert!(CbzParser::is_image_file("graphic.PNG"));
        }

        #[test]
        fn test_webp() {
            assert!(CbzParser::is_image_file("modern.webp"));
            assert!(CbzParser::is_image_file("modern.WEBP"));
        }

        #[test]
        fn test_gif() {
            assert!(CbzParser::is_image_file("animation.gif"));
            assert!(CbzParser::is_image_file("animation.GIF"));
        }

        #[test]
        fn test_bmp() {
            assert!(CbzParser::is_image_file("bitmap.bmp"));
            assert!(CbzParser::is_image_file("bitmap.BMP"));
        }

        #[test]
        fn test_mixed_case() {
            assert!(CbzParser::is_image_file("Image.JpG"));
            assert!(CbzParser::is_image_file("Photo.PnG"));
        }

        #[test]
        fn test_with_path() {
            assert!(CbzParser::is_image_file("path/to/image.jpg"));
            assert!(CbzParser::is_image_file("/absolute/path/image.png"));
        }

        #[test]
        fn test_non_image_files() {
            assert!(!CbzParser::is_image_file("document.txt"));
            assert!(!CbzParser::is_image_file("archive.zip"));
            assert!(!CbzParser::is_image_file("data.json"));
            assert!(!CbzParser::is_image_file("ComicInfo.xml"));
        }

        #[test]
        fn test_no_extension() {
            assert!(!CbzParser::is_image_file("noextension"));
        }

        #[test]
        fn test_empty_string() {
            assert!(!CbzParser::is_image_file(""));
        }
    }

    mod get_image_format {
        use super::*;

        #[test]
        fn test_jpg_format() {
            assert_eq!(
                CbzParser::get_image_format("image.jpg"),
                Some(ImageFormat::JPEG)
            );
            assert_eq!(
                CbzParser::get_image_format("image.JPG"),
                Some(ImageFormat::JPEG)
            );
        }

        #[test]
        fn test_jpeg_format() {
            assert_eq!(
                CbzParser::get_image_format("photo.jpeg"),
                Some(ImageFormat::JPEG)
            );
            assert_eq!(
                CbzParser::get_image_format("photo.JPEG"),
                Some(ImageFormat::JPEG)
            );
        }

        #[test]
        fn test_png_format() {
            assert_eq!(
                CbzParser::get_image_format("graphic.png"),
                Some(ImageFormat::PNG)
            );
            assert_eq!(
                CbzParser::get_image_format("graphic.PNG"),
                Some(ImageFormat::PNG)
            );
        }

        #[test]
        fn test_webp_format() {
            assert_eq!(
                CbzParser::get_image_format("modern.webp"),
                Some(ImageFormat::WEBP)
            );
        }

        #[test]
        fn test_gif_format() {
            assert_eq!(
                CbzParser::get_image_format("animation.gif"),
                Some(ImageFormat::GIF)
            );
        }

        #[test]
        fn test_bmp_format() {
            assert_eq!(
                CbzParser::get_image_format("bitmap.bmp"),
                Some(ImageFormat::BMP)
            );
        }

        #[test]
        fn test_mixed_case() {
            assert_eq!(
                CbzParser::get_image_format("Image.JpG"),
                Some(ImageFormat::JPEG)
            );
        }

        #[test]
        fn test_with_path() {
            assert_eq!(
                CbzParser::get_image_format("path/to/image.jpg"),
                Some(ImageFormat::JPEG)
            );
        }

        #[test]
        fn test_unsupported_format() {
            assert_eq!(CbzParser::get_image_format("document.txt"), None);
            assert_eq!(CbzParser::get_image_format("archive.zip"), None);
            assert_eq!(CbzParser::get_image_format("video.mp4"), None);
        }

        #[test]
        fn test_no_extension() {
            assert_eq!(CbzParser::get_image_format("noextension"), None);
        }

        #[test]
        fn test_empty_string() {
            assert_eq!(CbzParser::get_image_format(""), None);
        }
    }

    #[test]
    fn test_cbz_parser_new() {
        let parser = CbzParser::new();
        assert!(parser.can_parse("test.cbz"));
    }

    #[test]
    fn test_cbz_parser_default() {
        let parser = CbzParser::default();
        assert!(parser.can_parse("test.cbz"));
    }
}
