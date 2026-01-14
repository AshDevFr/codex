use crate::parsers::image_utils::{get_image_format, is_image_file};
use crate::parsers::isbn_utils::extract_isbns;
use crate::parsers::traits::FormatParser;
use crate::parsers::{BookMetadata, FileFormat, ImageFormat, PageInfo};
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

    /// Extract ISBNs from OPF XML metadata
    ///
    /// Looks for ISBN identifiers in:
    /// - <dc:identifier opf:scheme="ISBN">...</dc:identifier>
    /// - <dc:identifier>ISBN:...</dc:identifier>
    /// - Any identifier containing ISBN-like patterns
    fn extract_isbns_from_opf(opf_content: &str) -> Vec<String> {
        let mut isbns = Vec::new();

        // Look for dc:identifier tags
        let mut remaining = opf_content;
        while let Some(start) = remaining.find("<dc:identifier") {
            let tag_section = &remaining[start..];
            if let Some(tag_end) = tag_section.find("</dc:identifier>") {
                let full_tag = &tag_section[..tag_end + 16]; // +16 for "</dc:identifier>"

                // Check if it's explicitly marked as ISBN scheme
                let is_isbn_scheme = full_tag.contains("opf:scheme=\"ISBN\"")
                    || full_tag.contains("opf:scheme='ISBN'")
                    || full_tag.contains("scheme=\"ISBN\"")
                    || full_tag.contains("scheme='ISBN'");

                // Extract content between tags
                if let Some(content_start) = full_tag.find('>') {
                    let content = &full_tag[content_start + 1..full_tag.len() - 16];

                    // If explicitly marked as ISBN or contains "ISBN", try to extract
                    if is_isbn_scheme || content.to_uppercase().contains("ISBN") {
                        let extracted = extract_isbns(content, false);
                        isbns.extend(extracted);
                    }
                }

                remaining = &tag_section[tag_end + 16..];
            } else {
                break;
            }
        }

        // Remove duplicates while preserving order
        let mut seen = std::collections::HashSet::new();
        isbns.retain(|isbn| seen.insert(isbn.clone()));

        isbns
    }

    /// Parse the OPF file to get metadata and spine (reading order)
    fn parse_opf(
        archive: &mut ZipArchive<File>,
        opf_path: &str,
    ) -> Result<(HashMap<String, String>, Vec<String>)> {
        let mut opf_file = archive
            .by_name(opf_path)
            .map_err(|_| CodexError::ParseError(format!("OPF file not found: {}", opf_path)))?;

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
                    item_tag[id_value_start..].find('"').map(|id_end| &item_tag[id_value_start..id_value_start + id_end])
                } else {
                    None
                };

                // Extract href
                let href = if let Some(href_start) = item_tag.find("href=\"") {
                    let href_value_start = href_start + 6;
                    item_tag[href_value_start..].find('"').map(|href_end| &item_tag[href_value_start..href_value_start + href_end])
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
                                let idref =
                                    &itemref_tag[idref_value_start..idref_value_start + idref_end];
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

        // Read OPF content for ISBN extraction
        let opf_content = {
            let mut opf_file = archive
                .by_name(&opf_path)
                .map_err(|_| CodexError::ParseError(format!("OPF file not found: {}", opf_path)))?;
            let mut content = String::new();
            opf_file.read_to_string(&mut content)?;
            content
        };

        // Extract ISBNs from OPF metadata
        let isbns = Self::extract_isbns_from_opf(&opf_content);

        // Parse the OPF to get manifest and spine
        let (_manifest, spine_order) = Self::parse_opf(&mut archive, &opf_path)?;

        // Collect and sort image files
        let mut image_entries: Vec<(usize, String)> = Vec::new();
        for i in 0..archive.len() {
            let file = archive.by_index(i)?;
            let name = file.name().to_string();

            // Skip directories and non-image files
            if file.is_dir() || !is_image_file(&name) {
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

            let format = match get_image_format(name) {
                Some(f) => f,
                None => continue, // Skip if format is unknown
            };

            // Skip SVG files - they require rendering to get dimensions
            // and the `image` crate doesn't support SVG
            if format == ImageFormat::SVG {
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

            pages.push(PageInfo {
                page_number: page_num + 1,
                file_name: name.clone(),
                format,
                width,
                height,
                file_size,
            });
        }

        // Page count logic for EPUB:
        // EPUBs are primarily text-based documents with a spine (reading order) and optional images.
        // We use the maximum of:
        // - spine_order.len(): Number of content items (chapters/sections) in reading order
        // - pages.len(): Number of extracted images (covers, illustrations)
        //
        // This gives a reasonable page count estimate, though EPUBs don't have fixed "pages"
        // like comics do. For pure image-based EPUBs (like converted manga), pages.len()
        // will be higher. For text-heavy novels, spine_order.len() will be higher.
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
            isbns,
        })
    }
}

impl Default for EpubParser {
    fn default() -> Self {
        Self::new()
    }
}

/// Extract a specific page image from an EPUB file
///
/// # Arguments
/// * `path` - Path to the EPUB file
/// * `page_number` - Page number (1-indexed)
///
/// # Returns
/// The raw image data as bytes
pub fn extract_page_from_epub<P: AsRef<Path>>(
    path: P,
    page_number: i32,
) -> anyhow::Result<Vec<u8>> {
    let file = File::open(path)?;
    let mut archive = ZipArchive::new(file)?;

    // Get list of image files in EPUB
    let mut image_files: Vec<String> = Vec::new();
    for i in 0..archive.len() {
        let file = archive.by_index(i)?;
        let name = file.name().to_string();
        if !file.is_dir() && is_image_file(&name) {
            image_files.push(name);
        }
    }

    // Sort alphabetically
    image_files.sort();

    // Get the requested page (1-indexed)
    let index = (page_number - 1) as usize;
    if index >= image_files.len() {
        anyhow::bail!("Page {} not found in EPUB", page_number);
    }

    // Extract image data
    let mut file = archive.by_name(&image_files[index])?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)?;

    Ok(buffer)
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn test_extract_isbns_from_opf_with_scheme() {
        let opf_content = r#"
            <?xml version="1.0"?>
            <package xmlns="http://www.idpf.org/2007/opf" version="2.0">
                <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
                    <dc:identifier opf:scheme="ISBN">978-0-306-40615-7</dc:identifier>
                    <dc:title>Test Book</dc:title>
                </metadata>
            </package>
        "#;

        let isbns = EpubParser::extract_isbns_from_opf(opf_content);
        assert_eq!(isbns.len(), 1);
        assert_eq!(isbns[0], "9780306406157");
    }

    #[test]
    fn test_extract_isbns_from_opf_with_isbn_prefix() {
        let opf_content = r#"
            <?xml version="1.0"?>
            <package xmlns="http://www.idpf.org/2007/opf" version="2.0">
                <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
                    <dc:identifier>ISBN: 978-0-306-40615-7</dc:identifier>
                    <dc:title>Test Book</dc:title>
                </metadata>
            </package>
        "#;

        let isbns = EpubParser::extract_isbns_from_opf(opf_content);
        assert_eq!(isbns.len(), 1);
        assert_eq!(isbns[0], "9780306406157");
    }

    #[test]
    fn test_extract_isbns_from_opf_multiple() {
        let opf_content = r#"
            <?xml version="1.0"?>
            <package xmlns="http://www.idpf.org/2007/opf" version="2.0">
                <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
                    <dc:identifier opf:scheme="ISBN">978-0-306-40615-7</dc:identifier>
                    <dc:identifier opf:scheme="ISBN">0-306-40615-2</dc:identifier>
                    <dc:title>Test Book</dc:title>
                </metadata>
            </package>
        "#;

        let isbns = EpubParser::extract_isbns_from_opf(opf_content);
        assert_eq!(isbns.len(), 2);
        assert!(isbns.contains(&"9780306406157".to_string()));
        assert!(isbns.contains(&"0306406152".to_string()));
    }

    #[test]
    fn test_extract_isbns_from_opf_no_isbn() {
        let opf_content = r#"
            <?xml version="1.0"?>
            <package xmlns="http://www.idpf.org/2007/opf" version="2.0">
                <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
                    <dc:identifier>urn:uuid:12345</dc:identifier>
                    <dc:title>Test Book</dc:title>
                </metadata>
            </package>
        "#;

        let isbns = EpubParser::extract_isbns_from_opf(opf_content);
        assert_eq!(isbns.len(), 0);
    }

    #[test]
    fn test_extract_isbns_from_opf_deduplicates() {
        let opf_content = r#"
            <?xml version="1.0"?>
            <package xmlns="http://www.idpf.org/2007/opf" version="2.0">
                <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
                    <dc:identifier opf:scheme="ISBN">978-0-306-40615-7</dc:identifier>
                    <dc:identifier>ISBN: 978-0-306-40615-7</dc:identifier>
                    <dc:title>Test Book</dc:title>
                </metadata>
            </package>
        "#;

        let isbns = EpubParser::extract_isbns_from_opf(opf_content);
        assert_eq!(isbns.len(), 1);
        assert_eq!(isbns[0], "9780306406157");
    }
}
