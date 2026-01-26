use crate::parsers::image_utils::{
    get_jxl_dimensions, get_svg_dimensions, get_verified_image_format, is_image_file,
};
use crate::parsers::traits::FormatParser;
use crate::parsers::{parse_comic_info, BookMetadata, FileFormat, ImageFormat, PageInfo};
use crate::utils::{hash_file, CodexError, Result};
use chrono::{DateTime, Utc};
use image::GenericImageView;
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

        // Process each page
        let mut pages = Vec::new();
        for (page_num, (idx, name)) in image_entries.iter().enumerate() {
            let mut file = archive.by_index(*idx)?;
            let file_size = file.size();

            // Read image data first for magic byte detection
            let mut image_data = Vec::new();
            file.read_to_end(&mut image_data)?;

            // Detect format using both extension and magic bytes (with logging)
            let format = get_verified_image_format(name, &image_data)
                .ok_or_else(|| CodexError::UnsupportedFormat(name.clone()))?;

            // Get image dimensions (with special handling for SVG and JXL)
            let (width, height) = match format {
                ImageFormat::SVG => {
                    // Use resvg to get SVG dimensions
                    get_svg_dimensions(&image_data).ok_or_else(|| {
                        CodexError::ParseError(format!("Failed to parse SVG dimensions: {}", name))
                    })?
                }
                ImageFormat::JXL => {
                    // Use jxl-oxide to get JXL dimensions
                    get_jxl_dimensions(&image_data).ok_or_else(|| {
                        CodexError::ParseError(format!("Failed to parse JXL dimensions: {}", name))
                    })?
                }
                _ => {
                    // Use image crate for raster formats
                    let img = image::load_from_memory(&image_data)?;
                    img.dimensions()
                }
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
    let index = (page_number - 1) as usize;
    if index >= image_files.len() {
        anyhow::bail!("Page {} not found in archive", page_number);
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
