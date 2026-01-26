use crate::parsers::FileFormat;
use std::fs::File;
use std::io::Read;
use std::path::Path;

/// Magic bytes for archive format detection
const ZIP_MAGIC: &[u8] = &[0x50, 0x4B, 0x03, 0x04]; // "PK.."
const RAR_MAGIC: &[u8] = &[0x52, 0x61, 0x72, 0x21, 0x1A, 0x07]; // "Rar!.."
const PDF_MAGIC: &[u8] = &[0x25, 0x50, 0x44, 0x46]; // "%PDF"

/// Detect file format, preferring magic bytes over extension.
///
/// This handles cases where files have incorrect extensions (e.g., a ZIP file
/// named with .cbr extension). The detection order is:
/// 1. Read magic bytes from the file
/// 2. If magic bytes indicate a supported format, use that
/// 3. Otherwise, fall back to extension-based detection
pub fn detect_format<P: AsRef<Path>>(path: P) -> Option<FileFormat> {
    let path = path.as_ref();

    // Try magic bytes first
    if let Some(format) = detect_format_by_magic(path) {
        return Some(format);
    }

    // Fall back to extension
    detect_format_by_extension(path)
}

/// Detect file format from extension only
pub fn detect_format_by_extension<P: AsRef<Path>>(path: P) -> Option<FileFormat> {
    path.as_ref()
        .extension()
        .and_then(|e| e.to_str())
        .and_then(FileFormat::from_extension)
}

/// Detect file format from magic bytes
///
/// Returns the detected format based on the file's magic bytes, or None if:
/// - The file cannot be read
/// - The magic bytes don't match any known format
fn detect_format_by_magic<P: AsRef<Path>>(path: P) -> Option<FileFormat> {
    let mut file = File::open(path.as_ref()).ok()?;
    let mut buffer = [0u8; 8]; // Enough for all our magic byte checks
    file.read_exact(&mut buffer).ok()?;

    // Check RAR first (more specific magic bytes)
    if buffer.starts_with(RAR_MAGIC) {
        return Some(FileFormat::CBR);
    }

    // Check PDF
    if buffer.starts_with(PDF_MAGIC) {
        return Some(FileFormat::PDF);
    }

    // Check ZIP-based formats (CBZ and EPUB both use ZIP)
    if buffer.starts_with(ZIP_MAGIC) {
        // Need to distinguish between CBZ and EPUB
        // EPUB files have a specific structure with mimetype file
        return detect_zip_subtype(path.as_ref());
    }

    None
}

/// Detect whether a ZIP file is EPUB or CBZ
///
/// EPUB files are ZIP archives that contain a `mimetype` file as the first entry
/// with the content "application/epub+zip"
fn detect_zip_subtype<P: AsRef<Path>>(path: P) -> Option<FileFormat> {
    use std::io::BufReader;
    use zip::ZipArchive;

    let file = File::open(path.as_ref()).ok()?;
    let reader = BufReader::new(file);
    let mut archive = ZipArchive::new(reader).ok()?;

    // EPUB spec requires mimetype to be the first file in the archive
    if let Ok(mut mimetype_file) = archive.by_name("mimetype") {
        let mut content = String::new();
        if mimetype_file.read_to_string(&mut content).is_ok()
            && content.trim() == "application/epub+zip"
        {
            return Some(FileFormat::EPUB);
        }
    }

    // If not EPUB, assume CBZ (comic book archive)
    Some(FileFormat::CBZ)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::path::PathBuf;
    use tempfile::NamedTempFile;

    // Extension-based detection tests (for non-existent files)
    mod extension_detection {
        use super::*;

        #[test]
        fn test_detect_format_by_extension_cbz() {
            let path = PathBuf::from("/path/to/comic.cbz");
            assert_eq!(detect_format_by_extension(&path), Some(FileFormat::CBZ));
        }

        #[test]
        fn test_detect_format_by_extension_cbr() {
            let path = PathBuf::from("/path/to/comic.cbr");
            assert_eq!(detect_format_by_extension(&path), Some(FileFormat::CBR));
        }

        #[test]
        fn test_detect_format_by_extension_epub() {
            let path = PathBuf::from("/path/to/book.epub");
            assert_eq!(detect_format_by_extension(&path), Some(FileFormat::EPUB));
        }

        #[test]
        fn test_detect_format_by_extension_pdf() {
            let path = PathBuf::from("/path/to/document.pdf");
            assert_eq!(detect_format_by_extension(&path), Some(FileFormat::PDF));
        }

        #[test]
        fn test_detect_format_case_insensitive() {
            assert_eq!(
                detect_format_by_extension(PathBuf::from("file.CBZ")),
                Some(FileFormat::CBZ)
            );
            assert_eq!(
                detect_format_by_extension(PathBuf::from("file.CbZ")),
                Some(FileFormat::CBZ)
            );
            assert_eq!(
                detect_format_by_extension(PathBuf::from("file.PDF")),
                Some(FileFormat::PDF)
            );
        }

        #[test]
        fn test_detect_format_unsupported() {
            assert_eq!(detect_format_by_extension(PathBuf::from("file.txt")), None);
            assert_eq!(detect_format_by_extension(PathBuf::from("file.zip")), None);
            assert_eq!(detect_format_by_extension(PathBuf::from("file.jpg")), None);
        }

        #[test]
        fn test_detect_format_no_extension() {
            assert_eq!(
                detect_format_by_extension(PathBuf::from("file_without_extension")),
                None
            );
            assert_eq!(
                detect_format_by_extension(PathBuf::from("/path/to/file")),
                None
            );
        }
    }

    // Magic bytes detection tests (require actual files)
    mod magic_bytes_detection {
        use super::*;

        fn create_temp_file_with_content(content: &[u8], extension: &str) -> NamedTempFile {
            let mut file = tempfile::Builder::new()
                .suffix(extension)
                .tempfile()
                .unwrap();
            file.write_all(content).unwrap();
            file.flush().unwrap();
            file
        }

        #[test]
        fn test_detect_pdf_by_magic() {
            // Create a file with PDF magic bytes but wrong extension
            let content = b"%PDF-1.4 fake pdf content";
            let file = create_temp_file_with_content(content, ".txt");

            // Should detect as PDF based on magic bytes
            assert_eq!(detect_format(file.path()), Some(FileFormat::PDF));
        }

        #[test]
        fn test_detect_rar_by_magic() {
            // Create a file with RAR magic bytes but wrong extension
            let mut content = vec![0x52, 0x61, 0x72, 0x21, 0x1A, 0x07, 0x00]; // RAR magic
            content.extend_from_slice(b"fake rar content");
            let file = create_temp_file_with_content(&content, ".cbz");

            // Should detect as CBR based on magic bytes
            assert_eq!(detect_format(file.path()), Some(FileFormat::CBR));
        }

        #[test]
        fn test_detect_zip_by_magic_as_cbz() {
            // Create a minimal valid ZIP file (empty archive)
            let file = create_temp_file_with_content(&[], ".wrong");

            // Write an actual minimal ZIP using the zip crate
            {
                use std::fs::OpenOptions;
                let zip_file = OpenOptions::new()
                    .write(true)
                    .truncate(true)
                    .open(file.path())
                    .unwrap();
                let mut zip = zip::ZipWriter::new(zip_file);
                let options = zip::write::SimpleFileOptions::default();
                zip.start_file("dummy.jpg", options).unwrap();
                zip.write_all(b"not a real image").unwrap();
                zip.finish().unwrap();
            }

            // Should detect as CBZ (ZIP without EPUB mimetype)
            assert_eq!(detect_format(file.path()), Some(FileFormat::CBZ));
        }

        #[test]
        fn test_detect_zip_by_magic_as_epub() {
            let file = create_temp_file_with_content(&[], ".wrong");

            // Write a ZIP with EPUB mimetype
            {
                use std::fs::OpenOptions;
                let zip_file = OpenOptions::new()
                    .write(true)
                    .truncate(true)
                    .open(file.path())
                    .unwrap();
                let mut zip = zip::ZipWriter::new(zip_file);
                let options = zip::write::SimpleFileOptions::default();
                zip.start_file("mimetype", options).unwrap();
                zip.write_all(b"application/epub+zip").unwrap();
                zip.finish().unwrap();
            }

            // Should detect as EPUB
            assert_eq!(detect_format(file.path()), Some(FileFormat::EPUB));
        }

        #[test]
        fn test_mismatched_extension_cbr_is_actually_cbz() {
            // This is the scenario the user mentioned: a .cbr file that's actually a ZIP
            let file = create_temp_file_with_content(&[], ".cbr");

            // Write a ZIP file (CBZ content) with .cbr extension
            {
                use std::fs::OpenOptions;
                let zip_file = OpenOptions::new()
                    .write(true)
                    .truncate(true)
                    .open(file.path())
                    .unwrap();
                let mut zip = zip::ZipWriter::new(zip_file);
                let options = zip::write::SimpleFileOptions::default();
                zip.start_file("page001.jpg", options).unwrap();
                zip.write_all(b"fake image data").unwrap();
                zip.finish().unwrap();
            }

            // Even though extension is .cbr, magic bytes should detect as CBZ
            assert_eq!(detect_format(file.path()), Some(FileFormat::CBZ));
        }

        #[test]
        fn test_fallback_to_extension_for_unknown_magic() {
            // Create a file with unknown magic bytes but valid extension
            let content = b"unknown file format content here";
            let file = create_temp_file_with_content(content, ".cbz");

            // Should fall back to extension-based detection
            assert_eq!(detect_format(file.path()), Some(FileFormat::CBZ));
        }

        #[test]
        fn test_nonexistent_file_falls_back_to_extension() {
            // Non-existent file should fall back to extension
            let path = PathBuf::from("/nonexistent/file.cbz");
            assert_eq!(detect_format(&path), Some(FileFormat::CBZ));
        }
    }
}
