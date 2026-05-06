//! Repository for the `release_ledger` table.
//!
//! Sources write announcements; the inbox UI reads them. Two dedup keys live
//! at the schema level (`(source_id, external_release_id)` unique;
//! `info_hash` unique-where-non-null), so the repository's `record` method
//! is idempotent on either: callers don't need to pre-check.

#![allow(dead_code)]

use anyhow::Result;
use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, Order, PaginatorTrait,
    QueryFilter, QueryOrder, QuerySelect, Set, sea_query::NullOrdering,
};
use uuid::Uuid;

use crate::db::entities::release_ledger::{
    self, Entity as ReleaseLedger, Model as ReleaseLedgerRow, state,
};

/// New-row payload. Keys plus payload fields.
#[derive(Debug, Clone)]
pub struct NewReleaseEntry {
    pub series_id: Uuid,
    pub source_id: Uuid,
    pub external_release_id: String,
    pub info_hash: Option<String>,
    pub chapter: Option<f64>,
    pub volume: Option<i32>,
    pub language: Option<String>,
    pub format_hints: Option<serde_json::Value>,
    pub group_or_uploader: Option<String>,
    pub payload_url: String,
    pub media_url: Option<String>,
    pub media_url_kind: Option<String>,
    pub confidence: f64,
    pub metadata: Option<serde_json::Value>,
    pub observed_at: chrono::DateTime<Utc>,
}

/// Outcome of a `record` call.
#[derive(Debug, Clone, PartialEq)]
pub struct RecordOutcome {
    pub row: ReleaseLedgerRow,
    /// `true` if this call deduped onto an existing row, `false` if it inserted.
    pub deduped: bool,
}

/// Filters for the inbox query.
#[derive(Debug, Default, Clone)]
pub struct LedgerInboxFilter {
    /// Only rows in this state. `None` means "all states" (no filter).
    /// Note: `list_inbox` historically defaulted to `announced` when `None`;
    /// callers that want the "all states" view must opt in explicitly via
    /// the [`LedgerInboxFilter::all_states`] flag.
    pub state: Option<String>,
    /// When `true`, no state filter is applied even if `state` is `None`.
    /// Used by the inbox UI's "All" state option.
    pub all_states: bool,
    pub series_id: Option<Uuid>,
    pub source_id: Option<Uuid>,
    pub language: Option<String>,
    /// Restrict to series belonging to this library.
    pub library_id: Option<Uuid>,
}

/// Per-series facet entry.
#[derive(Debug, Clone, PartialEq)]
pub struct SeriesFacet {
    pub series_id: Uuid,
    pub library_id: Uuid,
    pub count: u64,
}

/// Per-library facet entry.
#[derive(Debug, Clone, PartialEq)]
pub struct LibraryFacet {
    pub library_id: Uuid,
    pub count: u64,
}

/// Per-language facet entry.
#[derive(Debug, Clone, PartialEq)]
pub struct LanguageFacet {
    pub language: String,
    pub count: u64,
}

pub struct ReleaseLedgerRepository;

impl ReleaseLedgerRepository {
    pub async fn get_by_id(db: &DatabaseConnection, id: Uuid) -> Result<Option<ReleaseLedgerRow>> {
        Ok(ReleaseLedger::find_by_id(id).one(db).await?)
    }

    /// Idempotent insert. Dedup priority:
    /// 1. `(source_id, external_release_id)` - cheapest, always present.
    /// 2. `info_hash` - cross-source dedup, only when present.
    ///
    /// Returns the existing row when either key matches, otherwise inserts.
    pub async fn record(db: &DatabaseConnection, entry: NewReleaseEntry) -> Result<RecordOutcome> {
        if entry.confidence.is_nan() {
            anyhow::bail!("confidence cannot be NaN");
        }
        if entry.payload_url.trim().is_empty() {
            anyhow::bail!("payload_url cannot be empty");
        }
        if entry.external_release_id.trim().is_empty() {
            anyhow::bail!("external_release_id cannot be empty");
        }

        // 1. Primary dedup: (source_id, external_release_id).
        if let Some(existing) = ReleaseLedger::find()
            .filter(release_ledger::Column::SourceId.eq(entry.source_id))
            .filter(release_ledger::Column::ExternalReleaseId.eq(&entry.external_release_id))
            .one(db)
            .await?
        {
            return Ok(RecordOutcome {
                row: existing,
                deduped: true,
            });
        }

        // 2. Secondary dedup: info_hash (cross-source).
        if let Some(ref hash) = entry.info_hash
            && let Some(existing) = ReleaseLedger::find()
                .filter(release_ledger::Column::InfoHash.eq(hash))
                .one(db)
                .await?
        {
            return Ok(RecordOutcome {
                row: existing,
                deduped: true,
            });
        }

        let active = release_ledger::ActiveModel {
            id: Set(Uuid::new_v4()),
            series_id: Set(entry.series_id),
            source_id: Set(entry.source_id),
            external_release_id: Set(entry.external_release_id),
            info_hash: Set(entry.info_hash),
            chapter: Set(entry.chapter),
            volume: Set(entry.volume),
            language: Set(entry.language),
            format_hints: Set(entry.format_hints),
            group_or_uploader: Set(entry.group_or_uploader),
            payload_url: Set(entry.payload_url),
            media_url: Set(entry.media_url),
            media_url_kind: Set(entry.media_url_kind),
            confidence: Set(entry.confidence),
            state: Set(state::ANNOUNCED.to_string()),
            metadata: Set(entry.metadata),
            observed_at: Set(entry.observed_at),
            created_at: Set(Utc::now()),
        };
        let inserted = active.insert(db).await?;
        Ok(RecordOutcome {
            row: inserted,
            deduped: false,
        })
    }

    /// Per-series ledger view: highest volume/chapter first, then most recent
    /// observation as a tie-breaker. Matches the inbox ordering so the series
    /// detail panel reads the same way as the cross-series list.
    pub async fn list_for_series(
        db: &DatabaseConnection,
        series_id: Uuid,
        state_filter: Option<&str>,
        limit: u64,
        offset: u64,
    ) -> Result<Vec<ReleaseLedgerRow>> {
        let mut query = ReleaseLedger::find()
            .filter(release_ledger::Column::SeriesId.eq(series_id))
            .order_by_with_nulls(
                release_ledger::Column::Volume,
                Order::Desc,
                NullOrdering::Last,
            )
            .order_by_with_nulls(
                release_ledger::Column::Chapter,
                Order::Desc,
                NullOrdering::Last,
            )
            .order_by_desc(release_ledger::Column::ObservedAt)
            .order_by_asc(release_ledger::Column::Id);
        if let Some(s) = state_filter {
            query = query.filter(release_ledger::Column::State.eq(s));
        }
        if limit > 0 {
            query = query.limit(limit);
        }
        if offset > 0 {
            query = query.offset(offset);
        }
        Ok(query.all(db).await?)
    }

    /// Inbox view across all series, with filters.
    ///
    /// Sort order: group all rows of a series together (highest volume/chapter
    /// on top), then break ties between series by the most recent observation.
    /// Grouping by series first matches how users read the inbox: they want
    /// every chapter of a series listed contiguously and descending, even when
    /// rows come from multiple poll batches with different `observed_at`s.
    ///
    /// Inner-joins `series` so the cross-series order is by `series.name`
    /// (alphabetical) rather than by `series_id` (a meaningless UUID order).
    pub async fn list_inbox(
        db: &DatabaseConnection,
        filter: LedgerInboxFilter,
        limit: u64,
        offset: u64,
    ) -> Result<Vec<ReleaseLedgerRow>> {
        use sea_orm::{JoinType, RelationTrait};
        let mut query = ReleaseLedger::find()
            .join(JoinType::InnerJoin, release_ledger::Relation::Series.def())
            .order_by_asc(crate::db::entities::series::Column::Name)
            .order_by_asc(release_ledger::Column::SeriesId)
            .order_by_with_nulls(
                release_ledger::Column::Volume,
                Order::Desc,
                NullOrdering::Last,
            )
            .order_by_with_nulls(
                release_ledger::Column::Chapter,
                Order::Desc,
                NullOrdering::Last,
            )
            .order_by_desc(release_ledger::Column::ObservedAt)
            .order_by_asc(release_ledger::Column::Id);
        // `series_already_joined: true` so apply_inbox_filter doesn't add
        // a duplicate join when `library_id` is present in the filter.
        query = apply_inbox_filter(query, &filter, true);
        if limit > 0 {
            query = query.limit(limit);
        }
        if offset > 0 {
            query = query.offset(offset);
        }
        Ok(query.all(db).await?)
    }

    /// Total count for the inbox view (paginator support).
    pub async fn count_inbox(db: &DatabaseConnection, filter: LedgerInboxFilter) -> Result<u64> {
        let mut query = ReleaseLedger::find();
        query = apply_inbox_filter(query, &filter, false);
        Ok(query.count(db).await?)
    }

    /// List the distinct series present in the inbox under a given filter,
    /// each with the row count. Used by the inbox UI to populate the series
    /// facet dropdown. Joins the `series` table to surface `library_id` so
    /// the frontend can group by library.
    pub async fn list_series_facets(
        db: &DatabaseConnection,
        filter: LedgerInboxFilter,
    ) -> Result<Vec<SeriesFacet>> {
        // We join via series.id to get library_id, then count rows. Excluding
        // `series_id` from the filter is the caller's job; the facet itself
        // _is_ the series dimension.
        use sea_orm::{FromQueryResult, JoinType, RelationTrait};
        #[derive(Debug, FromQueryResult)]
        struct Row {
            series_id: Uuid,
            library_id: Uuid,
            count: i64,
        }
        let mut query = ReleaseLedger::find()
            .select_only()
            .column(release_ledger::Column::SeriesId)
            .column(crate::db::entities::series::Column::LibraryId)
            .column_as(release_ledger::Column::Id.count(), "count")
            .join(JoinType::InnerJoin, release_ledger::Relation::Series.def())
            .group_by(release_ledger::Column::SeriesId)
            .group_by(crate::db::entities::series::Column::LibraryId);
        query = apply_inbox_filter(query, &filter, true);
        let rows = query.into_model::<Row>().all(db).await?;
        Ok(rows
            .into_iter()
            .map(|r| SeriesFacet {
                series_id: r.series_id,
                library_id: r.library_id,
                count: r.count.max(0) as u64,
            })
            .collect())
    }

    /// List the distinct libraries present in the inbox under a given filter,
    /// each with the row count.
    pub async fn list_library_facets(
        db: &DatabaseConnection,
        filter: LedgerInboxFilter,
    ) -> Result<Vec<LibraryFacet>> {
        use sea_orm::{FromQueryResult, JoinType, RelationTrait};
        #[derive(Debug, FromQueryResult)]
        struct Row {
            library_id: Uuid,
            count: i64,
        }
        let mut query = ReleaseLedger::find()
            .select_only()
            .column(crate::db::entities::series::Column::LibraryId)
            .column_as(release_ledger::Column::Id.count(), "count")
            .join(JoinType::InnerJoin, release_ledger::Relation::Series.def())
            .group_by(crate::db::entities::series::Column::LibraryId);
        query = apply_inbox_filter(query, &filter, true);
        let rows = query.into_model::<Row>().all(db).await?;
        Ok(rows
            .into_iter()
            .map(|r| LibraryFacet {
                library_id: r.library_id,
                count: r.count.max(0) as u64,
            })
            .collect())
    }

    /// List the distinct languages present in the inbox under a given filter,
    /// each with the row count. Skips rows with NULL/empty language.
    pub async fn list_language_facets(
        db: &DatabaseConnection,
        filter: LedgerInboxFilter,
    ) -> Result<Vec<LanguageFacet>> {
        use sea_orm::FromQueryResult;
        #[derive(Debug, FromQueryResult)]
        struct Row {
            language: Option<String>,
            count: i64,
        }
        let mut query = ReleaseLedger::find()
            .select_only()
            .column(release_ledger::Column::Language)
            .column_as(release_ledger::Column::Id.count(), "count")
            .filter(release_ledger::Column::Language.is_not_null())
            .group_by(release_ledger::Column::Language);
        query = apply_inbox_filter(query, &filter, false);
        let rows = query.into_model::<Row>().all(db).await?;
        Ok(rows
            .into_iter()
            .filter_map(|r| {
                let lang = r.language?;
                if lang.is_empty() {
                    return None;
                }
                Some(LanguageFacet {
                    language: lang,
                    count: r.count.max(0) as u64,
                })
            })
            .collect())
    }

    /// Set the state of a ledger row. Validates the state string.
    pub async fn set_state(
        db: &DatabaseConnection,
        id: Uuid,
        new_state: &str,
    ) -> Result<ReleaseLedgerRow> {
        if !state::is_valid(new_state) {
            anyhow::bail!("invalid state: {}", new_state);
        }
        let existing = ReleaseLedger::find_by_id(id)
            .one(db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("ledger row {} not found", id))?;
        let mut active: release_ledger::ActiveModel = existing.into();
        active.state = Set(new_state.to_string());
        Ok(active.update(db).await?)
    }

    /// Delete a ledger row by id. Used by admin tooling.
    pub async fn delete(db: &DatabaseConnection, id: Uuid) -> Result<bool> {
        let result = ReleaseLedger::delete_by_id(id).exec(db).await?;
        Ok(result.rows_affected > 0)
    }

    /// Delete all ledger rows for a source. Returns the number of rows
    /// removed. Used by the source-reset admin endpoint to give testers a
    /// clean slate without dropping the source itself.
    pub async fn delete_by_source(db: &DatabaseConnection, source_id: Uuid) -> Result<u64> {
        let result = ReleaseLedger::delete_many()
            .filter(release_ledger::Column::SourceId.eq(source_id))
            .exec(db)
            .await?;
        Ok(result.rows_affected)
    }

    /// Fetch rows by id list, in unspecified order.
    pub async fn find_by_ids(
        db: &DatabaseConnection,
        ids: &[Uuid],
    ) -> Result<Vec<ReleaseLedgerRow>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        Ok(ReleaseLedger::find()
            .filter(release_ledger::Column::Id.is_in(ids.to_vec()))
            .all(db)
            .await?)
    }

    /// Look up the distinct `source_id`s touched by a set of ledger rows.
    /// Used by the inbox's per-row "delete" so we can clear each affected
    /// source's etag in the same transaction (forcing the next poll to
    /// bypass `If-None-Match` and re-announce the deleted rows).
    pub async fn distinct_sources_for_ids(
        db: &DatabaseConnection,
        ids: &[Uuid],
    ) -> Result<Vec<Uuid>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let rows = ReleaseLedger::find()
            .filter(release_ledger::Column::Id.is_in(ids.to_vec()))
            .all(db)
            .await?;
        let mut sources: Vec<Uuid> = rows.into_iter().map(|r| r.source_id).collect();
        sources.sort_unstable();
        sources.dedup();
        Ok(sources)
    }

    /// Bulk-delete ledger rows by id. Returns the number of rows removed.
    pub async fn delete_many(db: &DatabaseConnection, ids: &[Uuid]) -> Result<u64> {
        if ids.is_empty() {
            return Ok(0);
        }
        let result = ReleaseLedger::delete_many()
            .filter(release_ledger::Column::Id.is_in(ids.to_vec()))
            .exec(db)
            .await?;
        Ok(result.rows_affected)
    }

    /// Bulk-update state on ledger rows by id. Returns the number of rows
    /// updated.
    pub async fn set_state_many(
        db: &DatabaseConnection,
        ids: &[Uuid],
        new_state: &str,
    ) -> Result<u64> {
        if !state::is_valid(new_state) {
            anyhow::bail!("invalid state: {}", new_state);
        }
        if ids.is_empty() {
            return Ok(0);
        }
        let result = ReleaseLedger::update_many()
            .col_expr(
                release_ledger::Column::State,
                sea_orm::sea_query::Expr::value(new_state.to_string()),
            )
            .filter(release_ledger::Column::Id.is_in(ids.to_vec()))
            .exec(db)
            .await?;
        Ok(result.rows_affected)
    }
}

/// Apply the inbox filter to a `Select` query. Centralised so the inbox
/// list/count and the facets queries stay in sync.
///
/// State semantics:
/// - `filter.all_states == true` → no state filter.
/// - `filter.state.is_some()` → exact match.
/// - otherwise → defaults to `announced` (legacy default).
///
/// `series_already_joined`: pass `true` when the caller has already inner
/// joined `release_ledger.series_id → series.id` (e.g. the facet queries
/// that need `series.library_id` in `SELECT`/`GROUP BY`). When `false`,
/// this function will add the join itself if the filter needs it.
fn apply_inbox_filter<E>(
    mut query: sea_orm::Select<E>,
    filter: &LedgerInboxFilter,
    series_already_joined: bool,
) -> sea_orm::Select<E>
where
    E: EntityTrait,
{
    use sea_orm::{JoinType, RelationTrait};

    if !filter.all_states {
        let state_filter = filter.state.as_deref().unwrap_or(state::ANNOUNCED);
        query = query.filter(release_ledger::Column::State.eq(state_filter));
    }
    if let Some(sid) = filter.series_id {
        query = query.filter(release_ledger::Column::SeriesId.eq(sid));
    }
    if let Some(src) = filter.source_id {
        query = query.filter(release_ledger::Column::SourceId.eq(src));
    }
    if let Some(ref lang) = filter.language {
        query = query.filter(release_ledger::Column::Language.eq(lang));
    }
    if let Some(lib_id) = filter.library_id {
        if !series_already_joined {
            query = query.join(JoinType::InnerJoin, release_ledger::Relation::Series.def());
        }
        query = query.filter(crate::db::entities::series::Column::LibraryId.eq(lib_id));
    }
    query
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::ScanningStrategy;
    use crate::db::entities::release_sources::kind;
    use crate::db::repositories::{
        LibraryRepository, NewReleaseSource, ReleaseSourceRepository, SeriesRepository,
    };
    use crate::db::test_helpers::create_test_db;

    async fn setup_world(db: &DatabaseConnection) -> (Uuid, Uuid) {
        let library = LibraryRepository::create(db, "Lib", "/lib", ScanningStrategy::Default)
            .await
            .unwrap();
        let series = SeriesRepository::create(db, library.id, "Series", None)
            .await
            .unwrap();
        let source = ReleaseSourceRepository::create(
            db,
            NewReleaseSource {
                plugin_id: "release-nyaa".to_string(),
                source_key: "nyaa:user:tsuna69".to_string(),
                display_name: "Nyaa - tsuna69".to_string(),
                kind: kind::RSS_UPLOADER.to_string(),
                poll_interval_s: 3600,
                enabled: None,
                config: None,
            },
        )
        .await
        .unwrap();
        (series.id, source.id)
    }

    fn entry(series_id: Uuid, source_id: Uuid, ext_id: &str) -> NewReleaseEntry {
        NewReleaseEntry {
            series_id,
            source_id,
            external_release_id: ext_id.to_string(),
            info_hash: None,
            chapter: Some(143.0),
            volume: None,
            language: Some("en".to_string()),
            format_hints: None,
            group_or_uploader: Some("tsuna69".to_string()),
            payload_url: format!("https://nyaa.si/view/{}", ext_id),
            media_url: None,
            media_url_kind: None,
            confidence: 0.95,
            metadata: None,
            observed_at: Utc::now(),
        }
    }

    #[tokio::test]
    async fn record_persists_media_url_pair() {
        let (db, _temp) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let (series_id, source_id) = setup_world(conn).await;

        let mut e = entry(series_id, source_id, "rel-media");
        e.media_url = Some("https://nyaa.si/download/1.torrent".to_string());
        e.media_url_kind = Some("torrent".to_string());
        let outcome = ReleaseLedgerRepository::record(conn, e).await.unwrap();
        assert!(!outcome.deduped);
        assert_eq!(
            outcome.row.media_url.as_deref(),
            Some("https://nyaa.si/download/1.torrent")
        );
        assert_eq!(outcome.row.media_url_kind.as_deref(), Some("torrent"));

        let fetched = ReleaseLedgerRepository::get_by_id(conn, outcome.row.id)
            .await
            .unwrap()
            .expect("row exists");
        assert_eq!(
            fetched.media_url.as_deref(),
            Some("https://nyaa.si/download/1.torrent")
        );
        assert_eq!(fetched.media_url_kind.as_deref(), Some("torrent"));
    }

    #[tokio::test]
    async fn record_inserts_then_dedups_on_external_id() {
        let (db, _temp) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let (series_id, source_id) = setup_world(conn).await;

        let first = ReleaseLedgerRepository::record(conn, entry(series_id, source_id, "rel-1"))
            .await
            .unwrap();
        assert!(!first.deduped);

        let second = ReleaseLedgerRepository::record(conn, entry(series_id, source_id, "rel-1"))
            .await
            .unwrap();
        assert!(second.deduped);
        assert_eq!(first.row.id, second.row.id);
    }

    #[tokio::test]
    async fn record_dedups_on_info_hash_across_sources() {
        let (db, _temp) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let (series_id, src_a) = setup_world(conn).await;
        // Second source - same plugin, different uploader.
        let src_b = ReleaseSourceRepository::create(
            conn,
            NewReleaseSource {
                plugin_id: "release-nyaa".to_string(),
                source_key: "nyaa:user:other".to_string(),
                display_name: "Nyaa - other".to_string(),
                kind: kind::RSS_UPLOADER.to_string(),
                poll_interval_s: 3600,
                enabled: None,
                config: None,
            },
        )
        .await
        .unwrap();

        let mut e1 = entry(series_id, src_a, "rel-A");
        e1.info_hash = Some("deadbeefcafe".to_string());
        let mut e2 = entry(series_id, src_b.id, "rel-B");
        e2.info_hash = Some("deadbeefcafe".to_string());

        let r1 = ReleaseLedgerRepository::record(conn, e1).await.unwrap();
        let r2 = ReleaseLedgerRepository::record(conn, e2).await.unwrap();
        assert!(!r1.deduped);
        assert!(
            r2.deduped,
            "same info_hash from different source must dedup onto the first row"
        );
        assert_eq!(r1.row.id, r2.row.id);
    }

    #[tokio::test]
    async fn record_validates_required_fields() {
        let (db, _temp) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let (series_id, source_id) = setup_world(conn).await;

        let mut bad = entry(series_id, source_id, "rel-x");
        bad.payload_url = "".to_string();
        let err = ReleaseLedgerRepository::record(conn, bad)
            .await
            .unwrap_err();
        assert!(err.to_string().contains("payload_url"));

        let mut bad = entry(series_id, source_id, "rel-x");
        bad.external_release_id = "".to_string();
        let err = ReleaseLedgerRepository::record(conn, bad)
            .await
            .unwrap_err();
        assert!(err.to_string().contains("external_release_id"));

        let mut bad = entry(series_id, source_id, "rel-x");
        bad.confidence = f64::NAN;
        let err = ReleaseLedgerRepository::record(conn, bad)
            .await
            .unwrap_err();
        assert!(err.to_string().contains("NaN"));
    }

    #[tokio::test]
    async fn list_for_series_sorts_chapter_desc_over_observed_at() {
        // The series detail panel must mirror the inbox's per-series order:
        // highest chapter wins, even if a lower chapter was observed later.
        let (db, _temp) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let (series_id, source_id) = setup_world(conn).await;

        let now = Utc::now();
        let mut high_old = entry(series_id, source_id, "rel-high");
        high_old.chapter = Some(200.0);
        high_old.observed_at = now - chrono::Duration::hours(6);
        let mut low_new = entry(series_id, source_id, "rel-low");
        low_new.chapter = Some(150.0);
        low_new.observed_at = now;
        ReleaseLedgerRepository::record(conn, high_old)
            .await
            .unwrap();
        ReleaseLedgerRepository::record(conn, low_new)
            .await
            .unwrap();

        let rows = ReleaseLedgerRepository::list_for_series(conn, series_id, None, 10, 0)
            .await
            .unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].chapter, Some(200.0));
        assert_eq!(rows[1].chapter, Some(150.0));
    }

    #[tokio::test]
    async fn list_for_series_orders_by_observed_at_desc() {
        let (db, _temp) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let (series_id, source_id) = setup_world(conn).await;

        let now = Utc::now();
        let mut older = entry(series_id, source_id, "rel-old");
        older.observed_at = now - chrono::Duration::hours(2);
        let mut newer = entry(series_id, source_id, "rel-new");
        newer.observed_at = now;
        ReleaseLedgerRepository::record(conn, older).await.unwrap();
        ReleaseLedgerRepository::record(conn, newer).await.unwrap();

        let rows = ReleaseLedgerRepository::list_for_series(conn, series_id, None, 10, 0)
            .await
            .unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].external_release_id, "rel-new");
        assert_eq!(rows[1].external_release_id, "rel-old");
    }

    #[tokio::test]
    async fn list_inbox_filters_by_state() {
        let (db, _temp) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let (series_id, source_id) = setup_world(conn).await;

        let r1 = ReleaseLedgerRepository::record(conn, entry(series_id, source_id, "rel-1"))
            .await
            .unwrap();
        let _r2 = ReleaseLedgerRepository::record(conn, entry(series_id, source_id, "rel-2"))
            .await
            .unwrap();

        // Dismiss one.
        ReleaseLedgerRepository::set_state(conn, r1.row.id, state::DISMISSED)
            .await
            .unwrap();

        let announced =
            ReleaseLedgerRepository::list_inbox(conn, LedgerInboxFilter::default(), 10, 0)
                .await
                .unwrap();
        assert_eq!(announced.len(), 1);
        assert_eq!(announced[0].external_release_id, "rel-2");

        let dismissed = ReleaseLedgerRepository::list_inbox(
            conn,
            LedgerInboxFilter {
                state: Some(state::DISMISSED.to_string()),
                ..Default::default()
            },
            10,
            0,
        )
        .await
        .unwrap();
        assert_eq!(dismissed.len(), 1);
        assert_eq!(dismissed[0].external_release_id, "rel-1");
    }

    #[tokio::test]
    async fn list_inbox_orders_series_alphabetically_by_name() {
        // Cross-series ordering used to be by `series_id` (UUID), which is
        // deterministic but meaningless to users. Now the inbox joins `series`
        // and orders by `name ASC`, so "A series" appears before "Z series".
        let (db, _temp) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let library = LibraryRepository::create(conn, "Lib", "/lib", ScanningStrategy::Default)
            .await
            .unwrap();
        let source = ReleaseSourceRepository::create(
            conn,
            NewReleaseSource {
                plugin_id: "release-nyaa".to_string(),
                source_key: "nyaa:user:tsuna69".to_string(),
                display_name: "Nyaa - tsuna69".to_string(),
                kind: kind::RSS_UPLOADER.to_string(),
                poll_interval_s: 3600,
                enabled: None,
                config: None,
            },
        )
        .await
        .unwrap();
        // Create series in reverse alphabetical order to prove the sort isn't
        // just preserving insertion order.
        let zebra = SeriesRepository::create(conn, library.id, "Zebra", None)
            .await
            .unwrap();
        let middle = SeriesRepository::create(conn, library.id, "Middle", None)
            .await
            .unwrap();
        let alpha = SeriesRepository::create(conn, library.id, "Alpha", None)
            .await
            .unwrap();

        for sid in [zebra.id, middle.id, alpha.id] {
            ReleaseLedgerRepository::record(conn, entry(sid, source.id, &format!("rel-{}", sid)))
                .await
                .unwrap();
        }

        let rows = ReleaseLedgerRepository::list_inbox(conn, LedgerInboxFilter::default(), 100, 0)
            .await
            .unwrap();
        let series_order: Vec<Uuid> = rows.iter().map(|r| r.series_id).collect();
        assert_eq!(
            series_order,
            vec![alpha.id, middle.id, zebra.id],
            "inbox should list series alphabetically by series.name"
        );
    }

    #[tokio::test]
    async fn list_inbox_groups_series_across_observation_batches() {
        // Bug repro: when a series has rows from two separate poll batches
        // (different `observed_at`s), the inbox must still list every chapter
        // contiguously and descending — not split into two desc clusters by
        // batch. A user reading the inbox doesn't care which poll surfaced a
        // chapter; they want the series' chapter list, in order.
        let (db, _temp) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let (series_id, source_id) = setup_world(conn).await;

        let now = Utc::now();
        let earlier = now - chrono::Duration::hours(6);
        // Earlier batch: lower chapters. Later batch: higher chapters.
        for ch in [122.0_f64, 123.0, 124.0, 125.0] {
            let mut e = entry(series_id, source_id, &format!("rel-{}", ch));
            e.chapter = Some(ch);
            e.observed_at = earlier;
            ReleaseLedgerRepository::record(conn, e).await.unwrap();
        }
        for ch in [150.0_f64, 151.0, 156.0] {
            let mut e = entry(series_id, source_id, &format!("rel-{}", ch));
            e.chapter = Some(ch);
            e.observed_at = now;
            ReleaseLedgerRepository::record(conn, e).await.unwrap();
        }

        let rows = ReleaseLedgerRepository::list_inbox(conn, LedgerInboxFilter::default(), 100, 0)
            .await
            .unwrap();
        let chapters: Vec<f64> = rows.iter().filter_map(|r| r.chapter).collect();
        assert_eq!(
            chapters,
            vec![156.0, 151.0, 150.0, 125.0, 124.0, 123.0, 122.0],
            "chapters of one series must be one contiguous desc list, regardless of observed_at batch"
        );
    }

    #[tokio::test]
    async fn list_inbox_orders_chapters_desc_within_series() {
        // A poll batch records every release with the same `observed_at`. The
        // inbox must still present the highest chapter first per series, not
        // the arbitrary order rows happened to be inserted in.
        let (db, _temp) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let (series_id, source_id) = setup_world(conn).await;

        let now = Utc::now();
        // Insert in shuffled chapter order to prove the DB is doing the sort.
        for ch in [129.0_f64, 145.0, 122.0, 150.5, 137.0, 156.0, 138.0] {
            let mut e = entry(series_id, source_id, &format!("rel-{}", ch));
            e.chapter = Some(ch);
            e.observed_at = now;
            ReleaseLedgerRepository::record(conn, e).await.unwrap();
        }

        let rows = ReleaseLedgerRepository::list_inbox(conn, LedgerInboxFilter::default(), 100, 0)
            .await
            .unwrap();
        let chapters: Vec<f64> = rows.iter().filter_map(|r| r.chapter).collect();
        assert_eq!(
            chapters,
            vec![156.0, 150.5, 145.0, 138.0, 137.0, 129.0, 122.0],
            "rows of the same series must be sorted by chapter desc"
        );
    }

    #[tokio::test]
    async fn list_inbox_groups_series_with_chapters_desc_inside() {
        // Two series in the same poll batch: the inbox must keep each series'
        // rows contiguous and sort their chapters descending. The cross-series
        // order is by series_id ASC (deterministic, but not user-meaningful).
        let (db, _temp) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let (series_a, src) = setup_world(conn).await;
        let library = LibraryRepository::create(conn, "Lib2", "/lib2", ScanningStrategy::Default)
            .await
            .unwrap();
        let series_b = SeriesRepository::create(conn, library.id, "Series B", None)
            .await
            .unwrap();

        let now = Utc::now();
        let mut a1 = entry(series_a, src, "a-1");
        a1.chapter = Some(10.0);
        a1.observed_at = now;
        let mut a2 = entry(series_a, src, "a-2");
        a2.chapter = Some(20.0);
        a2.observed_at = now;
        let mut b1 = entry(series_b.id, src, "b-1");
        b1.chapter = Some(5.0);
        b1.observed_at = now;
        let mut b2 = entry(series_b.id, src, "b-2");
        b2.chapter = Some(7.0);
        b2.observed_at = now;
        // Insert interleaved to prove ordering doesn't leak from insertion order.
        ReleaseLedgerRepository::record(conn, a1).await.unwrap();
        ReleaseLedgerRepository::record(conn, b1).await.unwrap();
        ReleaseLedgerRepository::record(conn, a2).await.unwrap();
        ReleaseLedgerRepository::record(conn, b2).await.unwrap();

        let rows = ReleaseLedgerRepository::list_inbox(conn, LedgerInboxFilter::default(), 100, 0)
            .await
            .unwrap();
        // Each series' rows must be contiguous and chapter-desc internally.
        let series_groups: Vec<Vec<(Uuid, f64)>> = rows
            .iter()
            .map(|r| (r.series_id, r.chapter.unwrap()))
            .fold(Vec::new(), |mut acc, (sid, ch)| {
                if acc.last().is_some_and(|g: &Vec<_>| g[0].0 == sid) {
                    acc.last_mut().unwrap().push((sid, ch));
                } else {
                    acc.push(vec![(sid, ch)]);
                }
                acc
            });
        assert_eq!(
            series_groups.len(),
            2,
            "rows of each series must be contiguous"
        );
        for group in &series_groups {
            let chs: Vec<f64> = group.iter().map(|(_, c)| *c).collect();
            let mut sorted = chs.clone();
            sorted.sort_by(|a, b| b.partial_cmp(a).unwrap());
            assert_eq!(chs, sorted, "chapters within a series must be desc");
        }
    }

    #[tokio::test]
    async fn list_inbox_supports_combined_filters() {
        let (db, _temp) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let (series_a, src_a) = setup_world(conn).await;
        // Second series.
        let library = LibraryRepository::create(conn, "Lib2", "/lib2", ScanningStrategy::Default)
            .await
            .unwrap();
        let series_b = SeriesRepository::create(conn, library.id, "Series B", None)
            .await
            .unwrap();

        // 2 entries on A, 1 on B.
        ReleaseLedgerRepository::record(conn, entry(series_a, src_a, "rel-1"))
            .await
            .unwrap();
        ReleaseLedgerRepository::record(conn, entry(series_a, src_a, "rel-2"))
            .await
            .unwrap();
        ReleaseLedgerRepository::record(conn, entry(series_b.id, src_a, "rel-3"))
            .await
            .unwrap();

        let only_a = ReleaseLedgerRepository::list_inbox(
            conn,
            LedgerInboxFilter {
                series_id: Some(series_a),
                ..Default::default()
            },
            10,
            0,
        )
        .await
        .unwrap();
        assert_eq!(only_a.len(), 2);

        let total = ReleaseLedgerRepository::count_inbox(conn, LedgerInboxFilter::default())
            .await
            .unwrap();
        assert_eq!(total, 3);
    }

    #[tokio::test]
    async fn set_state_validates_and_transitions() {
        let (db, _temp) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let (series_id, source_id) = setup_world(conn).await;
        let r = ReleaseLedgerRepository::record(conn, entry(series_id, source_id, "rel-1"))
            .await
            .unwrap();

        let updated = ReleaseLedgerRepository::set_state(conn, r.row.id, state::MARKED_ACQUIRED)
            .await
            .unwrap();
        assert_eq!(updated.state, "marked_acquired");

        let err = ReleaseLedgerRepository::set_state(conn, r.row.id, "garbage")
            .await
            .unwrap_err();
        assert!(err.to_string().contains("invalid state"));
    }

    #[tokio::test]
    async fn cascade_deletes_ledger_when_series_deleted() {
        let (db, _temp) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let (series_id, source_id) = setup_world(conn).await;
        ReleaseLedgerRepository::record(conn, entry(series_id, source_id, "rel-1"))
            .await
            .unwrap();

        SeriesRepository::delete(conn, series_id).await.unwrap();

        let rows = ReleaseLedgerRepository::list_for_series(conn, series_id, None, 10, 0)
            .await
            .unwrap();
        assert!(rows.is_empty(), "ledger rows cascaded with series");
    }

    #[tokio::test]
    async fn delete_by_source_removes_only_that_sources_rows() {
        let (db, _temp) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let (series_id, source_a) = setup_world(conn).await;

        // Add a second source so we can prove scoping.
        let source_b = ReleaseSourceRepository::create(
            conn,
            NewReleaseSource {
                plugin_id: "release-nyaa".to_string(),
                source_key: "nyaa:user:other".to_string(),
                display_name: "Nyaa - other".to_string(),
                kind: kind::RSS_UPLOADER.to_string(),
                poll_interval_s: 3600,
                enabled: None,
                config: None,
            },
        )
        .await
        .unwrap();

        ReleaseLedgerRepository::record(conn, entry(series_id, source_a, "rel-1"))
            .await
            .unwrap();
        ReleaseLedgerRepository::record(conn, entry(series_id, source_a, "rel-2"))
            .await
            .unwrap();
        ReleaseLedgerRepository::record(conn, entry(series_id, source_b.id, "rel-3"))
            .await
            .unwrap();

        let removed = ReleaseLedgerRepository::delete_by_source(conn, source_a)
            .await
            .unwrap();
        assert_eq!(removed, 2);

        // Source A is empty; source B still has its row.
        let after_a =
            ReleaseLedgerRepository::list_inbox(conn, LedgerInboxFilter::default(), 100, 0)
                .await
                .unwrap()
                .into_iter()
                .filter(|r| r.source_id == source_a)
                .count();
        assert_eq!(after_a, 0);
        let after_b =
            ReleaseLedgerRepository::list_inbox(conn, LedgerInboxFilter::default(), 100, 0)
                .await
                .unwrap()
                .into_iter()
                .filter(|r| r.source_id == source_b.id)
                .count();
        assert_eq!(after_b, 1);
    }

    #[tokio::test]
    async fn cascade_deletes_ledger_when_source_deleted() {
        let (db, _temp) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let (series_id, source_id) = setup_world(conn).await;
        ReleaseLedgerRepository::record(conn, entry(series_id, source_id, "rel-1"))
            .await
            .unwrap();

        ReleaseSourceRepository::delete(conn, source_id)
            .await
            .unwrap();

        let rows = ReleaseLedgerRepository::list_for_series(conn, series_id, None, 10, 0)
            .await
            .unwrap();
        assert!(rows.is_empty(), "ledger rows cascaded with source");
    }
}
