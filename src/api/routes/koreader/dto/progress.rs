use serde::{Deserialize, Serialize};

/// KOReader document progress DTO
///
/// Used for both request and response when syncing reading progress.
/// Field names use snake_case to match KOReader's expected JSON format.
#[derive(Debug, Serialize, Deserialize)]
pub struct DocumentProgressDto {
    /// KOReader partial MD5 hash identifying the document
    pub document: String,

    /// Reading progress as a string (page number for PDF/CBZ, XPath for EPUB)
    pub progress: String,

    /// Overall progress percentage (0.0 to 1.0)
    pub percentage: f64,

    /// Device name
    pub device: String,

    /// Device identifier
    pub device_id: String,
}

/// Response for successful authentication
#[derive(Debug, Serialize, Deserialize)]
pub struct AuthorizedDto {
    pub authorized: String,
}
