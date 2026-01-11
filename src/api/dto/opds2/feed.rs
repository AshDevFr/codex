use super::{FeedMetadata, Opds2Link, Publication};
use serde::{Deserialize, Serialize};

/// OPDS 2.0 Feed
///
/// The main container for OPDS 2.0 data. A feed contains metadata,
/// links, and one of: navigation, publications, or groups.
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct Opds2Feed {
    /// Feed metadata (title, pagination, etc.)
    pub metadata: FeedMetadata,

    /// Feed-level links (self, search, start, etc.)
    pub links: Vec<Opds2Link>,

    /// Navigation links (for navigation feeds)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub navigation: Option<Vec<Opds2Link>>,

    /// Publication entries (for acquisition feeds)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub publications: Option<Vec<Publication>>,

    /// Groups containing multiple collections
    #[serde(skip_serializing_if = "Option::is_none")]
    pub groups: Option<Vec<Group>>,
}

/// A group containing navigation or publications
///
/// Groups allow organizing multiple collections within a single feed.
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct Group {
    /// Group metadata (title required)
    pub metadata: FeedMetadata,

    /// Navigation links within this group
    #[serde(skip_serializing_if = "Option::is_none")]
    pub navigation: Option<Vec<Opds2Link>>,

    /// Publications within this group
    #[serde(skip_serializing_if = "Option::is_none")]
    pub publications: Option<Vec<Publication>>,
}

impl Opds2Feed {
    /// Create a navigation feed
    pub fn navigation(
        title: impl Into<String>,
        links: Vec<Opds2Link>,
        nav: Vec<Opds2Link>,
    ) -> Self {
        Self {
            metadata: FeedMetadata::new(title),
            links,
            navigation: Some(nav),
            publications: None,
            groups: None,
        }
    }

    /// Create a publications feed
    pub fn publications(
        title: impl Into<String>,
        links: Vec<Opds2Link>,
        pubs: Vec<Publication>,
    ) -> Self {
        let count = pubs.len() as i64;
        Self {
            metadata: FeedMetadata::new(title).with_pagination(count, count as i32, 1),
            links,
            navigation: None,
            publications: Some(pubs),
            groups: None,
        }
    }

    /// Create a grouped feed
    pub fn grouped(title: impl Into<String>, links: Vec<Opds2Link>, groups: Vec<Group>) -> Self {
        Self {
            metadata: FeedMetadata::new(title),
            links,
            navigation: None,
            publications: None,
            groups: Some(groups),
        }
    }

    /// Add a subtitle to the feed
    pub fn with_subtitle(mut self, subtitle: impl Into<String>) -> Self {
        self.metadata.subtitle = Some(subtitle.into());
        self
    }

    /// Add pagination metadata
    pub fn with_pagination(mut self, total: i64, per_page: i32, current: i32) -> Self {
        self.metadata.number_of_items = Some(total);
        self.metadata.items_per_page = Some(per_page);
        self.metadata.current_page = Some(current);
        self
    }

    /// Add a link to the feed
    pub fn add_link(mut self, link: Opds2Link) -> Self {
        self.links.push(link);
        self
    }

    /// Add a navigation link
    pub fn add_navigation(mut self, link: Opds2Link) -> Self {
        if self.navigation.is_none() {
            self.navigation = Some(Vec::new());
        }
        if let Some(nav) = &mut self.navigation {
            nav.push(link);
        }
        self
    }

    /// Add a publication
    pub fn add_publication(mut self, pub_: Publication) -> Self {
        if self.publications.is_none() {
            self.publications = Some(Vec::new());
        }
        if let Some(pubs) = &mut self.publications {
            pubs.push(pub_);
        }
        self
    }
}

impl Group {
    /// Create a new navigation group
    pub fn navigation(title: impl Into<String>, nav: Vec<Opds2Link>) -> Self {
        Self {
            metadata: FeedMetadata::new(title),
            navigation: Some(nav),
            publications: None,
        }
    }

    /// Create a new publications group
    pub fn publications(title: impl Into<String>, pubs: Vec<Publication>) -> Self {
        Self {
            metadata: FeedMetadata::new(title),
            navigation: None,
            publications: Some(pubs),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::dto::opds2::PublicationMetadata;

    #[test]
    fn test_navigation_feed_serialization() {
        let feed = Opds2Feed::navigation(
            "Test Catalog",
            vec![Opds2Link::self_link("/opds/v2")],
            vec![
                Opds2Link::navigation_link("/opds/v2/libraries", "Libraries"),
                Opds2Link::navigation_link("/opds/v2/recent", "Recent"),
            ],
        );

        let json = serde_json::to_string(&feed).unwrap();
        assert!(json.contains("\"title\":\"Test Catalog\""));
        assert!(json.contains("\"navigation\""));
        assert!(json.contains("\"Libraries\""));
        assert!(json.contains("\"Recent\""));
        // Should not have publications
        assert!(!json.contains("\"publications\""));
    }

    #[test]
    fn test_publications_feed_serialization() {
        let pubs = vec![
            Publication::new(PublicationMetadata::new("Book 1")),
            Publication::new(PublicationMetadata::new("Book 2")),
        ];

        let feed =
            Opds2Feed::publications("Books", vec![Opds2Link::self_link("/opds/v2/books")], pubs);

        let json = serde_json::to_string(&feed).unwrap();
        assert!(json.contains("\"title\":\"Books\""));
        assert!(json.contains("\"publications\""));
        assert!(json.contains("\"Book 1\""));
        assert!(json.contains("\"Book 2\""));
        // Should not have navigation
        assert!(!json.contains("\"navigation\""));
    }

    #[test]
    fn test_feed_with_pagination() {
        let feed = Opds2Feed::publications(
            "Paginated",
            vec![Opds2Link::self_link("/opds/v2/books?page=2")],
            vec![],
        )
        .with_pagination(100, 20, 2);

        let json = serde_json::to_string(&feed).unwrap();
        assert!(json.contains("\"numberOfItems\":100"));
        assert!(json.contains("\"itemsPerPage\":20"));
        assert!(json.contains("\"currentPage\":2"));
    }

    #[test]
    fn test_grouped_feed_serialization() {
        let groups = vec![
            Group::navigation(
                "Browse",
                vec![Opds2Link::navigation_link("/libraries", "Libraries")],
            ),
            Group::publications(
                "Recent",
                vec![Publication::new(PublicationMetadata::new("New Book"))],
            ),
        ];

        let feed = Opds2Feed::grouped("Home", vec![Opds2Link::self_link("/opds/v2")], groups);

        let json = serde_json::to_string(&feed).unwrap();
        assert!(json.contains("\"groups\""));
        assert!(json.contains("\"Browse\""));
        assert!(json.contains("\"Recent\""));
        assert!(json.contains("\"New Book\""));
    }

    #[test]
    fn test_add_methods() {
        let feed = Opds2Feed::navigation("Test", vec![], vec![])
            .add_link(Opds2Link::self_link("/test"))
            .add_navigation(Opds2Link::navigation_link("/nav", "Nav"));

        assert_eq!(feed.links.len(), 1);
        assert_eq!(feed.navigation.as_ref().unwrap().len(), 1);
    }
}
