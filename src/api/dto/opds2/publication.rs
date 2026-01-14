use super::{Opds2Link, PublicationMetadata};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// OPDS 2.0 Publication Entry
///
/// Represents a single publication (book) in an OPDS 2.0 feed.
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Publication {
    /// Publication metadata (title, author, etc.)
    pub metadata: PublicationMetadata,

    /// Links for the publication (acquisition, streaming, etc.)
    pub links: Vec<Opds2Link>,

    /// Cover images for the publication
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub images: Vec<ImageLink>,

    /// Reading progress extension (Codex-specific)
    /// This follows a similar pattern to other reading apps that extend OPDS 2.0
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reading_progress: Option<ReadingProgress>,
}

/// Reading progress information for a publication
///
/// Custom extension for tracking reading progress in OPDS 2.0.
/// Compatible with reading apps that support progress sync.
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReadingProgress {
    /// Current page (1-indexed)
    pub current_page: i32,

    /// Total number of pages in the book
    pub total_pages: i32,

    /// Progress as a percentage (0.0 - 100.0)
    pub progress_percent: f64,

    /// Whether the book has been completed
    pub is_completed: bool,

    /// Last time progress was updated
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_read_at: Option<DateTime<Utc>>,
}

impl ReadingProgress {
    /// Create new reading progress
    pub fn new(
        current_page: i32,
        total_pages: i32,
        is_completed: bool,
        last_read_at: Option<DateTime<Utc>>,
    ) -> Self {
        let progress_percent = if total_pages > 0 {
            (current_page as f64 / total_pages as f64) * 100.0
        } else {
            0.0
        };

        Self {
            current_page,
            total_pages,
            progress_percent,
            is_completed,
            last_read_at,
        }
    }
}

#[allow(dead_code)] // Public API for OPDS 2.0 publication building
impl Publication {
    /// Create a new publication with metadata
    pub fn new(metadata: PublicationMetadata) -> Self {
        Self {
            metadata,
            links: Vec::new(),
            images: Vec::new(),
            reading_progress: None,
        }
    }

    /// Add a link to the publication
    pub fn add_link(mut self, link: Opds2Link) -> Self {
        self.links.push(link);
        self
    }

    /// Add multiple links to the publication
    pub fn with_links(mut self, links: Vec<Opds2Link>) -> Self {
        self.links = links;
        self
    }

    /// Add an image to the publication
    pub fn add_image(mut self, image: ImageLink) -> Self {
        self.images.push(image);
        self
    }

    /// Add multiple images to the publication
    pub fn with_images(mut self, images: Vec<ImageLink>) -> Self {
        self.images = images;
        self
    }

    /// Set reading progress for the publication
    pub fn with_reading_progress(mut self, progress: ReadingProgress) -> Self {
        self.reading_progress = Some(progress);
        self
    }
}

/// Image link with optional dimensions
///
/// Used for cover images and thumbnails in publications.
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct ImageLink {
    /// URL to the image
    pub href: String,

    /// Media type of the image
    #[serde(rename = "type")]
    pub media_type: String,

    /// Width in pixels
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<i32>,

    /// Height in pixels
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<i32>,
}

#[allow(dead_code)] // Public API for OPDS 2.0 image link building
impl ImageLink {
    /// Create a new image link
    pub fn new(href: impl Into<String>, media_type: impl Into<String>) -> Self {
        Self {
            href: href.into(),
            media_type: media_type.into(),
            width: None,
            height: None,
        }
    }

    /// Create a JPEG thumbnail link
    pub fn thumbnail(href: impl Into<String>) -> Self {
        Self::new(href, "image/jpeg")
    }

    /// Create a PNG image link
    pub fn png(href: impl Into<String>) -> Self {
        Self::new(href, "image/png")
    }

    /// Set dimensions
    pub fn with_dimensions(mut self, width: i32, height: i32) -> Self {
        self.width = Some(width);
        self.height = Some(height);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::dto::opds2::Contributor;

    #[test]
    fn test_publication_serialization() {
        let metadata = PublicationMetadata::new("Test Book")
            .with_identifier("urn:uuid:12345")
            .with_authors(vec![Contributor::new("Author Name")]);

        let publication = Publication::new(metadata)
            .add_link(Opds2Link::acquisition_link("/book/file", "application/zip"))
            .add_image(ImageLink::thumbnail("/book/cover"));

        let json = serde_json::to_string(&publication).unwrap();
        assert!(json.contains("\"title\":\"Test Book\""));
        assert!(json.contains("\"links\""));
        assert!(json.contains("\"images\""));
        assert!(json.contains("\"href\":\"/book/file\""));
        assert!(json.contains("\"href\":\"/book/cover\""));
    }

    #[test]
    fn test_publication_without_images() {
        let metadata = PublicationMetadata::new("Minimal Book");
        let publication = Publication::new(metadata);

        let json = serde_json::to_string(&publication).unwrap();
        // Empty images array should be skipped
        assert!(!json.contains("\"images\""));
    }

    #[test]
    fn test_image_link_serialization() {
        let image = ImageLink::thumbnail("/cover.jpg").with_dimensions(200, 300);

        let json = serde_json::to_string(&image).unwrap();
        assert!(json.contains("\"href\":\"/cover.jpg\""));
        assert!(json.contains("\"type\":\"image/jpeg\""));
        assert!(json.contains("\"width\":200"));
        assert!(json.contains("\"height\":300"));
    }

    #[test]
    fn test_image_link_without_dimensions() {
        let image = ImageLink::thumbnail("/cover.jpg");

        let json = serde_json::to_string(&image).unwrap();
        assert!(!json.contains("\"width\""));
        assert!(!json.contains("\"height\""));
    }
}
