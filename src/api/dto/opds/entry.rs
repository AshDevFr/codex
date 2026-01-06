use super::OpdsLink;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// OPDS entry (represents a book, series, or catalog section)
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct OpdsEntry {
    pub id: String,
    pub title: String,
    pub updated: String, // ISO 8601 timestamp

    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<OpdsContent>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<Vec<OpdsAuthor>>,

    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub link: Vec<OpdsLink>,

    // Dublin Core metadata
    #[serde(rename = "dc:issued", skip_serializing_if = "Option::is_none")]
    pub dc_issued: Option<String>,

    #[serde(rename = "dc:publisher", skip_serializing_if = "Option::is_none")]
    pub dc_publisher: Option<String>,

    #[serde(rename = "dc:language", skip_serializing_if = "Option::is_none")]
    pub dc_language: Option<String>,

    // PSE specific
    #[serde(rename = "pse:lastRead", skip_serializing_if = "Option::is_none")]
    pub pse_last_read: Option<u32>,

    // OPDS specific
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<OpdsContent>,

    // Categories (genres, tags)
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub category: Vec<OpdsCategory>,
}

impl OpdsEntry {
    /// Create a new entry with required fields
    pub fn new(id: impl Into<String>, title: impl Into<String>, updated: DateTime<Utc>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            updated: updated.to_rfc3339(),
            content: None,
            author: None,
            link: Vec::new(),
            dc_issued: None,
            dc_publisher: None,
            dc_language: None,
            pse_last_read: None,
            summary: None,
            category: Vec::new(),
        }
    }

    /// Add a link to the entry
    pub fn add_link(mut self, link: OpdsLink) -> Self {
        self.link.push(link);
        self
    }

    /// Add multiple links
    pub fn with_links(mut self, links: Vec<OpdsLink>) -> Self {
        self.link.extend(links);
        self
    }

    /// Set content (description)
    pub fn with_content(mut self, content_type: &str, text: impl Into<String>) -> Self {
        self.content = Some(OpdsContent {
            content_type: content_type.to_string(),
            value: text.into(),
        });
        self
    }

    /// Set summary
    pub fn with_summary(mut self, content_type: &str, text: impl Into<String>) -> Self {
        self.summary = Some(OpdsContent {
            content_type: content_type.to_string(),
            value: text.into(),
        });
        self
    }

    /// Add an author
    pub fn add_author(mut self, name: impl Into<String>) -> Self {
        let author = OpdsAuthor {
            name: name.into(),
            uri: None,
        };
        if let Some(ref mut authors) = self.author {
            authors.push(author);
        } else {
            self.author = Some(vec![author]);
        }
        self
    }

    /// Set Dublin Core issued date
    pub fn with_dc_issued(mut self, issued: impl Into<String>) -> Self {
        self.dc_issued = Some(issued.into());
        self
    }

    /// Set Dublin Core publisher
    pub fn with_dc_publisher(mut self, publisher: impl Into<String>) -> Self {
        self.dc_publisher = Some(publisher.into());
        self
    }

    /// Set PSE last read page
    pub fn with_pse_last_read(mut self, page: u32) -> Self {
        self.pse_last_read = Some(page);
        self
    }

    /// Add a category (genre/tag)
    pub fn add_category(mut self, term: impl Into<String>, label: Option<String>) -> Self {
        self.category.push(OpdsCategory {
            term: term.into(),
            label,
        });
        self
    }
}

/// Content element (can be text or HTML)
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct OpdsContent {
    #[serde(rename = "@type")]
    pub content_type: String, // "text" or "html"

    #[serde(rename = "$value")]
    pub value: String,
}

/// Author element
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct OpdsAuthor {
    pub name: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub uri: Option<String>,
}

/// Category element (for genres, tags)
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct OpdsCategory {
    #[serde(rename = "@term")]
    pub term: String,

    #[serde(rename = "@label", skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}
