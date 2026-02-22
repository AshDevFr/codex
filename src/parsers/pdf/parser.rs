use crate::parsers::isbn_utils::extract_isbns;
use crate::parsers::pdf::renderer;
use crate::parsers::traits::FormatParser;
use crate::parsers::{BookMetadata, FileFormat, ImageFormat, PageInfo};
use crate::utils::{CodexError, Result, hash_file};
use chrono::{DateTime, Utc};
use image::GenericImageView;
use lopdf::{Document, Object, ObjectId};
use std::path::Path;

/// Default DPI for rendering PDF pages when no embedded image is available
const DEFAULT_RENDER_DPI: u16 = 150;

/// Extracted PDF image data: (data bytes, format, width, height, size)
type PdfImageData = (Vec<u8>, ImageFormat, u32, u32, u64);

pub struct PdfParser;

impl PdfParser {
    pub fn new() -> Self {
        Self
    }

    /// Extract ISBNs from PDF metadata
    ///
    /// Searches for ISBNs in:
    /// - Info dictionary (ISBN, Keywords, Subject fields)
    /// - XMP metadata
    fn extract_isbns_from_pdf(doc: &Document) -> Vec<String> {
        let mut all_text = String::new();

        // Try to get the Info dictionary
        if let Ok(trailer) = doc.trailer.get(b"Info")
            && let Ok(info_ref) = trailer.as_reference()
            && let Ok(info_obj) = doc.get_object(info_ref)
            && let Ok(info_dict) = info_obj.as_dict()
        {
            // Check common metadata fields that might contain ISBNs
            let fields = vec![
                b"ISBN".as_ref(),
                b"Keywords".as_ref(),
                b"Subject".as_ref(),
                b"Title".as_ref(),
                b"Description".as_ref(),
            ];

            for field in fields {
                if let Ok(value) = info_dict.get(field)
                    && let Ok(bytes) = value.as_str()
                {
                    // Convert bytes to UTF-8 string, replacing invalid sequences
                    if let Ok(string) = String::from_utf8(bytes.to_vec()) {
                        all_text.push_str(&string);
                        all_text.push(' ');
                    }
                }
            }
        }

        // Extract ISBNs from collected text
        extract_isbns(&all_text, false)
    }

    /// Extract images from a PDF page by page index (0-based)
    fn extract_images_from_page(doc: &Document, page_num: u32) -> Result<Vec<PdfImageData>> {
        // Get all page IDs using our custom method that handles indirect Kids arrays
        let page_ids = Self::collect_page_ids(doc);

        // Get the page ID for this index
        let page_id = match page_ids.get(page_num as usize) {
            Some(id) => *id,
            None => return Ok(Vec::new()),
        };

        Self::extract_images_from_page_id(doc, page_id)
    }

    /// Extract images from a PDF page by page ObjectId
    fn extract_images_from_page_id(doc: &Document, page_id: ObjectId) -> Result<Vec<PdfImageData>> {
        let mut images = Vec::new();

        // Try to get the page object
        let page = match doc.get_object(page_id) {
            Ok(obj) => obj,
            Err(_) => return Ok(images),
        };

        // Get the page dictionary
        let page_dict = match page.as_dict() {
            Ok(dict) => dict,
            Err(_) => return Ok(images),
        };

        // Look for Resources
        if let Ok(resources_obj) = page_dict.get(b"Resources")
            && let Ok(resources) = resources_obj.as_dict()
        {
            // Look for XObject in Resources
            if let Ok(xobject_obj) = resources.get(b"XObject") {
                if let Ok(xobject_ref) = xobject_obj.as_reference() {
                    if let Ok(xobject_dict_obj) = doc.get_object(xobject_ref)
                        && let Ok(xobject_dict) = xobject_dict_obj.as_dict()
                    {
                        // Iterate through XObjects
                        for (_name, obj_ref) in xobject_dict.iter() {
                            if let Ok(obj_id) = obj_ref.as_reference()
                                && let Ok(stream_obj) = doc.get_object(obj_id)
                                && let Ok(stream) = stream_obj.as_stream()
                            {
                                // Check if it's an image
                                if let Ok(subtype) = stream.dict.get(b"Subtype")
                                    && let Ok(subtype_name) = subtype.as_name()
                                    && subtype_name == b"Image"
                                {
                                    // Try to extract the image
                                    if let Some(image_data) =
                                        Self::extract_image_stream(doc, stream)
                                    {
                                        images.push(image_data);
                                    }
                                }
                            }
                        }
                    }
                } else if let Ok(xobject_dict) = xobject_obj.as_dict() {
                    // XObject is directly a dictionary
                    for (_name, obj_ref) in xobject_dict.iter() {
                        if let Ok(obj_id) = obj_ref.as_reference()
                            && let Ok(stream_obj) = doc.get_object(obj_id)
                            && let Ok(stream) = stream_obj.as_stream()
                            && let Ok(subtype) = stream.dict.get(b"Subtype")
                            && let Ok(subtype_name) = subtype.as_name()
                            && subtype_name == b"Image"
                            && let Some(image_data) = Self::extract_image_stream(doc, stream)
                        {
                            images.push(image_data);
                        }
                    }
                }
            }
        }

        Ok(images)
    }

    /// Check if a PDF page has text content in its content stream
    ///
    /// Scans the page's content stream for PDF text operators (`BT`, `Tj`, `TJ`, `Tf`).
    /// Returns `true` if any text operators are found, indicating the page has text
    /// content that would be lost if we only extracted embedded images.
    fn page_has_text_content(doc: &Document, page_id: ObjectId) -> bool {
        let page = match doc.get_object(page_id) {
            Ok(obj) => obj,
            Err(_) => return false,
        };

        let page_dict = match page.as_dict() {
            Ok(dict) => dict,
            Err(_) => return false,
        };

        // Get the content stream(s) for this page
        // Use get_plain_content() which handles both compressed and uncompressed streams
        let content_bytes = match page_dict.get(b"Contents") {
            Ok(Object::Reference(ref_id)) => {
                // Single content stream reference
                match doc.get_object(*ref_id) {
                    Ok(Object::Stream(stream)) => stream.get_plain_content().unwrap_or_default(),
                    _ => return false,
                }
            }
            Ok(Object::Array(arr)) => {
                // Multiple content streams - concatenate them
                let mut all_bytes = Vec::new();
                for item in arr {
                    if let Ok(ref_id) = item.as_reference()
                        && let Ok(Object::Stream(stream)) = doc.get_object(ref_id)
                    {
                        all_bytes.extend(stream.get_plain_content().unwrap_or_default());
                        all_bytes.push(b' ');
                    }
                }
                all_bytes
            }
            _ => return false,
        };

        let content_str = String::from_utf8_lossy(&content_bytes);

        // Look for PDF text operators in the content stream:
        // BT = Begin Text object, ET = End Text object
        // Tj = Show text string, TJ = Show text with positioning
        // Tf = Set text font and size
        // We check for "BT" as the primary indicator since all text must be
        // within a BT/ET block. We use word boundaries to avoid false positives
        // from operator names appearing inside string literals.
        for token in content_str.split_whitespace() {
            if token == "BT" || token == "Tj" || token == "TJ" || token == "Tf" {
                return true;
            }
        }

        false
    }

    /// Extract image data from a PDF stream
    ///
    /// This function extracts embedded images from PDF streams. It only returns
    /// data that can be directly loaded by the `image` crate (JPEG, PNG, etc.).
    ///
    /// **Important**: PDF streams with `FlateDecode` filter contain raw pixel data
    /// (not PNG files). We don't try to reconstruct these as it requires knowledge
    /// of the colorspace, bit depth, and pixel layout. Instead, we fall back to
    /// PDFium rendering for such pages.
    fn extract_image_stream(
        _doc: &Document,
        stream: &lopdf::Stream,
    ) -> Option<(Vec<u8>, ImageFormat, u32, u32, u64)> {
        // Get image dimensions
        let width = stream.dict.get(b"Width").ok()?.as_i64().ok()? as u32;
        let height = stream.dict.get(b"Height").ok()?.as_i64().ok()? as u32;

        // Check the PDF filter to determine how to handle the stream
        let filter_bytes = stream.dict.get(b"Filter").ok()?.as_name().ok()?;
        let filter = std::str::from_utf8(filter_bytes).ok()?;

        match filter {
            // DCTDecode means the stream contains raw JPEG data
            "DCTDecode" => {
                // For JPEG, the raw stream content is the JPEG file
                // (no decompression needed - DCT is the JPEG compression itself)
                let content = stream.content.clone();

                // Verify it's actually JPEG by checking magic bytes
                if content.len() >= 3 && content[0] == 0xFF && content[1] == 0xD8 {
                    let file_size = content.len() as u64;
                    return Some((content, ImageFormat::JPEG, width, height, file_size));
                }

                tracing::debug!("DCTDecode stream doesn't have JPEG magic bytes, skipping");
                None
            }

            // JPXDecode means JPEG 2000 - not widely supported, skip
            "JPXDecode" => None,

            // FlateDecode means zlib-compressed raw pixel data, not a PNG file!
            // We cannot use this directly - need PDFium to render the page.
            // FlateDecode produces raw pixel data (RGB/CMYK/Grayscale bytes),
            // not a PNG file. Converting this to a usable image requires knowing
            // the exact colorspace, handling masks/SMasks, and reconstructing headers.
            "FlateDecode" => None,

            // Other filters (CCITTFaxDecode, JBIG2Decode, etc.) - let PDFium handle
            _ => {
                tracing::debug!(
                    filter = filter,
                    "Unsupported PDF filter, will use PDFium rendering"
                );
                None
            }
        }
    }
}

impl FormatParser for PdfParser {
    fn can_parse<P: AsRef<Path>>(&self, path: P) -> bool {
        path.as_ref()
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase() == "pdf")
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

        // Load the PDF document with lopdf
        let doc = Document::load(path)
            .map_err(|e| CodexError::ParseError(format!("Failed to load PDF: {}", e)))?;

        // Extract ISBNs from PDF metadata
        let isbns = Self::extract_isbns_from_pdf(&doc);

        // Get the actual page count from the PDF
        // Note: We use a custom page counter because lopdf's get_pages() doesn't handle
        // PDFs where the Kids array is stored as an indirect reference (object reference)
        // instead of an inline array. This is valid per PDF spec but lopdf doesn't support it.
        let page_count = Self::count_pages(&doc);

        // Build page info for each PDF page
        // We create an entry for every page regardless of whether it has an embedded image
        // This ensures the page count matches the actual PDF structure
        let pages = Self::build_page_info_list(&doc, path, page_count);

        Ok(BookMetadata {
            file_path: path.to_string_lossy().to_string(),
            format: FileFormat::PDF,
            file_size,
            file_hash,
            modified_at,
            page_count,
            pages,
            comic_info: None, // PDF doesn't use ComicInfo.xml
            isbns,
        })
    }
}

impl PdfParser {
    /// Count the total number of pages in the PDF document
    ///
    /// This function manually traverses the page tree to count pages because
    /// lopdf's `get_pages()` method doesn't handle PDFs where the `Kids` array
    /// in the Pages dictionary is stored as an indirect reference instead of
    /// an inline array. This is a valid PDF structure per the PDF specification.
    fn count_pages(doc: &Document) -> usize {
        // First try lopdf's built-in method (works for most PDFs)
        let lopdf_count = doc.get_pages().len();
        if lopdf_count > 0 {
            return lopdf_count;
        }

        // Fallback: manually traverse the page tree for PDFs with indirect Kids arrays
        Self::count_pages_from_catalog(doc).unwrap_or(0)
    }

    /// Count pages by traversing from the catalog root
    fn count_pages_from_catalog(doc: &Document) -> Option<usize> {
        // Get Root -> Pages reference from trailer
        let root_ref = doc.trailer.get(b"Root").ok()?.as_reference().ok()?;
        let catalog = doc.get_object(root_ref).ok()?.as_dict().ok()?;
        let pages_ref = catalog.get(b"Pages").ok()?.as_reference().ok()?;

        Some(Self::count_pages_in_node(doc, pages_ref))
    }

    /// Recursively count pages in a page tree node
    fn count_pages_in_node(doc: &Document, node_id: ObjectId) -> usize {
        let node = match doc.get_object(node_id) {
            Ok(obj) => obj,
            Err(_) => return 0,
        };

        let node_dict = match node.as_dict() {
            Ok(d) => d,
            Err(_) => return 0,
        };

        // Check the Type field to determine if this is a Page or Pages node
        let node_type = node_dict
            .get(b"Type")
            .ok()
            .and_then(|t| t.as_name().ok())
            .unwrap_or(b"");

        match node_type {
            b"Page" => 1,
            b"Pages" => {
                // Get Kids array - may be inline or an indirect reference
                let kids = match node_dict.get(b"Kids") {
                    Ok(Object::Array(arr)) => arr.clone(),
                    Ok(Object::Reference(ref_id)) => {
                        // Dereference the Kids array (this is the case lopdf doesn't handle)
                        match doc.get_object(*ref_id) {
                            Ok(obj) => match obj.as_array() {
                                Ok(arr) => arr.clone(),
                                Err(_) => return 0,
                            },
                            Err(_) => return 0,
                        }
                    }
                    _ => return 0,
                };

                // Recursively count pages in all children
                let mut count = 0;
                for kid in kids {
                    if let Ok(kid_id) = kid.as_reference() {
                        count += Self::count_pages_in_node(doc, kid_id);
                    }
                }
                count
            }
            _ => 0,
        }
    }

    /// Collect all page object IDs in document order
    ///
    /// This function traverses the page tree and collects all Page object IDs,
    /// handling the case where Kids arrays are indirect references.
    fn collect_page_ids(doc: &Document) -> Vec<ObjectId> {
        // First try lopdf's built-in method
        let lopdf_pages = doc.get_pages();
        if !lopdf_pages.is_empty() {
            // get_pages() returns BTreeMap<page_number, ObjectId> - we want the ObjectIds in order
            return lopdf_pages.values().copied().collect();
        }

        // Fallback: manually traverse the page tree
        Self::collect_page_ids_from_catalog(doc).unwrap_or_default()
    }

    /// Collect page IDs by traversing from the catalog root
    fn collect_page_ids_from_catalog(doc: &Document) -> Option<Vec<ObjectId>> {
        let root_ref = doc.trailer.get(b"Root").ok()?.as_reference().ok()?;
        let catalog = doc.get_object(root_ref).ok()?.as_dict().ok()?;
        let pages_ref = catalog.get(b"Pages").ok()?.as_reference().ok()?;

        let mut page_ids = Vec::new();
        Self::collect_page_ids_in_node(doc, pages_ref, &mut page_ids);
        Some(page_ids)
    }

    /// Recursively collect page IDs from a page tree node
    fn collect_page_ids_in_node(doc: &Document, node_id: ObjectId, page_ids: &mut Vec<ObjectId>) {
        let node = match doc.get_object(node_id) {
            Ok(obj) => obj,
            Err(_) => return,
        };

        let node_dict = match node.as_dict() {
            Ok(d) => d,
            Err(_) => return,
        };

        let node_type = node_dict
            .get(b"Type")
            .ok()
            .and_then(|t| t.as_name().ok())
            .unwrap_or(b"");

        match node_type {
            b"Page" => {
                page_ids.push(node_id);
            }
            b"Pages" => {
                let kids = match node_dict.get(b"Kids") {
                    Ok(Object::Array(arr)) => arr.clone(),
                    Ok(Object::Reference(ref_id)) => match doc.get_object(*ref_id) {
                        Ok(obj) => match obj.as_array() {
                            Ok(arr) => arr.clone(),
                            Err(_) => return,
                        },
                        Err(_) => return,
                    },
                    _ => return,
                };

                for kid in kids {
                    if let Ok(kid_id) = kid.as_reference() {
                        Self::collect_page_ids_in_node(doc, kid_id, page_ids);
                    }
                }
            }
            _ => {}
        }
    }

    /// Build page info list for all PDF pages
    ///
    /// This creates a PageInfo entry for every page in the PDF using per-page
    /// content detection:
    /// - **Image-only pages** (no text in content stream): Use embedded image
    ///   dimensions directly for accurate metadata.
    /// - **Mixed/text pages**: Use PDFium page dimensions (if available) since
    ///   the page will be rendered at extraction time.
    fn build_page_info_list(doc: &Document, path: &Path, page_count: usize) -> Vec<PageInfo> {
        let mut pages = Vec::with_capacity(page_count);
        let page_ids = Self::collect_page_ids(doc);

        for page_idx in 0..page_count {
            let page_number = page_idx + 1; // 1-indexed for PageInfo

            // Check if this page has text content
            let has_text = page_ids
                .get(page_idx)
                .map(|&pid| Self::page_has_text_content(doc, pid))
                .unwrap_or(false);

            // Image-only page: try to use embedded image info (fast, accurate)
            if !has_text
                && let Ok(page_images) = Self::extract_images_from_page(doc, page_idx as u32)
                && let Some((image_data, format, width, height, img_file_size)) =
                    page_images.into_iter().next()
            {
                // Try to verify dimensions with image crate
                let (final_width, final_height) =
                    if let Ok(img) = image::load_from_memory(&image_data) {
                        img.dimensions()
                    } else {
                        (width, height)
                    };

                pages.push(PageInfo {
                    page_number,
                    file_name: format!(
                        "page_{}.{}",
                        page_number,
                        match format {
                            ImageFormat::JPEG => "jpg",
                            ImageFormat::PNG => "png",
                            ImageFormat::WEBP => "webp",
                            ImageFormat::GIF => "gif",
                            ImageFormat::BMP => "bmp",
                            ImageFormat::AVIF => "avif",
                            ImageFormat::SVG => "svg",
                            ImageFormat::JXL => "jxl",
                        }
                    ),
                    format,
                    width: final_width,
                    height: final_height,
                    file_size: img_file_size,
                });
                continue;
            }

            // Mixed/text page or no embedded image: use PDFium dimensions
            let (width, height) = if renderer::is_initialized() {
                renderer::get_page_dimensions_pixels(path, page_number as i32, DEFAULT_RENDER_DPI)
                    .unwrap_or((1275, 1650)) // Default to US Letter at 150 DPI
            } else {
                (1275, 1650)
            };

            pages.push(PageInfo {
                page_number,
                file_name: format!("page_{}.jpg", page_number), // Rendered pages are JPEG
                format: ImageFormat::JPEG,
                width,
                height,
                file_size: 0, // Unknown until rendered
            });
        }

        pages
    }
}

impl Default for PdfParser {
    fn default() -> Self {
        Self::new()
    }
}

/// Extract a specific page image from a PDF file
///
/// Uses per-page content detection to choose the best extraction strategy:
/// - **Image-only pages**: Extract embedded image directly (fast, preserves quality)
/// - **Mixed/text pages**: Render with PDFium (captures full page content)
///
/// # Arguments
/// * `path` - Path to the PDF file
/// * `page_number` - Page number (1-indexed)
///
/// # Returns
/// The raw image data as bytes (JPEG format for rendered pages, original format for embedded images)
pub fn extract_page_from_pdf<P: AsRef<Path>>(path: P, page_number: i32) -> anyhow::Result<Vec<u8>> {
    extract_page_from_pdf_with_dpi(path, page_number, DEFAULT_RENDER_DPI)
}

/// Extract a specific page image from a PDF file with configurable DPI
///
/// Uses per-page content detection to choose the best extraction strategy:
/// - **Image-only pages** (no text operators in content stream): Extract the embedded
///   image directly. This is faster and preserves original quality.
/// - **Mixed-content or text-only pages**: Render with PDFium to capture the full page
///   (text, images, vectors, annotations).
///
/// # Arguments
/// * `path` - Path to the PDF file
/// * `page_number` - Page number (1-indexed)
/// * `dpi` - DPI for rendering (only used if page needs to be rendered via PDFium)
///
/// # Returns
/// The raw image data as bytes (JPEG format for rendered pages, original format for embedded images)
pub fn extract_page_from_pdf_with_dpi<P: AsRef<Path>>(
    path: P,
    page_number: i32,
    dpi: u16,
) -> anyhow::Result<Vec<u8>> {
    let path = path.as_ref();

    let doc = Document::load(path).map_err(|e| anyhow::anyhow!("Failed to load PDF: {}", e))?;

    let page_count = PdfParser::count_pages(&doc);
    if page_number < 1 || page_number as usize > page_count {
        anyhow::bail!(
            "Page {} out of range (PDF has {} pages)",
            page_number,
            page_count
        );
    }

    let page_index = (page_number - 1) as u32;
    let page_ids = PdfParser::collect_page_ids(&doc);

    // Check if the page has text content to decide extraction strategy
    let has_text = page_ids
        .get(page_index as usize)
        .map(|&pid| PdfParser::page_has_text_content(&doc, pid))
        .unwrap_or(false);

    if !has_text
        && let Ok(page_images) = PdfParser::extract_images_from_page(&doc, page_index)
        && let Some((image_data, _format, _width, _height, _file_size)) =
            page_images.into_iter().next()
    {
        // Image-only page: extract embedded image directly (fast path)
        tracing::debug!(
            path = %path.display(),
            page = page_number,
            "Extracted embedded image from image-only PDF page"
        );
        return Ok(image_data);
    } else if has_text {
        tracing::debug!(
            path = %path.display(),
            page = page_number,
            "PDF page has text content, using PDFium for full-page rendering"
        );
    }

    // Mixed/text page or no embedded image found: render with PDFium
    if !renderer::is_initialized() {
        // Last resort: try embedded image even on mixed pages when PDFium unavailable
        if let Ok(page_images) = PdfParser::extract_images_from_page(&doc, page_index)
            && let Some((image_data, _format, _width, _height, _file_size)) =
                page_images.into_iter().next()
        {
            tracing::warn!(
                path = %path.display(),
                page = page_number,
                "PDFium not available, falling back to embedded image extraction for mixed-content page"
            );
            return Ok(image_data);
        }

        anyhow::bail!(
            "Page {} could not be extracted from PDF: PDFium renderer is not available and no embedded image found",
            page_number
        );
    }

    tracing::debug!(
        path = %path.display(),
        page = page_number,
        dpi = dpi,
        "Rendering PDF page with PDFium"
    );

    renderer::render_page(path, page_number, dpi)
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::dictionary;

    #[test]
    fn test_pdf_parser_new() {
        let parser = PdfParser::new();
        assert!(parser.can_parse("test.pdf"));
    }

    #[test]
    fn test_pdf_parser_default() {
        let parser = PdfParser;
        assert!(parser.can_parse("test.pdf"));
    }

    #[test]
    fn test_pdf_parser_can_parse() {
        let parser = PdfParser::new();

        assert!(parser.can_parse("test.pdf"));
        assert!(parser.can_parse("test.PDF"));
        assert!(parser.can_parse("/path/to/file.pdf"));

        assert!(!parser.can_parse("test.cbz"));
        assert!(!parser.can_parse("test.cbr"));
        assert!(!parser.can_parse("test.epub"));
        assert!(!parser.can_parse("test.txt"));
    }

    #[test]
    fn test_extract_isbns_from_pdf_empty() {
        // Create a minimal PDF document
        let doc = Document::with_version("1.4");
        let isbns = PdfParser::extract_isbns_from_pdf(&doc);
        assert_eq!(isbns.len(), 0);
    }

    #[test]
    fn test_extract_page_from_pdf_invalid_path() {
        let result = extract_page_from_pdf("/nonexistent/file.pdf", 1);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        // When PDFium is not initialized, the embedded image extraction fails silently
        // and we get an error about PDFium not being available
        // When PDFium IS initialized, we get a "Failed to load PDF" error from PDFium
        assert!(
            err_msg.contains("Failed to load PDF")
                || err_msg.contains("No such file")
                || err_msg.contains("PDFium renderer is not available"),
            "Unexpected error message: {}",
            err_msg
        );
    }

    #[test]
    fn test_extract_page_from_pdf_with_dpi_invalid_path() {
        let result = extract_page_from_pdf_with_dpi("/nonexistent/file.pdf", 1, 200);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        // When PDFium is not initialized, the embedded image extraction fails silently
        // and we get an error about PDFium not being available
        // When PDFium IS initialized, we get a "Failed to load PDF" error from PDFium
        assert!(
            err_msg.contains("Failed to load PDF")
                || err_msg.contains("No such file")
                || err_msg.contains("PDFium renderer is not available"),
            "Unexpected error message: {}",
            err_msg
        );
    }

    #[test]
    fn test_extract_page_from_pdf_invalid_page_number() {
        let result = extract_page_from_pdf("/nonexistent/file.pdf", 0);
        assert!(result.is_err());

        let result = extract_page_from_pdf("/nonexistent/file.pdf", -1);
        assert!(result.is_err());
    }

    #[test]
    fn test_page_has_text_content_with_text() {
        // Build a minimal PDF with text content on a page
        let mut doc = Document::with_version("1.5");
        let pages_id = doc.new_object_id();
        let page_id = doc.new_object_id();
        let content_id = doc.new_object_id();

        let content_text = "BT /F1 24 Tf 100 700 Td (Hello World) Tj ET";
        let content =
            lopdf::Stream::new(lopdf::Dictionary::new(), content_text.as_bytes().to_vec());
        doc.objects.insert(content_id, Object::Stream(content));

        let page_dict = dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "Contents" => Object::Reference(content_id),
            "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
        };
        doc.objects.insert(page_id, Object::Dictionary(page_dict));

        assert!(PdfParser::page_has_text_content(&doc, page_id));
    }

    #[test]
    fn test_page_has_text_content_image_only() {
        // Build a minimal PDF with only image placement (no text operators)
        let mut doc = Document::with_version("1.5");
        let pages_id = doc.new_object_id();
        let page_id = doc.new_object_id();
        let content_id = doc.new_object_id();

        // Content stream with only image placement, no text
        let content_text = "q 612 0 0 792 0 0 cm /Im1 Do Q";
        let content =
            lopdf::Stream::new(lopdf::Dictionary::new(), content_text.as_bytes().to_vec());
        doc.objects.insert(content_id, Object::Stream(content));

        let page_dict = dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "Contents" => Object::Reference(content_id),
            "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
        };
        doc.objects.insert(page_id, Object::Dictionary(page_dict));

        assert!(!PdfParser::page_has_text_content(&doc, page_id));
    }

    #[test]
    fn test_page_has_text_content_mixed() {
        // Build a minimal PDF with both text and image content
        let mut doc = Document::with_version("1.5");
        let pages_id = doc.new_object_id();
        let page_id = doc.new_object_id();
        let content_id = doc.new_object_id();

        let content_text = "q 612 0 0 792 0 0 cm /Im1 Do Q BT /F1 12 Tf 100 100 Td (Caption) Tj ET";
        let content =
            lopdf::Stream::new(lopdf::Dictionary::new(), content_text.as_bytes().to_vec());
        doc.objects.insert(content_id, Object::Stream(content));

        let page_dict = dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "Contents" => Object::Reference(content_id),
            "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
        };
        doc.objects.insert(page_id, Object::Dictionary(page_dict));

        assert!(PdfParser::page_has_text_content(&doc, page_id));
    }

    #[test]
    fn test_default_render_dpi_constant() {
        // Verify the default DPI constant is set correctly
        assert_eq!(DEFAULT_RENDER_DPI, 150);
    }

    #[test]
    fn test_extract_page_uses_default_dpi() {
        // Verify that extract_page_from_pdf delegates to extract_page_from_pdf_with_dpi
        // We can't easily test this without a real PDF, but we verify the code path exists
        // by checking that both functions fail with the same error for invalid input
        let result1 = extract_page_from_pdf("/nonexistent/file.pdf", 1);
        let result2 =
            extract_page_from_pdf_with_dpi("/nonexistent/file.pdf", 1, DEFAULT_RENDER_DPI);

        assert!(result1.is_err());
        assert!(result2.is_err());
        // Both should fail with the same error type
        assert_eq!(
            result1.unwrap_err().to_string(),
            result2.unwrap_err().to_string()
        );
    }

    #[test]
    fn test_build_page_info_list_empty_pdf() {
        // Create a minimal PDF document (no pages)
        let doc = Document::with_version("1.4");
        let path = Path::new("/test/file.pdf");
        let pages = PdfParser::build_page_info_list(&doc, path, 0);
        assert!(pages.is_empty());
    }

    #[test]
    fn test_count_pages_empty_document() {
        // Create a minimal PDF document with no pages
        let doc = Document::with_version("1.4");
        let count = PdfParser::count_pages(&doc);
        assert_eq!(count, 0);
    }

    #[test]
    fn test_collect_page_ids_empty_document() {
        // Create a minimal PDF document with no pages
        let doc = Document::with_version("1.4");
        let page_ids = PdfParser::collect_page_ids(&doc);
        assert!(page_ids.is_empty());
    }

    // Note: Testing PDF page extraction with actual files requires
    // integration tests with real PDF fixtures.
    // See tests/parsers/pdf_rendering.rs for comprehensive tests.
}
