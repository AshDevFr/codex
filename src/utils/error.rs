use thiserror::Error;

#[derive(Error, Debug)]
pub enum CodexError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("ZIP error: {0}")]
    Zip(#[from] zip::result::ZipError),

    #[error("Image error: {0}")]
    Image(#[from] image::ImageError),

    #[error("XML parsing error: {0}")]
    Xml(#[from] quick_xml::DeError),

    #[error("JSON serialization error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Unsupported file format: {0}")]
    UnsupportedFormat(String),

    #[error("File not found: {0}")]
    FileNotFound(String),

    #[error("Invalid metadata: {0}")]
    InvalidMetadata(String),

    #[error("Parse error: {0}")]
    ParseError(String),
}

pub type Result<T> = std::result::Result<T, CodexError>;
