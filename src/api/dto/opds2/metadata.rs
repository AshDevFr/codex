use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// OPDS 2.0 Feed Metadata
///
/// Metadata for navigation and publication feeds.
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct FeedMetadata {
    /// Title of the feed
    pub title: String,

    /// Optional subtitle
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subtitle: Option<String>,

    /// Last modification date
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modified: Option<DateTime<Utc>>,

    /// Total number of items in the collection (for pagination)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub number_of_items: Option<i64>,

    /// Items per page (for pagination)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub items_per_page: Option<i32>,

    /// Current page number (for pagination)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_page: Option<i32>,
}

#[allow(dead_code)] // Public API for OPDS 2.0 feed metadata building
impl FeedMetadata {
    /// Create new feed metadata with just a title
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            subtitle: None,
            modified: Some(Utc::now()),
            number_of_items: None,
            items_per_page: None,
            current_page: None,
        }
    }

    /// Add a subtitle
    pub fn with_subtitle(mut self, subtitle: impl Into<String>) -> Self {
        self.subtitle = Some(subtitle.into());
        self
    }

    /// Set modification date
    pub fn with_modified(mut self, modified: DateTime<Utc>) -> Self {
        self.modified = Some(modified);
        self
    }

    /// Add pagination information
    pub fn with_pagination(mut self, total: i64, per_page: i32, current: i32) -> Self {
        self.number_of_items = Some(total);
        self.items_per_page = Some(per_page);
        self.current_page = Some(current);
        self
    }
}

/// OPDS 2.0 Publication Metadata (schema.org based)
///
/// Metadata for a publication entry, based on schema.org vocabulary.
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PublicationMetadata {
    /// Schema.org type (e.g., "http://schema.org/Book")
    #[serde(rename = "@type", skip_serializing_if = "Option::is_none")]
    pub schema_type: Option<String>,

    /// Title of the publication
    pub title: String,

    /// Subtitle
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subtitle: Option<String>,

    /// Unique identifier (e.g., "urn:uuid:...")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub identifier: Option<String>,

    /// Authors
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<Vec<Contributor>>,

    /// Artists/illustrators
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artist: Option<Vec<Contributor>>,

    /// Language code (e.g., "en", "ja")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,

    /// Publisher name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub publisher: Option<String>,

    /// Last modification date
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modified: Option<DateTime<Utc>>,

    /// Publication date
    #[serde(skip_serializing_if = "Option::is_none")]
    pub published: Option<DateTime<Utc>>,

    /// Description/summary
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Number of pages
    #[serde(skip_serializing_if = "Option::is_none")]
    pub number_of_pages: Option<i32>,

    /// Series membership information
    #[serde(skip_serializing_if = "Option::is_none")]
    pub belongs_to: Option<BelongsTo>,
}

#[allow(dead_code)] // Public API for OPDS 2.0 publication metadata building
impl PublicationMetadata {
    /// Create new publication metadata with a title
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            schema_type: Some("http://schema.org/Book".to_string()),
            title: title.into(),
            subtitle: None,
            identifier: None,
            author: None,
            artist: None,
            language: None,
            publisher: None,
            modified: None,
            published: None,
            description: None,
            number_of_pages: None,
            belongs_to: None,
        }
    }

    /// Set the identifier
    pub fn with_identifier(mut self, id: impl Into<String>) -> Self {
        self.identifier = Some(id.into());
        self
    }

    /// Add authors
    pub fn with_authors(mut self, authors: Vec<Contributor>) -> Self {
        self.author = if authors.is_empty() {
            None
        } else {
            Some(authors)
        };
        self
    }

    /// Add artists
    pub fn with_artists(mut self, artists: Vec<Contributor>) -> Self {
        self.artist = if artists.is_empty() {
            None
        } else {
            Some(artists)
        };
        self
    }

    /// Set the language
    pub fn with_language(mut self, lang: impl Into<String>) -> Self {
        self.language = Some(lang.into());
        self
    }

    /// Set the publisher
    pub fn with_publisher(mut self, publisher: impl Into<String>) -> Self {
        self.publisher = Some(publisher.into());
        self
    }

    /// Set the modification date
    pub fn with_modified(mut self, modified: DateTime<Utc>) -> Self {
        self.modified = Some(modified);
        self
    }

    /// Set the description
    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    /// Set the page count
    pub fn with_page_count(mut self, pages: i32) -> Self {
        self.number_of_pages = Some(pages);
        self
    }

    /// Set series membership
    pub fn with_series(mut self, name: impl Into<String>, position: Option<f64>) -> Self {
        self.belongs_to = Some(BelongsTo {
            series: Some(SeriesInfo {
                name: name.into(),
                position,
            }),
        });
        self
    }
}

/// Contributor information (author, artist, etc.)
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Contributor {
    /// Name of the contributor
    pub name: String,

    /// Sort-friendly version of the name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort_as: Option<String>,
}

#[allow(dead_code)] // Public API for OPDS 2.0 metadata building
impl Contributor {
    /// Create a new contributor
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            sort_as: None,
        }
    }

    /// Add a sort name
    pub fn with_sort_as(mut self, sort_as: impl Into<String>) -> Self {
        self.sort_as = Some(sort_as.into());
        self
    }
}

/// Series membership information
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct BelongsTo {
    /// Series information
    #[serde(skip_serializing_if = "Option::is_none")]
    pub series: Option<SeriesInfo>,
}

/// Series information for a publication
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct SeriesInfo {
    /// Name of the series
    pub name: String,

    /// Position within the series (volume/issue number)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<f64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_feed_metadata_serialization() {
        let metadata = FeedMetadata::new("Test Feed").with_subtitle("A test feed");

        let json = serde_json::to_string(&metadata).unwrap();
        assert!(json.contains("\"title\":\"Test Feed\""));
        assert!(json.contains("\"subtitle\":\"A test feed\""));
    }

    #[test]
    fn test_feed_metadata_pagination() {
        let metadata = FeedMetadata::new("Paginated Feed").with_pagination(100, 20, 2);

        let json = serde_json::to_string(&metadata).unwrap();
        assert!(json.contains("\"numberOfItems\":100"));
        assert!(json.contains("\"itemsPerPage\":20"));
        assert!(json.contains("\"currentPage\":2"));
    }

    #[test]
    fn test_publication_metadata_serialization() {
        let metadata = PublicationMetadata::new("Test Book")
            .with_identifier("urn:uuid:12345")
            .with_authors(vec![Contributor::new("John Doe")])
            .with_page_count(200);

        let json = serde_json::to_string(&metadata).unwrap();
        assert!(json.contains("\"@type\":\"http://schema.org/Book\""));
        assert!(json.contains("\"title\":\"Test Book\""));
        assert!(json.contains("\"identifier\":\"urn:uuid:12345\""));
        assert!(json.contains("\"numberOfPages\":200"));
    }

    #[test]
    fn test_publication_metadata_with_series() {
        let metadata = PublicationMetadata::new("Issue #1").with_series("My Series", Some(1.0));

        let json = serde_json::to_string(&metadata).unwrap();
        assert!(json.contains("\"belongsTo\""));
        assert!(json.contains("\"series\""));
        assert!(json.contains("\"name\":\"My Series\""));
        assert!(json.contains("\"position\":1.0"));
    }

    #[test]
    fn test_contributor_serialization() {
        let contributor = Contributor::new("Jane Smith").with_sort_as("Smith, Jane");

        let json = serde_json::to_string(&contributor).unwrap();
        assert!(json.contains("\"name\":\"Jane Smith\""));
        assert!(json.contains("\"sortAs\":\"Smith, Jane\""));
    }

    #[test]
    fn test_skip_serializing_none_fields() {
        let metadata = PublicationMetadata::new("Minimal Book");
        let json = serde_json::to_string(&metadata).unwrap();

        // Should not contain optional fields that are None
        assert!(!json.contains("\"subtitle\""));
        assert!(!json.contains("\"author\""));
        assert!(!json.contains("\"language\""));
    }
}
