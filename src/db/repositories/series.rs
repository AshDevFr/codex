//! Repository for series operations
//!
//! TODO: Remove allow(dead_code) once all series features are fully integrated

#![allow(dead_code)]

use anyhow::{Context, Result};
use chrono::Utc;
use sea_orm::{
    sea_query::{Alias, Expr, Func},
    ActiveModelTrait, ColumnTrait, Condition, DatabaseConnection, EntityTrait, FromQueryResult,
    JoinType, Order, PaginatorTrait, QueryFilter, QueryOrder, QuerySelect, RelationTrait, Set,
};
use uuid::Uuid;

use crate::api::routes::v1::dto::series::{SeriesSortField, SeriesSortParam, SortDirection};
use crate::db::entities::{books, prelude::*, read_progress, series, series_metadata};
use crate::events::{EntityChangeEvent, EntityEvent, EventBroadcaster};
use std::sync::Arc;

/// Result type for series with aggregated data (used for date_read sorting)
#[derive(Debug, FromQueryResult)]
pub struct SeriesWithAggregates {
    pub id: Uuid,
    pub library_id: Uuid,
    pub fingerprint: Option<String>,
    pub path: String,
    pub name: String,
    pub normalized_name: String,
    pub created_at: chrono::DateTime<Utc>,
    pub updated_at: chrono::DateTime<Utc>,
    /// Aggregated field for date_read sort - used for SQL ORDER BY mapping
    #[allow(dead_code)]
    pub last_read_at: Option<chrono::DateTime<Utc>>,
}

impl From<SeriesWithAggregates> for series::Model {
    fn from(s: SeriesWithAggregates) -> Self {
        series::Model {
            id: s.id,
            library_id: s.library_id,
            fingerprint: s.fingerprint,
            path: s.path,
            name: s.name,
            normalized_name: s.normalized_name,
            created_at: s.created_at,
            updated_at: s.updated_at,
        }
    }
}

/// Repository for Series operations
pub struct SeriesRepository;

impl SeriesRepository {
    /// Normalize name for searching (lowercase, alphanumeric only)
    pub fn normalize_name(name: &str) -> String {
        name.to_lowercase()
            .chars()
            .filter(|c| c.is_alphanumeric() || c.is_whitespace())
            .collect::<String>()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Create a new series with a default path derived from the name
    /// For production use, prefer `create_with_fingerprint` which takes an explicit path
    pub async fn create(
        db: &DatabaseConnection,
        library_id: Uuid,
        name: &str,
        event_broadcaster: Option<&Arc<EventBroadcaster>>,
    ) -> Result<series::Model> {
        // Use name as path for backwards compatibility in tests
        Self::create_with_fingerprint(
            db,
            library_id,
            name,
            None,
            name.to_string(),
            event_broadcaster,
        )
        .await
    }

    /// Create a new series with optional fingerprint and required path
    /// Also creates the corresponding series_metadata record
    pub async fn create_with_fingerprint(
        db: &DatabaseConnection,
        library_id: Uuid,
        name: &str,
        fingerprint: Option<String>,
        path: String,
        event_broadcaster: Option<&Arc<EventBroadcaster>>,
    ) -> Result<series::Model> {
        let now = Utc::now();
        let series_id = Uuid::new_v4();
        let normalized_name = Self::normalize_name(name);

        let series = series::ActiveModel {
            id: Set(series_id),
            library_id: Set(library_id),
            fingerprint: Set(fingerprint),
            path: Set(path),
            name: Set(name.to_string()),
            normalized_name: Set(normalized_name),
            created_at: Set(now),
            updated_at: Set(now),
        };

        let created_series = series.insert(db).await.context("Failed to create series")?;

        // Create the corresponding series_metadata record
        let metadata = series_metadata::ActiveModel {
            series_id: Set(series_id),
            title: Set(name.to_string()),
            title_sort: Set(None),
            summary: Set(None),
            publisher: Set(None),
            imprint: Set(None),
            status: Set(Some("ongoing".to_string())),
            age_rating: Set(None),
            language: Set(None),
            reading_direction: Set(None),
            year: Set(None),
            total_book_count: Set(None),
            custom_metadata: Set(None),
            // Lock fields default to false
            total_book_count_lock: Set(false),
            title_lock: Set(false),
            title_sort_lock: Set(false),
            summary_lock: Set(false),
            publisher_lock: Set(false),
            imprint_lock: Set(false),
            status_lock: Set(false),
            age_rating_lock: Set(false),
            language_lock: Set(false),
            reading_direction_lock: Set(false),
            year_lock: Set(false),
            genres_lock: Set(false),
            tags_lock: Set(false),
            custom_metadata_lock: Set(false),
            created_at: Set(now),
            updated_at: Set(now),
        };
        metadata
            .insert(db)
            .await
            .context("Failed to create series metadata")?;

        // Emit SeriesCreated event if broadcaster is available
        if let Some(broadcaster) = event_broadcaster {
            let event = EntityChangeEvent::new(
                EntityEvent::SeriesCreated {
                    series_id: created_series.id,
                    library_id,
                },
                None, // System-triggered, no user_id
            );
            let _ = broadcaster.emit(event);
        }

        Ok(created_series)
    }

    /// Get a series by ID
    pub async fn get_by_id(db: &DatabaseConnection, id: Uuid) -> Result<Option<series::Model>> {
        Series::find_by_id(id)
            .one(db)
            .await
            .context("Failed to get series by ID")
    }

    /// Check if a series exists by ID (more efficient than get_by_id for existence checks)
    pub async fn exists(db: &DatabaseConnection, id: Uuid) -> Result<bool> {
        let count = Series::find_by_id(id)
            .count(db)
            .await
            .context("Failed to check series existence")?;
        Ok(count > 0)
    }

    /// Get existing series IDs from a list of candidates (batch existence check)
    ///
    /// Returns only the IDs that exist in the database. This is much more efficient
    /// than calling `exists()` for each ID individually.
    pub async fn get_existing_ids(
        db: &DatabaseConnection,
        ids: &[Uuid],
    ) -> Result<std::collections::HashSet<Uuid>> {
        use std::collections::HashSet;

        if ids.is_empty() {
            return Ok(HashSet::new());
        }

        let existing: Vec<Uuid> = Series::find()
            .filter(series::Column::Id.is_in(ids.to_vec()))
            .select_only()
            .column(series::Column::Id)
            .into_tuple()
            .all(db)
            .await
            .context("Failed to get existing series IDs")?;

        Ok(existing.into_iter().collect())
    }

    /// Get series with its metadata
    pub async fn get_with_metadata(
        db: &DatabaseConnection,
        id: Uuid,
    ) -> Result<Option<(series::Model, Option<series_metadata::Model>)>> {
        let series = Series::find_by_id(id).one(db).await?;

        if let Some(s) = series {
            let metadata = SeriesMetadata::find_by_id(id).one(db).await?;
            Ok(Some((s, metadata)))
        } else {
            Ok(None)
        }
    }

    /// Get all series in a library
    pub async fn list_by_library(
        db: &DatabaseConnection,
        library_id: Uuid,
    ) -> Result<Vec<series::Model>> {
        // Join with series_metadata to sort by title_sort/title
        Series::find()
            .filter(series::Column::LibraryId.eq(library_id))
            .join(JoinType::LeftJoin, series::Relation::SeriesMetadata.def())
            .order_by_asc(series_metadata::Column::TitleSort)
            .order_by_asc(series_metadata::Column::Title)
            .all(db)
            .await
            .context("Failed to list series by library")
    }

    /// Count series in a library
    pub async fn count_by_library(db: &DatabaseConnection, library_id: Uuid) -> Result<i64> {
        use sea_orm::PaginatorTrait;

        let count = Series::find()
            .filter(series::Column::LibraryId.eq(library_id))
            .paginate(db, 1)
            .num_items()
            .await
            .context("Failed to count series")?;

        Ok(count as i64)
    }

    /// Get all series across all libraries
    pub async fn list_all(db: &DatabaseConnection) -> Result<Vec<series::Model>> {
        Series::find()
            .join(JoinType::LeftJoin, series::Relation::SeriesMetadata.def())
            .order_by_asc(series_metadata::Column::TitleSort)
            .order_by_asc(series_metadata::Column::Title)
            .all(db)
            .await
            .context("Failed to list all series")
    }

    /// List recently added series (ordered by created_at descending)
    pub async fn list_recently_added(
        db: &DatabaseConnection,
        library_id: Option<Uuid>,
        limit: u64,
    ) -> Result<Vec<series::Model>> {
        let mut query = Series::find();

        if let Some(lib_id) = library_id {
            query = query.filter(series::Column::LibraryId.eq(lib_id));
        }

        query
            .order_by_desc(series::Column::CreatedAt)
            .limit(limit)
            .all(db)
            .await
            .context("Failed to list recently added series")
    }

    /// List recently updated series (ordered by updated_at descending)
    pub async fn list_recently_updated(
        db: &DatabaseConnection,
        library_id: Option<Uuid>,
        limit: u64,
    ) -> Result<Vec<series::Model>> {
        let mut query = Series::find();

        if let Some(lib_id) = library_id {
            query = query.filter(series::Column::LibraryId.eq(lib_id));
        }

        query
            .order_by_desc(series::Column::UpdatedAt)
            .limit(limit)
            .all(db)
            .await
            .context("Failed to list recently updated series")
    }

    /// Get series in a library with sorting, pagination, and optional user context
    ///
    /// This method handles all sort strategies including:
    /// - Simple sorts: name, date_added, date_updated, book_count
    /// - User-specific sorts: date_read (requires user_id and JOIN with read_progress)
    /// - release_date queries series_metadata.year
    pub async fn list_by_library_sorted(
        db: &DatabaseConnection,
        library_id: Uuid,
        sort: &SeriesSortParam,
        user_id: Option<Uuid>,
        offset: u64,
        limit: u64,
    ) -> Result<Vec<series::Model>> {
        let order = match sort.direction {
            SortDirection::Asc => Order::Asc,
            SortDirection::Desc => Order::Desc,
        };

        match sort.field {
            SeriesSortField::DateRead => {
                Self::list_with_date_read_sort(db, library_id, sort, user_id, offset, limit).await
            }
            SeriesSortField::ReleaseDate => {
                // Sort by year from series_metadata
                Series::find()
                    .filter(series::Column::LibraryId.eq(library_id))
                    .join(JoinType::LeftJoin, series::Relation::SeriesMetadata.def())
                    .order_by(series_metadata::Column::Year, order)
                    .offset(offset)
                    .limit(limit)
                    .all(db)
                    .await
                    .context("Failed to list series with release date sort")
            }
            SeriesSortField::BookCount => {
                // Dynamic book count - join with books and count
                // For now, order by series.id as a fallback (book count will be computed dynamically)
                // TODO: Consider adding a computed/virtual column or subquery for book count sorting
                Series::find()
                    .filter(series::Column::LibraryId.eq(library_id))
                    .order_by(series::Column::CreatedAt, order)
                    .offset(offset)
                    .limit(limit)
                    .all(db)
                    .await
                    .context("Failed to list series with book count sort")
            }
            _ => {
                // Simple sorts that may use metadata for name sort
                let query = Series::find().filter(series::Column::LibraryId.eq(library_id));

                // Apply sort
                let query = match sort.field {
                    SeriesSortField::Name => {
                        // Sort by title_sort first (if set), then title from metadata
                        query
                            .join(JoinType::LeftJoin, series::Relation::SeriesMetadata.def())
                            .order_by(series_metadata::Column::TitleSort, order.clone())
                            .order_by(series_metadata::Column::Title, order)
                    }
                    SeriesSortField::DateAdded => query.order_by(series::Column::CreatedAt, order),
                    SeriesSortField::DateUpdated => {
                        query.order_by(series::Column::UpdatedAt, order)
                    }
                    _ => query, // Handled above (DateRead, ReleaseDate, BookCount)
                };

                query
                    .offset(offset)
                    .limit(limit)
                    .all(db)
                    .await
                    .context("Failed to list series with sort")
            }
        }
    }

    /// List series by IDs with sorting (database-level)
    ///
    /// This method is used when filtering has already been done (e.g., by content filter,
    /// genre filter, tag filter) and we have a set of IDs to fetch with proper sorting.
    /// This avoids the broken in-memory sorting pattern.
    pub async fn list_by_ids_sorted(
        db: &DatabaseConnection,
        ids: &[Uuid],
        sort: &SeriesSortParam,
        user_id: Option<Uuid>,
        offset: u64,
        limit: u64,
    ) -> Result<(Vec<series::Model>, u64)> {
        if ids.is_empty() {
            return Ok((vec![], 0));
        }

        let total = ids.len() as u64;

        let order = match sort.direction {
            SortDirection::Asc => Order::Asc,
            SortDirection::Desc => Order::Desc,
        };

        let base_condition = series::Column::Id.is_in(ids.to_vec());

        let series = match sort.field {
            SeriesSortField::Name => {
                // Sort by title_sort first (if set), then title from metadata
                Series::find()
                    .filter(base_condition)
                    .join(JoinType::LeftJoin, series::Relation::SeriesMetadata.def())
                    .order_by(series_metadata::Column::TitleSort, order.clone())
                    .order_by(series_metadata::Column::Title, order)
                    .offset(offset)
                    .limit(limit)
                    .all(db)
                    .await
                    .context("Failed to list series by IDs with name sort")?
            }
            SeriesSortField::DateAdded => Series::find()
                .filter(base_condition)
                .order_by(series::Column::CreatedAt, order)
                .offset(offset)
                .limit(limit)
                .all(db)
                .await
                .context("Failed to list series by IDs with date added sort")?,
            SeriesSortField::DateUpdated => Series::find()
                .filter(base_condition)
                .order_by(series::Column::UpdatedAt, order)
                .offset(offset)
                .limit(limit)
                .all(db)
                .await
                .context("Failed to list series by IDs with date updated sort")?,
            SeriesSortField::ReleaseDate => {
                // Sort by year from series_metadata
                Series::find()
                    .filter(base_condition)
                    .join(JoinType::LeftJoin, series::Relation::SeriesMetadata.def())
                    .order_by(series_metadata::Column::Year, order)
                    .offset(offset)
                    .limit(limit)
                    .all(db)
                    .await
                    .context("Failed to list series by IDs with release date sort")?
            }
            SeriesSortField::BookCount => {
                // TODO: Implement proper book count sorting with subquery
                // For now, fall back to created_at
                Series::find()
                    .filter(base_condition)
                    .order_by(series::Column::CreatedAt, order)
                    .offset(offset)
                    .limit(limit)
                    .all(db)
                    .await
                    .context("Failed to list series by IDs with book count sort")?
            }
            SeriesSortField::DateRead => {
                // User-specific sort - requires user_id
                Self::list_by_ids_with_date_read_sort(db, ids, &order, user_id, offset, limit)
                    .await?
            }
        };

        Ok((series, total))
    }

    /// Helper for list_by_ids_sorted with DateRead sort
    async fn list_by_ids_with_date_read_sort(
        db: &DatabaseConnection,
        ids: &[Uuid],
        order: &Order,
        user_id: Option<Uuid>,
        offset: u64,
        limit: u64,
    ) -> Result<Vec<series::Model>> {
        use sea_orm::sea_query::Expr;

        let base_condition = series::Column::Id.is_in(ids.to_vec());

        let mut query = Series::find()
            .filter(base_condition)
            .join(JoinType::LeftJoin, series::Relation::Books.def())
            .join(JoinType::LeftJoin, books::Relation::ReadProgress.def());

        // Filter by user if provided
        if let Some(uid) = user_id {
            query = query.filter(
                Condition::any()
                    .add(read_progress::Column::UserId.eq(uid))
                    .add(read_progress::Column::UserId.is_null()),
            );
        }

        // Group by series and order by max read_at
        query
            .column_as(
                Expr::col((read_progress::Entity, read_progress::Column::UpdatedAt)).max(),
                "last_read_at",
            )
            .group_by(series::Column::Id)
            .order_by(Expr::col(Alias::new("last_read_at")), order.clone())
            .offset(offset)
            .limit(limit)
            .all(db)
            .await
            .context("Failed to list series by IDs with date read sort")
    }

    /// List series sorted by last read date (user-specific)
    async fn list_with_date_read_sort(
        db: &DatabaseConnection,
        library_id: Uuid,
        sort: &SeriesSortParam,
        user_id: Option<Uuid>,
        offset: u64,
        limit: u64,
    ) -> Result<Vec<series::Model>> {
        use sea_orm::sea_query::Expr;

        let order = match sort.direction {
            SortDirection::Asc => Order::Asc,
            SortDirection::Desc => Order::Desc,
        };

        let mut query = Series::find()
            .filter(series::Column::LibraryId.eq(library_id))
            .join(JoinType::LeftJoin, series::Relation::Books.def())
            .join(JoinType::LeftJoin, books::Relation::ReadProgress.def());

        // Filter by user if provided
        if let Some(uid) = user_id {
            query = query.filter(
                read_progress::Column::UserId
                    .eq(uid)
                    .or(read_progress::Column::UserId.is_null()),
            );
        }

        let results = query
            .column_as(
                Expr::col((
                    Alias::new("read_progress"),
                    read_progress::Column::UpdatedAt,
                ))
                .max(),
                "last_read_at",
            )
            .group_by(series::Column::Id)
            .order_by(Expr::col(Alias::new("last_read_at")), order)
            .offset(offset)
            .limit(limit)
            .into_model::<SeriesWithAggregates>()
            .all(db)
            .await
            .context("Failed to list series with date read sort")?;

        Ok(results.into_iter().map(Into::into).collect())
    }

    /// Get series with in-progress books (series that have at least one book with reading progress that is not completed)
    pub async fn list_in_progress(
        db: &DatabaseConnection,
        user_id: Uuid,
        library_id: Option<Uuid>,
    ) -> Result<Vec<series::Model>> {
        use crate::db::entities::{books, read_progress};
        use sea_orm::JoinType;

        let mut query = Series::find()
            .join(JoinType::InnerJoin, series::Relation::Books.def())
            .join(JoinType::InnerJoin, books::Relation::ReadProgress.def())
            .filter(read_progress::Column::UserId.eq(user_id))
            .filter(read_progress::Column::Completed.eq(false)); // Only in-progress books

        // Filter by library if specified
        if let Some(lib_id) = library_id {
            query = query.filter(series::Column::LibraryId.eq(lib_id));
        }

        // Join with metadata for sorting, group by series to avoid duplicates
        query
            .join(JoinType::LeftJoin, series::Relation::SeriesMetadata.def())
            .group_by(series::Column::Id)
            .order_by_asc(series_metadata::Column::TitleSort)
            .order_by_asc(series_metadata::Column::Title)
            .all(db)
            .await
            .context("Failed to list in-progress series")
    }

    /// Search series by title (case-insensitive via series_metadata)
    pub async fn search_by_name(
        db: &DatabaseConnection,
        query: &str,
    ) -> Result<Vec<series::Model>> {
        let pattern = format!("%{}%", query.to_lowercase());

        // Use LOWER(title) LIKE pattern from series_metadata for case-insensitive search
        let lower_title = Func::lower(Expr::col((
            series_metadata::Entity,
            series_metadata::Column::Title,
        )));
        let search_condition = Condition::all().add(Expr::expr(lower_title).like(&pattern));

        Series::find()
            .join(JoinType::LeftJoin, series::Relation::SeriesMetadata.def())
            .filter(search_condition)
            .order_by_asc(series_metadata::Column::Title)
            .limit(50)
            .all(db)
            .await
            .context("Failed to search series by name")
    }

    /// Full-text search series by title (case-insensitive using LOWER())
    pub async fn full_text_search(
        db: &DatabaseConnection,
        query: &str,
    ) -> Result<Vec<series::Model>> {
        let pattern = format!("%{}%", query.to_lowercase());

        // Use LOWER(title) LIKE pattern from series_metadata for case-insensitive search
        let lower_title = Func::lower(Expr::col((
            series_metadata::Entity,
            series_metadata::Column::Title,
        )));
        let search_condition = Condition::all().add(Expr::expr(lower_title).like(&pattern));

        Series::find()
            .join(JoinType::LeftJoin, series::Relation::SeriesMetadata.def())
            .filter(search_condition)
            .order_by_asc(series_metadata::Column::Title)
            .all(db)
            .await
            .context("Failed to execute full-text search")
    }

    /// Full-text search series by title within candidate IDs (case-insensitive)
    pub async fn full_text_search_filtered(
        db: &DatabaseConnection,
        query: &str,
        candidate_ids: &[Uuid],
    ) -> Result<Vec<series::Model>> {
        if candidate_ids.is_empty() {
            return Ok(vec![]);
        }

        let pattern = format!("%{}%", query.to_lowercase());

        // Use LOWER(title) LIKE pattern from series_metadata for case-insensitive search
        let lower_title = Func::lower(Expr::col((
            series_metadata::Entity,
            series_metadata::Column::Title,
        )));
        let search_condition = Condition::all()
            .add(Expr::expr(lower_title).like(&pattern))
            .add(series::Column::Id.is_in(candidate_ids.to_vec()));

        Series::find()
            .join(JoinType::LeftJoin, series::Relation::SeriesMetadata.def())
            .filter(search_condition)
            .order_by_asc(series_metadata::Column::Title)
            .all(db)
            .await
            .context("Failed to execute full-text search")
    }

    /// Update series core fields
    pub async fn update(
        db: &DatabaseConnection,
        series_model: &series::Model,
        event_broadcaster: Option<&Arc<EventBroadcaster>>,
    ) -> Result<()> {
        let active = series::ActiveModel {
            id: Set(series_model.id),
            library_id: Set(series_model.library_id),
            fingerprint: Set(series_model.fingerprint.clone()),
            path: Set(series_model.path.clone()),
            name: Set(series_model.name.clone()),
            normalized_name: Set(series_model.normalized_name.clone()),
            created_at: Set(series_model.created_at),
            updated_at: Set(Utc::now()),
        };

        active.update(db).await.context("Failed to update series")?;

        // Emit SeriesUpdated event if broadcaster is available
        if let Some(broadcaster) = event_broadcaster {
            let event = EntityChangeEvent::new(
                EntityEvent::SeriesUpdated {
                    series_id: series_model.id,
                    library_id: series_model.library_id,
                    fields: None, // Could track specific fields if needed
                },
                None, // System-triggered, no user_id
            );
            let _ = broadcaster.emit(event);
        }

        Ok(())
    }

    /// Update series name/title (updates series_metadata.title)
    /// Note: This now updates the title in series_metadata, not the series table
    pub async fn update_name(db: &DatabaseConnection, id: Uuid, name: &str) -> Result<()> {
        use crate::db::repositories::SeriesMetadataRepository;

        // Update the title in series_metadata
        SeriesMetadataRepository::update_title(db, id, name.to_string(), None).await?;

        // Also update the series updated_at timestamp
        let series = Series::find_by_id(id)
            .one(db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Series not found"))?;

        let mut active: series::ActiveModel = series.into();
        active.updated_at = Set(Utc::now());

        active
            .update(db)
            .await
            .context("Failed to update series timestamp")?;

        Ok(())
    }

    /// Touch series to update updated_at timestamp (used for cache busting after cover changes)
    pub async fn touch(db: &DatabaseConnection, id: Uuid) -> Result<()> {
        let series = Series::find_by_id(id)
            .one(db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Series not found"))?;

        let mut active: series::ActiveModel = series.into();
        active.updated_at = Set(Utc::now());

        active
            .update(db)
            .await
            .context("Failed to touch series timestamp")?;

        Ok(())
    }

    /// Update series fingerprint
    pub async fn update_fingerprint(
        db: &DatabaseConnection,
        id: Uuid,
        fingerprint: Option<String>,
    ) -> Result<()> {
        let series = Series::find_by_id(id)
            .one(db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Series not found"))?;

        let mut active: series::ActiveModel = series.into();
        active.fingerprint = Set(fingerprint);
        active.updated_at = Set(Utc::now());

        active
            .update(db)
            .await
            .context("Failed to update series fingerprint")?;

        Ok(())
    }

    /// Find a series by library_id and path (primary matching key)
    /// Used for step 1 of the deduplication strategy: same directory = same series
    pub async fn find_by_path(
        db: &DatabaseConnection,
        library_id: Uuid,
        path: &str,
    ) -> Result<Option<series::Model>> {
        Series::find()
            .filter(series::Column::LibraryId.eq(library_id))
            .filter(series::Column::Path.eq(path))
            .one(db)
            .await
            .context("Failed to find series by path")
    }

    /// Find a series by fingerprint within a library
    /// Used for step 2 of the deduplication strategy: directory renamed, same files
    pub async fn find_by_fingerprint(
        db: &DatabaseConnection,
        library_id: Uuid,
        fingerprint: &str,
    ) -> Result<Option<series::Model>> {
        Series::find()
            .filter(series::Column::LibraryId.eq(library_id))
            .filter(series::Column::Fingerprint.eq(fingerprint))
            .one(db)
            .await
            .context("Failed to find series by fingerprint")
    }

    /// Find a series by library_id and normalized_name
    /// Used for step 3 of the deduplication strategy: last resort fallback
    pub async fn find_by_normalized_name(
        db: &DatabaseConnection,
        library_id: Uuid,
        normalized_name: &str,
    ) -> Result<Option<series::Model>> {
        Series::find()
            .filter(series::Column::LibraryId.eq(library_id))
            .filter(series::Column::NormalizedName.eq(normalized_name))
            .one(db)
            .await
            .context("Failed to find series by normalized name")
    }

    /// Update series path (when directory is moved but fingerprint matches)
    pub async fn update_path(db: &DatabaseConnection, id: Uuid, path: String) -> Result<()> {
        let series = Series::find_by_id(id)
            .one(db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Series not found"))?;

        let mut active: series::ActiveModel = series.into();
        active.path = Set(path);
        active.updated_at = Set(Utc::now());

        active
            .update(db)
            .await
            .context("Failed to update series path")?;

        Ok(())
    }

    /// Update series fingerprint and name when matched by path
    /// Used when files change in the directory but path stays the same
    pub async fn update_fingerprint_and_name(
        db: &DatabaseConnection,
        id: Uuid,
        fingerprint: Option<String>,
        name: &str,
    ) -> Result<()> {
        let series = Series::find_by_id(id)
            .one(db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Series not found"))?;

        let normalized_name = Self::normalize_name(name);
        let mut active: series::ActiveModel = series.into();
        active.fingerprint = Set(fingerprint);
        active.name = Set(name.to_string());
        active.normalized_name = Set(normalized_name);
        active.updated_at = Set(Utc::now());

        active
            .update(db)
            .await
            .context("Failed to update series fingerprint and name")?;

        Ok(())
    }

    /// Update series path and name when matched by fingerprint
    /// Used when directory is renamed but files stay the same
    pub async fn update_path_and_name(
        db: &DatabaseConnection,
        id: Uuid,
        path: String,
        name: &str,
    ) -> Result<()> {
        let series = Series::find_by_id(id)
            .one(db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Series not found"))?;

        let normalized_name = Self::normalize_name(name);
        let mut active: series::ActiveModel = series.into();
        active.path = Set(path);
        active.name = Set(name.to_string());
        active.normalized_name = Set(normalized_name);
        active.updated_at = Set(Utc::now());

        active
            .update(db)
            .await
            .context("Failed to update series path and name")?;

        Ok(())
    }

    /// Update series fingerprint and path when matched by normalized_name
    /// Used as fallback when directory is moved AND renamed
    pub async fn update_fingerprint_and_path(
        db: &DatabaseConnection,
        id: Uuid,
        fingerprint: Option<String>,
        path: String,
    ) -> Result<()> {
        let series = Series::find_by_id(id)
            .one(db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Series not found"))?;

        let mut active: series::ActiveModel = series.into();
        active.fingerprint = Set(fingerprint);
        active.path = Set(path);
        active.updated_at = Set(Utc::now());

        active
            .update(db)
            .await
            .context("Failed to update series fingerprint and path")?;

        Ok(())
    }

    /// Count books in a series (computed dynamically, not stored on series)
    pub async fn get_book_count(db: &DatabaseConnection, id: Uuid) -> Result<i64> {
        let count = books::Entity::find()
            .filter(books::Column::SeriesId.eq(id))
            .filter(books::Column::Deleted.eq(false))
            .count(db)
            .await
            .context("Failed to count books in series")?;

        Ok(count as i64)
    }

    /// Delete a series
    pub async fn delete(db: &DatabaseConnection, id: Uuid) -> Result<()> {
        Series::delete_by_id(id)
            .exec(db)
            .await
            .context("Failed to delete series")?;
        Ok(())
    }

    /// Find and delete all series in a library that have no books
    pub async fn purge_empty_series_in_library(
        db: &DatabaseConnection,
        library_id: Uuid,
        event_broadcaster: Option<&Arc<crate::events::EventBroadcaster>>,
    ) -> Result<u64> {
        use crate::db::entities::{books, prelude::*};

        // Find all series in the library
        let all_series = Series::find()
            .filter(series::Column::LibraryId.eq(library_id))
            .all(db)
            .await
            .context("Failed to find series in library")?;

        let mut deleted_count = 0u64;

        // Check each series and delete if empty
        for series_model in all_series {
            let book_count = books::Entity::find()
                .filter(books::Column::SeriesId.eq(series_model.id))
                .count(db)
                .await
                .context("Failed to count books in series")?;

            if book_count == 0 {
                let series_id = series_model.id;

                Series::delete_by_id(series_id)
                    .exec(db)
                    .await
                    .context(format!("Failed to delete empty series {}", series_id))?;
                deleted_count += 1;

                // Emit SeriesDeleted event
                if let Some(broadcaster) = event_broadcaster {
                    use crate::events::{EntityChangeEvent, EntityEvent};
                    use tracing::warn;

                    let event = EntityChangeEvent {
                        event: EntityEvent::SeriesDeleted {
                            library_id,
                            series_id,
                        },
                        user_id: None,
                        timestamp: chrono::Utc::now(),
                    };

                    if let Err(e) = broadcaster.emit(event) {
                        warn!(
                            "Failed to emit SeriesDeleted event for series {}: {:?}",
                            series_id, e
                        );
                    }
                }
            }
        }

        Ok(deleted_count)
    }

    /// Check if a series has any books and delete it if empty
    pub async fn purge_if_empty(
        db: &DatabaseConnection,
        series_id: Uuid,
        event_broadcaster: Option<&Arc<crate::events::EventBroadcaster>>,
    ) -> Result<bool> {
        use crate::db::entities::books;

        // First get series info for library_id before deletion
        let series = Self::get_by_id(db, series_id)
            .await?
            .context("Series not found")?;
        let library_id = series.library_id;

        // Check if series has any books
        let book_count = books::Entity::find()
            .filter(books::Column::SeriesId.eq(series_id))
            .count(db)
            .await
            .context("Failed to count books in series")?;

        if book_count == 0 {
            // Series is empty, delete it
            Series::delete_by_id(series_id)
                .exec(db)
                .await
                .context("Failed to delete empty series")?;

            // Emit SeriesDeleted event
            if let Some(broadcaster) = event_broadcaster {
                use crate::events::{EntityChangeEvent, EntityEvent};
                use tracing::warn;

                let event = EntityChangeEvent {
                    event: EntityEvent::SeriesDeleted {
                        library_id,
                        series_id,
                    },
                    user_id: None,
                    timestamp: chrono::Utc::now(),
                };

                if let Err(e) = broadcaster.emit(event) {
                    warn!(
                        "Failed to emit SeriesDeleted event for series {}: {:?}",
                        series_id, e
                    );
                }
            }

            Ok(true)
        } else {
            Ok(false)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::entities::books;
    use crate::db::repositories::{BookRepository, LibraryRepository};
    use crate::db::test_helpers::create_test_db;
    use crate::db::ScanningStrategy;

    #[tokio::test]
    async fn test_create_series() {
        let (db, _temp_dir) = create_test_db().await;

        let library = LibraryRepository::create(
            db.sea_orm_connection(),
            "Test Library",
            "/test/path",
            ScanningStrategy::Default,
        )
        .await
        .unwrap();

        let series =
            SeriesRepository::create(db.sea_orm_connection(), library.id, "Test Series", None)
                .await
                .unwrap();

        assert_eq!(series.library_id, library.id);

        // Title is now in series_metadata
        let metadata = crate::db::repositories::SeriesMetadataRepository::get_by_series_id(
            db.sea_orm_connection(),
            series.id,
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(metadata.title, "Test Series");
    }

    #[tokio::test]
    async fn test_get_series_by_id() {
        let (db, _temp_dir) = create_test_db().await;

        let library = LibraryRepository::create(
            db.sea_orm_connection(),
            "Test Library",
            "/test/path",
            ScanningStrategy::Default,
        )
        .await
        .unwrap();

        let created =
            SeriesRepository::create(db.sea_orm_connection(), library.id, "Test Series", None)
                .await
                .unwrap();

        let retrieved = SeriesRepository::get_by_id(db.sea_orm_connection(), created.id)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(retrieved.id, created.id);
        assert_eq!(retrieved.library_id, library.id);
    }

    #[tokio::test]
    async fn test_list_series_by_library() {
        let (db, _temp_dir) = create_test_db().await;

        let library = LibraryRepository::create(
            db.sea_orm_connection(),
            "Test Library",
            "/test/path",
            ScanningStrategy::Default,
        )
        .await
        .unwrap();

        SeriesRepository::create(db.sea_orm_connection(), library.id, "Series 1", None)
            .await
            .unwrap();
        SeriesRepository::create(db.sea_orm_connection(), library.id, "Series 2", None)
            .await
            .unwrap();

        let series = SeriesRepository::list_by_library(db.sea_orm_connection(), library.id)
            .await
            .unwrap();

        assert_eq!(series.len(), 2);
    }

    #[tokio::test]
    async fn test_search_series_by_name() {
        let (db, _temp_dir) = create_test_db().await;

        let library = LibraryRepository::create(
            db.sea_orm_connection(),
            "Test Library",
            "/test/path",
            ScanningStrategy::Default,
        )
        .await
        .unwrap();

        SeriesRepository::create(db.sea_orm_connection(), library.id, "One Piece", None)
            .await
            .unwrap();
        SeriesRepository::create(db.sea_orm_connection(), library.id, "Naruto", None)
            .await
            .unwrap();

        let results = SeriesRepository::search_by_name(db.sea_orm_connection(), "piece")
            .await
            .unwrap();

        assert_eq!(results.len(), 1);
        // Verify the result by checking metadata
        let metadata = crate::db::repositories::SeriesMetadataRepository::get_by_series_id(
            db.sea_orm_connection(),
            results[0].id,
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(metadata.title, "One Piece");
    }

    #[tokio::test]
    async fn test_update_series() {
        let (db, _temp_dir) = create_test_db().await;

        let library = LibraryRepository::create(
            db.sea_orm_connection(),
            "Test Library",
            "/test/path",
            ScanningStrategy::Default,
        )
        .await
        .unwrap();

        let series =
            SeriesRepository::create(db.sea_orm_connection(), library.id, "Original Name", None)
                .await
                .unwrap();

        // Update name via update_name (which updates series_metadata.title)
        SeriesRepository::update_name(db.sea_orm_connection(), series.id, "Updated Name")
            .await
            .unwrap();

        // Verify metadata was updated
        let metadata = crate::db::repositories::SeriesMetadataRepository::get_by_series_id(
            db.sea_orm_connection(),
            series.id,
        )
        .await
        .unwrap()
        .unwrap();

        assert_eq!(metadata.title, "Updated Name");
    }

    #[tokio::test]
    async fn test_get_book_count() {
        let (db, _temp_dir) = create_test_db().await;

        let library = LibraryRepository::create(
            db.sea_orm_connection(),
            "Test Library",
            "/test/path",
            ScanningStrategy::Default,
        )
        .await
        .unwrap();

        let series =
            SeriesRepository::create(db.sea_orm_connection(), library.id, "Test Series", None)
                .await
                .unwrap();

        // Initial count should be 0
        let count = SeriesRepository::get_book_count(db.sea_orm_connection(), series.id)
            .await
            .unwrap();
        assert_eq!(count, 0);

        // Add a book
        let book = books::Model {
            id: Uuid::new_v4(),
            series_id: series.id,
            library_id: library.id,
            file_path: "/test/book1.cbz".to_string(),
            file_name: "book1.cbz".to_string(),
            file_size: 1024,
            file_hash: format!("hash_{}", Uuid::new_v4()),
            partial_hash: String::new(),
            format: "cbz".to_string(),
            page_count: 10,
            deleted: false,
            analyzed: false,
            analysis_error: None,
            analysis_errors: None,
            modified_at: Utc::now(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            thumbnail_path: None,
            thumbnail_generated_at: None,
        };
        BookRepository::create(db.sea_orm_connection(), &book, None)
            .await
            .unwrap();

        // Count should now be 1
        let count = SeriesRepository::get_book_count(db.sea_orm_connection(), series.id)
            .await
            .unwrap();
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn test_delete_series() {
        let (db, _temp_dir) = create_test_db().await;

        let library = LibraryRepository::create(
            db.sea_orm_connection(),
            "Test Library",
            "/test/path",
            ScanningStrategy::Default,
        )
        .await
        .unwrap();

        let series =
            SeriesRepository::create(db.sea_orm_connection(), library.id, "To Delete", None)
                .await
                .unwrap();

        SeriesRepository::delete(db.sea_orm_connection(), series.id)
            .await
            .unwrap();

        let result = SeriesRepository::get_by_id(db.sea_orm_connection(), series.id)
            .await
            .unwrap();

        assert!(result.is_none());
    }

    // Note: Tests for reading_direction, custom_cover_path, and selected_cover_source
    // have been removed as these fields are now in series_metadata and series_covers tables.
    // See the respective repository tests for these features.

    #[tokio::test]
    async fn test_list_in_progress() {
        let (db, _temp_dir) = create_test_db().await;

        // Create library
        let library = LibraryRepository::create(
            db.sea_orm_connection(),
            "Test Library",
            "/test",
            ScanningStrategy::Default,
        )
        .await
        .unwrap();

        // Create multiple series
        let series1 =
            SeriesRepository::create(db.sea_orm_connection(), library.id, "Series 1", None)
                .await
                .unwrap();
        let series2 =
            SeriesRepository::create(db.sea_orm_connection(), library.id, "Series 2", None)
                .await
                .unwrap();
        let series3 =
            SeriesRepository::create(db.sea_orm_connection(), library.id, "Series 3", None)
                .await
                .unwrap();

        // Create books in each series
        let book1 = books::Model {
            id: Uuid::new_v4(),
            series_id: series1.id,
            library_id: library.id,
            file_path: "/test/book1.cbz".to_string(),
            file_name: "book1.cbz".to_string(),
            file_size: 1024,
            file_hash: format!("hash_{}", Uuid::new_v4()),
            partial_hash: String::new(),
            format: "cbz".to_string(),
            page_count: 10,
            deleted: false,
            analyzed: false,
            analysis_error: None,
            analysis_errors: None,
            modified_at: Utc::now(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            thumbnail_path: None,
            thumbnail_generated_at: None,
        };
        let book1: books::Model = BookRepository::create(db.sea_orm_connection(), &book1, None)
            .await
            .unwrap();

        let book2 = books::Model {
            id: Uuid::new_v4(),
            series_id: series2.id,
            library_id: library.id,
            file_path: "/test/book2.cbz".to_string(),
            file_name: "book2.cbz".to_string(),
            file_size: 1024,
            file_hash: format!("hash_{}", Uuid::new_v4()),
            partial_hash: String::new(),
            format: "cbz".to_string(),
            page_count: 10,
            deleted: false,
            analyzed: false,
            analysis_error: None,
            analysis_errors: None,
            modified_at: Utc::now(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            thumbnail_path: None,
            thumbnail_generated_at: None,
        };
        let book2: books::Model = BookRepository::create(db.sea_orm_connection(), &book2, None)
            .await
            .unwrap();

        let book3 = books::Model {
            id: Uuid::new_v4(),
            series_id: series3.id,
            library_id: library.id,
            file_path: "/test/book3.cbz".to_string(),
            file_name: "book3.cbz".to_string(),
            file_size: 1024,
            file_hash: format!("hash_{}", Uuid::new_v4()),
            partial_hash: String::new(),
            format: "cbz".to_string(),
            page_count: 10,
            deleted: false,
            analyzed: false,
            analysis_error: None,
            analysis_errors: None,
            modified_at: Utc::now(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            thumbnail_path: None,
            thumbnail_generated_at: None,
        };
        let book3: books::Model = BookRepository::create(db.sea_orm_connection(), &book3, None)
            .await
            .unwrap();

        // Create user

        use crate::db::entities::users;
        use crate::db::repositories::{ReadProgressRepository, UserRepository};
        use crate::utils::password;

        let password_hash = password::hash_password("test123").unwrap();
        let user = users::Model {
            id: Uuid::new_v4(),
            username: "testuser".to_string(),
            email: "test@example.com".to_string(),
            password_hash,
            role: "reader".to_string(),
            is_active: true,
            email_verified: true,
            permissions: serde_json::json!([]),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            last_login_at: None,
        };
        let created_user = UserRepository::create(db.sea_orm_connection(), &user)
            .await
            .unwrap();

        // Add reading progress for series1 and series2 (in-progress)
        ReadProgressRepository::upsert(
            db.sea_orm_connection(),
            created_user.id,
            book1.id,
            5,
            false,
        )
        .await
        .unwrap();
        ReadProgressRepository::upsert(
            db.sea_orm_connection(),
            created_user.id,
            book2.id,
            5,
            false,
        )
        .await
        .unwrap();

        // Mark series3 as completed
        ReadProgressRepository::upsert(
            db.sea_orm_connection(),
            created_user.id,
            book3.id,
            10,
            true,
        )
        .await
        .unwrap();

        // Test getting started series (only in-progress, not completed)
        let started =
            SeriesRepository::list_in_progress(db.sea_orm_connection(), created_user.id, None)
                .await
                .unwrap();

        assert_eq!(started.len(), 2); // Only series1 and series2 with in-progress books
        let series_ids: Vec<_> = started.iter().map(|s| s.id).collect();
        assert!(series_ids.contains(&series1.id));
        assert!(series_ids.contains(&series2.id));
        assert!(!series_ids.contains(&series3.id)); // Completed books not included

        // Test filtering by library
        let started = SeriesRepository::list_in_progress(
            db.sea_orm_connection(),
            created_user.id,
            Some(library.id),
        )
        .await
        .unwrap();

        assert_eq!(started.len(), 2);
    }

    #[tokio::test]
    async fn test_find_by_path() {
        let (db, _temp_dir) = create_test_db().await;

        let library = LibraryRepository::create(
            db.sea_orm_connection(),
            "Test Library",
            "/test/path",
            ScanningStrategy::Default,
        )
        .await
        .unwrap();

        // Create a series with a specific path
        let series = SeriesRepository::create_with_fingerprint(
            db.sea_orm_connection(),
            library.id,
            "My Series",
            Some("fingerprint123".to_string()),
            "/test/path/My Series".to_string(),
            None,
        )
        .await
        .unwrap();

        // Find by path - should match
        let found = SeriesRepository::find_by_path(
            db.sea_orm_connection(),
            library.id,
            "/test/path/My Series",
        )
        .await
        .unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().id, series.id);

        // Find by different path - should not match
        let not_found =
            SeriesRepository::find_by_path(db.sea_orm_connection(), library.id, "/test/path/Other")
                .await
                .unwrap();
        assert!(not_found.is_none());

        // Find in different library - should not match
        let other_library = LibraryRepository::create(
            db.sea_orm_connection(),
            "Other Library",
            "/other/path",
            ScanningStrategy::Default,
        )
        .await
        .unwrap();

        let not_found = SeriesRepository::find_by_path(
            db.sea_orm_connection(),
            other_library.id,
            "/test/path/My Series",
        )
        .await
        .unwrap();
        assert!(not_found.is_none());
    }

    #[tokio::test]
    async fn test_find_by_fingerprint() {
        let (db, _temp_dir) = create_test_db().await;

        let library = LibraryRepository::create(
            db.sea_orm_connection(),
            "Test Library",
            "/test/path",
            ScanningStrategy::Default,
        )
        .await
        .unwrap();

        // Create a series with a fingerprint
        let series = SeriesRepository::create_with_fingerprint(
            db.sea_orm_connection(),
            library.id,
            "My Series",
            Some("unique_fingerprint_abc".to_string()),
            "/test/path/My Series".to_string(),
            None,
        )
        .await
        .unwrap();

        // Find by fingerprint - should match
        let found = SeriesRepository::find_by_fingerprint(
            db.sea_orm_connection(),
            library.id,
            "unique_fingerprint_abc",
        )
        .await
        .unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().id, series.id);

        // Find by different fingerprint - should not match
        let not_found = SeriesRepository::find_by_fingerprint(
            db.sea_orm_connection(),
            library.id,
            "different_fingerprint",
        )
        .await
        .unwrap();
        assert!(not_found.is_none());
    }

    #[tokio::test]
    async fn test_find_by_normalized_name() {
        let (db, _temp_dir) = create_test_db().await;

        let library = LibraryRepository::create(
            db.sea_orm_connection(),
            "Test Library",
            "/test/path",
            ScanningStrategy::Default,
        )
        .await
        .unwrap();

        // Create a series - normalized name will be "one piece" (lowercase, alphanumeric)
        let series = SeriesRepository::create_with_fingerprint(
            db.sea_orm_connection(),
            library.id,
            "One Piece",
            Some("fp123".to_string()),
            "/test/path/One Piece".to_string(),
            None,
        )
        .await
        .unwrap();

        // Find by normalized name - should match
        let found = SeriesRepository::find_by_normalized_name(
            db.sea_orm_connection(),
            library.id,
            "one piece",
        )
        .await
        .unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().id, series.id);

        // Find by different normalized name - should not match
        let not_found = SeriesRepository::find_by_normalized_name(
            db.sea_orm_connection(),
            library.id,
            "two piece",
        )
        .await
        .unwrap();
        assert!(not_found.is_none());
    }

    #[tokio::test]
    async fn test_update_path() {
        let (db, _temp_dir) = create_test_db().await;

        let library = LibraryRepository::create(
            db.sea_orm_connection(),
            "Test Library",
            "/test/path",
            ScanningStrategy::Default,
        )
        .await
        .unwrap();

        let series = SeriesRepository::create_with_fingerprint(
            db.sea_orm_connection(),
            library.id,
            "My Series",
            Some("fp".to_string()),
            "/old/path".to_string(),
            None,
        )
        .await
        .unwrap();

        // Update path
        SeriesRepository::update_path(db.sea_orm_connection(), series.id, "/new/path".to_string())
            .await
            .unwrap();

        // Verify path was updated
        let updated = SeriesRepository::get_by_id(db.sea_orm_connection(), series.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(updated.path, "/new/path");
    }

    #[tokio::test]
    async fn test_update_fingerprint_and_name() {
        let (db, _temp_dir) = create_test_db().await;

        let library = LibraryRepository::create(
            db.sea_orm_connection(),
            "Test Library",
            "/test/path",
            ScanningStrategy::Default,
        )
        .await
        .unwrap();

        let series = SeriesRepository::create_with_fingerprint(
            db.sea_orm_connection(),
            library.id,
            "Original Name",
            Some("old_fp".to_string()),
            "/test/path/series".to_string(),
            None,
        )
        .await
        .unwrap();

        // Update fingerprint and name
        SeriesRepository::update_fingerprint_and_name(
            db.sea_orm_connection(),
            series.id,
            Some("new_fp".to_string()),
            "New Name",
        )
        .await
        .unwrap();

        // Verify updates
        let updated = SeriesRepository::get_by_id(db.sea_orm_connection(), series.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(updated.fingerprint, Some("new_fp".to_string()));
        assert_eq!(updated.name, "New Name");
        assert_eq!(updated.normalized_name, "new name");
    }

    #[tokio::test]
    async fn test_update_path_and_name() {
        let (db, _temp_dir) = create_test_db().await;

        let library = LibraryRepository::create(
            db.sea_orm_connection(),
            "Test Library",
            "/test/path",
            ScanningStrategy::Default,
        )
        .await
        .unwrap();

        let series = SeriesRepository::create_with_fingerprint(
            db.sea_orm_connection(),
            library.id,
            "Original Name",
            Some("fp".to_string()),
            "/old/path".to_string(),
            None,
        )
        .await
        .unwrap();

        // Update path and name
        SeriesRepository::update_path_and_name(
            db.sea_orm_connection(),
            series.id,
            "/new/path".to_string(),
            "Renamed Series",
        )
        .await
        .unwrap();

        // Verify updates
        let updated = SeriesRepository::get_by_id(db.sea_orm_connection(), series.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(updated.path, "/new/path");
        assert_eq!(updated.name, "Renamed Series");
        assert_eq!(updated.normalized_name, "renamed series");
    }

    #[tokio::test]
    async fn test_update_fingerprint_and_path() {
        let (db, _temp_dir) = create_test_db().await;

        let library = LibraryRepository::create(
            db.sea_orm_connection(),
            "Test Library",
            "/test/path",
            ScanningStrategy::Default,
        )
        .await
        .unwrap();

        let series = SeriesRepository::create_with_fingerprint(
            db.sea_orm_connection(),
            library.id,
            "My Series",
            Some("old_fp".to_string()),
            "/old/path".to_string(),
            None,
        )
        .await
        .unwrap();

        // Update fingerprint and path
        SeriesRepository::update_fingerprint_and_path(
            db.sea_orm_connection(),
            series.id,
            Some("new_fp".to_string()),
            "/new/path".to_string(),
        )
        .await
        .unwrap();

        // Verify updates
        let updated = SeriesRepository::get_by_id(db.sea_orm_connection(), series.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(updated.fingerprint, Some("new_fp".to_string()));
        assert_eq!(updated.path, "/new/path");
        // Name should remain unchanged
        assert_eq!(updated.name, "My Series");
    }

    #[tokio::test]
    async fn test_normalize_name() {
        // Test various normalization cases
        assert_eq!(SeriesRepository::normalize_name("One Piece"), "one piece");
        assert_eq!(
            SeriesRepository::normalize_name("  Multiple   Spaces  "),
            "multiple spaces"
        );
        assert_eq!(
            SeriesRepository::normalize_name("Special!@#$Characters"),
            "specialcharacters"
        );
        assert_eq!(SeriesRepository::normalize_name("UPPERCASE"), "uppercase");
        assert_eq!(
            SeriesRepository::normalize_name("MixedCase123"),
            "mixedcase123"
        );
    }

    #[tokio::test]
    async fn test_get_existing_ids() {
        let (db, _temp_dir) = create_test_db().await;

        let library = LibraryRepository::create(
            db.sea_orm_connection(),
            "Test Library",
            "/test/path",
            ScanningStrategy::Default,
        )
        .await
        .unwrap();

        // Create two series
        let series1 =
            SeriesRepository::create(db.sea_orm_connection(), library.id, "Series 1", None)
                .await
                .unwrap();
        let series2 =
            SeriesRepository::create(db.sea_orm_connection(), library.id, "Series 2", None)
                .await
                .unwrap();

        // Create a non-existent ID
        let non_existent_id = Uuid::new_v4();

        // Test batch lookup
        let ids_to_check = vec![series1.id, series2.id, non_existent_id];
        let existing = SeriesRepository::get_existing_ids(db.sea_orm_connection(), &ids_to_check)
            .await
            .unwrap();

        // Should contain the two existing series but not the non-existent one
        assert_eq!(existing.len(), 2);
        assert!(existing.contains(&series1.id));
        assert!(existing.contains(&series2.id));
        assert!(!existing.contains(&non_existent_id));

        // Test with empty input
        let existing = SeriesRepository::get_existing_ids(db.sea_orm_connection(), &[])
            .await
            .unwrap();
        assert!(existing.is_empty());
    }
}
