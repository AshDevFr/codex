use crate::parsers::image_utils::{get_image_format, get_svg_dimensions, is_image_file};
use crate::parsers::isbn_utils::extract_isbns;
use crate::parsers::opf;
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
                    item_tag[id_value_start..]
                        .find('"')
                        .map(|id_end| &item_tag[id_value_start..id_value_start + id_end])
                } else {
                    None
                };

                // Extract href
                let href = if let Some(href_start) = item_tag.find("href=\"") {
                    let href_value_start = href_start + 6;
                    item_tag[href_value_start..]
                        .find('"')
                        .map(|href_end| &item_tag[href_value_start..href_value_start + href_end])
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

        // Read OPF content for metadata extraction
        let opf_content = {
            let mut opf_file = archive
                .by_name(&opf_path)
                .map_err(|_| CodexError::ParseError(format!("OPF file not found: {}", opf_path)))?;
            let mut content = String::new();
            opf_file.read_to_string(&mut content)?;
            content
        };

        // Extract metadata from OPF, falling back to ISBN-only extraction on failure
        let (comic_info, isbns) = match opf::parse_opf_metadata(&opf_content) {
            Ok(opf_meta) => {
                let ci = opf::opf_to_comic_info(&opf_meta);
                let isbns = opf_meta.isbns;
                (Some(ci), isbns)
            }
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    opf_path = %opf_path,
                    "Failed to parse OPF metadata, falling back to ISBN-only extraction"
                );
                let isbns = Self::extract_isbns_from_opf(&opf_content);
                (None, isbns)
            }
        };

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

            // Read image data
            let mut image_data = Vec::new();
            file.read_to_end(&mut image_data)?;

            // Get image dimensions (with special handling for SVG)
            let (width, height) = if format == ImageFormat::SVG {
                // Use resvg to get SVG dimensions
                match get_svg_dimensions(&image_data) {
                    Some((w, h)) => (w, h),
                    None => continue, // Skip if we can't parse the SVG
                }
            } else {
                // Use image crate for raster formats
                let img = match image::load_from_memory(&image_data) {
                    Ok(img) => img,
                    Err(_) => continue, // Skip if we can't load the image
                };
                img.dimensions()
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
            comic_info,
            isbns,
        })
    }
}

impl Default for EpubParser {
    fn default() -> Self {
        Self::new()
    }
}

/// Find the cover image path from the OPF manifest
///
/// EPUB cover images can be specified in several ways:
/// 1. `<meta name="cover" content="cover-image-id"/>` pointing to a manifest item
/// 2. `<item properties="cover-image" .../>` in EPUB3
/// 3. `<reference type="cover" href="..."/>` in the guide section
/// 4. Item with id containing "cover" and being an image type
///
/// Returns the full path to the cover image relative to the EPUB root.
fn find_cover_image_from_opf(archive: &mut ZipArchive<File>) -> Option<String> {
    // First, find the OPF file path from container.xml
    let opf_path = {
        let mut container_file = archive.by_name("META-INF/container.xml").ok()?;
        let mut xml_content = String::new();
        container_file.read_to_string(&mut xml_content).ok()?;

        // Parse container.xml to find rootfile path
        let start = xml_content.find("full-path=\"")?;
        let path_start = start + 11;
        let end = xml_content[path_start..].find('"')?;
        xml_content[path_start..path_start + end].to_string()
    };

    // Get the base path (directory containing OPF)
    let base_path = if let Some(pos) = opf_path.rfind('/') {
        &opf_path[..pos + 1]
    } else {
        ""
    };

    // Read the OPF file
    let opf_content = {
        let mut opf_file = archive.by_name(&opf_path).ok()?;
        let mut content = String::new();
        opf_file.read_to_string(&mut content).ok()?;
        content
    };

    // Build a map of manifest item IDs to hrefs
    let mut manifest_items: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();
    let mut remaining = &opf_content[..];

    while let Some(item_start) = remaining.find("<item ") {
        let item_section = &remaining[item_start..];
        if let Some(item_end) = item_section.find('>') {
            let item_tag = &item_section[..item_end];

            // Extract id
            let id = if let Some(id_start) = item_tag.find("id=\"") {
                let id_value_start = id_start + 4;
                item_tag[id_value_start..]
                    .find('"')
                    .map(|id_end| item_tag[id_value_start..id_value_start + id_end].to_string())
            } else {
                None
            };

            // Extract href
            let href = if let Some(href_start) = item_tag.find("href=\"") {
                let href_value_start = href_start + 6;
                item_tag[href_value_start..].find('"').map(|href_end| {
                    item_tag[href_value_start..href_value_start + href_end].to_string()
                })
            } else {
                None
            };

            // Check for EPUB3 cover-image property
            let has_cover_property = item_tag.contains("properties=\"cover-image\"")
                || item_tag.contains("properties='cover-image'");

            if let (Some(id), Some(href)) = (id, href) {
                let full_path = format!("{}{}", base_path, href);

                // If this item has the cover-image property (EPUB3), return it immediately
                if has_cover_property && is_image_file(&full_path) {
                    tracing::debug!(
                        cover_path = %full_path,
                        "Found cover image via EPUB3 cover-image property"
                    );
                    return Some(full_path);
                }

                manifest_items.insert(id, full_path);
            }

            remaining = &item_section[item_end..];
        } else {
            break;
        }
    }

    // Method 1: Look for <meta name="cover" content="item-id"/>
    if let Some(meta_start) = opf_content.find("<meta") {
        let meta_section = &opf_content[meta_start..];
        // Find meta tags with name="cover"
        let mut meta_remaining = meta_section;
        while let Some(tag_start) = meta_remaining.find("<meta") {
            let tag_section = &meta_remaining[tag_start..];
            if let Some(tag_end) = tag_section.find('>') {
                let meta_tag = &tag_section[..tag_end];

                if meta_tag.contains("name=\"cover\"") || meta_tag.contains("name='cover'") {
                    // Extract content attribute (the item ID)
                    if let Some(content_start) = meta_tag.find("content=\"") {
                        let value_start = content_start + 9;
                        if let Some(value_end) = meta_tag[value_start..].find('"') {
                            let cover_id = &meta_tag[value_start..value_start + value_end];
                            if let Some(cover_path) = manifest_items.get(cover_id) {
                                if is_image_file(cover_path) {
                                    tracing::debug!(
                                        cover_id = %cover_id,
                                        cover_path = %cover_path,
                                        "Found cover image via meta name=\"cover\""
                                    );
                                    return Some(cover_path.clone());
                                }
                            }
                        }
                    }
                }

                meta_remaining = &tag_section[tag_end..];
            } else {
                break;
            }
        }
    }

    // Method 2: Look for <reference type="cover" href="..."/> in guide section
    if let Some(guide_start) = opf_content.find("<guide") {
        if let Some(guide_end) = opf_content[guide_start..].find("</guide>") {
            let guide_section = &opf_content[guide_start..guide_start + guide_end];

            let mut ref_remaining = guide_section;
            while let Some(ref_start) = ref_remaining.find("<reference") {
                let ref_section = &ref_remaining[ref_start..];
                if let Some(ref_end) = ref_section.find('>') {
                    let ref_tag = &ref_section[..ref_end];

                    // Check for type="cover" or type containing "cover"
                    if ref_tag.contains("type=\"cover\"")
                        || ref_tag.contains("type='cover'")
                        || ref_tag.contains("coverimage")
                    {
                        if let Some(href_start) = ref_tag.find("href=\"") {
                            let value_start = href_start + 6;
                            if let Some(value_end) = ref_tag[value_start..].find('"') {
                                let href = &ref_tag[value_start..value_start + value_end];
                                let full_path = format!("{}{}", base_path, href);
                                if is_image_file(&full_path) {
                                    tracing::debug!(
                                        cover_path = %full_path,
                                        "Found cover image via guide reference"
                                    );
                                    return Some(full_path);
                                }
                            }
                        }
                    }

                    ref_remaining = &ref_section[ref_end..];
                } else {
                    break;
                }
            }
        }
    }

    // Method 3: Look for manifest item with ID containing "cover" that's an image
    for (id, path) in &manifest_items {
        let id_lower = id.to_lowercase();
        if (id_lower.contains("cover") || id_lower == "cvi") && is_image_file(path) {
            tracing::debug!(
                cover_id = %id,
                cover_path = %path,
                "Found cover image via manifest item ID heuristic"
            );
            return Some(path.clone());
        }
    }

    None
}

/// Extract the cover image from an EPUB file
///
/// This function first tries to find the cover image as specified in the OPF manifest,
/// then falls back to extracting images in alphabetical order if no cover is defined.
///
/// # Arguments
/// * `path` - Path to the EPUB file
///
/// # Returns
/// The raw image data as bytes
#[allow(dead_code)] // Public API - may be used by external callers
pub fn extract_cover_from_epub<P: AsRef<Path>>(path: P) -> anyhow::Result<Vec<u8>> {
    extract_cover_from_epub_with_fallback(path, true)
}

/// Extract the cover image from an EPUB file with optional fallback
///
/// This function:
/// 1. First tries to find the cover image as specified in the OPF manifest
/// 2. If no cover is defined in OPF, falls back to the first image alphabetically
/// 3. If `fallback_on_invalid` is true and the cover is corrupted, tries subsequent images
///
/// # Arguments
/// * `path` - Path to the EPUB file
/// * `fallback_on_invalid` - If true, try other images when the primary cover is corrupted
///
/// # Returns
/// The raw image data as bytes
pub fn extract_cover_from_epub_with_fallback<P: AsRef<Path>>(
    path: P,
    fallback_on_invalid: bool,
) -> anyhow::Result<Vec<u8>> {
    use crate::parsers::image_utils::is_valid_image_data;

    let path = path.as_ref();
    let file = File::open(path)?;
    let mut archive = ZipArchive::new(file)?;

    // Try to find the cover image from OPF first
    let opf_cover_path = find_cover_image_from_opf(&mut archive);

    if let Some(ref cover_path) = opf_cover_path {
        // Try to extract the OPF-specified cover image
        if let Ok(mut cover_file) = archive.by_name(cover_path) {
            let mut buffer = Vec::new();
            if cover_file.read_to_end(&mut buffer).is_ok() && is_valid_image_data(&buffer) {
                tracing::debug!(
                    cover_path = %cover_path,
                    size = buffer.len(),
                    "Successfully extracted cover image from OPF-specified path"
                );
                return Ok(buffer);
            } else {
                tracing::warn!(
                    cover_path = %cover_path,
                    "OPF-specified cover image is corrupted or unreadable"
                );
                // If fallback is disabled and the OPF cover is corrupted, fail
                if !fallback_on_invalid {
                    anyhow::bail!("Cover image specified in OPF is corrupted");
                }
            }
        }
    }

    // Fallback: get images from archive and try them in order
    // This happens when:
    // 1. No cover is defined in OPF (opf_cover_path is None)
    // 2. OPF cover is corrupted and fallback_on_invalid is true
    tracing::debug!("Falling back to alphabetical image order for cover extraction");

    // Re-open archive since we consumed it
    let file = File::open(path)?;
    let mut archive = ZipArchive::new(file)?;

    // Get list of image files in EPUB (only from archive, not checking manifest)
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

    if image_files.is_empty() {
        anyhow::bail!("No images found in EPUB");
    }

    // If fallback is disabled, only try the first image
    let images_to_try = if fallback_on_invalid {
        &image_files[..]
    } else {
        &image_files[..1]
    };

    // Try each image until we find a valid one
    for filename in images_to_try {
        let mut file = archive.by_name(filename)?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)?;

        if is_valid_image_data(&buffer) {
            if opf_cover_path.is_some() {
                tracing::info!(
                    filename = %filename,
                    "Using fallback image as cover (OPF cover was corrupted)"
                );
            } else {
                tracing::debug!(
                    filename = %filename,
                    "Using first image as cover (no cover defined in OPF)"
                );
            }
            return Ok(buffer);
        }

        tracing::warn!(
            filename = %filename,
            size = buffer.len(),
            "Skipping corrupted image in EPUB archive"
        );
    }

    anyhow::bail!("No valid images found in EPUB")
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
    extract_page_from_epub_with_fallback(path, page_number, false)
}

/// Extract a page image from an EPUB file with optional fallback for corrupted images
///
/// When `fallback_on_invalid` is true and the requested page image is corrupted,
/// this function will try subsequent images until it finds a valid one.
/// This is useful for thumbnail generation where any valid image is acceptable.
///
/// For page 1 (cover), this will first try to use the OPF-specified cover image.
///
/// # Arguments
/// * `path` - Path to the EPUB file
/// * `page_number` - Page number (1-indexed)
/// * `fallback_on_invalid` - If true, try subsequent images when the requested one is corrupted
///
/// # Returns
/// The raw image data as bytes, or an error if no valid images found
pub fn extract_page_from_epub_with_fallback<P: AsRef<Path>>(
    path: P,
    page_number: i32,
    fallback_on_invalid: bool,
) -> anyhow::Result<Vec<u8>> {
    // For page 1 (cover), use the smart cover extraction that checks OPF first
    if page_number == 1 {
        return extract_cover_from_epub_with_fallback(path, fallback_on_invalid);
    }

    // For other pages, use alphabetical order
    use crate::parsers::image_utils::is_valid_image_data;

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
    let start_index = (page_number - 1) as usize;
    if start_index >= image_files.len() {
        anyhow::bail!(
            "Page {} not found in EPUB (no images available)",
            page_number
        );
    }

    // Try to extract the requested page, with optional fallback to subsequent pages
    let end_index = if fallback_on_invalid {
        image_files.len()
    } else {
        start_index + 1
    };

    for (index, filename) in image_files
        .iter()
        .enumerate()
        .skip(start_index)
        .take(end_index - start_index)
    {
        let mut file = archive.by_name(filename)?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)?;

        // Validate the image data
        if is_valid_image_data(&buffer) {
            if index > start_index {
                tracing::info!(
                    original_page = page_number,
                    actual_index = index + 1,
                    filename = %filename,
                    "Using fallback image after skipping corrupted images in EPUB"
                );
            }
            return Ok(buffer);
        }

        tracing::warn!(
            page = index + 1,
            filename = %filename,
            size = buffer.len(),
            "Skipping corrupted image in EPUB archive"
        );
    }

    anyhow::bail!(
        "No valid images found in EPUB starting from page {}",
        page_number
    )
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
        let parser = EpubParser;
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
