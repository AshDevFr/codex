use crate::parsers::{BookMetadata, FileFormat, ImageFormat, PageInfo};
use crate::parsers::traits::FormatParser;
use crate::utils::{hash_file, CodexError, Result};
use chrono::{DateTime, Utc};
use image::GenericImageView;
use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use zip::ZipArchive;

pub struct EpubParser;

impl EpubParser {
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
            || lower.ends_with(".svg")
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

    /// Parse the EPUB container.xml to find the root file (usually content.opf)
    fn find_root_file(archive: &mut ZipArchive<File>) -> Result<String> {
        let mut container_file = archive
            .by_name("META-INF/container.xml")
            .map_err(|_| CodexError::ParseError("META-INF/container.xml not found".to_string()))?;

        let mut xml_content = String::new();
        container_file.read_to_string(&mut xml_content)?;

        // Parse container.xml to find rootfile path
        // Simple XML parsing for: <rootfile full-path="..." />
        if let Some(start) = xml_content.find("full-path=\"") {
            let path_start = start + 11; // length of "full-path=\""
            if let Some(end) = xml_content[path_start..].find('"') {
                return Ok(xml_content[path_start..path_start + end].to_string());
            }
        }

        Err(CodexError::ParseError(
            "Could not find rootfile path in container.xml".to_string(),
        ))
    }

    /// Parse the OPF file to get metadata and spine (reading order)
    fn parse_opf(
        archive: &mut ZipArchive<File>,
        opf_path: &str,
    ) -> Result<(HashMap<String, String>, Vec<String>)> {
        let mut opf_file = archive.by_name(opf_path).map_err(|_| {
            CodexError::ParseError(format!("OPF file not found: {}", opf_path))
        })?;

        let mut xml_content = String::new();
        opf_file.read_to_string(&mut xml_content)?;

        // Extract the base path (directory containing the OPF)
        let base_path = if let Some(pos) = opf_path.rfind('/') {
            &opf_path[..pos + 1]
        } else {
            ""
        };

        // Parse manifest to get id -> href mapping
        let mut manifest: HashMap<String, String> = HashMap::new();

        // Simple XML parsing for manifest items
        let mut remaining = &xml_content[..];
        while let Some(item_start) = remaining.find("<item ") {
            let item_section = &remaining[item_start..];
            if let Some(item_end) = item_section.find('>') {
                let item_tag = &item_section[..item_end];

                // Extract id
                let id = if let Some(id_start) = item_tag.find("id=\"") {
                    let id_value_start = id_start + 4;
                    if let Some(id_end) = item_tag[id_value_start..].find('"') {
                        Some(&item_tag[id_value_start..id_value_start + id_end])
                    } else {
                        None
                    }
                } else {
                    None
                };

                // Extract href
                let href = if let Some(href_start) = item_tag.find("href=\"") {
                    let href_value_start = href_start + 6;
                    if let Some(href_end) = item_tag[href_value_start..].find('"') {
                        Some(&item_tag[href_value_start..href_value_start + href_end])
                    } else {
                        None
                    }
                } else {
                    None
                };

                if let (Some(id), Some(href)) = (id, href) {
                    // Combine base path with href
                    let full_path = format!("{}{}", base_path, href);
                    manifest.insert(id.to_string(), full_path);
                }

                remaining = &item_section[item_end..];
            } else {
                break;
            }
        }

        // Parse spine to get reading order (idref list)
        let mut spine_order: Vec<String> = Vec::new();
        remaining = &xml_content[..];

        if let Some(spine_start) = remaining.find("<spine") {
            let spine_section = &remaining[spine_start..];
            if let Some(spine_end) = spine_section.find("</spine>") {
                let spine_content = &spine_section[..spine_end];

                // Extract itemrefs
                let mut itemref_remaining = spine_content;
                while let Some(itemref_start) = itemref_remaining.find("<itemref ") {
                    let itemref_section = &itemref_remaining[itemref_start..];
                    if let Some(itemref_end) = itemref_section.find('>') {
                        let itemref_tag = &itemref_section[..itemref_end];

                        // Extract idref
                        if let Some(idref_start) = itemref_tag.find("idref=\"") {
                            let idref_value_start = idref_start + 7;
                            if let Some(idref_end) = itemref_tag[idref_value_start..].find('"') {
                                let idref = &itemref_tag[idref_value_start..idref_value_start + idref_end];
                                if let Some(path) = manifest.get(idref) {
                                    spine_order.push(path.clone());
                                }
                            }
                        }

                        itemref_remaining = &itemref_section[itemref_end..];
                    } else {
                        break;
                    }
                }
            }
        }

        Ok((manifest, spine_order))
    }
}

impl FormatParser for EpubParser {
    fn can_parse<P: AsRef<Path>>(&self, path: P) -> bool {
        path.as_ref()
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase() == "epub")
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

        // Open ZIP archive (EPUB is a ZIP file)
        let file = File::open(path)?;
        let mut archive = ZipArchive::new(file)?;

        // Find the OPF file from container.xml
        let opf_path = Self::find_root_file(&mut archive)?;

        // Parse the OPF to get manifest and spine
        let (_manifest, spine_order) = Self::parse_opf(&mut archive, &opf_path)?;

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

        // Sort by name (natural ordering for images)
        image_entries.sort_by(|a, b| a.1.cmp(&b.1));

        // Process each image as a "page"
        let mut pages = Vec::new();
        for (page_num, (idx, name)) in image_entries.iter().enumerate() {
            let mut file = archive.by_index(*idx)?;
            let file_size = file.size();

            // Skip SVG files for now (can't get dimensions easily)
            if name.to_lowercase().ends_with(".svg") {
                continue;
            }

            // Read image data
            let mut image_data = Vec::new();
            file.read_to_end(&mut image_data)?;

            // Get image dimensions
            let img = match image::load_from_memory(&image_data) {
                Ok(img) => img,
                Err(_) => continue, // Skip if we can't load the image
            };
            let (width, height) = img.dimensions();

            let format = match Self::get_image_format(name) {
                Some(f) => f,
                None => continue, // Skip if format is unknown
            };

            pages.push(PageInfo {
                page_number: page_num + 1,
                file_name: name.clone(),
                format,
                width,
                height,
                file_size,
            });
        }

        // Note: EPUBs are primarily text-based, so page_count based on spine would be more accurate
        // but we're counting images for consistency with CBZ/CBR format
        let page_count = spine_order.len().max(pages.len());

        Ok(BookMetadata {
            file_path: path.to_string_lossy().to_string(),
            format: FileFormat::EPUB,
            file_size,
            file_hash,
            modified_at,
            page_count,
            pages,
            comic_info: None, // EPUB doesn't use ComicInfo.xml
            isbns: Vec::new(), // TODO: Extract from OPF metadata
        })
    }
}

impl Default for EpubParser {
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
            assert!(EpubParser::is_image_file("image.jpg"));
        }

        #[test]
        fn test_jpg_uppercase() {
            assert!(EpubParser::is_image_file("image.JPG"));
        }

        #[test]
        fn test_jpeg_lowercase() {
            assert!(EpubParser::is_image_file("photo.jpeg"));
        }

        #[test]
        fn test_jpeg_uppercase() {
            assert!(EpubParser::is_image_file("photo.JPEG"));
        }

        #[test]
        fn test_png() {
            assert!(EpubParser::is_image_file("graphic.png"));
            assert!(EpubParser::is_image_file("graphic.PNG"));
        }

        #[test]
        fn test_webp() {
            assert!(EpubParser::is_image_file("modern.webp"));
            assert!(EpubParser::is_image_file("modern.WEBP"));
        }

        #[test]
        fn test_gif() {
            assert!(EpubParser::is_image_file("animation.gif"));
            assert!(EpubParser::is_image_file("animation.GIF"));
        }

        #[test]
        fn test_bmp() {
            assert!(EpubParser::is_image_file("bitmap.bmp"));
            assert!(EpubParser::is_image_file("bitmap.BMP"));
        }

        #[test]
        fn test_svg() {
            assert!(EpubParser::is_image_file("vector.svg"));
            assert!(EpubParser::is_image_file("vector.SVG"));
        }

        #[test]
        fn test_mixed_case() {
            assert!(EpubParser::is_image_file("Image.JpG"));
            assert!(EpubParser::is_image_file("Photo.PnG"));
        }

        #[test]
        fn test_with_path() {
            assert!(EpubParser::is_image_file("path/to/image.jpg"));
            assert!(EpubParser::is_image_file("/absolute/path/image.png"));
        }

        #[test]
        fn test_non_image_files() {
            assert!(!EpubParser::is_image_file("document.txt"));
            assert!(!EpubParser::is_image_file("archive.zip"));
            assert!(!EpubParser::is_image_file("data.json"));
            assert!(!EpubParser::is_image_file("content.xhtml"));
        }

        #[test]
        fn test_no_extension() {
            assert!(!EpubParser::is_image_file("noextension"));
        }

        #[test]
        fn test_empty_string() {
            assert!(!EpubParser::is_image_file(""));
        }
    }

    mod get_image_format {
        use super::*;

        #[test]
        fn test_jpg_format() {
            assert_eq!(
                EpubParser::get_image_format("image.jpg"),
                Some(ImageFormat::JPEG)
            );
            assert_eq!(
                EpubParser::get_image_format("image.JPG"),
                Some(ImageFormat::JPEG)
            );
        }

        #[test]
        fn test_jpeg_format() {
            assert_eq!(
                EpubParser::get_image_format("photo.jpeg"),
                Some(ImageFormat::JPEG)
            );
            assert_eq!(
                EpubParser::get_image_format("photo.JPEG"),
                Some(ImageFormat::JPEG)
            );
        }

        #[test]
        fn test_png_format() {
            assert_eq!(
                EpubParser::get_image_format("graphic.png"),
                Some(ImageFormat::PNG)
            );
            assert_eq!(
                EpubParser::get_image_format("graphic.PNG"),
                Some(ImageFormat::PNG)
            );
        }

        #[test]
        fn test_webp_format() {
            assert_eq!(
                EpubParser::get_image_format("modern.webp"),
                Some(ImageFormat::WEBP)
            );
        }

        #[test]
        fn test_gif_format() {
            assert_eq!(
                EpubParser::get_image_format("animation.gif"),
                Some(ImageFormat::GIF)
            );
        }

        #[test]
        fn test_bmp_format() {
            assert_eq!(
                EpubParser::get_image_format("bitmap.bmp"),
                Some(ImageFormat::BMP)
            );
        }

        #[test]
        fn test_mixed_case() {
            assert_eq!(
                EpubParser::get_image_format("Image.JpG"),
                Some(ImageFormat::JPEG)
            );
        }

        #[test]
        fn test_with_path() {
            assert_eq!(
                EpubParser::get_image_format("path/to/image.jpg"),
                Some(ImageFormat::JPEG)
            );
        }

        #[test]
        fn test_unsupported_format() {
            assert_eq!(EpubParser::get_image_format("document.txt"), None);
            assert_eq!(EpubParser::get_image_format("archive.zip"), None);
            assert_eq!(EpubParser::get_image_format("video.mp4"), None);
        }

        #[test]
        fn test_svg_returns_none() {
            // SVG is detected as image file but has no ImageFormat enum
            assert_eq!(EpubParser::get_image_format("vector.svg"), None);
        }

        #[test]
        fn test_no_extension() {
            assert_eq!(EpubParser::get_image_format("noextension"), None);
        }

        #[test]
        fn test_empty_string() {
            assert_eq!(EpubParser::get_image_format(""), None);
        }
    }

    #[test]
    fn test_epub_parser_new() {
        let parser = EpubParser::new();
        assert!(parser.can_parse("test.epub"));
    }

    #[test]
    fn test_epub_parser_default() {
        let parser = EpubParser::default();
        assert!(parser.can_parse("test.epub"));
    }

    #[test]
    fn test_epub_parser_can_parse() {
        let parser = EpubParser::new();

        assert!(parser.can_parse("test.epub"));
        assert!(parser.can_parse("test.EPUB"));
        assert!(parser.can_parse("/path/to/file.epub"));

        assert!(!parser.can_parse("test.cbz"));
        assert!(!parser.can_parse("test.cbr"));
        assert!(!parser.can_parse("test.pdf"));
        assert!(!parser.can_parse("test.txt"));
    }
}

