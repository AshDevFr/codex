//! Content filter: what a user is allowed to see.
//!
//! A user's visibility comes from their access groups and their personal
//! sharing-tag grants. Each access group holds allow/deny rules on four
//! types: sharing tags, regular tags, genres and libraries.
//!
//! ## Rules
//!
//! 1. **Admins and maintainers see everything.** No group or grant applies to
//!    them: maintainers curate metadata, covers and scans, which needs the
//!    whole library.
//! 2. **Allows, per group.** In one group, values of the same type combine as
//!    "any of" (or "all of", per the group's switch for that type; libraries
//!    are always "any of"), and different types combine with AND. "Allow
//!    library Manga + allow genre Kids" is Kids-genre series in Manga.
//! 3. **Allows, across groups: OR.** Each group grants its own scope, and the
//!    user sees the union. Personal sharing-tag allows count as one more group.
//! 4. **Denies are global and always win.** Every deny, from every group and
//!    from personal grants, is collected into one set and removed at the end.
//!    A deny is not scoped to the group it is written in; that is what stops
//!    another group from granting it back.
//! 5. **No allow anywhere** (or only deny-only groups): the user sees
//!    everything except what is denied.
//!
//! visible = (union over groups with allows of: AND over types of: match) minus denies

use codex_db::entities::{
    access_groups::RuleMatch, user_access_groups, user_sharing_tags, user_sharing_tags::AccessMode,
};
use codex_db::repositories::{
    AccessGroupRules, AccessGroupRulesRepository, SeriesVisibility, UserRepository,
};
use codex_models::permissions::UserRole;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

/// Content filter based on the user's access groups and grants
///
/// This struct encapsulates the access rules for a user and can be used to
/// filter content queries efficiently.
#[derive(Debug, Clone, Default)]
pub struct ContentFilter {
    /// Series IDs that are explicitly excluded (matched by a deny rule)
    pub excluded_series_ids: HashSet<Uuid>,
    /// Series IDs that user is allowed to see (when in whitelist mode)
    /// Only populated when some group or grant allows something
    pub allowed_series_ids: Option<HashSet<Uuid>>,
    /// Whether any allow rule applies (triggers whitelist mode)
    pub whitelist_mode: bool,
    /// Whether the user has any restrictions at all
    pub has_restrictions: bool,
    /// Libraries hidden by a library deny rule
    pub hidden_library_ids: HashSet<Uuid>,
    /// In whitelist mode, the libraries the user may see: those named by an
    /// allow rule and those holding at least one visible series
    pub visible_library_ids: Option<HashSet<Uuid>>,
}

/// Series IDs per rule target, resolved once for every group of a user.
#[derive(Default)]
struct ResolvedTargets {
    sharing_tags: HashMap<Uuid, HashSet<Uuid>>,
    tags: HashMap<String, HashSet<Uuid>>,
    genres: HashMap<String, HashSet<Uuid>>,
    libraries: HashMap<Uuid, HashSet<Uuid>>,
}

impl ContentFilter {
    /// Build the content filter for a user.
    ///
    /// Runs a fixed number of queries regardless of how many groups or rules
    /// the user has. The result can be reused for multiple queries in the
    /// same request.
    pub async fn for_user(db: &DatabaseConnection, user_id: Uuid) -> anyhow::Result<Self> {
        let Some(user) = UserRepository::get_by_id(db, user_id).await? else {
            // Fail closed: an unknown user sees nothing.
            return Ok(Self::deny_all());
        };
        if bypasses_content_filter(user.get_role()) {
            return Ok(Self::allow_all());
        }

        // Personal sharing-tag grants act as one more group.
        let mut personal = AccessGroupRules::default();
        for grant in user_sharing_tags::Entity::find()
            .filter(user_sharing_tags::Column::UserId.eq(user_id))
            .all(db)
            .await?
        {
            personal
                .sharing_tags
                .push(codex_db::repositories::SharingTagRule {
                    sharing_tag_id: grant.sharing_tag_id,
                    access_mode: grant.get_access_mode(),
                });
        }

        let group_ids: Vec<Uuid> = user_access_groups::Entity::find()
            .filter(user_access_groups::Column::UserId.eq(user_id))
            .all(db)
            .await?
            .into_iter()
            .map(|m| m.access_group_id)
            .collect();
        let mut rule_sets: Vec<AccessGroupRules> =
            AccessGroupRulesRepository::load_for_groups(db, &group_ids)
                .await?
                .into_values()
                .collect();
        rule_sets.push(personal);

        let targets = Self::resolve_targets(db, &rule_sets).await?;

        let mut excluded_series_ids: HashSet<Uuid> = HashSet::new();
        let mut allowed: HashSet<Uuid> = HashSet::new();
        let mut whitelist_mode = false;

        for rules in &rule_sets {
            excluded_series_ids.extend(Self::denied_by(rules, &targets));
            if let Some(granted) = Self::granted_by(rules, &targets) {
                whitelist_mode = true;
                allowed.extend(granted);
            }
        }

        let hidden_library_ids: HashSet<Uuid> = rule_sets
            .iter()
            .flat_map(|r| r.libraries.iter())
            .filter(|r| r.access_mode == AccessMode::Deny)
            .map(|r| r.library_id)
            .collect();

        let visible_library_ids = if whitelist_mode {
            let visible_series: Vec<Uuid> = allowed
                .iter()
                .filter(|id| !excluded_series_ids.contains(id))
                .copied()
                .collect();
            let mut libraries =
                AccessGroupRulesRepository::library_ids_of_series(db, &visible_series).await?;
            // A library an allow rule names is shown even before it has series.
            libraries.extend(
                rule_sets
                    .iter()
                    .filter(|r| r.has_allows())
                    .flat_map(|r| r.libraries.iter())
                    .filter(|r| r.access_mode == AccessMode::Allow)
                    .map(|r| r.library_id),
            );
            Some(libraries)
        } else {
            None
        };

        let has_restrictions =
            !excluded_series_ids.is_empty() || whitelist_mode || !hidden_library_ids.is_empty();
        Ok(Self {
            excluded_series_ids,
            allowed_series_ids: whitelist_mode.then_some(allowed),
            whitelist_mode,
            has_restrictions,
            hidden_library_ids,
            visible_library_ids,
        })
    }

    /// The user's filter, reused from the per-process cache while it is valid.
    ///
    /// Handlers should use this rather than [`Self::for_user`]: see
    /// [`crate::content_filter_cache`] for when a cached filter is rebuilt.
    pub async fn cached_for_user(
        db: &DatabaseConnection,
        user_id: Uuid,
    ) -> anyhow::Result<std::sync::Arc<Self>> {
        crate::content_filter_cache::ContentFilterCache::global()
            .get(db, user_id)
            .await
    }

    /// Resolve every target named by any rule to its series, one query per type.
    async fn resolve_targets(
        db: &DatabaseConnection,
        rule_sets: &[AccessGroupRules],
    ) -> anyhow::Result<ResolvedTargets> {
        let mut sharing_tag_ids = HashSet::new();
        let mut tag_names = HashSet::new();
        let mut genre_names = HashSet::new();
        let mut library_ids = HashSet::new();
        for rules in rule_sets {
            sharing_tag_ids.extend(rules.sharing_tags.iter().map(|r| r.sharing_tag_id));
            tag_names.extend(rules.tags.iter().map(|r| r.normalized_name()));
            genre_names.extend(rules.genres.iter().map(|r| r.normalized_name()));
            library_ids.extend(rules.libraries.iter().map(|r| r.library_id));
        }

        let sharing_tag_ids: Vec<Uuid> = sharing_tag_ids.into_iter().collect();
        let tag_names: Vec<String> = tag_names.into_iter().collect();
        let genre_names: Vec<String> = genre_names.into_iter().collect();
        let library_ids: Vec<Uuid> = library_ids.into_iter().collect();

        Ok(ResolvedTargets {
            sharing_tags: AccessGroupRulesRepository::series_by_sharing_tag(db, &sharing_tag_ids)
                .await?,
            tags: AccessGroupRulesRepository::series_by_tag_name(db, &tag_names).await?,
            genres: AccessGroupRulesRepository::series_by_genre_name(db, &genre_names).await?,
            libraries: AccessGroupRulesRepository::series_by_library(db, &library_ids).await?,
        })
    }

    /// Every series matched by any deny rule of this rule set.
    fn denied_by(rules: &AccessGroupRules, targets: &ResolvedTargets) -> HashSet<Uuid> {
        let mut denied = HashSet::new();
        let empty = HashSet::new();
        for rule in rules
            .sharing_tags
            .iter()
            .filter(|r| r.access_mode == AccessMode::Deny)
        {
            denied.extend(
                targets
                    .sharing_tags
                    .get(&rule.sharing_tag_id)
                    .unwrap_or(&empty),
            );
        }
        for rule in rules
            .tags
            .iter()
            .filter(|r| r.access_mode == AccessMode::Deny)
        {
            denied.extend(targets.tags.get(&rule.normalized_name()).unwrap_or(&empty));
        }
        for rule in rules
            .genres
            .iter()
            .filter(|r| r.access_mode == AccessMode::Deny)
        {
            denied.extend(
                targets
                    .genres
                    .get(&rule.normalized_name())
                    .unwrap_or(&empty),
            );
        }
        for rule in rules
            .libraries
            .iter()
            .filter(|r| r.access_mode == AccessMode::Deny)
        {
            denied.extend(targets.libraries.get(&rule.library_id).unwrap_or(&empty));
        }
        denied
    }

    /// What this rule set's allow rules grant, or `None` if it has none.
    ///
    /// Each type with allows yields one set (any/all per the group's switch),
    /// and the sets of different types are intersected.
    fn granted_by(rules: &AccessGroupRules, targets: &ResolvedTargets) -> Option<HashSet<Uuid>> {
        let empty = HashSet::new();
        let mut per_type: Vec<HashSet<Uuid>> = Vec::new();

        let sharing: Vec<&HashSet<Uuid>> = rules
            .sharing_tags
            .iter()
            .filter(|r| r.access_mode == AccessMode::Allow)
            .map(|r| {
                targets
                    .sharing_tags
                    .get(&r.sharing_tag_id)
                    .unwrap_or(&empty)
            })
            .collect();
        if !sharing.is_empty() {
            per_type.push(combine(&sharing, rules.sharing_tag_match));
        }

        let tags: Vec<&HashSet<Uuid>> = rules
            .tags
            .iter()
            .filter(|r| r.access_mode == AccessMode::Allow)
            .map(|r| targets.tags.get(&r.normalized_name()).unwrap_or(&empty))
            .collect();
        if !tags.is_empty() {
            per_type.push(combine(&tags, rules.tag_match));
        }

        let genres: Vec<&HashSet<Uuid>> = rules
            .genres
            .iter()
            .filter(|r| r.access_mode == AccessMode::Allow)
            .map(|r| targets.genres.get(&r.normalized_name()).unwrap_or(&empty))
            .collect();
        if !genres.is_empty() {
            per_type.push(combine(&genres, rules.genre_match));
        }

        // A series is in one library, so "all of" can never match: always any.
        let libraries: Vec<&HashSet<Uuid>> = rules
            .libraries
            .iter()
            .filter(|r| r.access_mode == AccessMode::Allow)
            .map(|r| targets.libraries.get(&r.library_id).unwrap_or(&empty))
            .collect();
        if !libraries.is_empty() {
            per_type.push(combine(&libraries, RuleMatch::Any));
        }

        let mut iter = per_type.into_iter();
        let first = iter.next()?;
        Some(iter.fold(first, |acc, set| acc.intersection(&set).copied().collect()))
    }

    /// A filter under which nothing is visible.
    fn deny_all() -> Self {
        Self {
            excluded_series_ids: HashSet::new(),
            allowed_series_ids: Some(HashSet::new()),
            whitelist_mode: true,
            has_restrictions: true,
            hidden_library_ids: HashSet::new(),
            visible_library_ids: Some(HashSet::new()),
        }
    }

    /// Create a content filter that allows all content (no restrictions)
    pub fn allow_all() -> Self {
        Self {
            excluded_series_ids: HashSet::new(),
            allowed_series_ids: None,
            whitelist_mode: false,
            has_restrictions: false,
            hidden_library_ids: HashSet::new(),
            visible_library_ids: None,
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

    /// Check if a library is visible to the user
    pub fn is_library_visible(&self, library_id: Uuid) -> bool {
        if self.hidden_library_ids.contains(&library_id) {
            return false;
        }
        match &self.visible_library_ids {
            Some(visible) => visible.contains(&library_id),
            None => true,
        }
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

/// Roles that access groups and sharing-tag grants never restrict.
pub fn bypasses_content_filter(role: UserRole) -> bool {
    matches!(role, UserRole::Admin | UserRole::Maintainer)
}

/// Combine the series sets of one rule type: union for "any", intersection for "all".
fn combine(sets: &[&HashSet<Uuid>], mode: RuleMatch) -> HashSet<Uuid> {
    match mode {
        RuleMatch::Any => sets.iter().flat_map(|s| s.iter().copied()).collect(),
        RuleMatch::All => {
            let mut iter = sets.iter();
            let Some(first) = iter.next() else {
                return HashSet::new();
            };
            iter.fold((*first).clone(), |acc, set| {
                acc.intersection(set).copied().collect()
            })
        }
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
            ..Default::default()
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
            ..Default::default()
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
            ..Default::default()
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
            ..Default::default()
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
            ..Default::default()
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
            ..Default::default()
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
            ..Default::default()
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
            ..Default::default()
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
    //! `user_access_groups` rows and group rules into the
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

    // ---------- Rules on tags, genres and libraries ----------

    use codex_db::entities::access_groups::RuleMatch;
    use codex_db::repositories::{
        AccessGroupRules, AccessGroupRulesRepository, GenreRepository, LibraryRule, NameRule,
        SharingTagRule, TagRepository,
    };

    struct Fixture {
        db: codex_db::Database,
        _temp_dir: tempfile::TempDir,
    }

    impl Fixture {
        async fn new() -> Self {
            let (db, _temp_dir) = create_test_db().await;
            Self { db, _temp_dir }
        }

        fn conn(&self) -> &DatabaseConnection {
            self.db.sea_orm_connection()
        }

        async fn library(&self, name: &str) -> Uuid {
            LibraryRepository::create(
                self.conn(),
                name,
                &format!("/tmp/{name}"),
                ScanningStrategy::Default,
            )
            .await
            .unwrap()
            .id
        }

        async fn series(&self, library_id: Uuid, name: &str, genres: &[&str]) -> Uuid {
            let series = SeriesRepository::create(self.conn(), library_id, name, None)
                .await
                .unwrap();
            for genre in genres {
                GenreRepository::add_genre_to_series(self.conn(), series.id, genre)
                    .await
                    .unwrap();
            }
            series.id
        }

        async fn group(&self, name: &str, user_id: Uuid, rules: AccessGroupRules) -> Uuid {
            let group = AccessGroupRepository::create(self.conn(), name, None)
                .await
                .unwrap();
            AccessGroupRulesRepository::replace(self.conn(), group.id, &rules)
                .await
                .unwrap();
            AccessGroupRepository::add_member(
                self.conn(),
                group.id,
                user_id,
                MembershipSource::Manual,
            )
            .await
            .unwrap();
            group.id
        }

        async fn filter(&self, user_id: Uuid) -> ContentFilter {
            ContentFilter::for_user(self.conn(), user_id).await.unwrap()
        }
    }

    fn allow_library(library_id: Uuid) -> LibraryRule {
        LibraryRule {
            library_id,
            access_mode: AccessMode::Allow,
        }
    }

    fn deny_library(library_id: Uuid) -> LibraryRule {
        LibraryRule {
            library_id,
            access_mode: AccessMode::Deny,
        }
    }

    async fn make_with_role(db: &DatabaseConnection, username: &str, role: &str) -> Uuid {
        let id = make_user(db, username).await;
        let mut user = UserRepository::get_by_id(db, id).await.unwrap().unwrap();
        user.role = role.to_string();
        UserRepository::update(db, &user).await.unwrap();
        id
    }

    async fn make_admin(db: &DatabaseConnection, username: &str) -> Uuid {
        make_with_role(db, username, "admin").await
    }

    // ---------- Library visibility ----------

    #[tokio::test]
    async fn a_denied_library_is_hidden() {
        let f = Fixture::new().await;
        let user = make_user(f.conn(), "kid").await;
        let adult = f.library("Adult").await;
        let manga = f.library("Manga").await;
        f.group(
            "No adult library",
            user,
            AccessGroupRules {
                libraries: vec![deny_library(adult)],
                ..Default::default()
            },
        )
        .await;

        let filter = f.filter(user).await;
        assert!(!filter.is_library_visible(adult));
        assert!(
            filter.is_library_visible(manga),
            "open mode keeps other libraries"
        );
    }

    /// In whitelist mode a library is listed only if something in it is visible.
    #[tokio::test]
    async fn whitelist_mode_lists_only_libraries_with_visible_series() {
        let f = Fixture::new().await;
        let user = make_user(f.conn(), "kid").await;
        let manga = f.library("Manga").await;
        let comics = f.library("Comics").await;
        let empty = f.library("Empty").await;
        f.series(manga, "Kids manga", &["Kids"]).await;
        f.series(comics, "Action comic", &["Action"]).await;
        f.group(
            "Kids genre",
            user,
            AccessGroupRules {
                genres: vec![NameRule::new("Kids", AccessMode::Allow)],
                ..Default::default()
            },
        )
        .await;

        let filter = f.filter(user).await;
        assert!(filter.is_library_visible(manga));
        assert!(!filter.is_library_visible(comics));
        assert!(!filter.is_library_visible(empty));
    }

    /// A library allow shows the library even before it has any series.
    #[tokio::test]
    async fn an_allowed_library_is_listed_even_when_empty() {
        let f = Fixture::new().await;
        let user = make_user(f.conn(), "kid").await;
        let kids = f.library("Kids").await;
        let manga = f.library("Manga").await;
        f.group(
            "Kids",
            user,
            AccessGroupRules {
                libraries: vec![allow_library(kids)],
                ..Default::default()
            },
        )
        .await;

        let filter = f.filter(user).await;
        assert!(filter.is_library_visible(kids));
        assert!(!filter.is_library_visible(manga));
    }

    #[tokio::test]
    async fn no_rules_shows_every_library() {
        let f = Fixture::new().await;
        let user = make_user(f.conn(), "reader").await;
        let lib = f.library("Any").await;
        assert!(f.filter(user).await.is_library_visible(lib));
    }

    /// Maintainers curate content (metadata, scans, covers), so they see all of it.
    #[tokio::test]
    async fn maintainers_bypass_every_rule() {
        let f = Fixture::new().await;
        let maintainer = make_with_role(f.conn(), "curator", "maintainer").await;
        let lib = f.library("Adult").await;
        let adult = f.series(lib, "Adult", &["Adult"]).await;
        f.group(
            "Locked",
            maintainer,
            AccessGroupRules {
                genres: vec![NameRule::new("Adult", AccessMode::Deny)],
                libraries: vec![deny_library(lib)],
                ..Default::default()
            },
        )
        .await;

        let filter = f.filter(maintainer).await;
        assert!(!filter.has_restrictions);
        assert!(filter.is_series_visible(adult));
    }

    #[tokio::test]
    async fn admins_bypass_every_rule() {
        let f = Fixture::new().await;
        let admin = make_admin(f.conn(), "admin").await;
        let lib = f.library("Adult").await;
        let adult = f.series(lib, "Adult", &["Adult"]).await;
        let tag = SharingTagRepository::create(f.conn(), "18+", None)
            .await
            .unwrap();
        SharingTagRepository::add_tag_to_series(f.conn(), adult, tag.id)
            .await
            .unwrap();
        SharingTagRepository::set_user_grant(f.conn(), admin, tag.id, AccessMode::Deny)
            .await
            .unwrap();
        f.group(
            "Locked",
            admin,
            AccessGroupRules {
                genres: vec![NameRule::new("Adult", AccessMode::Deny)],
                libraries: vec![deny_library(lib)],
                ..Default::default()
            },
        )
        .await;

        let filter = f.filter(admin).await;
        assert!(!filter.has_restrictions);
        assert!(filter.is_series_visible(adult));
    }

    #[tokio::test]
    async fn genre_deny_hides_without_starting_a_whitelist() {
        let f = Fixture::new().await;
        let user = make_user(f.conn(), "kid").await;
        let lib = f.library("Manga").await;
        let adult = f.series(lib, "Adult", &["Adult", "Action"]).await;
        let action = f.series(lib, "Action", &["Action"]).await;
        let untagged = f.series(lib, "Untagged", &[]).await;
        f.group(
            "No adult",
            user,
            AccessGroupRules {
                genres: vec![NameRule::new("adult", AccessMode::Deny)],
                ..Default::default()
            },
        )
        .await;

        let filter = f.filter(user).await;
        assert!(
            !filter.whitelist_mode,
            "a deny-only group must not whitelist"
        );
        assert!(
            !filter.is_series_visible(adult),
            "matched case-insensitively"
        );
        assert!(filter.is_series_visible(action));
        assert!(filter.is_series_visible(untagged));
    }

    #[tokio::test]
    async fn tag_deny_matches_by_name() {
        let f = Fixture::new().await;
        let user = make_user(f.conn(), "kid").await;
        let lib = f.library("Manga").await;
        let gory = f.series(lib, "Gory", &[]).await;
        let clean = f.series(lib, "Clean", &[]).await;
        TagRepository::add_tag_to_series(f.conn(), gory, "Gore")
            .await
            .unwrap();
        f.group(
            "No gore",
            user,
            AccessGroupRules {
                tags: vec![NameRule::new("GORE", AccessMode::Deny)],
                ..Default::default()
            },
        )
        .await;

        let filter = f.filter(user).await;
        assert!(!filter.is_series_visible(gory));
        assert!(filter.is_series_visible(clean));
    }

    #[tokio::test]
    async fn library_allow_whitelists_the_whole_library() {
        let f = Fixture::new().await;
        let user = make_user(f.conn(), "kid").await;
        let kids_lib = f.library("Kids").await;
        let manga_lib = f.library("Manga").await;
        let in_kids = f.series(kids_lib, "Bluey", &[]).await;
        let in_manga = f.series(manga_lib, "Berserk", &[]).await;
        f.group(
            "Kids",
            user,
            AccessGroupRules {
                libraries: vec![allow_library(kids_lib)],
                ..Default::default()
            },
        )
        .await;

        let filter = f.filter(user).await;
        assert!(filter.whitelist_mode);
        assert!(
            filter.is_series_visible(in_kids),
            "no tags needed inside the library"
        );
        assert!(!filter.is_series_visible(in_manga));
    }

    #[tokio::test]
    async fn library_deny_hides_the_library() {
        let f = Fixture::new().await;
        let user = make_user(f.conn(), "kid").await;
        let adult_lib = f.library("Adult").await;
        let manga_lib = f.library("Manga").await;
        let in_adult = f.series(adult_lib, "Adult", &[]).await;
        let in_manga = f.series(manga_lib, "Manga", &[]).await;
        f.group(
            "Hide adult library",
            user,
            AccessGroupRules {
                libraries: vec![deny_library(adult_lib)],
                ..Default::default()
            },
        )
        .await;

        let filter = f.filter(user).await;
        assert!(!filter.is_series_visible(in_adult));
        assert!(filter.is_series_visible(in_manga));
    }

    /// Different rule types in one group narrow each other.
    #[tokio::test]
    async fn types_within_a_group_combine_with_and() {
        let f = Fixture::new().await;
        let user = make_user(f.conn(), "kid").await;
        let manga_lib = f.library("Manga").await;
        let comics_lib = f.library("Comics").await;
        let kids_manga = f.series(manga_lib, "Kids manga", &["Kids"]).await;
        let other_manga = f.series(manga_lib, "Other manga", &["Action"]).await;
        let kids_comic = f.series(comics_lib, "Kids comic", &["Kids"]).await;
        f.group(
            "Kids manga",
            user,
            AccessGroupRules {
                genres: vec![NameRule::new("Kids", AccessMode::Allow)],
                libraries: vec![allow_library(manga_lib)],
                ..Default::default()
            },
        )
        .await;

        let filter = f.filter(user).await;
        assert!(filter.is_series_visible(kids_manga));
        assert!(!filter.is_series_visible(other_manga));
        assert!(!filter.is_series_visible(kids_comic));
    }

    /// Each group grants its own scope; a user in two groups gets both.
    #[tokio::test]
    async fn groups_combine_with_or() {
        let f = Fixture::new().await;
        let user = make_user(f.conn(), "reader").await;
        let manga_lib = f.library("Manga").await;
        let comics_lib = f.library("Comics").await;
        let action_manga = f.series(manga_lib, "Action manga", &["Action"]).await;
        let kids_comic = f.series(comics_lib, "Kids comic", &["Kids"]).await;
        let other_comic = f.series(comics_lib, "Other comic", &["Action"]).await;
        f.group(
            "Manga",
            user,
            AccessGroupRules {
                libraries: vec![allow_library(manga_lib)],
                ..Default::default()
            },
        )
        .await;
        f.group(
            "Kids comics",
            user,
            AccessGroupRules {
                genres: vec![NameRule::new("Kids", AccessMode::Allow)],
                libraries: vec![allow_library(comics_lib)],
                ..Default::default()
            },
        )
        .await;

        let filter = f.filter(user).await;
        assert!(
            filter.is_series_visible(action_manga),
            "the genre rule of one group must not narrow another group"
        );
        assert!(filter.is_series_visible(kids_comic));
        assert!(!filter.is_series_visible(other_comic));
    }

    /// A deny written in one group applies to what another group grants.
    #[tokio::test]
    async fn denies_apply_across_groups() {
        let f = Fixture::new().await;
        let user = make_user(f.conn(), "reader").await;
        let manga_lib = f.library("Manga").await;
        let comics_lib = f.library("Comics").await;
        let adult_manga = f.series(manga_lib, "Adult manga", &["Adult"]).await;
        let manga = f.series(manga_lib, "Manga", &["Action"]).await;
        let comic = f.series(comics_lib, "Comic", &[]).await;
        f.group(
            "Manga",
            user,
            AccessGroupRules {
                libraries: vec![allow_library(manga_lib)],
                ..Default::default()
            },
        )
        .await;
        f.group(
            "Comics, no adult",
            user,
            AccessGroupRules {
                genres: vec![NameRule::new("Adult", AccessMode::Deny)],
                libraries: vec![allow_library(comics_lib)],
                ..Default::default()
            },
        )
        .await;

        let filter = f.filter(user).await;
        assert!(!filter.is_series_visible(adult_manga));
        assert!(filter.is_series_visible(manga));
        assert!(filter.is_series_visible(comic));
    }

    #[tokio::test]
    async fn genre_match_all_requires_every_allowed_genre() {
        let f = Fixture::new().await;
        let user = make_user(f.conn(), "kid").await;
        let lib = f.library("Manga").await;
        let both = f.series(lib, "Both", &["Kids", "Comedy"]).await;
        let kids_only = f.series(lib, "Kids only", &["Kids"]).await;
        f.group(
            "Kids comedy",
            user,
            AccessGroupRules {
                genre_match: RuleMatch::All,
                genres: vec![
                    NameRule::new("Kids", AccessMode::Allow),
                    NameRule::new("Comedy", AccessMode::Allow),
                ],
                ..Default::default()
            },
        )
        .await;

        let filter = f.filter(user).await;
        assert!(filter.is_series_visible(both));
        assert!(!filter.is_series_visible(kids_only));
    }

    #[tokio::test]
    async fn genre_match_any_is_the_default() {
        let f = Fixture::new().await;
        let user = make_user(f.conn(), "kid").await;
        let lib = f.library("Manga").await;
        let kids = f.series(lib, "Kids", &["Kids"]).await;
        let comedy = f.series(lib, "Comedy", &["Comedy"]).await;
        let action = f.series(lib, "Action", &["Action"]).await;
        f.group(
            "Kids or comedy",
            user,
            AccessGroupRules {
                genres: vec![
                    NameRule::new("Kids", AccessMode::Allow),
                    NameRule::new("Comedy", AccessMode::Allow),
                ],
                ..Default::default()
            },
        )
        .await;

        let filter = f.filter(user).await;
        assert!(filter.is_series_visible(kids));
        assert!(filter.is_series_visible(comedy));
        assert!(!filter.is_series_visible(action));
    }

    #[tokio::test]
    async fn sharing_tag_match_all_requires_every_allowed_tag() {
        let f = Fixture::new().await;
        let user = make_user(f.conn(), "kid").await;
        let lib = f.library("Manga").await;
        let both = f.series(lib, "Both", &[]).await;
        let one = f.series(lib, "One", &[]).await;
        let a = SharingTagRepository::create(f.conn(), "a", None)
            .await
            .unwrap();
        let b = SharingTagRepository::create(f.conn(), "b", None)
            .await
            .unwrap();
        for (series, tag) in [(both, a.id), (both, b.id), (one, a.id)] {
            SharingTagRepository::add_tag_to_series(f.conn(), series, tag)
                .await
                .unwrap();
        }
        f.group(
            "A and B",
            user,
            AccessGroupRules {
                sharing_tag_match: RuleMatch::All,
                sharing_tags: vec![
                    SharingTagRule {
                        sharing_tag_id: a.id,
                        access_mode: AccessMode::Allow,
                    },
                    SharingTagRule {
                        sharing_tag_id: b.id,
                        access_mode: AccessMode::Allow,
                    },
                ],
                ..Default::default()
            },
        )
        .await;

        let filter = f.filter(user).await;
        assert!(filter.is_series_visible(both));
        assert!(!filter.is_series_visible(one));
    }

    /// Personal sharing-tag allows act as one more group.
    #[tokio::test]
    async fn personal_allows_add_to_group_allows() {
        let f = Fixture::new().await;
        let user = make_user(f.conn(), "kid").await;
        let kids_lib = f.library("Kids").await;
        let manga_lib = f.library("Manga").await;
        let in_kids = f.series(kids_lib, "Bluey", &[]).await;
        let shared_manga = f.series(manga_lib, "Shared", &[]).await;
        let other_manga = f.series(manga_lib, "Other", &[]).await;
        let tag = SharingTagRepository::create(f.conn(), "shared", None)
            .await
            .unwrap();
        SharingTagRepository::add_tag_to_series(f.conn(), shared_manga, tag.id)
            .await
            .unwrap();
        SharingTagRepository::set_user_grant(f.conn(), user, tag.id, AccessMode::Allow)
            .await
            .unwrap();
        f.group(
            "Kids",
            user,
            AccessGroupRules {
                libraries: vec![allow_library(kids_lib)],
                ..Default::default()
            },
        )
        .await;

        let filter = f.filter(user).await;
        assert!(filter.is_series_visible(in_kids));
        assert!(filter.is_series_visible(shared_manga));
        assert!(!filter.is_series_visible(other_manga));
    }

    /// A genre removed by cleanup and recreated under a new id is still denied.
    #[tokio::test]
    async fn genre_deny_survives_genre_recreation() {
        let f = Fixture::new().await;
        let user = make_user(f.conn(), "kid").await;
        let lib = f.library("Manga").await;
        f.group(
            "No adult",
            user,
            AccessGroupRules {
                genres: vec![NameRule::new("Adult", AccessMode::Deny)],
                ..Default::default()
            },
        )
        .await;
        let first = f.series(lib, "First", &["Adult"]).await;
        let first_genre = GenreRepository::get_by_name(f.conn(), "Adult")
            .await
            .unwrap()
            .unwrap();
        GenreRepository::remove_genre_from_series(f.conn(), first, first_genre.id)
            .await
            .unwrap();
        GenreRepository::delete_unused(f.conn()).await.unwrap();

        let second = f.series(lib, "Second", &["Adult"]).await;
        let second_genre = GenreRepository::get_by_name(f.conn(), "Adult")
            .await
            .unwrap()
            .unwrap();
        assert_ne!(first_genre.id, second_genre.id, "genre was recreated");

        let filter = f.filter(user).await;
        assert!(!filter.is_series_visible(second));
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
