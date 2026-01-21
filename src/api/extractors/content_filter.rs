//! Content filter for sharing tag-based access control
//!
//! This module provides content filtering based on user sharing tag grants.
//! It allows filtering series/books based on which sharing tags the user
//! has access to (allow) or is restricted from (deny).

use crate::db::repositories::SharingTagRepository;
use sea_orm::DatabaseConnection;
use std::collections::HashSet;
use uuid::Uuid;

/// Content filter based on user's sharing tag grants
///
/// This struct encapsulates the sharing tag access rules for a user
/// and can be used to filter content queries efficiently.
#[derive(Debug, Clone, Default)]
pub struct ContentFilter {
    /// Series IDs that are explicitly excluded (user has deny grants for their tags)
    pub excluded_series_ids: HashSet<Uuid>,
    /// Whether the user has any sharing tag grants at all
    /// If false, the user can see all unrestricted content
    pub has_restrictions: bool,
}

impl ContentFilter {
    /// Create a content filter for a user based on their sharing tag grants
    ///
    /// This fetches the user's sharing tag grants and computes the excluded series IDs.
    /// The result can be reused for multiple queries in the same request.
    pub async fn for_user(db: &DatabaseConnection, user_id: Uuid) -> anyhow::Result<Self> {
        // Get excluded series IDs (series with tags the user has deny grants for)
        let excluded_series_ids =
            SharingTagRepository::get_excluded_series_ids_for_user(db, user_id).await?;

        let has_restrictions = !excluded_series_ids.is_empty();

        Ok(Self {
            excluded_series_ids: excluded_series_ids.into_iter().collect(),
            has_restrictions,
        })
    }

    /// Create a content filter that allows all content (no restrictions)
    #[allow(dead_code)]
    pub fn allow_all() -> Self {
        Self {
            excluded_series_ids: HashSet::new(),
            has_restrictions: false,
        }
    }

    /// Check if a series is visible to the user
    pub fn is_series_visible(&self, series_id: Uuid) -> bool {
        !self.excluded_series_ids.contains(&series_id)
    }

    /// Check if a book is visible to the user (based on its parent series)
    pub fn is_book_visible(&self, series_id: Uuid) -> bool {
        self.is_series_visible(series_id)
    }

    /// Filter a list of series IDs to only those visible to the user
    #[allow(dead_code)]
    pub fn filter_series_ids(&self, series_ids: Vec<Uuid>) -> Vec<Uuid> {
        if !self.has_restrictions {
            return series_ids;
        }
        series_ids
            .into_iter()
            .filter(|id| !self.excluded_series_ids.contains(id))
            .collect()
    }

    /// Get the excluded series IDs as a Vec (for SQL IN clauses)
    #[allow(dead_code)]
    pub fn excluded_ids(&self) -> Vec<Uuid> {
        self.excluded_series_ids.iter().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_allow_all() {
        let filter = ContentFilter::allow_all();
        assert!(!filter.has_restrictions);
        assert!(filter.excluded_series_ids.is_empty());

        let series_id = Uuid::new_v4();
        assert!(filter.is_series_visible(series_id));
    }

    #[test]
    fn test_filter_series_ids() {
        let excluded1 = Uuid::new_v4();
        let excluded2 = Uuid::new_v4();
        let allowed1 = Uuid::new_v4();
        let allowed2 = Uuid::new_v4();

        let filter = ContentFilter {
            excluded_series_ids: [excluded1, excluded2].into_iter().collect(),
            has_restrictions: true,
        };

        let input = vec![allowed1, excluded1, allowed2, excluded2];
        let output = filter.filter_series_ids(input);

        assert_eq!(output.len(), 2);
        assert!(output.contains(&allowed1));
        assert!(output.contains(&allowed2));
        assert!(!output.contains(&excluded1));
        assert!(!output.contains(&excluded2));
    }

    #[test]
    fn test_is_series_visible() {
        let excluded = Uuid::new_v4();
        let allowed = Uuid::new_v4();

        let filter = ContentFilter {
            excluded_series_ids: [excluded].into_iter().collect(),
            has_restrictions: true,
        };

        assert!(filter.is_series_visible(allowed));
        assert!(!filter.is_series_visible(excluded));
    }
}
