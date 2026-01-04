use crate::parsers::{BookMetadata, FileFormat, ImageFormat, PageInfo};
use crate::parsers::traits::FormatParser;
use crate::utils::{hash_file, CodexError, Result};
use chrono::{DateTime, Utc};
use image::GenericImageView;
use lopdf::Document;
use std::path::Path;

pub struct PdfParser;

impl PdfParser {
    pub fn new() -> Self {
        Self
    }

    /// Extract images from a PDF page
    fn extract_images_from_page(
        doc: &Document,
        page_num: u32,
    ) -> Result<Vec<(Vec<u8>, ImageFormat, u32, u32, u64)>> {
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
                                                    if let Ok(subtype_name) = subtype.as_name_str() {
                                                        if subtype_name == "Image" {
                                                            // Try to extract the image
                                                            if let Some(image_data) = Self::extract_image_stream(doc, stream) {
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
                                                    if let Some(image_data) = Self::extract_image_stream(doc, stream) {
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
        let width = stream.dict.get(b"Width")
            .ok()?
            .as_i64()
            .ok()? as u32;

        let height = stream.dict.get(b"Height")
            .ok()?
            .as_i64()
            .ok()? as u32;

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

        // Load the PDF document
        let doc = Document::load(path)
            .map_err(|e| CodexError::ParseError(format!("Failed to load PDF: {}", e)))?;

        // Get page count
        let page_count = doc.get_pages().len();

        // Extract images from all pages
        let mut pages = Vec::new();
        let mut page_image_counter = 1;

        for page_num in 0..page_count as u32 {
            if let Ok(page_images) = Self::extract_images_from_page(&doc, page_num) {
                for (image_data, format, width, height, file_size) in page_images {
                    // Try to verify dimensions with image crate
                    let (final_width, final_height) = if let Ok(img) = image::load_from_memory(&image_data) {
                        img.dimensions()
                    } else {
                        (width, height)
                    };

                    pages.push(PageInfo {
                        page_number: page_image_counter,
                        file_name: format!("page_{}_image_{}.{}",
                            page_num + 1,
                            page_image_counter,
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
                        file_size,
                    });

                    page_image_counter += 1;
                }
            }
        }

        Ok(BookMetadata {
            file_path: path.to_string_lossy().to_string(),
            format: FileFormat::PDF,
            file_size,
            file_hash,
            modified_at,
            page_count,
            pages,
            comic_info: None, // PDF doesn't use ComicInfo.xml
            isbns: Vec::new(), // TODO: Extract from PDF metadata
        })
    }
}

impl Default for PdfParser {
    fn default() -> Self {
        Self::new()
    }
}

/// Extract a specific page image from a PDF file
///
/// # Arguments
/// * `path` - Path to the PDF file
/// * `page_number` - Page number (1-indexed)
///
/// # Returns
/// The raw image data as bytes
pub fn extract_page_from_pdf<P: AsRef<Path>>(path: P, page_number: i32) -> anyhow::Result<Vec<u8>> {
    let doc = Document::load(path)
        .map_err(|e| anyhow::anyhow!("Failed to load PDF: {}", e))?;

    // Get the total number of pages
    let page_count = doc.get_pages().len();

    // Extract images from all pages and find the one we need
    let mut current_image_index = 0;
    let target_index = (page_number - 1) as usize;

    for pdf_page_num in 0..page_count as u32 {
        if let Ok(page_images) = PdfParser::extract_images_from_page(&doc, pdf_page_num) {
            for (image_data, _format, _width, _height, _file_size) in page_images {
                if current_image_index == target_index {
                    return Ok(image_data);
                }
                current_image_index += 1;
            }
        }
    }

    anyhow::bail!("Page {} not found in PDF", page_number)
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
        let parser = PdfParser::default();
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
}

