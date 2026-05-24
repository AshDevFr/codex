use crate::detect_format;
use codex_parsers::BookMetadata;
#[cfg(feature = "rar")]
use codex_parsers::cbr::CbrParser;
use codex_parsers::cbz::CbzParser;
use codex_parsers::epub::EpubParser;
use codex_parsers::error::{ParserError, Result};
use codex_parsers::pdf::PdfParser;
use codex_parsers::traits::FormatParser;
use std::path::Path;

/// Analyze a file and extract metadata
pub fn analyze_file<P: AsRef<Path>>(path: P) -> Result<BookMetadata> {
    let path = path.as_ref();

    // Detect format
    let format = detect_format(path)
        .ok_or_else(|| ParserError::UnsupportedFormat(path.to_string_lossy().to_string()))?;

    // Select appropriate parser
    let metadata = match format {
        codex_parsers::FileFormat::CBZ => {
            let parser = CbzParser::new();
            parser.parse(path)?
        }
        #[cfg(feature = "rar")]
        codex_parsers::FileFormat::CBR => {
            let parser = CbrParser::new();
            parser.parse(path)?
        }
        #[cfg(not(feature = "rar"))]
        codex_parsers::FileFormat::CBR => {
            return Err(ParserError::UnsupportedFormat(
                "CBR support requires the 'rar' feature to be enabled".to_string(),
            ));
        }
        codex_parsers::FileFormat::EPUB => {
            let parser = EpubParser::new();
            parser.parse(path)?
        }
        codex_parsers::FileFormat::PDF => {
            let parser = PdfParser::new();
            parser.parse(path)?
        }
    };

    Ok(metadata)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_analyze_file_unsupported_format() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(b"test content").unwrap();
        temp_file.flush().unwrap();

        // Rename to unsupported extension
        let path = temp_file.path().with_extension("txt");
        std::fs::copy(temp_file.path(), &path).unwrap();

        let result = analyze_file(&path);
        assert!(result.is_err());

        if let Err(ParserError::UnsupportedFormat(msg)) = result {
            assert!(msg.contains(".txt"));
        } else {
            panic!("Expected UnsupportedFormat error");
        }

        // Cleanup
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_analyze_file_nonexistent() {
        let result = analyze_file("/nonexistent/file.cbz");
        assert!(result.is_err());
    }
}
