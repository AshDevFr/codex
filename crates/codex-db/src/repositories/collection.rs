//! Repository for collections and the collection_series junction.
//!
//! Collections are shared, named groupings of series. Manual order is held by
//! the `position` column on the junction and is always maintained; the
//! collection's `ordered` flag only picks the default sort when a caller
//! requests none (see [`CollectionRepository::get_series`]).

#![allow(dead_code)]

use std::collections::HashMap;

use anyhow::Result;
use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, IntoActiveModel, JoinType,
    Order, PaginatorTrait, QueryFilter, QueryOrder, QuerySelect, RelationTrait, Set,
    sea_query::{Expr, Func, NullOrdering},
};
use uuid::Uuid;

use crate::entities::{
    collection_series, collection_series::Entity as CollectionSeries, collections,
    collections::Entity as Collections, series, series::Entity as Series, series_metadata,
};
use crate::repositories::visibility::{SeriesVisibility, visibility_predicate};
use codex_models::sort::{CollectionSeriesSort, SortDirection};

/// Repository for collection operations.
pub struct CollectionRepository;

impl CollectionRepository {
    /// Get a collection by ID.
    pub async fn get_by_id(
        db: &DatabaseConnection,
        id: Uuid,
    ) -> Result<Option<collections::Model>> {
        Ok(Collections::find_by_id(id).one(db).await?)
    }

    /// Get a collection by (case-insensitive) name.
    pub async fn get_by_name(
        db: &DatabaseConnection,
        name: &str,
    ) -> Result<Option<collections::Model>> {
        let normalized = name.trim().to_lowercase();
        Ok(Collections::find()
            .filter(collections::Column::NormalizedName.eq(normalized))
            .one(db)
            .await?)
    }

    /// List all collections sorted by name.
    pub async fn list_all(db: &DatabaseConnection) -> Result<Vec<collections::Model>> {
        Ok(Collections::find()
            .order_by_asc(collections::Column::Name)
            .all(db)
            .await?)
    }

    /// Total number of collections.
    pub async fn count(db: &DatabaseConnection) -> Result<u64> {
        Ok(Collections::find().count(db).await?)
    }

    /// Get the set of series IDs that belong to at least one collection.
    ///
    /// Used by the filter service to evaluate the "in collection" membership
    /// filter. Returns distinct series IDs across all collections.
    ///
    /// Rule-backed collections are invisible here, and that is what makes rule
    /// recursion structurally impossible: if `inCollection` could see them, then
    /// evaluating a rule that itself contains `inCollection` would have to
    /// resolve every other rule first. Because an automatic collection has no
    /// junction rows, that exclusion needs no filter of its own; this method
    /// only ever sees manual membership.
    pub async fn all_member_series_ids(
        db: &DatabaseConnection,
    ) -> Result<std::collections::HashSet<Uuid>> {
        let ids: Vec<Uuid> = CollectionSeries::find()
            .select_only()
            .column(collection_series::Column::SeriesId)
            .distinct()
            .into_tuple()
            .all(db)
            .await?;
        Ok(ids.into_iter().collect())
    }

    /// Create a new collection. Fails if the (normalized) name already exists.
    ///
    /// A `condition` makes the collection rule-backed: membership is resolved
    /// at read time from the stored `SeriesCondition` and no junction rows are
    /// ever written. `ordered` is forced to `false` in that case, since there is
    /// no manual arrangement to preserve.
    pub async fn create(
        db: &DatabaseConnection,
        name: &str,
        summary: Option<&str>,
        ordered: bool,
        condition: Option<serde_json::Value>,
    ) -> Result<collections::Model> {
        let now = Utc::now();
        let is_auto = condition.is_some();
        let model = collections::ActiveModel {
            id: Set(Uuid::new_v4()),
            name: Set(name.trim().to_string()),
            normalized_name: Set(name.trim().to_lowercase()),
            summary: Set(summary.map(|s| s.to_string())),
            condition: Set(condition),
            ordered: Set(ordered && !is_auto),
            created_at: Set(now),
            updated_at: Set(now),
        };
        Ok(model.insert(db).await?)
    }

    /// Update a collection's name, summary, condition, and/or ordered flag.
    /// Returns `None` if the collection does not exist.
    ///
    /// The nullable fields take a tri-state: `None` leaves the field alone,
    /// `Some(None)` clears it, `Some(Some(v))` sets it. Clearing `condition`
    /// converts a rule-backed collection to a manual one, which leaves it empty
    /// because it never had junction rows.
    ///
    /// Setting a condition forces `ordered` off for the same reason `create`
    /// does.
    pub async fn update(
        db: &DatabaseConnection,
        id: Uuid,
        name: Option<&str>,
        summary: Option<Option<&str>>,
        ordered: Option<bool>,
        condition: Option<Option<serde_json::Value>>,
    ) -> Result<Option<collections::Model>> {
        let Some(existing) = Collections::find_by_id(id).one(db).await? else {
            return Ok(None);
        };
        // Whether the collection is rule-backed *after* this update, which is
        // what decides if `ordered` may be true.
        let is_auto = match &condition {
            Some(next) => next.is_some(),
            None => existing.condition.is_some(),
        };
        let mut active = existing.into_active_model();
        if let Some(name) = name {
            active.name = Set(name.trim().to_string());
            active.normalized_name = Set(name.trim().to_lowercase());
        }
        if let Some(summary) = summary {
            active.summary = Set(summary.map(|s| s.to_string()));
        }
        if let Some(condition) = condition {
            active.condition = Set(condition);
        }
        if let Some(ordered) = ordered {
            active.ordered = Set(ordered && !is_auto);
        } else if is_auto {
            active.ordered = Set(false);
        }
        active.updated_at = Set(Utc::now());
        Ok(Some(active.update(db).await?))
    }

    /// Delete a collection (cascades its membership rows). Returns whether a row
    /// was removed.
    pub async fn delete(db: &DatabaseConnection, id: Uuid) -> Result<bool> {
        let result = Collections::delete_by_id(id).exec(db).await?;
        Ok(result.rows_affected > 0)
    }

    /// Whether the collection exists and is rule-backed.
    ///
    /// The mutation guards below use this rather than trusting callers to have
    /// checked: a stray junction row on an automatic collection would be
    /// invisible (its members come from the rule) and would resurface the moment
    /// the rule was cleared.
    async fn is_rule_backed(db: &DatabaseConnection, collection_id: Uuid) -> Result<bool> {
        Ok(Collections::find_by_id(collection_id)
            .one(db)
            .await?
            .is_some_and(|c| c.condition.is_some()))
    }

    /// Add a series to a collection at the end of the order. Idempotent: if the
    /// series is already a member, returns the existing link unchanged.
    ///
    /// Errors on a rule-backed collection: its membership is the rule, and the
    /// fix for a series that should or shouldn't be in it is the series'
    /// metadata, which also corrects search and every other rule.
    pub async fn add_series(
        db: &DatabaseConnection,
        collection_id: Uuid,
        series_id: Uuid,
    ) -> Result<collection_series::Model> {
        if Self::is_rule_backed(db, collection_id).await? {
            anyhow::bail!(
                "collection {collection_id} is automatic: its members come from its rule, so they cannot be added by hand"
            );
        }

        if let Some(existing) = CollectionSeries::find()
            .filter(collection_series::Column::CollectionId.eq(collection_id))
            .filter(collection_series::Column::SeriesId.eq(series_id))
            .one(db)
            .await?
        {
            return Ok(existing);
        }

        let position = Self::next_position(db, collection_id).await?;
        let link = collection_series::ActiveModel {
            id: Set(Uuid::new_v4()),
            collection_id: Set(collection_id),
            series_id: Set(series_id),
            position: Set(position),
            created_at: Set(Utc::now()),
        };
        Ok(link.insert(db).await?)
    }

    /// Remove a series from a collection. Returns whether a row was removed.
    ///
    /// Errors on a rule-backed collection, for the reasons on
    /// [`Self::add_series`].
    pub async fn remove_series(
        db: &DatabaseConnection,
        collection_id: Uuid,
        series_id: Uuid,
    ) -> Result<bool> {
        if Self::is_rule_backed(db, collection_id).await? {
            anyhow::bail!(
                "collection {collection_id} is automatic: its members come from its rule, so they cannot be removed by hand"
            );
        }

        let result = CollectionSeries::delete_many()
            .filter(collection_series::Column::CollectionId.eq(collection_id))
            .filter(collection_series::Column::SeriesId.eq(series_id))
            .exec(db)
            .await?;
        Ok(result.rows_affected > 0)
    }

    /// Set explicit positions for the given series in the order provided. Series
    /// not currently members are skipped.
    ///
    /// Errors on a rule-backed collection: there is no manual order to set, and
    /// `ordered` is forced off for those collections anyway.
    pub async fn reorder(
        db: &DatabaseConnection,
        collection_id: Uuid,
        ordered_series_ids: &[Uuid],
    ) -> Result<()> {
        if Self::is_rule_backed(db, collection_id).await? {
            anyhow::bail!(
                "collection {collection_id} is automatic: it has no manual order to rearrange"
            );
        }

        for (idx, series_id) in ordered_series_ids.iter().enumerate() {
            if let Some(link) = CollectionSeries::find()
                .filter(collection_series::Column::CollectionId.eq(collection_id))
                .filter(collection_series::Column::SeriesId.eq(*series_id))
                .one(db)
                .await?
            {
                let mut active = link.into_active_model();
                active.position = Set(idx as i32);
                active.update(db).await?;
            }
        }
        Ok(())
    }

    /// Get the member series of a collection, filtered by the caller's
    /// visibility.
    ///
    /// An explicit `sort` always wins. When omitted, the collection's
    /// `ordered` flag picks the default: manual position order when set,
    /// displayed title (metadata `title_sort`, falling back to `title`, then
    /// the scan-derived series name) otherwise. `direction` applies to every
    /// sort except `Manual`, whose order is exactly what the user arranged.
    pub async fn get_series(
        db: &DatabaseConnection,
        collection: &collections::Model,
        vis: Option<&SeriesVisibility>,
        sort: Option<CollectionSeriesSort>,
        direction: SortDirection,
    ) -> Result<Vec<series::Model>> {
        if matches!(vis, Some(v) if v.is_empty_whitelist()) {
            return Ok(vec![]);
        }

        let sort = sort.unwrap_or(if collection.ordered {
            CollectionSeriesSort::Manual
        } else {
            CollectionSeriesSort::Title
        });
        let order = match direction {
            SortDirection::Asc => Order::Asc,
            SortDirection::Desc => Order::Desc,
        };

        let mut junction = CollectionSeries::find()
            .filter(collection_series::Column::CollectionId.eq(collection.id));
        junction = match sort {
            CollectionSeriesSort::Manual => junction
                .order_by_asc(collection_series::Column::Position)
                .order_by_asc(collection_series::Column::CreatedAt),
            CollectionSeriesSort::Added => junction
                .order_by(collection_series::Column::CreatedAt, order.clone())
                .order_by(collection_series::Column::Position, order.clone()),
            // Title/year order lives on the series side; junction order is
            // irrelevant for those.
            _ => junction,
        };
        if let Some(vis) = vis
            && let Some(expr) = visibility_predicate(collection_series::Column::SeriesId, vis)
        {
            junction = junction.filter(expr);
        }

        let ordered_ids: Vec<Uuid> = junction
            .all(db)
            .await?
            .into_iter()
            .map(|l| l.series_id)
            .collect();
        if ordered_ids.is_empty() {
            return Ok(vec![]);
        }

        match sort {
            CollectionSeriesSort::Title | CollectionSeriesSort::Year => {
                Self::hydrate_by_title_or_year(db, ordered_ids, sort, order).await
            }
            // Manual position / date-added order comes from the junction query;
            // re-project the hydrated models into that order.
            _ => {
                let series_models = Series::find()
                    .filter(series::Column::Id.is_in(ordered_ids.clone()))
                    .all(db)
                    .await?;
                let by_id: HashMap<Uuid, series::Model> =
                    series_models.into_iter().map(|s| (s.id, s)).collect();

                Ok(ordered_ids
                    .iter()
                    .filter_map(|id| by_id.get(id).cloned())
                    .collect())
            }
        }
    }

    /// Hydrate the given series in title or year order.
    ///
    /// Shared by the junction-backed and rule-backed read paths: both need the
    /// same case-insensitive title expression and the same "unknown years last"
    /// behaviour, and only differ in where the id list came from.
    async fn hydrate_by_title_or_year(
        db: &DatabaseConnection,
        ids: Vec<Uuid>,
        sort: CollectionSeriesSort,
        order: Order,
    ) -> Result<Vec<series::Model>> {
        // LOWER makes the order case-insensitive: binary collation would
        // sort every uppercase title ahead of any lowercase one.
        let title_expr = Expr::expr(Func::lower(Func::coalesce([
            Expr::col((series_metadata::Entity, series_metadata::Column::TitleSort)).into(),
            Expr::col((series_metadata::Entity, series_metadata::Column::Title)).into(),
            Expr::col((series::Entity, series::Column::Name)).into(),
        ])));
        let mut query = Series::find()
            .filter(series::Column::Id.is_in(ids))
            .join(JoinType::LeftJoin, series::Relation::SeriesMetadata.def());
        if matches!(sort, CollectionSeriesSort::Year) {
            // Unknown years stay last in both directions.
            query = query.order_by_with_nulls(
                series_metadata::Column::Year,
                order.clone(),
                NullOrdering::Last,
            );
            // Tie-break years by title ascending regardless of direction.
            query = query.order_by(title_expr, Order::Asc);
        } else {
            query = query.order_by(title_expr, order);
        }
        Ok(query
            .order_by(series::Column::Id, Order::Asc)
            .all(db)
            .await?)
    }

    /// Sort and hydrate an explicit set of series, filtered by the caller's
    /// visibility.
    ///
    /// This is the read path for rule-backed collections: the caller (the
    /// membership service in `codex-services`) resolves the rule to an id set
    /// and hands it here. The repository stays rule-unaware, because
    /// `codex-services` depends on `codex-db` and not the reverse, so this
    /// crate cannot reach the filter engine.
    ///
    /// Two sorts mean something different without a junction to read:
    ///
    /// * `Manual` has no manual order to honour and falls back to `Title`.
    /// * `Added` means the date the series entered the *library*
    ///   (`series.created_at`), not the date it entered the collection, because
    ///   a rule-backed collection has no per-member join date.
    ///
    /// Visibility is applied to the id list in memory before the query, both to
    /// keep the emitted SQL to a single `IN` and because the visibility sets are
    /// already in memory. This mirrors
    /// [`crate::repositories::SeriesRepository::list_by_ids_sorted`].
    pub async fn get_series_by_ids(
        db: &DatabaseConnection,
        series_ids: &[Uuid],
        vis: Option<&SeriesVisibility>,
        sort: CollectionSeriesSort,
        direction: SortDirection,
    ) -> Result<Vec<series::Model>> {
        if matches!(vis, Some(v) if v.is_empty_whitelist()) {
            return Ok(vec![]);
        }

        let visible_ids: Vec<Uuid> = match vis {
            None => series_ids.to_vec(),
            Some(v) => series_ids
                .iter()
                .copied()
                .filter(|id| {
                    if v.excluded_series_ids.contains(id) {
                        return false;
                    }
                    if let Some(allowed) = &v.allowed_series_ids {
                        return allowed.contains(id);
                    }
                    true
                })
                .collect(),
        };

        if visible_ids.is_empty() {
            return Ok(vec![]);
        }

        let order = match direction {
            SortDirection::Asc => Order::Asc,
            SortDirection::Desc => Order::Desc,
        };

        match sort {
            CollectionSeriesSort::Year => {
                Self::hydrate_by_title_or_year(db, visible_ids, CollectionSeriesSort::Year, order)
                    .await
            }
            CollectionSeriesSort::Added => Ok(Series::find()
                .filter(series::Column::Id.is_in(visible_ids))
                .order_by(series::Column::CreatedAt, order)
                .order_by(series::Column::Id, Order::Asc)
                .all(db)
                .await?),
            // Title, and Manual which has nothing to fall back on but title.
            _ => {
                Self::hydrate_by_title_or_year(db, visible_ids, CollectionSeriesSort::Title, order)
                    .await
            }
        }
    }

    /// Count the visible member series of a collection.
    pub async fn count_series(
        db: &DatabaseConnection,
        collection_id: Uuid,
        vis: Option<&SeriesVisibility>,
    ) -> Result<u64> {
        if matches!(vis, Some(v) if v.is_empty_whitelist()) {
            return Ok(0);
        }
        let mut query = CollectionSeries::find()
            .filter(collection_series::Column::CollectionId.eq(collection_id));
        if let Some(vis) = vis
            && let Some(expr) = visibility_predicate(collection_series::Column::SeriesId, vis)
        {
            query = query.filter(expr);
        }
        Ok(query.count(db).await?)
    }

    /// Get the collections that contain a given series, sorted by name.
    ///
    /// Reports manual membership only. A rule-backed collection is a view over
    /// the library rather than a container that holds anything, so "which
    /// collections is this series in?" has no answer for one: it would mean
    /// evaluating every stored rule on every series page, and the answer could
    /// differ per viewer for a rule over personal ratings. The explicit
    /// `condition IS NULL` filter is belt-and-braces on top of the fact that an
    /// automatic collection has no junction rows.
    pub async fn get_collections_for_series(
        db: &DatabaseConnection,
        series_id: Uuid,
    ) -> Result<Vec<collections::Model>> {
        let collection_ids: Vec<Uuid> = CollectionSeries::find()
            .filter(collection_series::Column::SeriesId.eq(series_id))
            .all(db)
            .await?
            .into_iter()
            .map(|l| l.collection_id)
            .collect();
        if collection_ids.is_empty() {
            return Ok(vec![]);
        }
        Ok(Collections::find()
            .filter(collections::Column::Id.is_in(collection_ids))
            .filter(collections::Column::Condition.is_null())
            .order_by_asc(collections::Column::Name)
            .all(db)
            .await?)
    }

    /// Get the collections containing each of the given series, name-sorted.
    /// Series with no memberships are absent from the returned map.
    ///
    /// Manual membership only, for the reasons on
    /// [`Self::get_collections_for_series`].
    pub async fn get_collections_for_series_ids(
        db: &DatabaseConnection,
        series_ids: &[Uuid],
    ) -> Result<HashMap<Uuid, Vec<collections::Model>>> {
        if series_ids.is_empty() {
            return Ok(HashMap::new());
        }

        let links: Vec<collection_series::Model> = CollectionSeries::find()
            .filter(collection_series::Column::SeriesId.is_in(series_ids.to_vec()))
            .all(db)
            .await?;
        if links.is_empty() {
            return Ok(HashMap::new());
        }

        let collection_ids: Vec<Uuid> = links
            .iter()
            .map(|l| l.collection_id)
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();
        let collections_by_id: HashMap<Uuid, collections::Model> = Collections::find()
            .filter(collections::Column::Id.is_in(collection_ids))
            .filter(collections::Column::Condition.is_null())
            .all(db)
            .await?
            .into_iter()
            .map(|c| (c.id, c))
            .collect();

        let mut map: HashMap<Uuid, Vec<collections::Model>> = HashMap::new();
        for link in links {
            if let Some(coll) = collections_by_id.get(&link.collection_id) {
                map.entry(link.series_id).or_default().push(coll.clone());
            }
        }
        for members in map.values_mut() {
            members.sort_by(|a, b| a.name.cmp(&b.name));
        }
        Ok(map)
    }

    /// Next position value for a new member (max existing + 1, or 0 when empty).
    async fn next_position(db: &DatabaseConnection, collection_id: Uuid) -> Result<i32> {
        let positions: Vec<i32> = CollectionSeries::find()
            .filter(collection_series::Column::CollectionId.eq(collection_id))
            .all(db)
            .await?
            .into_iter()
            .map(|l| l.position)
            .collect();
        Ok(positions.into_iter().max().map(|m| m + 1).unwrap_or(0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ScanningStrategy;
    use crate::repositories::{LibraryRepository, SeriesMetadataRepository, SeriesRepository};
    use crate::test_helpers::create_test_db;
    use codex_models::sort::{CollectionSeriesSort, SortDirection};

    async fn lib_and_series(db: &DatabaseConnection) -> (Uuid, Vec<series::Model>) {
        let library = LibraryRepository::create(db, "Lib", "/lib", ScanningStrategy::Default)
            .await
            .unwrap();
        let mut series = Vec::new();
        for name in ["Alpha", "Bravo", "Charlie"] {
            series.push(
                SeriesRepository::create(db, library.id, name, None)
                    .await
                    .unwrap(),
            );
        }
        (library.id, series)
    }

    #[tokio::test]
    async fn test_create_update_delete() {
        let (db, _t) = create_test_db().await;
        let conn = db.sea_orm_connection();

        let coll = CollectionRepository::create(conn, "  Batman  ", None, false, None)
            .await
            .unwrap();
        assert_eq!(coll.name, "Batman");
        assert_eq!(coll.normalized_name, "batman");
        assert!(!coll.ordered);

        let found = CollectionRepository::get_by_name(conn, "BATMAN")
            .await
            .unwrap();
        assert_eq!(found.unwrap().id, coll.id);

        let updated = CollectionRepository::update(
            conn,
            coll.id,
            Some("Dark Knight"),
            None,
            Some(true),
            None,
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(updated.name, "Dark Knight");
        assert!(updated.ordered);

        assert!(CollectionRepository::delete(conn, coll.id).await.unwrap());
        assert!(
            CollectionRepository::get_by_id(conn, coll.id)
                .await
                .unwrap()
                .is_none()
        );
    }

    #[tokio::test]
    async fn test_membership_add_dedupe_and_order() {
        let (db, _t) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let (_lib, series) = lib_and_series(conn).await;

        let coll = CollectionRepository::create(conn, "Coll", None, true, None)
            .await
            .unwrap();

        // Add in a deliberate order.
        for s in &series {
            CollectionRepository::add_series(conn, coll.id, s.id)
                .await
                .unwrap();
        }
        // Re-adding is idempotent (no duplicate, same row).
        let again = CollectionRepository::add_series(conn, coll.id, series[0].id)
            .await
            .unwrap();
        assert_eq!(again.position, 0);

        let members = CollectionRepository::get_series(conn, &coll, None, None, SortDirection::Asc)
            .await
            .unwrap();
        assert_eq!(members.len(), 3);
        assert_eq!(members[0].id, series[0].id);
        assert_eq!(members[2].id, series[2].id);

        // Reverse the order and re-read.
        let reversed: Vec<Uuid> = series.iter().rev().map(|s| s.id).collect();
        CollectionRepository::reorder(conn, coll.id, &reversed)
            .await
            .unwrap();
        let members = CollectionRepository::get_series(conn, &coll, None, None, SortDirection::Asc)
            .await
            .unwrap();
        assert_eq!(members[0].id, series[2].id);
        assert_eq!(members[2].id, series[0].id);

        // An explicit sort always wins, even on an ordered collection; the
        // flag only picks the default. Series names are Alpha/Bravo/Charlie.
        let members = CollectionRepository::get_series(
            conn,
            &coll,
            None,
            Some(CollectionSeriesSort::Title),
            SortDirection::Asc,
        )
        .await
        .unwrap();
        assert_eq!(members[0].id, series[0].id);
        assert_eq!(members[2].id, series[2].id);

        // And manual order can be requested explicitly regardless of the flag.
        let members = CollectionRepository::get_series(
            conn,
            &coll,
            None,
            Some(CollectionSeriesSort::Manual),
            SortDirection::Asc,
        )
        .await
        .unwrap();
        assert_eq!(members[0].id, series[2].id);
        assert_eq!(members[2].id, series[0].id);

        // Remove one.
        assert!(
            CollectionRepository::remove_series(conn, coll.id, series[1].id)
                .await
                .unwrap()
        );
        assert_eq!(
            CollectionRepository::count_series(conn, coll.id, None)
                .await
                .unwrap(),
            2
        );
    }

    #[tokio::test]
    async fn test_unordered_collection_sorts_by_title() {
        let (db, _t) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let library = LibraryRepository::create(conn, "Lib", "/lib", ScanningStrategy::Default)
            .await
            .unwrap();

        // Deliberately add in non-alphabetical order so insertion order and
        // title order differ.
        let mut by_name = HashMap::new();
        for name in ["Banana", "Cherry", "Apple"] {
            let s = SeriesRepository::create(conn, library.id, name, None)
                .await
                .unwrap();
            by_name.insert(name, s);
        }

        let coll = CollectionRepository::create(conn, "Coll", None, false, None)
            .await
            .unwrap();
        for name in ["Banana", "Cherry", "Apple"] {
            CollectionRepository::add_series(conn, coll.id, by_name[name].id)
                .await
                .unwrap();
        }

        // Default sort for an unordered collection is by title.
        let members = CollectionRepository::get_series(conn, &coll, None, None, SortDirection::Asc)
            .await
            .unwrap();
        let names: Vec<&str> = members.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, ["Apple", "Banana", "Cherry"]);

        // The sort must follow metadata title_sort, not the series name.
        SeriesMetadataRepository::update_title(
            conn,
            by_name["Cherry"].id,
            "Cherry".to_string(),
            Some("0-Cherry".to_string()),
            None,
        )
        .await
        .unwrap();
        let members = CollectionRepository::get_series(conn, &coll, None, None, SortDirection::Asc)
            .await
            .unwrap();
        let names: Vec<&str> = members.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, ["Cherry", "Apple", "Banana"]);

        // Case-insensitive: a lowercase title must not sort after every
        // uppercase one (binary collation would put "apple" last).
        SeriesMetadataRepository::update_title(
            conn,
            by_name["Apple"].id,
            "apple".to_string(),
            None,
            None,
        )
        .await
        .unwrap();
        let members = CollectionRepository::get_series(conn, &coll, None, None, SortDirection::Asc)
            .await
            .unwrap();
        let names: Vec<&str> = members.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, ["Cherry", "Apple", "Banana"]);

        // Descending direction reverses the title order.
        let members =
            CollectionRepository::get_series(conn, &coll, None, None, SortDirection::Desc)
                .await
                .unwrap();
        let names: Vec<&str> = members.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, ["Banana", "Apple", "Cherry"]);
    }

    #[tokio::test]
    async fn test_unordered_collection_added_and_year_sorts() {
        let (db, _t) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let library = LibraryRepository::create(conn, "Lib", "/lib", ScanningStrategy::Default)
            .await
            .unwrap();

        let mut by_name = HashMap::new();
        for name in ["Banana", "Cherry", "Apple"] {
            let s = SeriesRepository::create(conn, library.id, name, None)
                .await
                .unwrap();
            by_name.insert(name, s);
        }
        let coll = CollectionRepository::create(conn, "Coll", None, false, None)
            .await
            .unwrap();
        for name in ["Banana", "Cherry", "Apple"] {
            CollectionRepository::add_series(conn, coll.id, by_name[name].id)
                .await
                .unwrap();
        }

        // "added" follows insertion order, not title order.
        let members = CollectionRepository::get_series(
            conn,
            &coll,
            None,
            Some(CollectionSeriesSort::Added),
            SortDirection::Asc,
        )
        .await
        .unwrap();
        let names: Vec<&str> = members.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, ["Banana", "Cherry", "Apple"]);

        // "year" sorts by release year ascending, unknown years last.
        SeriesMetadataRepository::update_year(conn, by_name["Banana"].id, Some(2020))
            .await
            .unwrap();
        SeriesMetadataRepository::update_year(conn, by_name["Cherry"].id, Some(1999))
            .await
            .unwrap();
        let members = CollectionRepository::get_series(
            conn,
            &coll,
            None,
            Some(CollectionSeriesSort::Year),
            SortDirection::Asc,
        )
        .await
        .unwrap();
        let names: Vec<&str> = members.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, ["Cherry", "Banana", "Apple"]);

        // Descending year reverses the dated members; unknown years stay last.
        let members = CollectionRepository::get_series(
            conn,
            &coll,
            None,
            Some(CollectionSeriesSort::Year),
            SortDirection::Desc,
        )
        .await
        .unwrap();
        let names: Vec<&str> = members.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, ["Banana", "Cherry", "Apple"]);
    }

    #[tokio::test]
    async fn test_all_member_series_ids() {
        let (db, _t) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let (_lib, series) = lib_and_series(conn).await;

        // No collections yet => empty set.
        let members = CollectionRepository::all_member_series_ids(conn)
            .await
            .unwrap();
        assert!(members.is_empty());

        // Two collections, with one series shared between them.
        let coll_a = CollectionRepository::create(conn, "A", None, false, None)
            .await
            .unwrap();
        let coll_b = CollectionRepository::create(conn, "B", None, false, None)
            .await
            .unwrap();
        CollectionRepository::add_series(conn, coll_a.id, series[0].id)
            .await
            .unwrap();
        CollectionRepository::add_series(conn, coll_a.id, series[1].id)
            .await
            .unwrap();
        // series[1] also belongs to B => must be de-duplicated.
        CollectionRepository::add_series(conn, coll_b.id, series[1].id)
            .await
            .unwrap();

        let members = CollectionRepository::all_member_series_ids(conn)
            .await
            .unwrap();
        assert_eq!(members.len(), 2);
        assert!(members.contains(&series[0].id));
        assert!(members.contains(&series[1].id));
        // series[2] is in no collection.
        assert!(!members.contains(&series[2].id));
    }

    #[tokio::test]
    async fn test_visibility_filtering_and_containers() {
        let (db, _t) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let (_lib, series) = lib_and_series(conn).await;

        let coll = CollectionRepository::create(conn, "Coll", None, false, None)
            .await
            .unwrap();
        for s in &series {
            CollectionRepository::add_series(conn, coll.id, s.id)
                .await
                .unwrap();
        }

        // Exclude the middle series for this viewer.
        let vis = SeriesVisibility {
            excluded_series_ids: vec![series[1].id],
            allowed_series_ids: None,
        };
        let visible =
            CollectionRepository::get_series(conn, &coll, Some(&vis), None, SortDirection::Asc)
                .await
                .unwrap();
        assert_eq!(visible.len(), 2);
        assert!(visible.iter().all(|s| s.id != series[1].id));
        assert_eq!(
            CollectionRepository::count_series(conn, coll.id, Some(&vis))
                .await
                .unwrap(),
            2
        );

        // Empty whitelist => nothing visible.
        let empty = SeriesVisibility {
            excluded_series_ids: vec![],
            allowed_series_ids: Some(vec![]),
        };
        assert!(
            CollectionRepository::get_series(conn, &coll, Some(&empty), None, SortDirection::Asc)
                .await
                .unwrap()
                .is_empty()
        );

        // Containers-for-series lookup.
        let containers = CollectionRepository::get_collections_for_series(conn, series[0].id)
            .await
            .unwrap();
        assert_eq!(containers.len(), 1);
        assert_eq!(containers[0].id, coll.id);
    }

    #[tokio::test]
    async fn test_get_collections_for_series_ids_batched() {
        let (db, _t) = create_test_db().await;
        let conn = db.sea_orm_connection();
        let (_lib, series) = lib_and_series(conn).await;

        let zeta = CollectionRepository::create(conn, "Zeta", None, false, None)
            .await
            .unwrap();
        let alpha = CollectionRepository::create(conn, "Alpha Picks", None, false, None)
            .await
            .unwrap();

        // series[0] in both, series[1] in one, series[2] in none.
        for (coll_id, series_id) in [
            (zeta.id, series[0].id),
            (alpha.id, series[0].id),
            (zeta.id, series[1].id),
        ] {
            CollectionRepository::add_series(conn, coll_id, series_id)
                .await
                .unwrap();
        }

        let ids: Vec<Uuid> = series.iter().map(|s| s.id).collect();
        let map = CollectionRepository::get_collections_for_series_ids(conn, &ids)
            .await
            .unwrap();

        // Memberships come back name-sorted per series.
        let names: Vec<&str> = map[&series[0].id].iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, vec!["Alpha Picks", "Zeta"]);
        assert_eq!(map[&series[1].id].len(), 1);
        assert_eq!(map[&series[1].id][0].id, zeta.id);
        // A series with no membership is absent from the map.
        assert!(!map.contains_key(&series[2].id));

        // Empty input short-circuits to an empty map.
        let empty = CollectionRepository::get_collections_for_series_ids(conn, &[])
            .await
            .unwrap();
        assert!(empty.is_empty());
    }
}
