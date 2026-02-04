//! OPF (Open Packaging Format) metadata parser
//!
//! Parses Dublin Core metadata and Calibre extensions from OPF XML files.
//! Used for both embedded EPUB OPF content and Calibre sidecar `metadata.opf` files.

use crate::parsers::isbn_utils::extract_isbns;
use crate::parsers::ComicInfo;
use crate::utils::{CodexError, Result};
use std::path::Path;

/// Parsed OPF metadata
#[derive(Debug, Clone, Default)]
pub struct OpfMetadata {
    pub title: Option<String>,
    pub creators: Vec<String>,
    pub publisher: Option<String>,
    pub date: Option<String>,
    pub language: Option<String>,
    pub description: Option<String>,
    pub subjects: Vec<String>,
    pub isbns: Vec<String>,
    pub calibre_series: Option<String>,
    pub calibre_series_index: Option<f64>,
}

/// Parse OPF metadata from XML content.
///
/// Uses manual string-based XML parsing to handle Dublin Core namespace
/// variations (`dc:title`, `title`) reliably across EPUB 2/3 files.
pub fn parse_opf_metadata(xml: &str) -> Result<OpfMetadata> {
    let mut meta = OpfMetadata::default();

    // Extract metadata section for DC elements
    let metadata_section = extract_metadata_section(xml);
    let search_content = metadata_section.unwrap_or(xml);

    // Parse Dublin Core elements
    meta.title = extract_dc_element(search_content, "title");
    meta.publisher = extract_dc_element(search_content, "publisher");
    meta.date = extract_dc_element(search_content, "date");
    meta.language = extract_dc_element(search_content, "language");
    meta.description = extract_dc_element(search_content, "description");

    // Parse multiple creators
    meta.creators = extract_dc_elements(search_content, "creator");

    // Parse multiple subjects
    meta.subjects = extract_dc_elements(search_content, "subject");

    // Parse ISBNs from dc:identifier elements
    meta.isbns = extract_isbns_from_identifiers(search_content);

    // Parse Calibre extensions from <meta> tags
    parse_calibre_meta_tags(search_content, &mut meta);

    Ok(meta)
}

/// Read and parse an OPF file from disk.
pub fn parse_opf_file(path: &Path) -> Result<OpfMetadata> {
    let content = std::fs::read_to_string(path).map_err(|e| {
        CodexError::ParseError(format!("Failed to read OPF file {}: {}", path.display(), e))
    })?;
    parse_opf_metadata(&content)
}

/// Convert OPF metadata to a ComicInfo struct for integration with the analysis pipeline.
pub fn opf_to_comic_info(opf: &OpfMetadata) -> ComicInfo {
    let genre = if opf.subjects.is_empty() {
        None
    } else {
        Some(opf.subjects.join(", "))
    };

    let number = opf.calibre_series_index.map(|idx| {
        // Integer values render without decimal (e.g., "1"), floats keep it (e.g., "1.5")
        if idx.fract() == 0.0 {
            format!("{}", idx as i64)
        } else {
            format!("{idx}")
        }
    });

    let mut ci = ComicInfo {
        title: opf.title.clone(),
        writer: opf.creators.first().cloned(),
        publisher: opf.publisher.clone(),
        language_iso: opf.language.clone(),
        summary: opf.description.clone(),
        genre,
        series: opf.calibre_series.clone(),
        number,
        ..Default::default()
    };

    // Parse date into year/month/day
    if let Some(ref date_str) = opf.date {
        parse_date_into_comic_info(date_str, &mut ci);
    }

    ci
}

/// Merge two ComicInfo structs, with overlay fields taking precedence over base.
///
/// For each field, if the overlay has a `Some` value, it wins.
/// Otherwise, the base value is preserved.
pub fn merge_comic_info(base: &ComicInfo, overlay: &ComicInfo) -> ComicInfo {
    ComicInfo {
        title: overlay.title.clone().or_else(|| base.title.clone()),
        series: overlay.series.clone().or_else(|| base.series.clone()),
        number: overlay.number.clone().or_else(|| base.number.clone()),
        count: overlay.count.or(base.count),
        volume: overlay.volume.or(base.volume),
        summary: overlay.summary.clone().or_else(|| base.summary.clone()),
        year: overlay.year.or(base.year),
        month: overlay.month.or(base.month),
        day: overlay.day.or(base.day),
        writer: overlay.writer.clone().or_else(|| base.writer.clone()),
        penciller: overlay.penciller.clone().or_else(|| base.penciller.clone()),
        inker: overlay.inker.clone().or_else(|| base.inker.clone()),
        colorist: overlay.colorist.clone().or_else(|| base.colorist.clone()),
        letterer: overlay.letterer.clone().or_else(|| base.letterer.clone()),
        cover_artist: overlay
            .cover_artist
            .clone()
            .or_else(|| base.cover_artist.clone()),
        editor: overlay.editor.clone().or_else(|| base.editor.clone()),
        publisher: overlay.publisher.clone().or_else(|| base.publisher.clone()),
        imprint: overlay.imprint.clone().or_else(|| base.imprint.clone()),
        genre: overlay.genre.clone().or_else(|| base.genre.clone()),
        web: overlay.web.clone().or_else(|| base.web.clone()),
        page_count: overlay.page_count.or(base.page_count),
        language_iso: overlay
            .language_iso
            .clone()
            .or_else(|| base.language_iso.clone()),
        format: overlay.format.clone().or_else(|| base.format.clone()),
        black_and_white: overlay
            .black_and_white
            .clone()
            .or_else(|| base.black_and_white.clone()),
        manga: overlay.manga.clone().or_else(|| base.manga.clone()),
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Decode basic XML entities in text content.
fn decode_xml_entities(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
}

/// Extract the `<metadata>...</metadata>` section from the OPF XML.
fn extract_metadata_section(xml: &str) -> Option<&str> {
    // Handle both <metadata> and <metadata ...attributes...>
    let start = xml.find("<metadata")?;
    let section = &xml[start..];
    let end = section.find("</metadata>")?;
    Some(&section[..end + "</metadata>".len()])
}

/// Extract the text content of a single Dublin Core element.
///
/// Tries both `dc:tag` and bare `tag` forms to handle namespace variations.
fn extract_dc_element(xml: &str, tag: &str) -> Option<String> {
    // Try dc: prefixed form first (most common)
    if let Some(val) = extract_tag_content(xml, &format!("dc:{}", tag)) {
        return Some(val);
    }
    // Try bare form (some OPF files omit namespace prefix)
    extract_tag_content(xml, tag)
}

/// Extract the text content of all occurrences of a Dublin Core element.
fn extract_dc_elements(xml: &str, tag: &str) -> Vec<String> {
    let mut results = extract_all_tag_contents(xml, &format!("dc:{}", tag));
    // Also try bare form
    results.extend(extract_all_tag_contents(xml, tag));

    // Deduplicate while preserving order
    let mut seen = std::collections::HashSet::new();
    results.retain(|v| seen.insert(v.clone()));
    results
}

/// Extract the text content between `<tag ...>content</tag>` for the first occurrence.
fn extract_tag_content(xml: &str, tag: &str) -> Option<String> {
    let open = format!("<{}", tag);
    let close = format!("</{}>", tag);

    let start = xml.find(&open)?;
    let section = &xml[start..];
    let close_pos = section.find(&close)?;

    let tag_content = &section[..close_pos];
    // Find end of opening tag
    let content_start = tag_content.find('>')?;
    let content = &tag_content[content_start + 1..];
    let trimmed = content.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(decode_xml_entities(trimmed))
    }
}

/// Extract the text content of all occurrences of a tag.
fn extract_all_tag_contents(xml: &str, tag: &str) -> Vec<String> {
    let mut results = Vec::new();
    let open = format!("<{}", tag);
    let close = format!("</{}>", tag);

    let mut remaining = xml;
    while let Some(start) = remaining.find(&open) {
        let section = &remaining[start..];
        if let Some(close_pos) = section.find(&close) {
            let tag_content = &section[..close_pos];
            if let Some(content_start) = tag_content.find('>') {
                let content = tag_content[content_start + 1..].trim();
                if !content.is_empty() {
                    results.push(decode_xml_entities(content));
                }
            }
            remaining = &section[close_pos + close.len()..];
        } else {
            break;
        }
    }

    results
}

/// Extract ISBNs from `<dc:identifier>` elements.
///
/// Checks for explicit ISBN scheme attributes and ISBN patterns in content.
fn extract_isbns_from_identifiers(xml: &str) -> Vec<String> {
    let mut isbns = Vec::new();

    let mut remaining = xml;
    while let Some(start) = remaining.find("<dc:identifier") {
        let section = &remaining[start..];
        if let Some(close_pos) = section.find("</dc:identifier>") {
            let full_tag = &section[..close_pos + "</dc:identifier>".len()];

            let is_isbn_scheme = full_tag.contains("opf:scheme=\"ISBN\"")
                || full_tag.contains("opf:scheme='ISBN'")
                || full_tag.contains("scheme=\"ISBN\"")
                || full_tag.contains("scheme='ISBN'");

            if let Some(content_start) = full_tag.find('>') {
                let content =
                    &full_tag[content_start + 1..full_tag.len() - "</dc:identifier>".len()];

                if is_isbn_scheme || content.to_uppercase().contains("ISBN") {
                    isbns.extend(extract_isbns(content, false));
                }
            }

            remaining = &section[close_pos + "</dc:identifier>".len()..];
        } else {
            break;
        }
    }

    // Deduplicate while preserving order
    let mut seen = std::collections::HashSet::new();
    isbns.retain(|isbn| seen.insert(isbn.clone()));
    isbns
}

/// Parse Calibre-specific `<meta>` tags for series information.
///
/// Calibre stores series info as:
/// - `<meta name="calibre:series" content="Series Name"/>`
/// - `<meta name="calibre:series_index" content="1.0"/>`
fn parse_calibre_meta_tags(xml: &str, meta: &mut OpfMetadata) {
    let mut remaining = xml;
    while let Some(start) = remaining.find("<meta") {
        let section = &remaining[start..];
        // Find end of this meta tag (could be self-closing or have closing tag)
        let tag_end = if let Some(end) = section.find("/>") {
            end + 2
        } else if let Some(end) = section.find('>') {
            end + 1
        } else {
            break;
        };

        let meta_tag = &section[..tag_end];

        // Check for calibre:series
        if meta_tag.contains("name=\"calibre:series\"")
            || meta_tag.contains("name='calibre:series'")
        {
            if let Some(value) = extract_meta_content(meta_tag) {
                if !value.is_empty() {
                    meta.calibre_series = Some(value);
                }
            }
        }

        // Check for calibre:series_index
        if meta_tag.contains("name=\"calibre:series_index\"")
            || meta_tag.contains("name='calibre:series_index'")
        {
            if let Some(value) = extract_meta_content(meta_tag) {
                if let Ok(idx) = value.parse::<f64>() {
                    meta.calibre_series_index = Some(idx);
                }
            }
        }

        remaining = &section[tag_end..];
    }
}

/// Extract the `content` attribute value from a `<meta>` tag.
fn extract_meta_content(meta_tag: &str) -> Option<String> {
    // Try double quotes
    if let Some(start) = meta_tag.find("content=\"") {
        let value_start = start + "content=\"".len();
        if let Some(end) = meta_tag[value_start..].find('"') {
            return Some(meta_tag[value_start..value_start + end].to_string());
        }
    }
    // Try single quotes
    if let Some(start) = meta_tag.find("content='") {
        let value_start = start + "content='".len();
        if let Some(end) = meta_tag[value_start..].find('\'') {
            return Some(meta_tag[value_start..value_start + end].to_string());
        }
    }
    None
}

/// Parse a date string into year/month/day fields on ComicInfo.
///
/// Handles formats: `2024-01-15`, `2024-01`, `2024`, and ISO 8601 with time.
fn parse_date_into_comic_info(date_str: &str, ci: &mut ComicInfo) {
    // Strip any time portion (e.g., "2024-01-15T00:00:00")
    let date_part = date_str.split('T').next().unwrap_or(date_str).trim();

    let parts: Vec<&str> = date_part.split('-').collect();

    if let Some(year_str) = parts.first() {
        if let Ok(year) = year_str.parse::<i32>() {
            ci.year = Some(year);
        }
    }
    if let Some(month_str) = parts.get(1) {
        if let Ok(month) = month_str.parse::<i32>() {
            ci.month = Some(month);
        }
    }
    if let Some(day_str) = parts.get(2) {
        if let Ok(day) = day_str.parse::<i32>() {
            ci.day = Some(day);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    const BASIC_OPF: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="2.0">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:opf="http://www.idpf.org/2007/opf">
    <dc:title>Moby Dick</dc:title>
    <dc:creator opf:role="aut">Herman Melville</dc:creator>
    <dc:publisher>Harper &amp; Brothers</dc:publisher>
    <dc:date>1851-10-18</dc:date>
    <dc:language>en</dc:language>
    <dc:description>A novel about a whaling voyage.</dc:description>
    <dc:subject>Fiction</dc:subject>
    <dc:subject>Adventure</dc:subject>
    <dc:identifier opf:scheme="ISBN">978-0-14-243724-7</dc:identifier>
  </metadata>
</package>"#;

    const CALIBRE_OPF: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="2.0">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:opf="http://www.idpf.org/2007/opf">
    <dc:title>The Way of Kings</dc:title>
    <dc:creator opf:role="aut">Brandon Sanderson</dc:creator>
    <dc:publisher>Tor Books</dc:publisher>
    <dc:date>2010-08-31</dc:date>
    <dc:language>en</dc:language>
    <dc:description>Epic fantasy novel.</dc:description>
    <dc:subject>Fantasy</dc:subject>
    <dc:subject>Epic</dc:subject>
    <dc:identifier opf:scheme="ISBN">978-0-7653-2635-5</dc:identifier>
    <meta name="calibre:series" content="The Stormlight Archive"/>
    <meta name="calibre:series_index" content="1.0"/>
  </metadata>
</package>"#;

    #[test]
    fn test_parse_basic_opf() {
        let meta = parse_opf_metadata(BASIC_OPF).unwrap();
        assert_eq!(meta.title.as_deref(), Some("Moby Dick"));
        assert_eq!(meta.creators, vec!["Herman Melville"]);
        assert_eq!(meta.publisher.as_deref(), Some("Harper & Brothers"));
        assert_eq!(meta.date.as_deref(), Some("1851-10-18"));
        assert_eq!(meta.language.as_deref(), Some("en"));
        assert_eq!(
            meta.description.as_deref(),
            Some("A novel about a whaling voyage.")
        );
    }

    #[test]
    fn test_parse_calibre_extensions() {
        let meta = parse_opf_metadata(CALIBRE_OPF).unwrap();
        assert_eq!(
            meta.calibre_series.as_deref(),
            Some("The Stormlight Archive")
        );
        assert_eq!(meta.calibre_series_index, Some(1.0));
    }

    #[test]
    fn test_parse_multiple_subjects() {
        let meta = parse_opf_metadata(BASIC_OPF).unwrap();
        assert_eq!(meta.subjects, vec!["Fiction", "Adventure"]);
    }

    #[test]
    fn test_parse_isbn_extraction() {
        let meta = parse_opf_metadata(BASIC_OPF).unwrap();
        assert_eq!(meta.isbns.len(), 1);
        assert_eq!(meta.isbns[0], "9780142437247");
    }

    #[test]
    fn test_parse_date_full() {
        let meta = parse_opf_metadata(BASIC_OPF).unwrap();
        let ci = opf_to_comic_info(&meta);
        assert_eq!(ci.year, Some(1851));
        assert_eq!(ci.month, Some(10));
        assert_eq!(ci.day, Some(18));
    }

    #[test]
    fn test_parse_date_year_month() {
        let xml = r#"<?xml version="1.0"?>
<package><metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:date>2024-03</dc:date>
</metadata></package>"#;
        let meta = parse_opf_metadata(xml).unwrap();
        let ci = opf_to_comic_info(&meta);
        assert_eq!(ci.year, Some(2024));
        assert_eq!(ci.month, Some(3));
        assert!(ci.day.is_none());
    }

    #[test]
    fn test_parse_date_year_only() {
        let xml = r#"<?xml version="1.0"?>
<package><metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:date>2024</dc:date>
</metadata></package>"#;
        let meta = parse_opf_metadata(xml).unwrap();
        let ci = opf_to_comic_info(&meta);
        assert_eq!(ci.year, Some(2024));
        assert!(ci.month.is_none());
        assert!(ci.day.is_none());
    }

    #[test]
    fn test_parse_date_iso8601_with_time() {
        let xml = r#"<?xml version="1.0"?>
<package><metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:date>2024-01-15T00:00:00+00:00</dc:date>
</metadata></package>"#;
        let meta = parse_opf_metadata(xml).unwrap();
        let ci = opf_to_comic_info(&meta);
        assert_eq!(ci.year, Some(2024));
        assert_eq!(ci.month, Some(1));
        assert_eq!(ci.day, Some(15));
    }

    #[test]
    fn test_opf_to_comic_info() {
        let meta = parse_opf_metadata(CALIBRE_OPF).unwrap();
        let ci = opf_to_comic_info(&meta);

        assert_eq!(ci.title.as_deref(), Some("The Way of Kings"));
        assert_eq!(ci.writer.as_deref(), Some("Brandon Sanderson"));
        assert_eq!(ci.publisher.as_deref(), Some("Tor Books"));
        assert_eq!(ci.language_iso.as_deref(), Some("en"));
        assert_eq!(ci.summary.as_deref(), Some("Epic fantasy novel."));
        assert_eq!(ci.genre.as_deref(), Some("Fantasy, Epic"));
        assert_eq!(ci.series.as_deref(), Some("The Stormlight Archive"));
        assert_eq!(ci.number.as_deref(), Some("1"));
        assert_eq!(ci.year, Some(2010));
        assert_eq!(ci.month, Some(8));
        assert_eq!(ci.day, Some(31));
    }

    #[test]
    fn test_merge_comic_info_overlay_precedence() {
        let base = ComicInfo {
            title: Some("Base Title".to_string()),
            writer: Some("Base Writer".to_string()),
            publisher: Some("Base Publisher".to_string()),
            year: Some(2000),
            ..Default::default()
        };

        let overlay = ComicInfo {
            title: Some("Overlay Title".to_string()),
            writer: Some("Overlay Writer".to_string()),
            year: Some(2024),
            ..Default::default()
        };

        let merged = merge_comic_info(&base, &overlay);
        assert_eq!(merged.title.as_deref(), Some("Overlay Title"));
        assert_eq!(merged.writer.as_deref(), Some("Overlay Writer"));
        assert_eq!(merged.publisher.as_deref(), Some("Base Publisher"));
        assert_eq!(merged.year, Some(2024));
    }

    #[test]
    fn test_merge_comic_info_base_preserved() {
        let base = ComicInfo {
            title: Some("Base Title".to_string()),
            writer: Some("Base Writer".to_string()),
            publisher: Some("Base Publisher".to_string()),
            genre: Some("Fiction".to_string()),
            year: Some(2020),
            month: Some(6),
            ..Default::default()
        };

        let overlay = ComicInfo::default();

        let merged = merge_comic_info(&base, &overlay);
        assert_eq!(merged.title.as_deref(), Some("Base Title"));
        assert_eq!(merged.writer.as_deref(), Some("Base Writer"));
        assert_eq!(merged.publisher.as_deref(), Some("Base Publisher"));
        assert_eq!(merged.genre.as_deref(), Some("Fiction"));
        assert_eq!(merged.year, Some(2020));
        assert_eq!(merged.month, Some(6));
    }

    #[test]
    fn test_parse_empty_metadata() {
        let xml = r#"<?xml version="1.0"?>
<package><metadata></metadata></package>"#;
        let meta = parse_opf_metadata(xml).unwrap();
        assert!(meta.title.is_none());
        assert!(meta.creators.is_empty());
        assert!(meta.publisher.is_none());
        assert!(meta.date.is_none());
        assert!(meta.language.is_none());
        assert!(meta.description.is_none());
        assert!(meta.subjects.is_empty());
        assert!(meta.isbns.is_empty());
        assert!(meta.calibre_series.is_none());
        assert!(meta.calibre_series_index.is_none());
    }

    #[test]
    fn test_parse_multiple_creators() {
        let xml = r#"<?xml version="1.0"?>
<package><metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:creator opf:role="aut">Author One</dc:creator>
    <dc:creator opf:role="aut">Author Two</dc:creator>
    <dc:creator opf:role="ill">Illustrator</dc:creator>
</metadata></package>"#;
        let meta = parse_opf_metadata(xml).unwrap();
        assert_eq!(meta.creators.len(), 3);
        assert_eq!(meta.creators[0], "Author One");
        assert_eq!(meta.creators[1], "Author Two");
        assert_eq!(meta.creators[2], "Illustrator");

        // First creator becomes the writer
        let ci = opf_to_comic_info(&meta);
        assert_eq!(ci.writer.as_deref(), Some("Author One"));
    }

    #[test]
    fn test_parse_opf_file() {
        let mut file = NamedTempFile::new().unwrap();
        write!(file, "{}", BASIC_OPF).unwrap();

        let meta = parse_opf_file(file.path()).unwrap();
        assert_eq!(meta.title.as_deref(), Some("Moby Dick"));
        assert_eq!(meta.creators, vec!["Herman Melville"]);
    }

    #[test]
    fn test_parse_opf_file_not_found() {
        let result = parse_opf_file(Path::new("/nonexistent/metadata.opf"));
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_series_index_float() {
        let xml = r#"<?xml version="1.0"?>
<package><metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:title>Side Story</dc:title>
    <meta name="calibre:series" content="My Series"/>
    <meta name="calibre:series_index" content="1.5"/>
</metadata></package>"#;
        let meta = parse_opf_metadata(xml).unwrap();
        assert_eq!(meta.calibre_series_index, Some(1.5));

        let ci = opf_to_comic_info(&meta);
        assert_eq!(ci.number.as_deref(), Some("1.5"));
    }

    #[test]
    fn test_parse_series_index_integer() {
        let xml = r#"<?xml version="1.0"?>
<package><metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <meta name="calibre:series" content="My Series"/>
    <meta name="calibre:series_index" content="3.0"/>
</metadata></package>"#;
        let meta = parse_opf_metadata(xml).unwrap();
        let ci = opf_to_comic_info(&meta);
        assert_eq!(ci.number.as_deref(), Some("3"));
    }

    #[test]
    fn test_parse_isbn_from_content_prefix() {
        let xml = r#"<?xml version="1.0"?>
<package><metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:identifier>ISBN: 978-0-306-40615-7</dc:identifier>
</metadata></package>"#;
        let meta = parse_opf_metadata(xml).unwrap();
        assert_eq!(meta.isbns.len(), 1);
        assert_eq!(meta.isbns[0], "9780306406157");
    }

    #[test]
    fn test_parse_isbn_deduplicates() {
        let xml = r#"<?xml version="1.0"?>
<package><metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:identifier opf:scheme="ISBN">978-0-306-40615-7</dc:identifier>
    <dc:identifier>ISBN: 978-0-306-40615-7</dc:identifier>
</metadata></package>"#;
        let meta = parse_opf_metadata(xml).unwrap();
        assert_eq!(meta.isbns.len(), 1);
    }

    #[test]
    fn test_parse_non_isbn_identifier_ignored() {
        let xml = r#"<?xml version="1.0"?>
<package><metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:identifier>urn:uuid:12345-abcde</dc:identifier>
</metadata></package>"#;
        let meta = parse_opf_metadata(xml).unwrap();
        assert!(meta.isbns.is_empty());
    }

    #[test]
    fn test_opf_to_comic_info_no_subjects() {
        let xml = r#"<?xml version="1.0"?>
<package><metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:title>No Genre Book</dc:title>
</metadata></package>"#;
        let meta = parse_opf_metadata(xml).unwrap();
        let ci = opf_to_comic_info(&meta);
        assert!(ci.genre.is_none());
    }

    #[test]
    fn test_opf_to_comic_info_no_calibre_series() {
        let meta = parse_opf_metadata(BASIC_OPF).unwrap();
        let ci = opf_to_comic_info(&meta);
        assert!(ci.series.is_none());
        assert!(ci.number.is_none());
    }

    #[test]
    fn test_parse_bare_dc_elements() {
        // Some OPF files use bare element names without dc: prefix
        let xml = r#"<?xml version="1.0"?>
<package><metadata>
    <title>Bare Title</title>
    <creator>Bare Author</creator>
    <publisher>Bare Publisher</publisher>
    <language>fr</language>
</metadata></package>"#;
        let meta = parse_opf_metadata(xml).unwrap();
        assert_eq!(meta.title.as_deref(), Some("Bare Title"));
        assert_eq!(meta.creators, vec!["Bare Author"]);
        assert_eq!(meta.publisher.as_deref(), Some("Bare Publisher"));
        assert_eq!(meta.language.as_deref(), Some("fr"));
    }

    #[test]
    fn test_parse_calibre_single_quotes() {
        let xml = r#"<?xml version="1.0"?>
<package><metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <meta name='calibre:series' content='Test Series'/>
    <meta name='calibre:series_index' content='2.0'/>
</metadata></package>"#;
        let meta = parse_opf_metadata(xml).unwrap();
        assert_eq!(meta.calibre_series.as_deref(), Some("Test Series"));
        assert_eq!(meta.calibre_series_index, Some(2.0));
    }

    #[test]
    fn test_parse_empty_title_ignored() {
        let xml = r#"<?xml version="1.0"?>
<package><metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:title>  </dc:title>
</metadata></package>"#;
        let meta = parse_opf_metadata(xml).unwrap();
        assert!(meta.title.is_none());
    }

    #[test]
    fn test_parse_description_with_html() {
        let xml = r#"<?xml version="1.0"?>
<package><metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:description>&lt;p&gt;A &lt;b&gt;bold&lt;/b&gt; description.&lt;/p&gt;</dc:description>
</metadata></package>"#;
        let meta = parse_opf_metadata(xml).unwrap();
        // HTML entities are stored as-is (the XML layer already decoded &lt; etc.)
        assert!(meta.description.is_some());
    }
}
