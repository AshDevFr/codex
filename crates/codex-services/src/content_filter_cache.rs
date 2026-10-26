//! Per-user cache of built content filters.
//!
//! Building a [`ContentFilter`] takes several queries, and readers hit it on
//! every page image during webtoon prefetch. Each process keeps the built
//! filter per user for a short time instead.
//!
//! An entry is used only while both hold:
//! - it is younger than the TTL (30 seconds by default), and
//! - it was built under the current [`access_control_version`], which every
//!   repository write that changes visibility bumps.
//!
//! So a rule, membership, grant or role change in this process applies on the
//! next request, another process catches up within the TTL, and a genre or tag
//! change from a scan applies within the TTL. Entries can be evicted at any
//! time (size cap, expiry, restart): a miss just rebuilds from the database.
//!
//! [`access_control_version`]: codex_db::access_control_version

use crate::content_filter::ContentFilter;
use codex_db::access_control_version;
use lru::LruCache;
use sea_orm::DatabaseConnection;
use std::num::NonZeroUsize;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};
use uuid::Uuid;

/// How long a built filter is reused.
pub const DEFAULT_TTL: Duration = Duration::from_secs(30);

/// How many users' filters are kept at once.
pub const DEFAULT_CAPACITY: usize = 1024;

struct Entry {
    version: u64,
    built_at: Instant,
    filter: Arc<ContentFilter>,
}

pub struct ContentFilterCache {
    ttl: Duration,
    entries: Mutex<LruCache<Uuid, Entry>>,
}

impl ContentFilterCache {
    pub fn new(capacity: usize, ttl: Duration) -> Self {
        let capacity = NonZeroUsize::new(capacity).unwrap_or(NonZeroUsize::MIN);
        Self {
            ttl,
            entries: Mutex::new(LruCache::new(capacity)),
        }
    }

    /// The process-wide cache.
    pub fn global() -> &'static Self {
        static CACHE: OnceLock<ContentFilterCache> = OnceLock::new();
        CACHE.get_or_init(|| Self::new(DEFAULT_CAPACITY, DEFAULT_TTL))
    }

    /// The user's filter, from the cache when it is still valid.
    pub async fn get(
        &self,
        db: &DatabaseConnection,
        user_id: Uuid,
    ) -> anyhow::Result<Arc<ContentFilter>> {
        // Read the version before building: a change that lands while the
        // filter is being built leaves it tagged with the older version, so
        // the next request rebuilds.
        let version = access_control_version::current();
        if let Some(filter) = self.lookup(user_id, version, Instant::now()) {
            return Ok(filter);
        }
        let filter = Arc::new(ContentFilter::for_user(db, user_id).await?);
        self.store(user_id, version, Instant::now(), filter.clone());
        Ok(filter)
    }

    fn lookup(&self, user_id: Uuid, version: u64, now: Instant) -> Option<Arc<ContentFilter>> {
        let mut entries = self.entries.lock().unwrap_or_else(|e| e.into_inner());
        let valid = entries.get(&user_id).is_some_and(|entry| {
            entry.version == version && now.duration_since(entry.built_at) < self.ttl
        });
        if valid {
            entries.get(&user_id).map(|e| e.filter.clone())
        } else {
            entries.pop(&user_id);
            None
        }
    }

    fn store(&self, user_id: Uuid, version: u64, built_at: Instant, filter: Arc<ContentFilter>) {
        let mut entries = self.entries.lock().unwrap_or_else(|e| e.into_inner());
        entries.put(
            user_id,
            Entry {
                version,
                built_at,
                filter,
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn filter() -> Arc<ContentFilter> {
        Arc::new(ContentFilter::allow_all())
    }

    #[test]
    fn a_fresh_entry_is_reused() {
        let cache = ContentFilterCache::new(8, Duration::from_secs(30));
        let user = Uuid::new_v4();
        let now = Instant::now();
        let built = filter();
        cache.store(user, 7, now, built.clone());

        let hit = cache
            .lookup(user, 7, now + Duration::from_secs(29))
            .unwrap();
        assert!(Arc::ptr_eq(&hit, &built));
    }

    #[test]
    fn an_expired_entry_is_dropped() {
        let cache = ContentFilterCache::new(8, Duration::from_secs(30));
        let user = Uuid::new_v4();
        let now = Instant::now();
        cache.store(user, 7, now, filter());

        assert!(
            cache
                .lookup(user, 7, now + Duration::from_secs(30))
                .is_none()
        );
        // Dropped, not just skipped
        assert!(cache.lookup(user, 7, now).is_none());
    }

    #[test]
    fn an_entry_from_an_older_version_is_dropped() {
        let cache = ContentFilterCache::new(8, Duration::from_secs(30));
        let user = Uuid::new_v4();
        let now = Instant::now();
        cache.store(user, 7, now, filter());

        assert!(cache.lookup(user, 8, now).is_none());
    }

    #[test]
    fn capacity_evicts_the_least_recently_used_user() {
        let cache = ContentFilterCache::new(2, Duration::from_secs(30));
        let (a, b, c) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
        let now = Instant::now();
        cache.store(a, 1, now, filter());
        cache.store(b, 1, now, filter());
        cache.lookup(a, 1, now).unwrap();
        cache.store(c, 1, now, filter());

        assert!(cache.lookup(a, 1, now).is_some());
        assert!(cache.lookup(b, 1, now).is_none());
        assert!(cache.lookup(c, 1, now).is_some());
    }
}

#[cfg(test)]
mod db_tests {
    use super::*;
    use codex_db::ScanningStrategy;
    use codex_db::entities::user_access_groups::MembershipSource;
    use codex_db::entities::user_sharing_tags::AccessMode;
    use codex_db::repositories::{
        AccessGroupRepository, AccessGroupRules, AccessGroupRulesRepository, LibraryRepository,
        NameRule, SeriesRepository, SharingTagRepository, UserRepository,
    };
    use codex_db::test_helpers::create_test_db;

    async fn make_user(db: &DatabaseConnection) -> Uuid {
        let now = chrono::Utc::now();
        let model = codex_db::entities::users::Model {
            id: Uuid::new_v4(),
            username: format!("u-{}", Uuid::new_v4()),
            email: format!("{}@test.com", Uuid::new_v4()),
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

    /// A long TTL, so only invalidation can explain a rebuild.
    fn cache() -> ContentFilterCache {
        ContentFilterCache::new(8, Duration::from_secs(3600))
    }

    #[tokio::test]
    async fn a_new_personal_deny_applies_on_the_next_request() {
        let (db, _tmp) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let cache = cache();
        let user = make_user(conn).await;
        let lib = LibraryRepository::create(conn, "L", "/tmp/l", ScanningStrategy::Default)
            .await
            .unwrap();
        let series = SeriesRepository::create(conn, lib.id, "S", None)
            .await
            .unwrap();
        let tag = SharingTagRepository::create(conn, "t", None).await.unwrap();
        SharingTagRepository::add_tag_to_series(conn, series.id, tag.id)
            .await
            .unwrap();

        assert!(
            cache
                .get(conn, user)
                .await
                .unwrap()
                .is_series_visible(series.id)
        );

        SharingTagRepository::set_user_grant(conn, user, tag.id, AccessMode::Deny)
            .await
            .unwrap();

        assert!(
            !cache
                .get(conn, user)
                .await
                .unwrap()
                .is_series_visible(series.id)
        );
    }

    #[tokio::test]
    async fn rule_and_membership_changes_apply_on_the_next_request() {
        let (db, _tmp) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let cache = cache();
        let user = make_user(conn).await;
        let lib = LibraryRepository::create(conn, "L", "/tmp/l", ScanningStrategy::Default)
            .await
            .unwrap();
        let series = SeriesRepository::create(conn, lib.id, "S", None)
            .await
            .unwrap();
        codex_db::repositories::GenreRepository::add_genre_to_series(conn, series.id, "Adult")
            .await
            .unwrap();
        let group = AccessGroupRepository::create(conn, "Kids", None)
            .await
            .unwrap();
        AccessGroupRepository::add_member(conn, group.id, user, MembershipSource::Manual)
            .await
            .unwrap();
        assert!(
            cache
                .get(conn, user)
                .await
                .unwrap()
                .is_series_visible(series.id)
        );

        AccessGroupRulesRepository::replace(
            conn,
            group.id,
            &AccessGroupRules {
                genres: vec![NameRule::new("Adult", AccessMode::Deny)],
                ..Default::default()
            },
        )
        .await
        .unwrap();
        assert!(
            !cache
                .get(conn, user)
                .await
                .unwrap()
                .is_series_visible(series.id)
        );

        AccessGroupRepository::remove_member(conn, group.id, user)
            .await
            .unwrap();
        assert!(
            cache
                .get(conn, user)
                .await
                .unwrap()
                .is_series_visible(series.id)
        );
    }

    #[tokio::test]
    async fn a_role_change_applies_on_the_next_request() {
        let (db, _tmp) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let cache = cache();
        let user = make_user(conn).await;
        let lib = LibraryRepository::create(conn, "L", "/tmp/l", ScanningStrategy::Default)
            .await
            .unwrap();
        let series = SeriesRepository::create(conn, lib.id, "S", None)
            .await
            .unwrap();
        let tag = SharingTagRepository::create(conn, "t", None).await.unwrap();
        SharingTagRepository::add_tag_to_series(conn, series.id, tag.id)
            .await
            .unwrap();
        SharingTagRepository::set_user_grant(conn, user, tag.id, AccessMode::Deny)
            .await
            .unwrap();
        assert!(
            !cache
                .get(conn, user)
                .await
                .unwrap()
                .is_series_visible(series.id)
        );

        let mut model = UserRepository::get_by_id(conn, user)
            .await
            .unwrap()
            .unwrap();
        model.role = "admin".to_string();
        UserRepository::update(conn, &model).await.unwrap();

        assert!(
            cache
                .get(conn, user)
                .await
                .unwrap()
                .is_series_visible(series.id)
        );
    }
}
