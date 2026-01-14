use super::{OpdsAuthor, OpdsEntry, OpdsLink};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// OPDS feed (top-level catalog or acquisition feed)
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename = "feed")]
pub struct OpdsFeed {
    // Namespaces
    #[serde(rename = "@xmlns")]
    pub xmlns: String, // "http://www.w3.org/2005/Atom"

    #[serde(rename = "@xmlns:opds")]
    pub xmlns_opds: String, // "http://opds-spec.org/2010/catalog"

    #[serde(rename = "@xmlns:dc")]
    pub xmlns_dc: String, // "http://purl.org/dc/elements/1.1/"

    #[serde(rename = "@xmlns:pse", skip_serializing_if = "Option::is_none")]
    pub xmlns_pse: Option<String>, // "http://vaemendis.net/opds-pse/ns"

    // Required Atom elements
    pub id: String,
    pub title: String,
    pub updated: String, // ISO 8601 timestamp
    pub author: OpdsAuthor,

    // Links
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub link: Vec<OpdsLink>,

    // Entries (books, series, or subsections)
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub entry: Vec<OpdsEntry>,

    // Optional subtitle
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subtitle: Option<String>,

    // Optional icon
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,

    // Pagination metadata (for clients)
    #[serde(
        rename = "opensearch:totalResults",
        skip_serializing_if = "Option::is_none"
    )]
    pub total_results: Option<u64>,

    #[serde(
        rename = "opensearch:itemsPerPage",
        skip_serializing_if = "Option::is_none"
    )]
    pub items_per_page: Option<u32>,

    #[serde(
        rename = "opensearch:startIndex",
        skip_serializing_if = "Option::is_none"
    )]
    pub start_index: Option<u32>,
}

#[allow(dead_code)] // Public API for OPDS feed building
impl OpdsFeed {
    /// Create a new OPDS feed
    pub fn new(
        id: impl Into<String>,
        title: impl Into<String>,
        updated: DateTime<Utc>,
        include_pse: bool,
    ) -> Self {
        Self {
            xmlns: "http://www.w3.org/2005/Atom".to_string(),
            xmlns_opds: "http://opds-spec.org/2010/catalog".to_string(),
            xmlns_dc: "http://purl.org/dc/elements/1.1/".to_string(),
            xmlns_pse: if include_pse {
                Some("http://vaemendis.net/opds-pse/ns".to_string())
            } else {
                None
            },
            id: id.into(),
            title: title.into(),
            updated: updated.to_rfc3339(),
            author: OpdsAuthor {
                name: "Codex".to_string(),
                uri: None,
            },
            link: Vec::new(),
            entry: Vec::new(),
            subtitle: None,
            icon: None,
            total_results: None,
            items_per_page: None,
            start_index: None,
        }
    }

    /// Add a link to the feed
    pub fn add_link(mut self, link: OpdsLink) -> Self {
        self.link.push(link);
        self
    }

    /// Add multiple links
    pub fn with_links(mut self, links: Vec<OpdsLink>) -> Self {
        self.link.extend(links);
        self
    }

    /// Add an entry
    pub fn add_entry(mut self, entry: OpdsEntry) -> Self {
        self.entry.push(entry);
        self
    }

    /// Add multiple entries
    pub fn with_entries(mut self, entries: Vec<OpdsEntry>) -> Self {
        self.entry = entries;
        self
    }

    /// Set subtitle
    pub fn with_subtitle(mut self, subtitle: impl Into<String>) -> Self {
        self.subtitle = Some(subtitle.into());
        self
    }

    /// Set pagination metadata
    pub fn with_pagination(mut self, total: u64, per_page: u32, start: u32) -> Self {
        self.total_results = Some(total);
        self.items_per_page = Some(per_page);
        self.start_index = Some(start);
        self
    }

    /// Serialize to XML string
    pub fn to_xml(&self) -> Result<String, quick_xml::DeError> {
        let mut buffer = String::new();
        buffer.push_str(r#"<?xml version="1.0" encoding="UTF-8"?>"#);
        buffer.push('\n');

        let xml = quick_xml::se::to_string(self)?;
        buffer.push_str(&xml);

        Ok(buffer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_opds_feed_serialization() {
        let now = Utc::now();
        let feed = OpdsFeed::new("urn:uuid:root", "Test Catalog", now, false)
            .add_link(OpdsLink::self_link("/opds"))
            .add_link(OpdsLink::start_link("/opds"));

        let xml = feed.to_xml().unwrap();

        assert!(xml.contains(r#"<?xml version="1.0" encoding="UTF-8"?>"#));
        assert!(xml.contains("<feed"));
        assert!(xml.contains(r#"xmlns="http://www.w3.org/2005/Atom""#));
        assert!(xml.contains("<title>Test Catalog</title>"));
        assert!(xml.contains("<author><name>Codex</name></author>"));
    }

    #[test]
    fn test_opds_feed_with_pse() {
        let now = Utc::now();
        let feed = OpdsFeed::new("urn:uuid:root", "Test Catalog", now, true);

        let xml = feed.to_xml().unwrap();

        assert!(xml.contains(r#"xmlns:pse="http://vaemendis.net/opds-pse/ns""#));
    }

    #[test]
    fn test_opds_feed_with_entries() {
        let now = Utc::now();

        let entry = OpdsEntry::new("urn:uuid:book-1", "Test Book", now)
            .add_link(OpdsLink::acquisition_link("/books/1", "application/zip"));

        let feed = OpdsFeed::new("urn:uuid:root", "Test Catalog", now, false).add_entry(entry);

        let xml = feed.to_xml().unwrap();

        assert!(xml.contains("<entry>"));
        assert!(xml.contains("<title>Test Book</title>"));
    }
}
