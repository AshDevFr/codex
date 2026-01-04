use crate::parsers::{parse_comic_info, BookMetadata, FileFormat, PageInfo};
use crate::parsers::traits::FormatParser;
use crate::parsers::image_utils::{is_image_file, get_image_format};
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

            // Read image data
            let mut image_data = Vec::new();
            file.read_to_end(&mut image_data)?;

            // Get image dimensions
            let img = image::load_from_memory(&image_data)?;
            let (width, height) = img.dimensions();

            let format = get_image_format(name)
                .ok_or_else(|| CodexError::UnsupportedFormat(name.clone()))?;

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
            isbns: Vec::new(), // TODO: Implement barcode detection
        })
    }
}

impl Default for CbzParser {
    fn default() -> Self {
        Self::new()
    }
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
        let parser = CbzParser::default();
        assert!(parser.can_parse("test.cbz"));
    }
}
