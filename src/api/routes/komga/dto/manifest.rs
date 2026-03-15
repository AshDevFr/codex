//! Readium WebPub Manifest DTOs for Komga-compatible EPUB reading
//!
//! These structures represent the Readium WebPub Manifest format that Komga returns
//! for EPUB books, enabling streaming EPUB reading in compatible apps like Komic.

use serde::Serialize;
use utoipa::ToSchema;

/// Readium WebPub Manifest
///
/// Root structure for the manifest returned by the EPUB manifest endpoint.
/// Conforms to the Readium WebPub Manifest specification.
#[derive(Debug, Serialize, ToSchema)]
pub struct WebPubManifest {
    #[serde(rename = "@context")]
    pub context: String,
    pub metadata: WebPubMetadata,
    #[serde(rename = "readingOrder")]
    pub reading_order: Vec<WebPubLink>,
    pub resources: Vec<WebPubLink>,
    pub toc: Vec<WebPubTocEntry>,
}

/// Metadata section of the WebPub Manifest
#[derive(Debug, Serialize, ToSchema)]
pub struct WebPubMetadata {
    pub identifier: String,
    pub title: String,
    #[serde(rename = "@type")]
    pub schema_type: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub author: Vec<String>,
    #[serde(rename = "numberOfPages")]
    pub number_of_pages: i32,
}

/// A link entry in readingOrder or resources
#[derive(Debug, Serialize, ToSchema)]
pub struct WebPubLink {
    pub href: String,
    #[serde(rename = "type")]
    pub media_type: String,
}

/// A table of contents entry
#[derive(Debug, Serialize, ToSchema)]
pub struct WebPubTocEntry {
    pub href: String,
    pub title: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<WebPubTocEntry>,
}
