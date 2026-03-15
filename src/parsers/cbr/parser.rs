use crate::parsers::image_utils::{create_page_info, is_image_file, process_image_data};
use crate::parsers::traits::FormatParser;
use crate::parsers::{BookMetadata, FileFormat, parse_comic_info};
use crate::utils::{CodexError, Result, hash_file};
use chrono::{DateTime, Utc};
use std::path::Path;
use unrar::Archive;

pub struct CbrParser;

impl CbrParser {
    pub fn new() -> Self {
        Self
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
        let mut archive = Archive::new(
            path.to_str()
                .ok_or_else(|| CodexError::ParseError("Invalid path encoding".to_string()))?,
        )
        .open_for_processing()
        .map_err(|e| CodexError::ParseError(format!("Failed to open RAR archive: {}", e)))?;

        // Collect all entries with their data
        let mut image_data_entries: Vec<(String, Vec<u8>, u64)> = Vec::new();
        let mut comic_info = None;

        loop {
            let header = match archive.read_header() {
                Ok(Some(h)) => h,
                Ok(None) => break,
                Err(e) => {
                    return Err(CodexError::ParseError(format!(
                        "Failed to read RAR header: {}",
                        e
                    )));
                }
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
            } else if is_image_file(&filename) {
                // Read image data
                let (data, next) = header
                    .read()
                    .map_err(|e| CodexError::ParseError(format!("Failed to read image: {}", e)))?;

                image_data_entries.push((filename, data, unpacked_size));
                archive = next;
            } else {
                // Skip non-image, non-ComicInfo files
                archive = header
                    .skip()
                    .map_err(|e| CodexError::ParseError(format!("Failed to skip file: {}", e)))?;
            }
        }

        // Sort images by filename for page order
        image_data_entries.sort_by(|a, b| a.0.cmp(&b.0));

        // Process images to extract dimensions - skip files that fail verification
        let mut pages = Vec::new();
        for (filename, data, unpacked_size) in image_data_entries.iter() {
            // Process image: verify format and extract dimensions
            // Skip files that don't pass verification
            let Some(processed) = process_image_data(filename, data) else {
                tracing::debug!(
                    filename = %filename,
                    "Skipping file: could not verify image format or dimensions"
                );
                continue;
            };

            // Assign page number based on successfully processed pages
            let page_number = pages.len() + 1;
            pages.push(create_page_info(
                page_number,
                filename.clone(),
                processed,
                *unpacked_size,
            ));
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
            // TODO: Implement barcode detection for CBR files (deferred)
            //
            // Barcode detection for ISBN extraction from comic book covers has been
            // intentionally deferred due to complexity vs. benefit trade-offs.
            //
            // WHY DEFERRED:
            // - High implementation complexity (barcode detection + image processing)
            // - Limited practical benefit (most comic barcodes are not machine-readable)
            // - Significant performance impact (scanning images is expensive)
            // - Lower priority compared to other metadata extraction features
            //
            // WHAT WOULD BE REQUIRED:
            // 1. Barcode detection library (e.g., bardecoder, rxing)
            // 2. Image processing to locate and extract barcodes from cover pages
            // 3. Performance optimization for large archives (thousands of images)
            // 4. Accuracy validation and error handling
            // 5. Configuration option to enable/disable (due to performance cost)
            //
            // ALTERNATIVES:
            // - Manual metadata editing via API (recommended)
            // - External tools to extract ISBNs before import
            // - ComicInfo.xml metadata (if available)
            //
            // IF IMPLEMENTING IN THE FUTURE:
            // - Only scan first/last few pages (likely cover locations)
            // - Cache results to avoid re-scanning on every parse
            // - Make it opt-in via configuration flag
            // - Test with real-world comic archives for accuracy
            // - Validate extracted ISBNs against checksum before accepting
            // - Consider async/parallel processing for performance
            //
            // RELATED: EPUB and PDF parsers successfully extract ISBNs from metadata
            // (see epub/parser.rs and pdf/parser.rs for implemented approaches)
            isbns: Vec::new(),
            epub_positions: None,
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
    extract_page_from_cbr_with_fallback(path, page_number, false)
}

/// Extract a page image from a CBR file with optional fallback for corrupted images
///
/// When `fallback_on_invalid` is true and the requested page image is corrupted,
/// this function will try subsequent images until it finds a valid one.
/// This is useful for thumbnail generation where any valid image is acceptable.
///
/// # Arguments
/// * `path` - Path to the CBR file
/// * `page_number` - Page number (1-indexed)
/// * `fallback_on_invalid` - If true, try subsequent images when the requested one is corrupted
///
/// # Returns
/// The raw image data as bytes
pub fn extract_page_from_cbr_with_fallback<P: AsRef<Path>>(
    path: P,
    page_number: i32,
    fallback_on_invalid: bool,
) -> anyhow::Result<Vec<u8>> {
    use crate::parsers::image_utils::is_valid_image_data;

    let mut archive = unrar::Archive::new(path.as_ref())
        .open_for_processing()
        .map_err(|e| anyhow::anyhow!("Failed to open RAR archive: {}", e))?;

    let mut image_files: Vec<(String, Vec<u8>)> = Vec::new();

    while let Some(header) = archive
        .read_header()
        .map_err(|e| anyhow::anyhow!("Failed to read RAR header: {}", e))?
    {
        let entry_name = header.entry().filename.to_string_lossy().to_string();

        if is_image_file(&entry_name) {
            let (data, next_archive) = header
                .read()
                .map_err(|e| anyhow::anyhow!("Failed to read RAR entry: {}", e))?;
            archive = next_archive;
            image_files.push((entry_name, data));
        } else {
            archive = header
                .skip()
                .map_err(|e| anyhow::anyhow!("Failed to skip RAR entry: {}", e))?;
        }
    }

    // Sort by filename
    image_files.sort_by(|a, b| a.0.cmp(&b.0));

    // Get the requested page (1-indexed)
    let start_index = (page_number - 1) as usize;
    if start_index >= image_files.len() {
        anyhow::bail!("Page {} not found in archive", page_number);
    }

    // Try to extract the requested page, with optional fallback to subsequent pages
    let end_index = if fallback_on_invalid {
        image_files.len()
    } else {
        start_index + 1
    };

    for (index, (filename, data)) in image_files
        .iter()
        .enumerate()
        .skip(start_index)
        .take(end_index - start_index)
    {
        // Validate the image data
        if is_valid_image_data(data) {
            if index > start_index {
                tracing::info!(
                    original_page = page_number,
                    actual_index = index + 1,
                    filename = %filename,
                    "Using fallback image after skipping corrupted images in CBR"
                );
            }
            return Ok(data.clone());
        }

        tracing::warn!(
            page = index + 1,
            filename = %filename,
            size = data.len(),
            "Skipping corrupted image in CBR archive"
        );
    }

    anyhow::bail!(
        "No valid images found in archive starting from page {}",
        page_number
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: is_image_file and get_image_format tests are in image_utils.rs
    // since those functions are now shared across parsers

    #[test]
    fn test_cbr_parser_new() {
        let parser = CbrParser::new();
        assert!(parser.can_parse("test.cbr"));
    }

    #[test]
    fn test_cbr_parser_default() {
        let parser = CbrParser;
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
