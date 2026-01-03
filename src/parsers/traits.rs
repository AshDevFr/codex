use crate::parsers::BookMetadata;
use crate::utils::Result;
use std::path::Path;

/// Trait for parsing different file formats
pub trait FormatParser {
    /// Parse a file and extract its metadata
    fn parse<P: AsRef<Path>>(&self, path: P) -> Result<BookMetadata>;

    /// Check if this parser can handle the given file
    fn can_parse<P: AsRef<Path>>(&self, path: P) -> bool;
}
