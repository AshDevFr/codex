//! Komga-compatible EPUB manifest and resource handlers
//!
//! Provides endpoints for streaming EPUB reading via the Readium WebPub Manifest format.
//! This enables apps like Komic to read EPUBs without downloading the entire file.

use super::super::dto::manifest::{WebPubLink, WebPubManifest, WebPubMetadata, WebPubTocEntry};
use crate::api::{
    error::ApiError,
    extractors::{AuthState, FlexibleAuthContext},
    permissions::Permission,
};
use crate::db::repositories::{BookMetadataRepository, BookRepository};
use crate::parsers::epub::EpubParser;
use crate::require_permission;
use axum::{
    body::Body,
    extract::{OriginalUri, Path, State},
    http::{StatusCode, header},
    response::Response,
};
use std::collections::HashSet;
use std::io::Read;
use std::sync::Arc;
use uuid::Uuid;
use zip::ZipArchive;

/// Get EPUB manifest (Readium WebPub Manifest)
///
/// Returns a Readium WebPub Manifest JSON for an EPUB book, enabling
/// streaming EPUB reading in compatible apps.
///
/// ## Endpoint
/// `GET /{prefix}/api/v1/books/{bookId}/manifest/epub`
///
/// ## Authentication
/// - Bearer token (JWT)
/// - Basic Auth
/// - API Key
#[utoipa::path(
    get,
    path = "/{prefix}/api/v1/books/{book_id}/manifest/epub",
    responses(
        (status = 200, description = "EPUB WebPub Manifest", body = WebPubManifest),
        (status = 400, description = "Book is not EPUB format"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Book not found"),
    ),
    params(
        ("prefix" = String, Path, description = "Komga API prefix (default: komga)"),
        ("book_id" = Uuid, Path, description = "Book ID")
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Komga"
)]
pub async fn get_epub_manifest(
    State(state): State<Arc<AuthState>>,
    FlexibleAuthContext(auth): FlexibleAuthContext,
    OriginalUri(uri): OriginalUri,
    Path(book_id): Path<Uuid>,
) -> Result<Response, ApiError> {
    require_permission!(auth, Permission::BooksRead)?;

    let book = BookRepository::get_by_id(&state.db, book_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch book: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Book not found".to_string()))?;

    if book.format.to_lowercase() != "epub" {
        return Err(ApiError::BadRequest(
            "Book is not in EPUB format".to_string(),
        ));
    }

    // Derive base URL for resource links from the request URI.
    // URI is like: /{prefix}/api/v1/books/{id}/manifest/epub
    // We need:     /{prefix}/api/v1/books/{id}/resource/
    let uri_path = uri.path().to_string();
    let base_url = uri_path
        .rfind("/manifest/epub")
        .map(|pos| &uri_path[..pos])
        .unwrap_or(&uri_path);

    // Open EPUB as ZIP
    let file_path = book.file_path.clone();
    let (manifest_items, spine_order, toc_entries, metadata) =
        tokio::task::spawn_blocking(move || -> Result<_, ApiError> {
            let file = std::fs::File::open(&file_path)
                .map_err(|e| ApiError::Internal(format!("Failed to open EPUB file: {}", e)))?;
            let mut archive = ZipArchive::new(file)
                .map_err(|e| ApiError::Internal(format!("Failed to read EPUB archive: {}", e)))?;

            let opf_path = EpubParser::find_root_file(&mut archive)
                .map_err(|e| ApiError::Internal(format!("Failed to find OPF: {}", e)))?;

            let (manifest, spine) = EpubParser::parse_opf(&mut archive, &opf_path)
                .map_err(|e| ApiError::Internal(format!("Failed to parse OPF: {}", e)))?;

            // Parse TOC from NCX file
            let toc = parse_toc(&mut archive, &manifest, &opf_path);

            Ok((manifest, spine, toc, opf_path))
        })
        .await
        .map_err(|e| ApiError::Internal(format!("Task join error: {}", e)))??;

    let _ = metadata; // opf_path not needed further

    // Get book metadata for title/author
    let book_metadata = BookMetadataRepository::get_by_book_id(&state.db, book_id)
        .await
        .ok()
        .flatten();

    let title = book_metadata
        .as_ref()
        .and_then(|m| m.title.clone())
        .unwrap_or_else(|| book.file_name.clone());

    let authors: Vec<String> = book_metadata
        .as_ref()
        .and_then(|m| m.authors_json.as_ref())
        .and_then(|json| {
            // authors_json is stored as JSON array of objects with "name" and "role"
            serde_json::from_str::<Vec<serde_json::Value>>(json)
                .ok()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.get("name").and_then(|n| n.as_str()).map(String::from))
                        .collect()
                })
        })
        .unwrap_or_default();

    // Build spine href set for separating reading_order from resources
    let spine_hrefs: HashSet<&str> = spine_order.iter().map(|(href, _)| href.as_str()).collect();

    // Build readingOrder
    let reading_order: Vec<WebPubLink> = spine_order
        .iter()
        .map(|(href, media_type)| WebPubLink {
            href: format!("{}/resource/{}", base_url, encode_resource_path(href)),
            media_type: media_type.clone(),
            rel: None,
        })
        .collect();

    // Build resources (manifest items not in spine)
    let resources: Vec<WebPubLink> = manifest_items
        .values()
        .filter(|(href, _)| !spine_hrefs.contains(href.as_str()))
        .map(|(href, media_type)| WebPubLink {
            href: format!("{}/resource/{}", base_url, encode_resource_path(href)),
            media_type: media_type.clone(),
            rel: None,
        })
        .collect();

    // Build TOC with rewritten hrefs
    let toc: Vec<WebPubTocEntry> = toc_entries
        .into_iter()
        .map(|entry| rewrite_toc_hrefs(entry, base_url))
        .collect();

    // Build self and acquisition links (matches Komga format)
    let manifest_href = format!("{}/manifest", base_url);
    let file_href = format!("{}/file", base_url);
    let links = vec![
        WebPubLink {
            href: manifest_href,
            media_type: "application/webpub+json".to_string(),
            rel: Some("self".to_string()),
        },
        WebPubLink {
            href: file_href,
            media_type: "application/epub+zip".to_string(),
            rel: Some("http://opds-spec.org/acquisition".to_string()),
        },
    ];

    let manifest = WebPubManifest {
        context: "https://readium.org/webpub-manifest/context.jsonld".to_string(),
        metadata: WebPubMetadata {
            identifier: format!("urn:uuid:{}", book_id),
            title,
            conforms_to: "https://readium.org/webpub-manifest/profiles/epub".to_string(),
            contributor: authors,
            language: None,
            modified: None,
            number_of_pages: book.page_count,
            rendition: super::super::dto::manifest::WebPubRendition {
                layout: "reflowable".to_string(),
            },
        },
        reading_order,
        resources,
        toc,
        images: Vec::new(),
        landmarks: Vec::new(),
        links,
        page_list: Vec::new(),
    };

    let body = serde_json::to_vec(&manifest)
        .map_err(|e| ApiError::Internal(format!("Failed to serialize manifest: {}", e)))?;

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/webpub+json")
        .header(header::CONTENT_LENGTH, body.len())
        .body(Body::from(body))
        .unwrap())
}

/// Get a resource file from within an EPUB
///
/// Serves individual files (XHTML chapters, CSS, images, fonts) from within
/// an EPUB archive. Used by EPUB readers to load content referenced in the manifest.
///
/// ## Endpoint
/// `GET /{prefix}/api/v1/books/{bookId}/resource/*resource`
///
/// ## Authentication
/// - Bearer token (JWT)
/// - Basic Auth
/// - API Key
#[utoipa::path(
    get,
    path = "/{prefix}/api/v1/books/{book_id}/resource/{resource}",
    responses(
        (status = 200, description = "Resource file content"),
        (status = 400, description = "Invalid resource path"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Book or resource not found"),
    ),
    params(
        ("prefix" = String, Path, description = "Komga API prefix (default: komga)"),
        ("book_id" = Uuid, Path, description = "Book ID"),
        ("resource" = String, Path, description = "Resource path within the EPUB")
    ),
    security(
        ("jwt_bearer" = []),
        ("api_key" = [])
    ),
    tag = "Komga"
)]
pub async fn get_epub_resource(
    State(state): State<Arc<AuthState>>,
    FlexibleAuthContext(auth): FlexibleAuthContext,
    Path((book_id, resource)): Path<(Uuid, String)>,
) -> Result<Response, ApiError> {
    require_permission!(auth, Permission::BooksRead)?;

    // Decode percent-encoded path; strip leading '/' from wildcard capture
    let resource = resource.strip_prefix('/').unwrap_or(&resource);
    let resource = percent_decode(resource);

    // Security: reject path traversal attempts
    if resource.contains("..") || resource.starts_with('/') {
        return Err(ApiError::BadRequest("Invalid resource path".to_string()));
    }

    let book = BookRepository::get_by_id(&state.db, book_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to fetch book: {}", e)))?
        .ok_or_else(|| ApiError::NotFound("Book not found".to_string()))?;

    if book.format.to_lowercase() != "epub" {
        return Err(ApiError::BadRequest(
            "Book is not in EPUB format".to_string(),
        ));
    }

    let file_path = book.file_path.clone();
    let resource_path = resource.clone();

    let (data, content_type) =
        tokio::task::spawn_blocking(move || -> Result<(Vec<u8>, String), ApiError> {
            let file = std::fs::File::open(&file_path)
                .map_err(|e| ApiError::Internal(format!("Failed to open EPUB file: {}", e)))?;
            let mut archive = ZipArchive::new(file)
                .map_err(|e| ApiError::Internal(format!("Failed to read EPUB archive: {}", e)))?;

            let mut entry = archive.by_name(&resource_path).map_err(|_| {
                ApiError::NotFound(format!("Resource not found in EPUB: {}", resource_path))
            })?;

            let mut buf = Vec::with_capacity(entry.size() as usize);
            entry
                .read_to_end(&mut buf)
                .map_err(|e| ApiError::Internal(format!("Failed to read resource: {}", e)))?;

            // Determine content type from file extension
            let ct = mime_guess::from_path(&resource_path)
                .first_or_octet_stream()
                .to_string();

            Ok((buf, ct))
        })
        .await
        .map_err(|e| ApiError::Internal(format!("Task join error: {}", e)))??;

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, content_type)
        .header(header::CONTENT_LENGTH, data.len())
        .header(header::CACHE_CONTROL, "public, max-age=86400")
        .body(Body::from(data))
        .unwrap())
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Parse TOC from NCX (EPUB 2) or nav document (EPUB 3)
fn parse_toc(
    archive: &mut ZipArchive<std::fs::File>,
    manifest: &std::collections::HashMap<String, (String, String)>,
    _opf_path: &str,
) -> Vec<WebPubTocEntry> {
    // Try NCX first (EPUB 2) - look for application/x-dtbncx+xml in manifest
    if let Some((ncx_href, _)) = manifest
        .values()
        .find(|(_, mt)| mt == "application/x-dtbncx+xml")
        && let Ok(entries) = parse_ncx(archive, ncx_href)
        && !entries.is_empty()
    {
        return entries;
    }

    // Try EPUB 3 nav document: check all xhtml files for epub:type="toc"
    for (nav_href, _) in manifest
        .values()
        .filter(|(_, mt)| mt == "application/xhtml+xml")
    {
        if let Ok(entries) = parse_nav_doc(archive, nav_href)
            && !entries.is_empty()
        {
            return entries;
        }
    }

    Vec::new()
}

/// Parse NCX file for table of contents
fn parse_ncx(
    archive: &mut ZipArchive<std::fs::File>,
    ncx_href: &str,
) -> Result<Vec<WebPubTocEntry>, ()> {
    let mut ncx_file = archive.by_name(ncx_href).map_err(|_| ())?;
    let mut content = String::new();
    ncx_file.read_to_string(&mut content).map_err(|_| ())?;

    // Determine base path from NCX href for resolving relative paths
    let base_path = ncx_href
        .rfind('/')
        .map(|pos| &ncx_href[..pos + 1])
        .unwrap_or("");

    Ok(parse_nav_points(&content, base_path))
}

/// Recursively parse navPoint elements from NCX content
fn parse_nav_points(content: &str, base_path: &str) -> Vec<WebPubTocEntry> {
    let mut entries = Vec::new();
    let mut remaining = content;

    while let Some(np_start) = remaining.find("<navPoint") {
        let section = &remaining[np_start..];

        // Find the matching closing tag, accounting for nesting
        let Some(inner_start) = section.find('>') else {
            break;
        };
        let inner = &section[inner_start + 1..];

        // Extract navLabel > text
        let title = extract_between(inner, "<text>", "</text>")
            .or_else(|| extract_between(inner, "<text >", "</text>"))
            .unwrap_or_default();

        // Extract content src
        let href = inner
            .find("<content")
            .and_then(|pos| {
                let tag = &inner[pos..];
                extract_attr(tag, "src")
            })
            .unwrap_or_default();

        // Resolve relative href
        let full_href = if href.is_empty() || href.starts_with('/') {
            href.clone()
        } else {
            format!("{}{}", base_path, href)
        };

        // Find nested navPoints (children)
        // Look for the next </navPoint> to delimit this entry
        let children_content = find_nav_point_children(inner);
        let children = if !children_content.is_empty() {
            parse_nav_points(children_content, base_path)
        } else {
            Vec::new()
        };

        if !title.is_empty() {
            entries.push(WebPubTocEntry {
                href: full_href,
                title,
                children,
            });
        }

        // Move past this navPoint's opening tag to find the next sibling
        // We need to skip past nested navPoints, so find the closing </navPoint>
        if let Some(close_pos) = find_closing_nav_point(section) {
            remaining = &section[close_pos..];
        } else {
            break;
        }
    }

    entries
}

/// Find the content between the first nested navPoint and the closing </navPoint>
fn find_nav_point_children(content: &str) -> &str {
    // Check if there are nested navPoints
    if let Some(first_child) = content.find("<navPoint")
        && let Some(close) = content.rfind("</navPoint>")
        && first_child < close
    {
        return &content[first_child..close];
    }
    ""
}

/// Find the position after the matching closing </navPoint> tag
fn find_closing_nav_point(content: &str) -> Option<usize> {
    let mut depth = 0;
    let mut pos = 0;

    while pos < content.len() {
        if content[pos..].starts_with("<navPoint") {
            depth += 1;
            pos += 9; // skip "<navPoint"
        } else if content[pos..].starts_with("</navPoint>") {
            depth -= 1;
            if depth == 0 {
                return Some(pos + 11); // skip "</navPoint>"
            }
            pos += 11;
        } else {
            pos += 1;
        }
    }
    None
}

/// Parse EPUB 3 nav document for table of contents
fn parse_nav_doc(
    archive: &mut ZipArchive<std::fs::File>,
    nav_href: &str,
) -> Result<Vec<WebPubTocEntry>, ()> {
    let mut nav_file = archive.by_name(nav_href).map_err(|_| ())?;
    let mut content = String::new();
    nav_file.read_to_string(&mut content).map_err(|_| ())?;

    // Look for <nav epub:type="toc"> ... </nav>
    let toc_nav = content
        .find("epub:type=\"toc\"")
        .or_else(|| content.find("epub:type='toc'"));

    let Some(nav_pos) = toc_nav else {
        return Ok(Vec::new());
    };

    // Find the <ol> within this nav
    let nav_section = &content[nav_pos..];
    let Some(ol_start) = nav_section.find("<ol") else {
        return Ok(Vec::new());
    };
    let ol_section = &nav_section[ol_start..];

    let base_path = nav_href
        .rfind('/')
        .map(|pos| &nav_href[..pos + 1])
        .unwrap_or("");

    Ok(parse_nav_ol(ol_section, base_path))
}

/// Parse an <ol> element from EPUB 3 nav document
fn parse_nav_ol(content: &str, base_path: &str) -> Vec<WebPubTocEntry> {
    let mut entries = Vec::new();
    let mut remaining = content;

    while let Some(li_start) = remaining.find("<li") {
        let section = &remaining[li_start..];
        let Some(li_end) = find_closing_tag(section, "li") else {
            break;
        };
        let li_content = &section[..li_end];

        // Extract <a href="...">Title</a>
        if let Some(a_start) = li_content.find("<a") {
            let a_section = &li_content[a_start..];
            let href = extract_attr(a_section, "href").unwrap_or_default();
            let title = extract_between(a_section, ">", "</a>")
                .map(|t| strip_html_tags(&t))
                .unwrap_or_default();

            let full_href = if href.is_empty() || href.starts_with('/') {
                href
            } else {
                format!("{}{}", base_path, href)
            };

            // Check for nested <ol> (children)
            let children = if let Some(ol_pos) = li_content.find("<ol") {
                parse_nav_ol(&li_content[ol_pos..], base_path)
            } else {
                Vec::new()
            };

            if !title.is_empty() {
                entries.push(WebPubTocEntry {
                    href: full_href,
                    title,
                    children,
                });
            }
        }

        remaining = &section[li_end..];
    }

    entries
}

/// Find the position after the closing tag for the given element
fn find_closing_tag(content: &str, tag: &str) -> Option<usize> {
    let close_tag = format!("</{}>", tag);
    content.find(&close_tag).map(|pos| pos + close_tag.len())
}

/// Extract text between two delimiters
fn extract_between(content: &str, start: &str, end: &str) -> Option<String> {
    let s = content.find(start)?;
    let after = &content[s + start.len()..];
    let e = after.find(end)?;
    Some(after[..e].trim().to_string())
}

/// Extract an attribute value from an XML/HTML tag
fn extract_attr(tag: &str, attr: &str) -> Option<String> {
    let pattern = format!("{}=\"", attr);
    let start = tag.find(&pattern)?;
    let after = &tag[start + pattern.len()..];
    let end = after.find('"')?;
    Some(after[..end].to_string())
}

/// Strip HTML tags from a string, leaving only text content
fn strip_html_tags(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut in_tag = false;
    for ch in input.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => result.push(ch),
            _ => {}
        }
    }
    result.trim().to_string()
}

/// Rewrite TOC entry hrefs to point to the resource endpoint
fn rewrite_toc_hrefs(entry: WebPubTocEntry, base_url: &str) -> WebPubTocEntry {
    WebPubTocEntry {
        href: format!(
            "{}/resource/{}",
            base_url,
            encode_resource_path(&entry.href)
        ),
        title: entry.title,
        children: entry
            .children
            .into_iter()
            .map(|child| rewrite_toc_hrefs(child, base_url))
            .collect(),
    }
}

/// Percent-encode a resource path for use in URLs, preserving path separators and common chars
fn encode_resource_path(path: &str) -> String {
    // For resource paths, we mostly just need to handle spaces and special chars.
    // Keep path separators, alphanumeric, dots, hyphens, and underscores as-is.
    let mut result = String::with_capacity(path.len());
    for byte in path.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'/' | b'#' => {
                result.push(byte as char);
            }
            _ => {
                result.push('%');
                result.push_str(&format!("{:02X}", byte));
            }
        }
    }
    result
}

/// Decode a percent-encoded path
fn percent_decode(path: &str) -> String {
    let mut result = Vec::with_capacity(path.len());
    let bytes = path.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let Ok(byte) = u8::from_str_radix(&path[i + 1..i + 3], 16)
        {
            result.push(byte);
            i += 3;
            continue;
        }
        result.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&result).into_owned()
}
