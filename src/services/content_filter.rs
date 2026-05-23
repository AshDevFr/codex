//! Content filter for sharing tag-based access control
//!
//! This module provides content filtering based on user sharing tag grants.
//! It allows filtering series/books based on which sharing tags the user
//! has access to (allow) or is restricted from (deny).
//!
//! ## Access Rules (in order of precedence)
//!
//! 1. **Deny always wins**: If user has `deny` grant for ANY tag on a series → hidden
//! 2. **Whitelist mode** (user has any `allow` grants): User only sees series with allowed tags
//! 3. **No grants**: User sees everything (default-open behavior)

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
    /// Series IDs that user is allowed to see (when in whitelist mode)
    /// Only populated when user has allow grants
    pub allowed_series_ids: Option<HashSet<Uuid>>,
    /// Whether user has any allow grants (triggers whitelist mode)
    pub whitelist_mode: bool,
    /// Whether the user has any sharing tag restrictions at all
    pub has_restrictions: bool,
}

impl ContentFilter {
    /// Create a content filter for a user based on their sharing tag grants
    ///
    /// This fetches the user's sharing tag grants and computes the access rules.
    /// The result can be reused for multiple queries in the same request.
    pub async fn for_user(db: &DatabaseConnection, user_id: Uuid) -> anyhow::Result<Self> {
        // Get user's allow and deny tag IDs
        let allowed_tag_ids =
            SharingTagRepository::get_allowed_tag_ids_for_user(db, user_id).await?;

        // Get excluded series IDs (series with tags the user has deny grants for)
        let excluded_series_ids =
            SharingTagRepository::get_excluded_series_ids_for_user(db, user_id).await?;

        // Whitelist mode: if user has any allow grants
        let whitelist_mode = !allowed_tag_ids.is_empty();

        // Get allowed series IDs (series with tags the user has allow grants for)
        let allowed_series_ids = if whitelist_mode {
            let ids =
                SharingTagRepository::get_series_ids_with_any_tags(db, &allowed_tag_ids).await?;
            Some(ids.into_iter().collect())
        } else {
            None
        };

        let has_restrictions = !excluded_series_ids.is_empty() || whitelist_mode;

        Ok(Self {
            excluded_series_ids: excluded_series_ids.into_iter().collect(),
            allowed_series_ids,
            whitelist_mode,
            has_restrictions,
        })
    }

    /// Create a content filter that allows all content (no restrictions)
    #[allow(dead_code)]
    pub fn allow_all() -> Self {
        Self {
            excluded_series_ids: HashSet::new(),
            allowed_series_ids: None,
            whitelist_mode: false,
            has_restrictions: false,
        }
    }

    /// Check if a series is visible to the user
    ///
    /// Access rules:
    /// 1. Deny always wins - if series is in excluded set, return false
    /// 2. In whitelist mode: user only sees series with allowed tags
    /// 3. Otherwise, series is visible
    pub fn is_series_visible(&self, series_id: Uuid) -> bool {
        // Deny always wins
        if self.excluded_series_ids.contains(&series_id) {
            return false;
        }

        // Whitelist mode: user only sees series with allowed tags
        if self.whitelist_mode {
            if let Some(allowed) = &self.allowed_series_ids {
                return allowed.contains(&series_id);
            }
            return false;
        }

        true
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
            .filter(|id| self.is_series_visible(*id))
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
        assert!(!filter.whitelist_mode);

        let series_id = Uuid::new_v4();
        assert!(filter.is_series_visible(series_id));
    }

    #[test]
    fn test_deny_only_mode() {
        // User with only deny grants - sees everything except denied series
        let excluded = Uuid::new_v4();
        let allowed = Uuid::new_v4();

        let filter = ContentFilter {
            excluded_series_ids: [excluded].into_iter().collect(),
            allowed_series_ids: None,
            whitelist_mode: false,
            has_restrictions: true,
        };

        assert!(filter.is_series_visible(allowed));
        assert!(!filter.is_series_visible(excluded));
    }

    #[test]
    fn test_whitelist_mode_untagged_hidden() {
        // User with allow grants cannot see untagged series (whitelist = only allowed tags)
        let untagged_series = Uuid::new_v4();
        let allowed_tagged_series = Uuid::new_v4();

        let filter = ContentFilter {
            excluded_series_ids: HashSet::new(),
            allowed_series_ids: Some([allowed_tagged_series].into_iter().collect()),
            whitelist_mode: true,
            has_restrictions: true,
        };

        // Untagged series should NOT be visible in whitelist mode
        assert!(!filter.is_series_visible(untagged_series));
        // But allowed tagged series should be visible
        assert!(filter.is_series_visible(allowed_tagged_series));
    }

    #[test]
    fn test_whitelist_mode_allowed_tag_visible() {
        // User with allow grants can see series with allowed tags
        let allowed_series = Uuid::new_v4();

        let filter = ContentFilter {
            excluded_series_ids: HashSet::new(),
            allowed_series_ids: Some([allowed_series].into_iter().collect()),
            whitelist_mode: true,
            has_restrictions: true,
        };

        assert!(filter.is_series_visible(allowed_series));
    }

    #[test]
    fn test_whitelist_mode_unallowed_tag_hidden() {
        // User with allow grants cannot see series with tags they don't have allow for
        let allowed_series = Uuid::new_v4();
        let other_tagged_series = Uuid::new_v4();

        let filter = ContentFilter {
            excluded_series_ids: HashSet::new(),
            allowed_series_ids: Some([allowed_series].into_iter().collect()),
            whitelist_mode: true,
            has_restrictions: true,
        };

        assert!(filter.is_series_visible(allowed_series));
        assert!(!filter.is_series_visible(other_tagged_series));
    }

    #[test]
    fn test_deny_wins_over_allow() {
        // Series with both allowed and denied tags is hidden (deny wins)
        let mixed_series = Uuid::new_v4();

        let filter = ContentFilter {
            excluded_series_ids: [mixed_series].into_iter().collect(),
            allowed_series_ids: Some([mixed_series].into_iter().collect()),
            whitelist_mode: true,
            has_restrictions: true,
        };

        // Deny wins, so series should be hidden
        assert!(!filter.is_series_visible(mixed_series));
    }

    #[test]
    fn test_no_grants_sees_all() {
        // User with no grants sees everything
        let any_series = Uuid::new_v4();
        let tagged_series = Uuid::new_v4();

        let filter = ContentFilter {
            excluded_series_ids: HashSet::new(),
            allowed_series_ids: None,
            whitelist_mode: false,
            has_restrictions: false,
        };

        assert!(filter.is_series_visible(any_series));
        assert!(filter.is_series_visible(tagged_series));
    }

    #[test]
    fn test_filter_series_ids() {
        let excluded = Uuid::new_v4();
        let allowed = Uuid::new_v4();
        let untagged = Uuid::new_v4();

        let filter = ContentFilter {
            excluded_series_ids: [excluded].into_iter().collect(),
            allowed_series_ids: Some([allowed].into_iter().collect()),
            whitelist_mode: true,
            has_restrictions: true,
        };

        let input = vec![allowed, excluded, untagged];
        let output = filter.filter_series_ids(input);

        // In whitelist mode: only allowed series visible, excluded and untagged are hidden
        assert_eq!(output.len(), 1);
        assert!(output.contains(&allowed));
        assert!(!output.contains(&untagged)); // Untagged hidden in whitelist mode
        assert!(!output.contains(&excluded)); // Denied series hidden
    }

    #[test]
    fn test_is_book_visible() {
        let allowed_series = Uuid::new_v4();
        let denied_series = Uuid::new_v4();

        let filter = ContentFilter {
            excluded_series_ids: [denied_series].into_iter().collect(),
            allowed_series_ids: None,
            whitelist_mode: false,
            has_restrictions: true,
        };

        assert!(filter.is_book_visible(allowed_series));
        assert!(!filter.is_book_visible(denied_series));
    }
}
