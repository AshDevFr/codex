use crate::parsers::{parse_comic_info, BookMetadata, FileFormat, ImageFormat, PageInfo};
use crate::parsers::traits::FormatParser;
use crate::utils::{hash_file, CodexError, Result};
use chrono::{DateTime, Utc};
use image::GenericImageView;
use std::path::Path;
use unrar::Archive;

pub struct CbrParser;

impl CbrParser {
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

impl FormatParser for CbrParser {
    fn can_parse<P: AsRef<Path>>(&self, path: P) -> bool {
        path.as_ref()
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase() == "cbr")
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

        // Open RAR archive for processing - we'll do everything in one pass
        let mut archive = Archive::new(path.to_str().ok_or_else(|| {
            CodexError::ParseError("Invalid path encoding".to_string())
        })?)
        .open_for_processing()
        .map_err(|e| CodexError::ParseError(format!("Failed to open RAR archive: {}", e)))?;

        // Collect all entries with their data
        let mut image_data_entries: Vec<(String, Vec<u8>, u64)> = Vec::new();
        let mut comic_info = None;

        loop {
            let header = match archive.read_header() {
                Ok(Some(h)) => h,
                Ok(None) => break,
                Err(e) => return Err(CodexError::ParseError(format!("Failed to read RAR header: {}", e))),
            };

            let filename = header.entry().filename.to_string_lossy().to_string();
            let unpacked_size = header.entry().unpacked_size;

            // Skip directories
            if header.entry().is_directory() {
                archive = header.skip().map_err(|e| {
                    CodexError::ParseError(format!("Failed to skip directory: {}", e))
                })?;
                continue;
            }

            // Check for ComicInfo.xml
            if filename == "ComicInfo.xml" {
                let (xml_content, next) = header.read().map_err(|e| {
                    CodexError::ParseError(format!("Failed to read ComicInfo.xml: {}", e))
                })?;

                let xml_str = String::from_utf8_lossy(&xml_content).to_string();
                if let Ok(info) = parse_comic_info(&xml_str) {
                    comic_info = Some(info);
                }
                archive = next;
            } else if Self::is_image_file(&filename) {
                // Read image data
                let (data, next) = header.read().map_err(|e| {
                    CodexError::ParseError(format!("Failed to read image: {}", e))
                })?;

                image_data_entries.push((filename, data, unpacked_size));
                archive = next;
            } else {
                // Skip non-image, non-ComicInfo files
                archive = header.skip().map_err(|e| {
                    CodexError::ParseError(format!("Failed to skip file: {}", e))
                })?;
            }
        }

        // Sort images by filename for page order
        image_data_entries.sort_by(|a, b| a.0.cmp(&b.0));

        // Process images to extract dimensions
        let mut pages = Vec::new();
        for (page_num, (filename, data, unpacked_size)) in image_data_entries.iter().enumerate() {
            // Get image dimensions
            let img = image::load_from_memory(data)?;
            let (width, height) = img.dimensions();

            let format = Self::get_image_format(filename)
                .ok_or_else(|| CodexError::UnsupportedFormat(filename.clone()))?;

            pages.push(PageInfo {
                page_number: page_num + 1,
                file_name: filename.clone(),
                format,
                width,
                height,
                file_size: *unpacked_size,
            });
        }

        let page_count = pages.len();

        Ok(BookMetadata {
            file_path: path.to_string_lossy().to_string(),
            format: FileFormat::CBR,
            file_size,
            file_hash,
            modified_at,
            page_count,
            pages,
            comic_info,
            isbns: Vec::new(),
        })
    }
}

impl Default for CbrParser {
    fn default() -> Self {
        Self::new()
    }
}

/// Extract a specific page image from a CBR file
///
/// # Arguments
/// * `path` - Path to the CBR file
/// * `page_number` - Page number (1-indexed)
///
/// # Returns
/// The raw image data as bytes
pub fn extract_page_from_cbr<P: AsRef<Path>>(path: P, page_number: i32) -> anyhow::Result<Vec<u8>> {
    let mut archive = unrar::Archive::new(path.as_ref()).open_for_processing()
        .map_err(|e| anyhow::anyhow!("Failed to open RAR archive: {}", e))?;

    let mut image_files: Vec<(String, Vec<u8>)> = Vec::new();

    while let Some(header) = archive.read_header()
        .map_err(|e| anyhow::anyhow!("Failed to read RAR header: {}", e))?
    {
        let entry_name = header.entry().filename.to_string_lossy().to_string();

        if CbrParser::is_image_file(&entry_name) {
            let (data, next_archive) = header.read()
                .map_err(|e| anyhow::anyhow!("Failed to read RAR entry: {}", e))?;
            archive = next_archive;
            image_files.push((entry_name, data));
        } else {
            archive = header.skip()
                .map_err(|e| anyhow::anyhow!("Failed to skip RAR entry: {}", e))?;
        }
    }

    // Sort by filename
    image_files.sort_by(|a, b| a.0.cmp(&b.0));

    // Get the requested page (1-indexed)
    let index = (page_number - 1) as usize;
    if index >= image_files.len() {
        anyhow::bail!("Page {} not found in archive", page_number);
    }

    Ok(image_files[index].1.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    mod is_image_file {
        use super::*;

        #[test]
        fn test_jpg_lowercase() {
            assert!(CbrParser::is_image_file("image.jpg"));
        }

        #[test]
        fn test_jpg_uppercase() {
            assert!(CbrParser::is_image_file("image.JPG"));
        }

        #[test]
        fn test_jpeg_lowercase() {
            assert!(CbrParser::is_image_file("photo.jpeg"));
        }

        #[test]
        fn test_jpeg_uppercase() {
            assert!(CbrParser::is_image_file("photo.JPEG"));
        }

        #[test]
        fn test_png() {
            assert!(CbrParser::is_image_file("graphic.png"));
            assert!(CbrParser::is_image_file("graphic.PNG"));
        }

        #[test]
        fn test_webp() {
            assert!(CbrParser::is_image_file("modern.webp"));
            assert!(CbrParser::is_image_file("modern.WEBP"));
        }

        #[test]
        fn test_gif() {
            assert!(CbrParser::is_image_file("animation.gif"));
            assert!(CbrParser::is_image_file("animation.GIF"));
        }

        #[test]
        fn test_bmp() {
            assert!(CbrParser::is_image_file("bitmap.bmp"));
            assert!(CbrParser::is_image_file("bitmap.BMP"));
        }

        #[test]
        fn test_mixed_case() {
            assert!(CbrParser::is_image_file("Image.JpG"));
            assert!(CbrParser::is_image_file("Photo.PnG"));
        }

        #[test]
        fn test_with_path() {
            assert!(CbrParser::is_image_file("path/to/image.jpg"));
            assert!(CbrParser::is_image_file("/absolute/path/image.png"));
        }

        #[test]
        fn test_non_image_files() {
            assert!(!CbrParser::is_image_file("document.txt"));
            assert!(!CbrParser::is_image_file("archive.rar"));
            assert!(!CbrParser::is_image_file("data.json"));
            assert!(!CbrParser::is_image_file("ComicInfo.xml"));
        }

        #[test]
        fn test_no_extension() {
            assert!(!CbrParser::is_image_file("noextension"));
        }

        #[test]
        fn test_empty_string() {
            assert!(!CbrParser::is_image_file(""));
        }
    }

    mod get_image_format {
        use super::*;

        #[test]
        fn test_jpg_format() {
            assert_eq!(
                CbrParser::get_image_format("image.jpg"),
                Some(ImageFormat::JPEG)
            );
            assert_eq!(
                CbrParser::get_image_format("image.JPG"),
                Some(ImageFormat::JPEG)
            );
        }

        #[test]
        fn test_jpeg_format() {
            assert_eq!(
                CbrParser::get_image_format("photo.jpeg"),
                Some(ImageFormat::JPEG)
            );
            assert_eq!(
                CbrParser::get_image_format("photo.JPEG"),
                Some(ImageFormat::JPEG)
            );
        }

        #[test]
        fn test_png_format() {
            assert_eq!(
                CbrParser::get_image_format("graphic.png"),
                Some(ImageFormat::PNG)
            );
            assert_eq!(
                CbrParser::get_image_format("graphic.PNG"),
                Some(ImageFormat::PNG)
            );
        }

        #[test]
        fn test_webp_format() {
            assert_eq!(
                CbrParser::get_image_format("modern.webp"),
                Some(ImageFormat::WEBP)
            );
        }

        #[test]
        fn test_gif_format() {
            assert_eq!(
                CbrParser::get_image_format("animation.gif"),
                Some(ImageFormat::GIF)
            );
        }

        #[test]
        fn test_bmp_format() {
            assert_eq!(
                CbrParser::get_image_format("bitmap.bmp"),
                Some(ImageFormat::BMP)
            );
        }

        #[test]
        fn test_mixed_case() {
            assert_eq!(
                CbrParser::get_image_format("Image.JpG"),
                Some(ImageFormat::JPEG)
            );
        }

        #[test]
        fn test_with_path() {
            assert_eq!(
                CbrParser::get_image_format("path/to/image.jpg"),
                Some(ImageFormat::JPEG)
            );
        }

        #[test]
        fn test_unsupported_format() {
            assert_eq!(CbrParser::get_image_format("document.txt"), None);
            assert_eq!(CbrParser::get_image_format("archive.rar"), None);
            assert_eq!(CbrParser::get_image_format("video.mp4"), None);
        }

        #[test]
        fn test_no_extension() {
            assert_eq!(CbrParser::get_image_format("noextension"), None);
        }

        #[test]
        fn test_empty_string() {
            assert_eq!(CbrParser::get_image_format(""), None);
        }
    }

    #[test]
    fn test_cbr_parser_new() {
        let parser = CbrParser::new();
        assert!(parser.can_parse("test.cbr"));
    }

    #[test]
    fn test_cbr_parser_default() {
        let parser = CbrParser::default();
        assert!(parser.can_parse("test.cbr"));
    }

    #[test]
    fn test_cbr_parser_can_parse() {
        let parser = CbrParser::new();

        assert!(parser.can_parse("test.cbr"));
        assert!(parser.can_parse("test.CBR"));
        assert!(parser.can_parse("/path/to/file.cbr"));

        assert!(!parser.can_parse("test.cbz"));
        assert!(!parser.can_parse("test.epub"));
        assert!(!parser.can_parse("test.pdf"));
        assert!(!parser.can_parse("test.txt"));
    }
}
