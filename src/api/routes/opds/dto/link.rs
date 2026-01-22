use serde::{Deserialize, Serialize};

/// OPDS link element
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct OpdsLink {
    #[serde(rename = "@rel")]
    pub rel: String,

    #[serde(rename = "@href")]
    pub href: String,

    #[serde(rename = "@type", skip_serializing_if = "Option::is_none")]
    pub link_type: Option<String>,

    #[serde(rename = "@title", skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,

    // PSE (Page Streaming Extension) specific attributes
    #[serde(rename = "@pse:count", skip_serializing_if = "Option::is_none")]
    pub pse_count: Option<u32>,

    #[serde(rename = "@pse:lastRead", skip_serializing_if = "Option::is_none")]
    pub pse_last_read: Option<u32>,
}

impl OpdsLink {
    /// Create a new link
    pub fn new(rel: impl Into<String>, href: impl Into<String>) -> Self {
        Self {
            rel: rel.into(),
            href: href.into(),
            link_type: None,
            title: None,
            pse_count: None,
            pse_last_read: None,
        }
    }

    /// Set the link type (MIME type)
    pub fn with_type(mut self, link_type: impl Into<String>) -> Self {
        self.link_type = Some(link_type.into());
        self
    }

    /// Set the link title
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Set PSE count (total pages)
    pub fn with_pse_count(mut self, count: u32) -> Self {
        self.pse_count = Some(count);
        self
    }

    /// Set PSE last read page
    pub fn with_pse_last_read(mut self, page: u32) -> Self {
        self.pse_last_read = Some(page);
        self
    }

    // Common link relation constructors
    pub fn self_link(href: impl Into<String>) -> Self {
        Self::new("self", href)
            .with_type("application/atom+xml;profile=opds-catalog;kind=navigation")
    }

    pub fn start_link(href: impl Into<String>) -> Self {
        Self::new("start", href)
            .with_type("application/atom+xml;profile=opds-catalog;kind=navigation")
            .with_title("Home")
    }

    pub fn up_link(href: impl Into<String>, title: impl Into<String>) -> Self {
        Self::new("up", href)
            .with_type("application/atom+xml;profile=opds-catalog;kind=navigation")
            .with_title(title)
    }

    pub fn next_link(href: impl Into<String>) -> Self {
        Self::new("next", href)
            .with_type("application/atom+xml;profile=opds-catalog;kind=acquisition")
    }

    pub fn prev_link(href: impl Into<String>) -> Self {
        Self::new("previous", href)
            .with_type("application/atom+xml;profile=opds-catalog;kind=acquisition")
    }

    pub fn subsection_link(href: impl Into<String>, title: impl Into<String>) -> Self {
        Self::new("subsection", href)
            .with_type("application/atom+xml;profile=opds-catalog;kind=acquisition")
            .with_title(title)
    }

    pub fn acquisition_link(href: impl Into<String>, mime_type: impl Into<String>) -> Self {
        Self::new("http://opds-spec.org/acquisition", href).with_type(mime_type)
    }

    pub fn thumbnail_link(href: impl Into<String>) -> Self {
        Self::new("http://opds-spec.org/image/thumbnail", href).with_type("image/jpeg")
    }

    pub fn cover_link(href: impl Into<String>) -> Self {
        Self::new("http://opds-spec.org/image", href).with_type("image/jpeg")
    }

    pub fn pse_stream_link(
        href: impl Into<String>,
        page_count: u32,
        last_read: Option<u32>,
    ) -> Self {
        let mut link = Self::new("http://vaemendis.net/opds-pse/stream", href)
            .with_type("application/atom+xml;profile=opds-catalog")
            .with_pse_count(page_count);

        if let Some(page) = last_read {
            link = link.with_pse_last_read(page);
        }

        link
    }

    pub fn search_link(href: impl Into<String>) -> Self {
        Self::new("search", href)
            .with_type("application/opensearchdescription+xml")
            .with_title("Search")
    }
}
