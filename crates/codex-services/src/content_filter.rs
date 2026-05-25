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

use codex_db::repositories::{SeriesVisibility, SharingTagRepository};
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
    /// Create a content filter for a user based on their effective sharing tag grants.
    ///
    /// Effective grants are the union of the user's per-user `user_sharing_tags`
    /// rows and grants attached to every access group the user belongs to.
    /// Access groups can supply both `allow` and `deny` rules; users can
    /// override either side. Downstream behavior (deny-wins, whitelist mode)
    /// is unchanged — the only difference vs. the old implementation is the
    /// data source feeding the rule set.
    ///
    /// The result can be reused for multiple queries in the same request.
    pub async fn for_user(db: &DatabaseConnection, user_id: Uuid) -> anyhow::Result<Self> {
        let (allowed_tag_ids, denied_tag_ids) =
            SharingTagRepository::get_effective_grants(db, user_id).await?;

        // Excluded series = series tagged with any effective deny tag.
        let excluded_series_ids = if denied_tag_ids.is_empty() {
            Vec::new()
        } else {
            SharingTagRepository::get_series_ids_with_any_tags(db, &denied_tag_ids).await?
        };

        // Whitelist mode: any effective allow grant flips the user into
        // "only-tagged" mode (untagged content disappears).
        let whitelist_mode = !allowed_tag_ids.is_empty();

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

    /// Convert to a SQL-level visibility filter for use with repository methods.
    ///
    /// Returns `None` when the user has no restrictions, so callers can skip
    /// adding any visibility clause and let SeaORM emit the natural query.
    pub fn to_visibility(&self) -> Option<SeriesVisibility> {
        if !self.has_restrictions {
            return None;
        }
        Some(SeriesVisibility {
            excluded_series_ids: self.excluded_series_ids.iter().copied().collect(),
            allowed_series_ids: self
                .allowed_series_ids
                .as_ref()
                .map(|s| s.iter().copied().collect()),
        })
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

#[cfg(test)]
mod db_tests {
    //! DB-backed integration tests for the group-aware `ContentFilter`.
    //!
    //! These exercise the full path from `user_sharing_tags` and
    //! `user_access_groups` rows through `get_effective_grants` into the
    //! filter's boolean output. The pure unit tests above cover the
    //! in-memory rules; here we cover the data wiring.

    use super::*;
    use codex_db::ScanningStrategy;
    use codex_db::entities::user_access_groups::MembershipSource;
    use codex_db::entities::user_sharing_tags::AccessMode;
    use codex_db::repositories::{
        AccessGroupRepository, LibraryRepository, SeriesRepository, SharingTagRepository,
        UserRepository,
    };
    use codex_db::test_helpers::create_test_db;

    async fn make_user(db: &DatabaseConnection, username: &str) -> Uuid {
        use chrono::Utc;
        use codex_db::entities::users;
        let now = Utc::now();
        let model = users::Model {
            id: Uuid::new_v4(),
            username: username.to_string(),
            email: format!("{}@test.com", username),
            password_hash: "hash".to_string(),
            role: "reader".to_string(),
            is_active: true,
            email_verified: true,
            permissions: serde_json::json!([]),
            created_at: now,
            updated_at: now,
            last_login_at: None,
        };
        UserRepository::create(db, &model).await.unwrap().id
    }

    /// The canonical scenario from the plan:
    /// - Group "Manga Readers" allows "manga"
    /// - User is a member of that group
    /// - User has a personal deny on "18+"
    /// Expectations:
    /// - manga-tagged series → visible (allow from group)
    /// - manga + 18+ series → hidden (deny-wins, even across sources)
    /// - 18+-only series → hidden (never had an allow; also explicitly denied)
    /// - untagged series → hidden (whitelist mode is triggered by any allow)
    #[tokio::test]
    async fn test_for_user_group_allow_plus_user_deny() {
        let (db, _temp_dir) = create_test_db().await;
        let conn = db.sea_orm_connection();

        let user_id = make_user(conn, "reader").await;
        let library = LibraryRepository::create(conn, "Lib", "/tmp/lib", ScanningStrategy::Default)
            .await
            .unwrap();
        let manga_only = SeriesRepository::create(conn, library.id, "Manga Only", None)
            .await
            .unwrap();
        let manga_adult = SeriesRepository::create(conn, library.id, "Manga Adult", None)
            .await
            .unwrap();
        let adult_only = SeriesRepository::create(conn, library.id, "Adult Only", None)
            .await
            .unwrap();
        let untagged = SeriesRepository::create(conn, library.id, "Untagged", None)
            .await
            .unwrap();

        let manga = SharingTagRepository::create(conn, "manga", None)
            .await
            .unwrap();
        let adult = SharingTagRepository::create(conn, "18+", None)
            .await
            .unwrap();

        SharingTagRepository::add_tag_to_series(conn, manga_only.id, manga.id)
            .await
            .unwrap();
        SharingTagRepository::add_tag_to_series(conn, manga_adult.id, manga.id)
            .await
            .unwrap();
        SharingTagRepository::add_tag_to_series(conn, manga_adult.id, adult.id)
            .await
            .unwrap();
        SharingTagRepository::add_tag_to_series(conn, adult_only.id, adult.id)
            .await
            .unwrap();

        let group = AccessGroupRepository::create(conn, "Manga Readers", None)
            .await
            .unwrap();
        AccessGroupRepository::set_grant(conn, group.id, manga.id, AccessMode::Allow)
            .await
            .unwrap();
        AccessGroupRepository::add_member(conn, group.id, user_id, MembershipSource::Manual)
            .await
            .unwrap();

        SharingTagRepository::set_user_grant(conn, user_id, adult.id, AccessMode::Deny)
            .await
            .unwrap();

        let filter = ContentFilter::for_user(conn, user_id).await.unwrap();

        assert!(filter.whitelist_mode, "any allow flips whitelist mode on");
        assert!(filter.has_restrictions);
        assert!(filter.is_series_visible(manga_only.id), "manga visible");
        assert!(
            !filter.is_series_visible(manga_adult.id),
            "deny-wins across sources"
        );
        assert!(!filter.is_series_visible(adult_only.id), "deny hides it");
        assert!(
            !filter.is_series_visible(untagged.id),
            "whitelist mode hides untagged content"
        );
    }

    /// Group-only allow, no user-side grants. The group's tag should drive
    /// visibility just as a per-user allow would.
    #[tokio::test]
    async fn test_for_user_group_allow_only() {
        let (db, _temp_dir) = create_test_db().await;
        let conn = db.sea_orm_connection();

        let user_id = make_user(conn, "u").await;
        let library = LibraryRepository::create(conn, "Lib", "/tmp/lib", ScanningStrategy::Default)
            .await
            .unwrap();
        let allowed_series = SeriesRepository::create(conn, library.id, "Allowed", None)
            .await
            .unwrap();
        let other_series = SeriesRepository::create(conn, library.id, "Other", None)
            .await
            .unwrap();

        let allow_tag = SharingTagRepository::create(conn, "allow", None)
            .await
            .unwrap();
        let other_tag = SharingTagRepository::create(conn, "other", None)
            .await
            .unwrap();
        SharingTagRepository::add_tag_to_series(conn, allowed_series.id, allow_tag.id)
            .await
            .unwrap();
        SharingTagRepository::add_tag_to_series(conn, other_series.id, other_tag.id)
            .await
            .unwrap();

        let group = AccessGroupRepository::create(conn, "G", None)
            .await
            .unwrap();
        AccessGroupRepository::set_grant(conn, group.id, allow_tag.id, AccessMode::Allow)
            .await
            .unwrap();
        AccessGroupRepository::add_member(conn, group.id, user_id, MembershipSource::Manual)
            .await
            .unwrap();

        let filter = ContentFilter::for_user(conn, user_id).await.unwrap();

        assert!(filter.is_series_visible(allowed_series.id));
        assert!(!filter.is_series_visible(other_series.id));
    }

    /// User in two groups: one allows the tag, the other denies it.
    /// Deny must win.
    #[tokio::test]
    async fn test_for_user_multi_group_deny_wins() {
        let (db, _temp_dir) = create_test_db().await;
        let conn = db.sea_orm_connection();

        let user_id = make_user(conn, "u").await;
        let library = LibraryRepository::create(conn, "Lib", "/tmp/lib", ScanningStrategy::Default)
            .await
            .unwrap();
        let series = SeriesRepository::create(conn, library.id, "Series", None)
            .await
            .unwrap();
        let tag = SharingTagRepository::create(conn, "tag", None)
            .await
            .unwrap();
        SharingTagRepository::add_tag_to_series(conn, series.id, tag.id)
            .await
            .unwrap();

        let g_allow = AccessGroupRepository::create(conn, "Allow Group", None)
            .await
            .unwrap();
        let g_deny = AccessGroupRepository::create(conn, "Deny Group", None)
            .await
            .unwrap();
        AccessGroupRepository::set_grant(conn, g_allow.id, tag.id, AccessMode::Allow)
            .await
            .unwrap();
        AccessGroupRepository::set_grant(conn, g_deny.id, tag.id, AccessMode::Deny)
            .await
            .unwrap();
        AccessGroupRepository::add_member(conn, g_allow.id, user_id, MembershipSource::Manual)
            .await
            .unwrap();
        AccessGroupRepository::add_member(conn, g_deny.id, user_id, MembershipSource::Manual)
            .await
            .unwrap();

        let filter = ContentFilter::for_user(conn, user_id).await.unwrap();
        assert!(!filter.is_series_visible(series.id));
    }

    /// User in two groups, each allowing a different tag. Both tags' series
    /// must be visible (union of allows).
    #[tokio::test]
    async fn test_for_user_multi_group_union_of_allows() {
        let (db, _temp_dir) = create_test_db().await;
        let conn = db.sea_orm_connection();

        let user_id = make_user(conn, "u").await;
        let library = LibraryRepository::create(conn, "Lib", "/tmp/lib", ScanningStrategy::Default)
            .await
            .unwrap();
        let s1 = SeriesRepository::create(conn, library.id, "S1", None)
            .await
            .unwrap();
        let s2 = SeriesRepository::create(conn, library.id, "S2", None)
            .await
            .unwrap();
        let t1 = SharingTagRepository::create(conn, "t1", None)
            .await
            .unwrap();
        let t2 = SharingTagRepository::create(conn, "t2", None)
            .await
            .unwrap();
        SharingTagRepository::add_tag_to_series(conn, s1.id, t1.id)
            .await
            .unwrap();
        SharingTagRepository::add_tag_to_series(conn, s2.id, t2.id)
            .await
            .unwrap();

        let g1 = AccessGroupRepository::create(conn, "G1", None)
            .await
            .unwrap();
        let g2 = AccessGroupRepository::create(conn, "G2", None)
            .await
            .unwrap();
        AccessGroupRepository::set_grant(conn, g1.id, t1.id, AccessMode::Allow)
            .await
            .unwrap();
        AccessGroupRepository::set_grant(conn, g2.id, t2.id, AccessMode::Allow)
            .await
            .unwrap();
        AccessGroupRepository::add_member(conn, g1.id, user_id, MembershipSource::Manual)
            .await
            .unwrap();
        AccessGroupRepository::add_member(conn, g2.id, user_id, MembershipSource::Manual)
            .await
            .unwrap();

        let filter = ContentFilter::for_user(conn, user_id).await.unwrap();
        assert!(filter.is_series_visible(s1.id));
        assert!(filter.is_series_visible(s2.id));
    }

    /// User in zero groups behaves identically to the pre-group implementation:
    /// only per-user grants matter.
    #[tokio::test]
    async fn test_for_user_no_groups_matches_legacy_behavior() {
        let (db, _temp_dir) = create_test_db().await;
        let conn = db.sea_orm_connection();

        let user_id = make_user(conn, "u").await;
        let library = LibraryRepository::create(conn, "Lib", "/tmp/lib", ScanningStrategy::Default)
            .await
            .unwrap();
        let allowed_series = SeriesRepository::create(conn, library.id, "Allowed", None)
            .await
            .unwrap();
        let denied_series = SeriesRepository::create(conn, library.id, "Denied", None)
            .await
            .unwrap();
        let untagged_series = SeriesRepository::create(conn, library.id, "Untagged", None)
            .await
            .unwrap();

        let allow_tag = SharingTagRepository::create(conn, "ok", None)
            .await
            .unwrap();
        let deny_tag = SharingTagRepository::create(conn, "no", None)
            .await
            .unwrap();
        SharingTagRepository::add_tag_to_series(conn, allowed_series.id, allow_tag.id)
            .await
            .unwrap();
        SharingTagRepository::add_tag_to_series(conn, denied_series.id, deny_tag.id)
            .await
            .unwrap();

        SharingTagRepository::set_user_grant(conn, user_id, allow_tag.id, AccessMode::Allow)
            .await
            .unwrap();
        SharingTagRepository::set_user_grant(conn, user_id, deny_tag.id, AccessMode::Deny)
            .await
            .unwrap();

        let filter = ContentFilter::for_user(conn, user_id).await.unwrap();
        assert!(filter.whitelist_mode);
        assert!(filter.is_series_visible(allowed_series.id));
        assert!(!filter.is_series_visible(denied_series.id));
        assert!(!filter.is_series_visible(untagged_series.id));
    }

    /// User with neither group memberships nor per-user grants sees everything.
    #[tokio::test]
    async fn test_for_user_no_grants_no_groups_default_open() {
        let (db, _temp_dir) = create_test_db().await;
        let conn = db.sea_orm_connection();

        let user_id = make_user(conn, "u").await;
        let library = LibraryRepository::create(conn, "Lib", "/tmp/lib", ScanningStrategy::Default)
            .await
            .unwrap();
        let series = SeriesRepository::create(conn, library.id, "Series", None)
            .await
            .unwrap();

        let filter = ContentFilter::for_user(conn, user_id).await.unwrap();
        assert!(!filter.has_restrictions);
        assert!(!filter.whitelist_mode);
        assert!(filter.is_series_visible(series.id));
    }

    /// Smoke check: building a filter for a user in 10 groups with ~50 grants
    /// each (~500 group grants total) should be well under 50ms on SQLite.
    /// Not a hard performance contract — just catches O(n*m) regressions.
    #[tokio::test]
    async fn test_for_user_build_is_fast_for_many_groups() {
        use std::time::Instant;

        let (db, _temp_dir) = create_test_db().await;
        let conn = db.sea_orm_connection();

        let user_id = make_user(conn, "perf").await;

        let mut all_tags = Vec::with_capacity(500);
        for i in 0..500 {
            let tag = SharingTagRepository::create(conn, &format!("tag-{i}"), None)
                .await
                .unwrap();
            all_tags.push(tag.id);
        }

        for g in 0..10 {
            let group = AccessGroupRepository::create(conn, &format!("group-{g}"), None)
                .await
                .unwrap();
            for j in 0..50 {
                let tag_id = all_tags[g * 50 + j];
                AccessGroupRepository::set_grant(conn, group.id, tag_id, AccessMode::Allow)
                    .await
                    .unwrap();
            }
            AccessGroupRepository::add_member(conn, group.id, user_id, MembershipSource::Manual)
                .await
                .unwrap();
        }

        let start = Instant::now();
        let filter = ContentFilter::for_user(conn, user_id).await.unwrap();
        let elapsed = start.elapsed();

        assert!(filter.whitelist_mode);
        // Loose bound: 250ms accounts for slow CI; the SQL is two queries
        // regardless of grant count, so this would only fail on a real regression.
        assert!(
            elapsed.as_millis() < 250,
            "ContentFilter::for_user took {}ms with 10 groups x 50 grants",
            elapsed.as_millis()
        );
    }
}
