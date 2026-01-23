use crate::parsers::isbn_utils::extract_isbns;
use crate::parsers::pdf::renderer;
use crate::parsers::traits::FormatParser;
use crate::parsers::{BookMetadata, FileFormat, ImageFormat, PageInfo};
use crate::utils::{hash_file, CodexError, Result};
use chrono::{DateTime, Utc};
use image::GenericImageView;
use lopdf::Document;
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
        if let Ok(trailer) = doc.trailer.get(b"Info") {
            if let Ok(info_ref) = trailer.as_reference() {
                if let Ok(info_obj) = doc.get_object(info_ref) {
                    if let Ok(info_dict) = info_obj.as_dict() {
                        // Check common metadata fields that might contain ISBNs
                        let fields = vec![
                            b"ISBN".as_ref(),
                            b"Keywords".as_ref(),
                            b"Subject".as_ref(),
                            b"Title".as_ref(),
                            b"Description".as_ref(),
                        ];

                        for field in fields {
                            if let Ok(value) = info_dict.get(field) {
                                if let Ok(bytes) = value.as_str() {
                                    // Convert bytes to UTF-8 string, replacing invalid sequences
                                    if let Ok(string) = String::from_utf8(bytes.to_vec()) {
                                        all_text.push_str(&string);
                                        all_text.push(' ');
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // Extract ISBNs from collected text
        extract_isbns(&all_text, false)
    }

    /// Extract images from a PDF page
    fn extract_images_from_page(doc: &Document, page_num: u32) -> Result<Vec<PdfImageData>> {
        let mut images = Vec::new();

        // Get the page dictionary
        let page_id = match doc.page_iter().nth(page_num as usize) {
            Some(id) => id,
            None => return Ok(images),
        };

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
        if let Ok(resources_obj) = page_dict.get(b"Resources") {
            if let Ok(resources) = resources_obj.as_dict() {
                // Look for XObject in Resources
                if let Ok(xobject_obj) = resources.get(b"XObject") {
                    if let Ok(xobject_ref) = xobject_obj.as_reference() {
                        if let Ok(xobject_dict_obj) = doc.get_object(xobject_ref) {
                            if let Ok(xobject_dict) = xobject_dict_obj.as_dict() {
                                // Iterate through XObjects
                                for (_name, obj_ref) in xobject_dict.iter() {
                                    if let Ok(obj_id) = obj_ref.as_reference() {
                                        if let Ok(stream_obj) = doc.get_object(obj_id) {
                                            if let Ok(stream) = stream_obj.as_stream() {
                                                // Check if it's an image
                                                if let Ok(subtype) = stream.dict.get(b"Subtype") {
                                                    if let Ok(subtype_name) = subtype.as_name_str()
                                                    {
                                                        if subtype_name == "Image" {
                                                            // Try to extract the image
                                                            if let Some(image_data) =
                                                                Self::extract_image_stream(
                                                                    doc, stream,
                                                                )
                                                            {
                                                                images.push(image_data);
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    } else if let Ok(xobject_dict) = xobject_obj.as_dict() {
                        // XObject is directly a dictionary
                        for (_name, obj_ref) in xobject_dict.iter() {
                            if let Ok(obj_id) = obj_ref.as_reference() {
                                if let Ok(stream_obj) = doc.get_object(obj_id) {
                                    if let Ok(stream) = stream_obj.as_stream() {
                                        if let Ok(subtype) = stream.dict.get(b"Subtype") {
                                            if let Ok(subtype_name) = subtype.as_name_str() {
                                                if subtype_name == "Image" {
                                                    if let Some(image_data) =
                                                        Self::extract_image_stream(doc, stream)
                                                    {
                                                        images.push(image_data);
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(images)
    }

    /// Extract image data from a PDF stream
    fn extract_image_stream(
        _doc: &Document,
        stream: &lopdf::Stream,
    ) -> Option<(Vec<u8>, ImageFormat, u32, u32, u64)> {
        // Get image dimensions
        let width = stream.dict.get(b"Width").ok()?.as_i64().ok()? as u32;

        let height = stream.dict.get(b"Height").ok()?.as_i64().ok()? as u32;

        // Try to decode the stream content
        let content = match stream.decompressed_content() {
            Ok(data) => data,
            Err(_) => stream.content.clone(),
        };

        let file_size = content.len() as u64;

        // Try to determine the image format
        // Check for DCTDecode (JPEG) filter
        let format = if let Ok(filter) = stream.dict.get(b"Filter") {
            if let Ok(filter_name) = filter.as_name_str() {
                match filter_name {
                    "DCTDecode" => Some(ImageFormat::JPEG),
                    "FlateDecode" => Some(ImageFormat::PNG), // Usually PNG-like compression
                    _ => None,
                }
            } else {
                None
            }
        } else {
            None
        };

        // If we couldn't determine format from filter, try to detect from content
        let format = format.or_else(|| {
            // Check magic bytes
            if content.len() >= 4 {
                if &content[0..2] == b"\xFF\xD8" {
                    Some(ImageFormat::JPEG)
                } else if &content[0..4] == b"\x89PNG" {
                    Some(ImageFormat::PNG)
                } else {
                    None
                }
            } else {
                None
            }
        })?;

        Some((content, format, width, height, file_size))
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
        let page_count = doc.get_pages().len();

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
    /// Build page info list for all PDF pages
    ///
    /// This creates a PageInfo entry for every page in the PDF. For pages with
    /// embedded images, we extract the image dimensions. For pages without
    /// embedded images (text-only, vector graphics), we try to get dimensions
    /// from PDFium if available, otherwise use default US Letter dimensions.
    fn build_page_info_list(doc: &Document, path: &Path, page_count: usize) -> Vec<PageInfo> {
        let mut pages = Vec::with_capacity(page_count);

        for page_idx in 0..page_count {
            let page_number = page_idx + 1; // 1-indexed for PageInfo

            // Try to get embedded image info first (fast path)
            if let Ok(page_images) = Self::extract_images_from_page(doc, page_idx as u32) {
                if let Some((image_data, format, width, height, img_file_size)) =
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
                            }
                        ),
                        format,
                        width: final_width,
                        height: final_height,
                        file_size: img_file_size,
                    });
                    continue;
                }
            }

            // No embedded image found - try to get dimensions from PDFium renderer
            let (width, height) = if renderer::is_initialized() {
                // Get pixel dimensions at default rendering DPI (page_number is 1-indexed for API)
                renderer::get_page_dimensions_pixels(path, page_number as i32, DEFAULT_RENDER_DPI)
                    .unwrap_or((1275, 1650)) // Default to US Letter at 150 DPI
            } else {
                // PDFium not available, use default US Letter dimensions at 150 DPI
                // 8.5" x 11" at 150 DPI = 1275 x 1650 pixels
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
/// This function uses a two-step strategy:
/// 1. **Fast path**: Try to extract an embedded image from the PDF page (works for scanned PDFs)
/// 2. **Fallback**: Render the page using PDFium (handles text-only and vector graphics PDFs)
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
/// This function uses a two-step strategy:
/// 1. **Fast path**: Try to extract an embedded image from the PDF page (works for scanned PDFs)
/// 2. **Fallback**: Render the page using PDFium (handles text-only and vector graphics PDFs)
///
/// # Arguments
/// * `path` - Path to the PDF file
/// * `page_number` - Page number (1-indexed)
/// * `dpi` - DPI for rendering (only used if page needs to be rendered)
///
/// # Returns
/// The raw image data as bytes (JPEG format for rendered pages, original format for embedded images)
pub fn extract_page_from_pdf_with_dpi<P: AsRef<Path>>(
    path: P,
    page_number: i32,
    dpi: u16,
) -> anyhow::Result<Vec<u8>> {
    let path = path.as_ref();

    // Fast path: try to extract embedded image first
    // This is much faster for PDFs that contain embedded images (scanned documents)
    if let Ok(image_data) = try_extract_embedded_image(path, page_number) {
        tracing::debug!(
            path = %path.display(),
            page = page_number,
            "Extracted embedded image from PDF page"
        );
        return Ok(image_data);
    }

    // Fallback: render the page using PDFium
    // This handles text-only PDFs, vector graphics, and mixed content
    if !renderer::is_initialized() {
        anyhow::bail!(
            "Page {} could not be extracted from PDF: no embedded image found and PDFium renderer is not available",
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

/// Try to extract an embedded image from a specific PDF page
///
/// This function attempts to find an embedded image XObject on the specified page.
/// It's a "fast path" that works well for scanned PDFs where each page is essentially
/// a single large image.
///
/// # Arguments
/// * `path` - Path to the PDF file
/// * `page_number` - Page number (1-indexed)
///
/// # Returns
/// * `Ok(Vec<u8>)` - The raw image data
/// * `Err` - If no suitable embedded image could be found on this page
fn try_extract_embedded_image<P: AsRef<Path>>(
    path: P,
    page_number: i32,
) -> anyhow::Result<Vec<u8>> {
    let doc = Document::load(path).map_err(|e| anyhow::anyhow!("Failed to load PDF: {}", e))?;

    // Validate page number
    let page_count = doc.get_pages().len();
    if page_number < 1 || page_number as usize > page_count {
        anyhow::bail!(
            "Page {} out of range (PDF has {} pages)",
            page_number,
            page_count
        );
    }

    // Get images from the specific page (0-indexed)
    let page_index = (page_number - 1) as u32;
    let page_images = PdfParser::extract_images_from_page(&doc, page_index)?;

    // Return the first (and typically only) image from this page
    // For scanned PDFs, each page usually has exactly one full-page image
    if let Some((image_data, _format, _width, _height, _file_size)) = page_images.into_iter().next()
    {
        return Ok(image_data);
    }

    anyhow::bail!("No embedded image found on page {}", page_number)
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_try_extract_embedded_image_invalid_path() {
        let result = try_extract_embedded_image("/nonexistent/file.pdf", 1);
        assert!(result.is_err());
    }

    #[test]
    fn test_try_extract_embedded_image_invalid_page_number() {
        // Create a minimal PDF to test page validation
        // We can't easily create a valid PDF in memory, so we test with a path
        // that would fail at the Document::load step
        let result = try_extract_embedded_image("/nonexistent/file.pdf", 0);
        assert!(result.is_err());

        let result = try_extract_embedded_image("/nonexistent/file.pdf", -1);
        assert!(result.is_err());
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

    // Note: Testing PDF page extraction with actual files requires
    // integration tests with real PDF fixtures.
    // See tests/parsers/pdf_rendering.rs for comprehensive tests.
}
