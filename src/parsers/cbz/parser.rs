use crate::parsers::image_utils::{create_page_info, is_image_file, process_image_data};
use crate::parsers::traits::FormatParser;
use crate::parsers::{BookMetadata, FileFormat, parse_comic_info};
use crate::utils::{Result, hash_file};
use chrono::{DateTime, Utc};
use std::fs::File;
use std::io::Read;
use std::path::Path;
use zip::ZipArchive;

pub struct CbzParser;

impl CbzParser {
    pub fn new() -> Self {
        Self
    }
}

impl FormatParser for CbzParser {
    fn can_parse<P: AsRef<Path>>(&self, path: P) -> bool {
        path.as_ref()
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase() == "cbz")
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

        // Open ZIP archive
        let file = File::open(path)?;
        let mut archive = ZipArchive::new(file)?;

        // Look for ComicInfo.xml
        let mut comic_info = None;
        if let Ok(mut comic_info_file) = archive.by_name("ComicInfo.xml") {
            let mut xml_content = String::new();
            comic_info_file.read_to_string(&mut xml_content)?;
            if let Ok(info) = parse_comic_info(&xml_content) {
                comic_info = Some(info);
            }
        }

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

        // Sort by name (this gives us page order)
        image_entries.sort_by(|a, b| a.1.cmp(&b.1));

        // Process each page - collect valid images and assign page numbers
        let mut pages = Vec::new();
        for (idx, name) in image_entries.iter() {
            let mut file = archive.by_index(*idx)?;
            let file_size = file.size();

            // Read image data
            let mut image_data = Vec::new();
            file.read_to_end(&mut image_data)?;

            // Process image: verify format and extract dimensions
            // Skip files that don't pass verification
            let Some(processed) = process_image_data(name, &image_data) else {
                tracing::debug!(
                    filename = %name,
                    "Skipping file: could not verify image format or dimensions"
                );
                continue;
            };

            // Assign page number based on successfully processed pages
            let page_number = pages.len() + 1;
            pages.push(create_page_info(
                page_number,
                name.clone(),
                processed,
                file_size,
            ));
        }

        let page_count = pages.len();

        Ok(BookMetadata {
            file_path: path.to_string_lossy().to_string(),
            format: FileFormat::CBZ,
            file_size,
            file_hash,
            modified_at,
            page_count,
            pages,
            comic_info,
            // TODO: Implement barcode detection for CBZ files (deferred)
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
            epub_spine_items: None,
        })
    }
}

impl Default for CbzParser {
    fn default() -> Self {
        Self::new()
    }
}

/// Extract a specific page image from a CBZ file
///
/// # Arguments
/// * `path` - Path to the CBZ file
/// * `page_number` - Page number (1-indexed)
///
/// # Returns
/// The raw image data as bytes
pub fn extract_page_from_cbz<P: AsRef<Path>>(path: P, page_number: i32) -> anyhow::Result<Vec<u8>> {
    extract_page_from_cbz_with_fallback(path, page_number, false)
}

/// Extract a page image from a CBZ file with optional fallback for corrupted images
///
/// When `fallback_on_invalid` is true and the requested page image is corrupted,
/// this function will try subsequent images until it finds a valid one.
/// This is useful for thumbnail generation where any valid image is acceptable.
///
/// # Arguments
/// * `path` - Path to the CBZ file
/// * `page_number` - Page number (1-indexed)
/// * `fallback_on_invalid` - If true, try subsequent images when the requested one is corrupted
///
/// # Returns
/// The raw image data as bytes
pub fn extract_page_from_cbz_with_fallback<P: AsRef<Path>>(
    path: P,
    page_number: i32,
    fallback_on_invalid: bool,
) -> anyhow::Result<Vec<u8>> {
    use crate::parsers::image_utils::is_valid_image_data;

    let file = File::open(path)?;
    let mut archive = ZipArchive::new(file)?;

    // Get list of image files in archive
    let mut image_files: Vec<String> = Vec::new();
    for i in 0..archive.len() {
        let file = archive.by_index(i)?;
        let name = file.name().to_string();
        if !file.is_dir() && is_image_file(&name) {
            image_files.push(name);
        }
    }

    // Sort alphabetically to match page order
    image_files.sort();

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
                    "Using fallback image after skipping corrupted images"
                );
            }
            return Ok(buffer);
        }

        tracing::warn!(
            page = index + 1,
            filename = %filename,
            size = buffer.len(),
            "Skipping corrupted image in CBZ archive"
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

    #[test]
    fn test_cbz_parser_new() {
        let parser = CbzParser::new();
        assert!(parser.can_parse("test.cbz"));
    }

    #[test]
    fn test_cbz_parser_default() {
        let parser = CbzParser;
        assert!(parser.can_parse("test.cbz"));
    }
}
