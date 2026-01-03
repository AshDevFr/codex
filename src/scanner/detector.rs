use crate::parsers::FileFormat;
use std::path::Path;

/// Detect file format from extension
pub fn detect_format<P: AsRef<Path>>(path: P) -> Option<FileFormat> {
    path.as_ref()
        .extension()
        .and_then(|e| e.to_str())
        .and_then(FileFormat::from_extension)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_detect_format_cbz() {
        let path = PathBuf::from("/path/to/comic.cbz");
        assert_eq!(detect_format(&path), Some(FileFormat::CBZ));
    }

    #[test]
    fn test_detect_format_cbr() {
        let path = PathBuf::from("/path/to/comic.cbr");
        assert_eq!(detect_format(&path), Some(FileFormat::CBR));
    }

    #[test]
    fn test_detect_format_epub() {
        let path = PathBuf::from("/path/to/book.epub");
        assert_eq!(detect_format(&path), Some(FileFormat::EPUB));
    }

    #[test]
    fn test_detect_format_pdf() {
        let path = PathBuf::from("/path/to/document.pdf");
        assert_eq!(detect_format(&path), Some(FileFormat::PDF));
    }

    #[test]
    fn test_detect_format_case_insensitive() {
        assert_eq!(
            detect_format(PathBuf::from("file.CBZ")),
            Some(FileFormat::CBZ)
        );
        assert_eq!(
            detect_format(PathBuf::from("file.CbZ")),
            Some(FileFormat::CBZ)
        );
        assert_eq!(
            detect_format(PathBuf::from("file.PDF")),
            Some(FileFormat::PDF)
        );
    }

    #[test]
    fn test_detect_format_unsupported() {
        assert_eq!(detect_format(PathBuf::from("file.txt")), None);
        assert_eq!(detect_format(PathBuf::from("file.zip")), None);
        assert_eq!(detect_format(PathBuf::from("file.jpg")), None);
    }

    #[test]
    fn test_detect_format_no_extension() {
        assert_eq!(detect_format(PathBuf::from("file_without_extension")), None);
        assert_eq!(detect_format(PathBuf::from("/path/to/file")), None);
    }

    #[test]
    fn test_detect_format_complex_path() {
        let path = PathBuf::from("/home/user/Documents/Comics/My Comic Book.cbz");
        assert_eq!(detect_format(&path), Some(FileFormat::CBZ));
    }
}
