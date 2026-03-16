use crate::parsers::image_utils::{get_image_format, get_svg_dimensions, is_image_file};
use crate::parsers::isbn_utils::extract_isbns;
use crate::parsers::metadata::{SpineItem, compute_epub_positions};
use crate::parsers::opf;
use crate::parsers::traits::FormatParser;
use crate::parsers::{BookMetadata, FileFormat, ImageFormat, PageInfo};
use crate::utils::{CodexError, Result, hash_file};
use chrono::{DateTime, Utc};
use image::GenericImageView;
use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use zip::ZipArchive;

pub struct EpubParser;

/// Count text characters in XHTML content, excluding HTML markup, scripts, and styles.
///
/// Uses a simple state-machine to strip tags and count visible text characters.
/// Returns 0 for empty or non-UTF-8 content.
pub fn count_text_chars(xhtml: &[u8]) -> u64 {
    let text = match std::str::from_utf8(xhtml) {
        Ok(s) => s,
        Err(_) => return 0,
    };

    let mut char_count: u64 = 0;
    let mut in_tag = false;
    let mut in_script = false;
    let mut in_style = false;
    let mut tag_buf = String::new();

    for ch in text.chars() {
        if ch == '<' {
            in_tag = true;
            tag_buf.clear();
            continue;
        }

        if in_tag {
            if ch == '>' {
                in_tag = false;
                let tag_lower = tag_buf.to_ascii_lowercase();
                let tag_name = tag_lower
                    .split(|c: char| c.is_whitespace())
                    .next()
                    .unwrap_or("");

                match tag_name.trim_start_matches('/') {
                    "script" => in_script = !tag_name.starts_with('/'),
                    "style" => in_style = !tag_name.starts_with('/'),
                    _ => {}
                }
            } else {
                tag_buf.push(ch);
            }
            continue;
        }

        if !in_script && !in_style {
            char_count += 1;
        }
    }

    char_count
}

/// Find the next occurrence of an XML tag, handling optional namespace prefixes.
/// For example, searching for "item" will match both `<item ` and `<opf:item `.
/// Returns the byte offset of the `<` character and the full tag prefix length
/// (e.g., `<item ` or `<opf:item `).
fn find_xml_tag<'a>(haystack: &'a str, local_name: &str) -> Option<(usize, &'a str)> {
    let bare_space = format!("<{} ", local_name);
    let bare_gt = format!("<{}>", local_name);
    let mut search_from = 0;
    while search_from < haystack.len() {
        let remaining = &haystack[search_from..];
        // Try bare tag first: `<tag ` (with attributes) or `<tag>` (no attributes)
        if let Some(pos) = remaining
            .find(bare_space.as_str())
            .or_else(|| remaining.find(bare_gt.as_str()))
        {
            return Some((search_from + pos, &haystack[search_from + pos..]));
        }
        // Try namespace-prefixed: look for `:<local_name> ` or `:<local_name>` preceded by `<` and a prefix
        let prefixed_suffix = format!(":{}", local_name);
        if let Some(colon_pos) = remaining.find(prefixed_suffix.as_str()) {
            // Check the character after the local_name to ensure it's a complete tag name
            let after_pos = colon_pos + prefixed_suffix.len();
            if after_pos < remaining.len() {
                let next_char = remaining.as_bytes()[after_pos];
                if next_char == b' ' || next_char == b'>' || next_char == b'/' {
                    // Walk backwards from colon to find `<`
                    let before_colon = &remaining[..colon_pos];
                    if let Some(lt_pos) = before_colon.rfind('<') {
                        // Verify the prefix between `<` and `:` is a valid XML name (no spaces)
                        let prefix = &before_colon[lt_pos + 1..];
                        if !prefix.is_empty() && !prefix.contains(' ') && !prefix.contains('>') {
                            let abs_pos = search_from + lt_pos;
                            return Some((abs_pos, &haystack[abs_pos..]));
                        }
                    }
                }
            }
            search_from += colon_pos + prefixed_suffix.len();
        } else {
            break;
        }
    }
    None
}

/// Find the closing tag for an XML element, handling optional namespace prefixes.
/// For example, searching for "spine" will match both `</spine>` and `</opf:spine>`.
fn find_xml_closing_tag(haystack: &str, local_name: &str) -> Option<usize> {
    let bare = format!("</{}>", local_name);
    if let Some(pos) = haystack.find(bare.as_str()) {
        return Some(pos);
    }
    // Try namespace-prefixed closing tags
    let suffix = format!(":{}>", local_name);
    if let Some(suffix_pos) = haystack.find(suffix.as_str()) {
        // Walk backwards to find `</`
        let before = &haystack[..suffix_pos];
        if let Some(lt_pos) = before.rfind("</") {
            let prefix = &before[lt_pos + 2..];
            if !prefix.is_empty() && !prefix.contains(' ') && !prefix.contains('>') {
                return Some(lt_pos);
            }
        }
    }
    None
}

impl EpubParser {
    pub fn new() -> Self {
        Self
    }

    /// Parse the EPUB container.xml to find the root file (usually content.opf)
    pub(crate) fn find_root_file(archive: &mut ZipArchive<File>) -> Result<String> {
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
    ///
    /// Returns (manifest: id -> (href, media_type), spine_order: Vec<(href, media_type)>)
    #[allow(clippy::type_complexity)]
    pub(crate) fn parse_opf(
        archive: &mut ZipArchive<File>,
        opf_path: &str,
    ) -> Result<(HashMap<String, (String, String)>, Vec<(String, String)>)> {
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

        // Parse manifest to get id -> (href, media_type) mapping
        let mut manifest: HashMap<String, (String, String)> = HashMap::new();

        // Simple XML parsing for manifest items (handles both <item> and <opf:item>)
        let mut remaining = &xml_content[..];
        while let Some((_pos, item_section)) = find_xml_tag(remaining, "item") {
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

                // Extract media-type
                let media_type = if let Some(mt_start) = item_tag.find("media-type=\"") {
                    let mt_value_start = mt_start + 12;
                    item_tag[mt_value_start..]
                        .find('"')
                        .map(|mt_end| &item_tag[mt_value_start..mt_value_start + mt_end])
                } else {
                    None
                };

                if let (Some(id), Some(href)) = (id, href) {
                    // Combine base path with href
                    let full_path = format!("{}{}", base_path, href);
                    let mt = media_type.unwrap_or("application/octet-stream").to_string();
                    manifest.insert(id.to_string(), (full_path, mt));
                }

                remaining = &item_section[item_end..];
            } else {
                break;
            }
        }

        // Parse spine to get reading order (idref list)
        // Handles both <spine> and <opf:spine>, <itemref> and <opf:itemref>
        let mut spine_order: Vec<(String, String)> = Vec::new();
        remaining = &xml_content[..];

        if let Some((_pos, spine_section)) = find_xml_tag(remaining, "spine")
            && let Some(spine_end) = find_xml_closing_tag(spine_section, "spine")
        {
            let spine_content = &spine_section[..spine_end];

            // Extract itemrefs
            let mut itemref_remaining = spine_content;
            while let Some((_pos, itemref_section)) = find_xml_tag(itemref_remaining, "itemref") {
                if let Some(itemref_end) = itemref_section.find('>') {
                    let itemref_tag = &itemref_section[..itemref_end];

                    // Extract idref
                    if let Some(idref_start) = itemref_tag.find("idref=\"") {
                        let idref_value_start = idref_start + 7;
                        if let Some(idref_end) = itemref_tag[idref_value_start..].find('"') {
                            let idref =
                                &itemref_tag[idref_value_start..idref_value_start + idref_end];
                            if let Some((path, mt)) = manifest.get(idref) {
                                spine_order.push((path.clone(), mt.clone()));
                            }
                        }
                    }

                    itemref_remaining = &itemref_section[itemref_end..];
                } else {
                    break;
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

        // Build spine items with file sizes and character counts for position normalization
        let spine_items: Vec<SpineItem> = spine_order
            .iter()
            .filter_map(|(href, media_type)| {
                let mut entry = archive.by_name(href).ok()?;
                let file_size = entry.size();

                // Count text characters for XHTML spine items
                let char_count = if media_type.contains("xhtml") || media_type.contains("html") {
                    let mut content = Vec::new();
                    std::io::Read::read_to_end(&mut entry, &mut content).ok();
                    count_text_chars(&content)
                } else {
                    0
                };

                Some(SpineItem {
                    href: href.clone(),
                    media_type: media_type.clone(),
                    file_size,
                    char_count,
                })
            })
            .collect();

        // Compute Readium positions (1 position per 1024 bytes)
        let epub_positions = compute_epub_positions(&spine_items);

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
        // Use the Readium positions count if available (the standard way to count EPUB "pages").
        // This matches Komga's approach and provides consistent page counts across apps.
        // Fall back to max(spine items, image count) for edge cases.
        let page_count = if !epub_positions.is_empty() {
            epub_positions.len()
        } else {
            spine_order.len().max(pages.len())
        };

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
            epub_positions: if epub_positions.is_empty() {
                None
            } else {
                Some(epub_positions)
            },
            epub_spine_items: if spine_items.is_empty() {
                None
            } else {
                Some(spine_items)
            },
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
    // Handles both <item> and <opf:item> namespace-prefixed tags
    let mut manifest_items: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();
    let mut remaining = &opf_content[..];

    while let Some((_pos, item_section)) = find_xml_tag(remaining, "item") {
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
                            if let Some(cover_path) = manifest_items.get(cover_id)
                                && is_image_file(cover_path)
                            {
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

                meta_remaining = &tag_section[tag_end..];
            } else {
                break;
            }
        }
    }

    // Method 2: Look for <reference type="cover" href="..."/> in guide section
    if let Some(guide_start) = opf_content.find("<guide")
        && let Some(guide_end) = opf_content[guide_start..].find("</guide>")
    {
        let guide_section = &opf_content[guide_start..guide_start + guide_end];

        let mut ref_remaining = guide_section;
        while let Some(ref_start) = ref_remaining.find("<reference") {
            let ref_section = &ref_remaining[ref_start..];
            if let Some(ref_end) = ref_section.find('>') {
                let ref_tag = &ref_section[..ref_end];

                // Check for type="cover" or type containing "cover"
                if (ref_tag.contains("type=\"cover\"")
                    || ref_tag.contains("type='cover'")
                    || ref_tag.contains("coverimage"))
                    && let Some(href_start) = ref_tag.find("href=\"")
                {
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

                ref_remaining = &ref_section[ref_end..];
            } else {
                break;
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

    #[test]
    fn test_find_xml_tag_bare() {
        let xml = r#"<item id="ch1" href="ch1.xhtml"/>"#;
        let result = find_xml_tag(xml, "item");
        assert!(result.is_some());
        let (pos, _section) = result.unwrap();
        assert_eq!(pos, 0);
    }

    #[test]
    fn test_find_xml_tag_namespaced() {
        let xml = r#"<opf:item id="ch1" href="ch1.xhtml"/>"#;
        let result = find_xml_tag(xml, "item");
        assert!(result.is_some());
        let (pos, _section) = result.unwrap();
        assert_eq!(pos, 0);
    }

    #[test]
    fn test_find_xml_tag_no_match() {
        let xml = r#"<spine toc="ncx">"#;
        let result = find_xml_tag(xml, "item");
        assert!(result.is_none());
    }

    #[test]
    fn test_find_xml_closing_tag_bare() {
        let xml = r#"<spine toc="ncx"><itemref idref="ch1"/></spine>"#;
        let result = find_xml_closing_tag(xml, "spine");
        assert!(result.is_some());
        assert_eq!(&xml[result.unwrap()..], "</spine>");
    }

    #[test]
    fn test_find_xml_closing_tag_namespaced() {
        let xml = r#"<opf:spine toc="ncx"><opf:itemref idref="ch1"/></opf:spine>"#;
        let result = find_xml_closing_tag(xml, "spine");
        assert!(result.is_some());
        assert_eq!(&xml[result.unwrap()..], "</opf:spine>");
    }

    #[test]
    fn test_parse_opf_with_namespace_prefixed_tags() {
        // Create a minimal EPUB with namespace-prefixed OPF tags
        use std::io::Write;
        let temp_dir = tempfile::tempdir().unwrap();
        let epub_path = temp_dir.path().join("test.epub");

        let mut zip = zip::ZipWriter::new(File::create(&epub_path).unwrap());

        // mimetype
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        zip.start_file("mimetype", options).unwrap();
        zip.write_all(b"application/epub+zip").unwrap();

        // container.xml
        let options = zip::write::SimpleFileOptions::default();
        zip.start_file("META-INF/container.xml", options).unwrap();
        zip.write_all(
            br#"<?xml version="1.0"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
  <rootfiles>
    <rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/>
  </rootfiles>
</container>"#,
        )
        .unwrap();

        // OPF with opf: namespace prefix (like the Merlin EPUB)
        zip.start_file("OEBPS/content.opf", options).unwrap();
        zip.write_all(br#"<?xml version="1.0" encoding="utf-8"?>
<opf:package xmlns:opf="http://www.idpf.org/2007/opf" xmlns:dc="http://purl.org/dc/elements/1.1/" unique-identifier="id" version="2.0">
  <opf:metadata>
    <dc:title>Test Book</dc:title>
    <dc:creator>Test Author</dc:creator>
  </opf:metadata>
  <opf:manifest>
    <opf:item id="ch1" href="ch1.xhtml" media-type="application/xhtml+xml"/>
    <opf:item id="ch2" href="ch2.xhtml" media-type="application/xhtml+xml"/>
    <opf:item id="ch3" href="ch3.xhtml" media-type="application/xhtml+xml"/>
  </opf:manifest>
  <opf:spine toc="ncx">
    <opf:itemref idref="ch1"/>
    <opf:itemref idref="ch2"/>
    <opf:itemref idref="ch3"/>
  </opf:spine>
</opf:package>"#).unwrap();

        // Create dummy XHTML files
        for name in &["OEBPS/ch1.xhtml", "OEBPS/ch2.xhtml", "OEBPS/ch3.xhtml"] {
            zip.start_file(*name, options).unwrap();
            zip.write_all(b"<html><body><p>Content</p></body></html>")
                .unwrap();
        }

        zip.finish().unwrap();

        // Parse and verify
        let parser = EpubParser::new();
        let metadata = parser.parse(&epub_path).unwrap();
        // Should find 3 spine items (not fall back to 0 due to namespace issues)
        assert_eq!(
            metadata.page_count, 3,
            "Should parse 3 spine items from namespace-prefixed OPF"
        );
    }

    #[test]
    fn test_parse_opf_without_namespace_prefix() {
        // Verify bare tags still work
        use std::io::Write;
        let temp_dir = tempfile::tempdir().unwrap();
        let epub_path = temp_dir.path().join("test.epub");

        let mut zip = zip::ZipWriter::new(File::create(&epub_path).unwrap());

        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        zip.start_file("mimetype", options).unwrap();
        zip.write_all(b"application/epub+zip").unwrap();

        let options = zip::write::SimpleFileOptions::default();
        zip.start_file("META-INF/container.xml", options).unwrap();
        zip.write_all(
            br#"<?xml version="1.0"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
  <rootfiles>
    <rootfile full-path="content.opf" media-type="application/oebps-package+xml"/>
  </rootfiles>
</container>"#,
        )
        .unwrap();

        zip.start_file("content.opf", options).unwrap();
        zip.write_all(
            br#"<?xml version="1.0"?>
<package xmlns="http://www.idpf.org/2007/opf" version="2.0">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:title>Test</dc:title>
  </metadata>
  <manifest>
    <item id="ch1" href="ch1.xhtml" media-type="application/xhtml+xml"/>
    <item id="ch2" href="ch2.xhtml" media-type="application/xhtml+xml"/>
  </manifest>
  <spine toc="ncx">
    <itemref idref="ch1"/>
    <itemref idref="ch2"/>
  </spine>
</package>"#,
        )
        .unwrap();

        for name in &["ch1.xhtml", "ch2.xhtml"] {
            zip.start_file(*name, options).unwrap();
            zip.write_all(b"<html><body><p>Content</p></body></html>")
                .unwrap();
        }

        zip.finish().unwrap();

        let parser = EpubParser::new();
        let metadata = parser.parse(&epub_path).unwrap();
        assert_eq!(
            metadata.page_count, 2,
            "Should parse 2 spine items from bare OPF tags"
        );
    }

    mod count_text_chars_tests {
        use super::*;

        #[test]
        fn test_plain_xhtml() {
            let xhtml = b"<html><body><p>Hello world</p></body></html>";
            assert_eq!(count_text_chars(xhtml), 11);
        }

        #[test]
        fn test_script_excluded() {
            let xhtml =
                b"<html><body><p>Hello</p><script>var x = 1;</script><p>World</p></body></html>";
            assert_eq!(count_text_chars(xhtml), 10); // "Hello" + "World"
        }

        #[test]
        fn test_style_excluded() {
            let xhtml =
                b"<html><head><style>body { color: red; }</style></head><body>Text</body></html>";
            assert_eq!(count_text_chars(xhtml), 4);
        }

        #[test]
        fn test_cjk_characters() {
            let xhtml = "<html><body><p>\u{4F60}\u{597D}\u{4E16}\u{754C}</p></body></html>";
            assert_eq!(count_text_chars(xhtml.as_bytes()), 4);
        }

        #[test]
        fn test_empty_content() {
            assert_eq!(count_text_chars(b""), 0);
        }

        #[test]
        fn test_whitespace_counted() {
            let xhtml = b"<p>Hello World</p>";
            // "Hello World" = 11 chars including the space
            assert_eq!(count_text_chars(xhtml), 11);
        }

        #[test]
        fn test_nested_tags() {
            let xhtml = b"<div><span>A</span><em>B</em></div>";
            assert_eq!(count_text_chars(xhtml), 2);
        }

        #[test]
        fn test_invalid_utf8() {
            let invalid = [0xFF, 0xFE, 0x00];
            assert_eq!(count_text_chars(&invalid), 0);
        }
    }
}
