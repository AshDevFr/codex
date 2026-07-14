//! Repository for collections and the collection_series junction.
//!
//! Collections are shared, named groupings of series. Membership order is held
//! by the `position` column on the junction; it is honored only when the
//! collection's `ordered` flag is set. Unordered collections sort by the
//! displayed series title by default (see [`CollectionRepository::get_series`]).

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
use codex_models::sort::CollectionSeriesSort;

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
    pub async fn create(
        db: &DatabaseConnection,
        name: &str,
        ordered: bool,
    ) -> Result<collections::Model> {
        let now = Utc::now();
        let model = collections::ActiveModel {
            id: Set(Uuid::new_v4()),
            name: Set(name.trim().to_string()),
            normalized_name: Set(name.trim().to_lowercase()),
            ordered: Set(ordered),
            created_at: Set(now),
            updated_at: Set(now),
        };
        Ok(model.insert(db).await?)
    }

    /// Update a collection's name and/or ordered flag. Returns `None` if the
    /// collection does not exist.
    pub async fn update(
        db: &DatabaseConnection,
        id: Uuid,
        name: Option<&str>,
        ordered: Option<bool>,
    ) -> Result<Option<collections::Model>> {
        let Some(existing) = Collections::find_by_id(id).one(db).await? else {
            return Ok(None);
        };
        let mut active = existing.into_active_model();
        if let Some(name) = name {
            active.name = Set(name.trim().to_string());
            active.normalized_name = Set(name.trim().to_lowercase());
        }
        if let Some(ordered) = ordered {
            active.ordered = Set(ordered);
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

    /// Add a series to a collection at the end of the order. Idempotent: if the
    /// series is already a member, returns the existing link unchanged.
    pub async fn add_series(
        db: &DatabaseConnection,
        collection_id: Uuid,
        series_id: Uuid,
    ) -> Result<collection_series::Model> {
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
    pub async fn remove_series(
        db: &DatabaseConnection,
        collection_id: Uuid,
        series_id: Uuid,
    ) -> Result<bool> {
        let result = CollectionSeries::delete_many()
            .filter(collection_series::Column::CollectionId.eq(collection_id))
            .filter(collection_series::Column::SeriesId.eq(series_id))
            .exec(db)
            .await?;
        Ok(result.rows_affected > 0)
    }

    /// Set explicit positions for the given series in the order provided. Series
    /// not currently members are skipped.
    pub async fn reorder(
        db: &DatabaseConnection,
        collection_id: Uuid,
        ordered_series_ids: &[Uuid],
    ) -> Result<()> {
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
    /// Ordered collections always return manual order (position, then insertion
    /// time); `sort` is ignored for them. Unordered collections honor `sort`,
    /// defaulting to the displayed title (metadata `title_sort`, falling back
    /// to `title`, then the scan-derived series name).
    pub async fn get_series(
        db: &DatabaseConnection,
        collection: &collections::Model,
        vis: Option<&SeriesVisibility>,
        sort: Option<CollectionSeriesSort>,
    ) -> Result<Vec<series::Model>> {
        if matches!(vis, Some(v) if v.is_empty_whitelist()) {
            return Ok(vec![]);
        }

        // Manual order wins on ordered collections.
        let sort = (!collection.ordered).then_some(sort.unwrap_or_default());

        let mut junction = CollectionSeries::find()
            .filter(collection_series::Column::CollectionId.eq(collection.id));
        junction = match sort {
            None => junction
                .order_by_asc(collection_series::Column::Position)
                .order_by_asc(collection_series::Column::CreatedAt),
            Some(CollectionSeriesSort::Added) => junction
                .order_by_asc(collection_series::Column::CreatedAt)
                .order_by_asc(collection_series::Column::Position),
            // Title/year order lives on the series side; junction order is
            // irrelevant for those.
            Some(_) => junction,
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
            Some(CollectionSeriesSort::Title) | Some(CollectionSeriesSort::Year) => {
                let title_expr = Expr::expr(Func::coalesce([
                    Expr::col((series_metadata::Entity, series_metadata::Column::TitleSort)).into(),
                    Expr::col((series_metadata::Entity, series_metadata::Column::Title)).into(),
                    Expr::col((series::Entity, series::Column::Name)).into(),
                ]));
                let mut query = Series::find()
                    .filter(series::Column::Id.is_in(ordered_ids))
                    .join(JoinType::LeftJoin, series::Relation::SeriesMetadata.def());
                if matches!(sort, Some(CollectionSeriesSort::Year)) {
                    query = query.order_by_with_nulls(
                        series_metadata::Column::Year,
                        Order::Asc,
                        NullOrdering::Last,
                    );
                }
                Ok(query
                    .order_by(title_expr, Order::Asc)
                    .order_by(series::Column::Id, Order::Asc)
                    .all(db)
                    .await?)
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
            .order_by_asc(collections::Column::Name)
            .all(db)
            .await?)
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
    use codex_models::sort::CollectionSeriesSort;

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

        let coll = CollectionRepository::create(conn, "  Batman  ", false)
            .await
            .unwrap();
        assert_eq!(coll.name, "Batman");
        assert_eq!(coll.normalized_name, "batman");
        assert!(!coll.ordered);

        let found = CollectionRepository::get_by_name(conn, "BATMAN")
            .await
            .unwrap();
        assert_eq!(found.unwrap().id, coll.id);

        let updated = CollectionRepository::update(conn, coll.id, Some("Dark Knight"), Some(true))
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

        let coll = CollectionRepository::create(conn, "Coll", true)
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

        let members = CollectionRepository::get_series(conn, &coll, None, None)
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
        let members = CollectionRepository::get_series(conn, &coll, None, None)
            .await
            .unwrap();
        assert_eq!(members[0].id, series[2].id);
        assert_eq!(members[2].id, series[0].id);

        // A sort param must not override manual order on an ordered collection.
        let members =
            CollectionRepository::get_series(conn, &coll, None, Some(CollectionSeriesSort::Title))
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

        let coll = CollectionRepository::create(conn, "Coll", false)
            .await
            .unwrap();
        for name in ["Banana", "Cherry", "Apple"] {
            CollectionRepository::add_series(conn, coll.id, by_name[name].id)
                .await
                .unwrap();
        }

        // Default sort for an unordered collection is by title.
        let members = CollectionRepository::get_series(conn, &coll, None, None)
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
        let members = CollectionRepository::get_series(conn, &coll, None, None)
            .await
            .unwrap();
        let names: Vec<&str> = members.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, ["Cherry", "Apple", "Banana"]);
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
        let coll = CollectionRepository::create(conn, "Coll", false)
            .await
            .unwrap();
        for name in ["Banana", "Cherry", "Apple"] {
            CollectionRepository::add_series(conn, coll.id, by_name[name].id)
                .await
                .unwrap();
        }

        // "added" follows insertion order, not title order.
        let members =
            CollectionRepository::get_series(conn, &coll, None, Some(CollectionSeriesSort::Added))
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
        let members =
            CollectionRepository::get_series(conn, &coll, None, Some(CollectionSeriesSort::Year))
                .await
                .unwrap();
        let names: Vec<&str> = members.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, ["Cherry", "Banana", "Apple"]);
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
        let coll_a = CollectionRepository::create(conn, "A", false)
            .await
            .unwrap();
        let coll_b = CollectionRepository::create(conn, "B", false)
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

        let coll = CollectionRepository::create(conn, "Coll", false)
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
        let visible = CollectionRepository::get_series(conn, &coll, Some(&vis), None)
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
            CollectionRepository::get_series(conn, &coll, Some(&empty), None)
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
}
