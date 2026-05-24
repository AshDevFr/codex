//! Error types for file-format parsing.

use thiserror::Error;

#[derive(Error, Debug)]
pub enum ParserError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("ZIP error: {0}")]
    Zip(#[from] zip::result::ZipError),

    #[error("Parse error: {0}")]
    ParseError(String),

    #[error("Unsupported file format: {0}")]
    UnsupportedFormat(String),
}

pub type Result<T> = std::result::Result<T, ParserError>;
