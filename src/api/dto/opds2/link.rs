use serde::{Deserialize, Serialize};

/// OPDS 2.0 Link Object
///
/// Represents a link in an OPDS 2.0 feed, based on the Web Publication Manifest model.
/// Links can be templated using URI templates (RFC 6570).
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Opds2Link {
    /// The URI or URI template for the link
    pub href: String,

    /// Relation type (e.g., "self", "search", "http://opds-spec.org/acquisition")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rel: Option<String>,

    /// Media type of the linked resource
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub media_type: Option<String>,

    /// Human-readable title for the link
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,

    /// Whether the href is a URI template
    #[serde(skip_serializing_if = "Option::is_none")]
    pub templated: Option<bool>,

    /// Additional properties for the link
    #[serde(skip_serializing_if = "Option::is_none")]
    pub properties: Option<LinkProperties>,
}

/// Additional properties that can be attached to links
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct LinkProperties {
    /// Number of items in the linked collection
    #[serde(skip_serializing_if = "Option::is_none")]
    pub number_of_items: Option<i64>,
}

impl Opds2Link {
    /// Create a new link with just an href
    pub fn new(href: impl Into<String>) -> Self {
        Self {
            href: href.into(),
            rel: None,
            media_type: None,
            title: None,
            templated: None,
            properties: None,
        }
    }

    /// Add a relation type to the link
    pub fn with_rel(mut self, rel: impl Into<String>) -> Self {
        self.rel = Some(rel.into());
        self
    }

    /// Add a media type to the link
    pub fn with_type(mut self, media_type: impl Into<String>) -> Self {
        self.media_type = Some(media_type.into());
        self
    }

    /// Add a human-readable title to the link
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Mark the link as templated (URI template)
    pub fn templated(mut self) -> Self {
        self.templated = Some(true);
        self
    }

    /// Add number of items property
    pub fn with_number_of_items(mut self, count: i64) -> Self {
        self.properties = Some(LinkProperties {
            number_of_items: Some(count),
        });
        self
    }

    // Convenience constructors for common link types

    /// Create a self link for the current feed
    pub fn self_link(href: impl Into<String>) -> Self {
        Self::new(href)
            .with_rel("self")
            .with_type("application/opds+json")
    }

    /// Create a start link (root catalog)
    pub fn start_link(href: impl Into<String>) -> Self {
        Self::new(href)
            .with_rel("start")
            .with_type("application/opds+json")
    }

    /// Create a navigation link
    pub fn navigation_link(href: impl Into<String>, title: impl Into<String>) -> Self {
        Self::new(href)
            .with_type("application/opds+json")
            .with_title(title)
    }

    /// Create an up link (parent navigation)
    pub fn up_link(href: impl Into<String>, title: impl Into<String>) -> Self {
        Self::new(href)
            .with_rel("up")
            .with_type("application/opds+json")
            .with_title(title)
    }

    /// Create a search template link
    pub fn search_template(href: impl Into<String>) -> Self {
        Self::new(href)
            .with_rel("search")
            .with_type("application/opds+json")
            .templated()
    }

    /// Create an open-access acquisition link (free download)
    pub fn acquisition_link(href: impl Into<String>, media_type: impl Into<String>) -> Self {
        Self::new(href)
            .with_rel("http://opds-spec.org/acquisition/open-access")
            .with_type(media_type)
    }

    /// Create a "new" link for recent additions
    pub fn new_link(href: impl Into<String>, title: impl Into<String>) -> Self {
        Self::new(href)
            .with_rel("http://opds-spec.org/sort/new")
            .with_type("application/opds+json")
            .with_title(title)
    }

    /// Create a next page link
    pub fn next_link(href: impl Into<String>) -> Self {
        Self::new(href)
            .with_rel("next")
            .with_type("application/opds+json")
    }

    /// Create a previous page link
    pub fn prev_link(href: impl Into<String>) -> Self {
        Self::new(href)
            .with_rel("prev")
            .with_type("application/opds+json")
    }

    /// Create a first page link
    pub fn first_link(href: impl Into<String>) -> Self {
        Self::new(href)
            .with_rel("first")
            .with_type("application/opds+json")
    }

    /// Create a last page link
    pub fn last_link(href: impl Into<String>) -> Self {
        Self::new(href)
            .with_rel("last")
            .with_type("application/opds+json")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_link_serialization() {
        let link = Opds2Link::new("/test")
            .with_rel("self")
            .with_type("application/opds+json");

        let json = serde_json::to_string(&link).unwrap();
        assert!(json.contains("\"href\":\"/test\""));
        assert!(json.contains("\"rel\":\"self\""));
        assert!(json.contains("\"type\":\"application/opds+json\""));
    }

    #[test]
    fn test_self_link() {
        let link = Opds2Link::self_link("/opds/v2");
        assert_eq!(link.href, "/opds/v2");
        assert_eq!(link.rel, Some("self".to_string()));
        assert_eq!(link.media_type, Some("application/opds+json".to_string()));
    }

    #[test]
    fn test_navigation_link() {
        let link = Opds2Link::navigation_link("/opds/v2/libraries", "All Libraries");
        assert_eq!(link.href, "/opds/v2/libraries");
        assert_eq!(link.title, Some("All Libraries".to_string()));
        assert_eq!(link.media_type, Some("application/opds+json".to_string()));
    }

    #[test]
    fn test_search_template() {
        let link = Opds2Link::search_template("/opds/v2/search{?query}");
        assert_eq!(link.href, "/opds/v2/search{?query}");
        assert_eq!(link.rel, Some("search".to_string()));
        assert_eq!(link.templated, Some(true));
    }

    #[test]
    fn test_acquisition_link() {
        let link = Opds2Link::acquisition_link("/books/123/file", "application/zip");
        assert_eq!(link.href, "/books/123/file");
        assert_eq!(
            link.rel,
            Some("http://opds-spec.org/acquisition/open-access".to_string())
        );
        assert_eq!(link.media_type, Some("application/zip".to_string()));
    }

    #[test]
    fn test_link_with_properties() {
        let link =
            Opds2Link::navigation_link("/libraries/123", "My Library").with_number_of_items(42);
        assert!(link.properties.is_some());
        assert_eq!(link.properties.unwrap().number_of_items, Some(42));
    }

    #[test]
    fn test_skip_serializing_none_fields() {
        let link = Opds2Link::new("/test");
        let json = serde_json::to_string(&link).unwrap();
        // Should only have href
        assert_eq!(json, r#"{"href":"/test"}"#);
    }
}
