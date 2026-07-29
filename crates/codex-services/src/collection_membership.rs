//! Resolving what a collection contains.
//!
//! A collection's members come from one of two places, and every read path goes
//! through here so it does not have to know which:
//!
//! * **Manual** (`condition IS NULL`): the `collection_series` junction, with
//!   the order the user arranged.
//! * **Automatic** (`condition IS NOT NULL`): a stored `SeriesCondition`,
//!   evaluated against the library on every read.
//!
//! Automatic membership is never materialized. The collection *is* the query, so
//! the query is what gets stored: a series that starts matching appears on the
//! next read with no refresh step, no staleness window and no cache to
//! invalidate.
//!
//! This lives in `codex-services` rather than `codex-db` because the crate graph
//! only points one way: `codex-services` depends on `codex-db`, so
//! `CollectionRepository` cannot reach [`FilterService`]. The repository gained
//! the ability to sort and paginate a caller-supplied id set
//! ([`CollectionRepository::get_series_by_ids`]); deciding *which* ids happens
//! here.

use anyhow::{Context, Result};
use codex_db::entities::{collections, series};
use codex_db::repositories::CollectionRepository;
use codex_db::repositories::visibility::SeriesVisibility;
use codex_models::filter::SeriesCondition;
use codex_models::sort::{CollectionSeriesSort, SortDirection};
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use crate::filter::FilterService;

/// Resolves collection membership for both manual and rule-backed collections.
pub struct CollectionMembershipService;

impl CollectionMembershipService {
    /// `true` when this collection's membership comes from a rule.
    pub fn is_automatic(collection: &collections::Model) -> bool {
        collection.condition.is_some()
    }

    /// Parse the stored rule, if there is one.
    ///
    /// Fails loudly rather than treating an unparseable rule as "no members":
    /// silently returning an empty collection would look exactly like a rule
    /// that matches nothing, and the administrator would have no way to tell
    /// their collection is broken rather than just too narrow.
    pub fn rule(collection: &collections::Model) -> Result<Option<SeriesCondition>> {
        let Some(raw) = &collection.condition else {
            return Ok(None);
        };
        let condition: SeriesCondition =
            serde_json::from_value(raw.clone()).with_context(|| {
                format!(
                    "collection {} has a stored rule that is not a valid SeriesCondition",
                    collection.id
                )
            })?;
        Ok(Some(condition))
    }

    /// The series a collection contains, in the requested order, filtered by the
    /// caller's visibility.
    ///
    /// `user_id` is the caller, and it matters: a rule may reference the
    /// viewer's own data (`userRating`, `readStatus`, `hasUserRating`), in which
    /// case the same collection legitimately holds different series for
    /// different people. Passing `None` resolves those sub-conditions as
    /// "nobody", which is the right answer for an unauthenticated feed.
    pub async fn members(
        db: &DatabaseConnection,
        collection: &collections::Model,
        vis: Option<&SeriesVisibility>,
        sort: Option<CollectionSeriesSort>,
        direction: SortDirection,
        user_id: Option<Uuid>,
    ) -> Result<Vec<series::Model>> {
        let Some(condition) = Self::rule(collection)? else {
            return CollectionRepository::get_series(db, collection, vis, sort, direction).await;
        };

        let matching = FilterService::get_matching_series_for_user(db, &condition, None, user_id)
            .await
            .with_context(|| format!("failed to resolve rule for collection {}", collection.id))?;

        if matching.is_empty() {
            return Ok(vec![]);
        }

        // `ordered` is forced off for rule-backed collections, so the default
        // sort is title. An explicit `Manual` has no arrangement to honour and
        // degrades to the same thing.
        let sort = sort.unwrap_or(CollectionSeriesSort::Title);
        let ids: Vec<Uuid> = matching.into_iter().collect();

        CollectionRepository::get_series_by_ids(db, &ids, vis, sort, direction).await
    }

    /// How many series a collection contains for this caller.
    ///
    /// Only meaningful to call when you actually need the number. For a
    /// rule-backed collection this resolves the whole rule, so list endpoints
    /// deliberately do not call it: rendering one page of collections would mean
    /// running every rule on the server.
    pub async fn count(
        db: &DatabaseConnection,
        collection: &collections::Model,
        vis: Option<&SeriesVisibility>,
        user_id: Option<Uuid>,
    ) -> Result<u64> {
        if !Self::is_automatic(collection) {
            return CollectionRepository::count_series(db, collection.id, vis).await;
        }
        let members = Self::members(
            db,
            collection,
            vis,
            Some(CollectionSeriesSort::Title),
            SortDirection::default(),
            user_id,
        )
        .await?;
        Ok(members.len() as u64)
    }
}
