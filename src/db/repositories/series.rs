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

use crate::api::dto::series::{SeriesSortField, SeriesSortParam, SortDirection};
use crate::db::entities::{books, prelude::*, read_progress, series, series_metadata};
use crate::events::{EntityChangeEvent, EntityEvent, EventBroadcaster};
use std::sync::Arc;

/// Result type for series with aggregated data (used for date_read sorting)
#[derive(Debug, FromQueryResult)]
pub struct SeriesWithAggregates {
    pub id: Uuid,
    pub library_id: Uuid,
    pub fingerprint: Option<String>,
    pub path: Option<String>,
    pub custom_metadata: Option<String>,
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
            custom_metadata: s.custom_metadata,
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

    /// Create a new series
    pub async fn create(
        db: &DatabaseConnection,
        library_id: Uuid,
        name: &str,
        event_broadcaster: Option<&Arc<EventBroadcaster>>,
    ) -> Result<series::Model> {
        Self::create_with_fingerprint(db, library_id, name, None, None, event_broadcaster).await
    }

    /// Create a new series with optional fingerprint
    /// Also creates the corresponding series_metadata record
    pub async fn create_with_fingerprint(
        db: &DatabaseConnection,
        library_id: Uuid,
        name: &str,
        fingerprint: Option<String>,
        path: Option<String>,
        event_broadcaster: Option<&Arc<EventBroadcaster>>,
    ) -> Result<series::Model> {
        let now = Utc::now();
        let series_id = Uuid::new_v4();

        let series = series::ActiveModel {
            id: Set(series_id),
            library_id: Set(library_id),
            fingerprint: Set(fingerprint),
            path: Set(path),
            custom_metadata: Set(None),
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
            custom_metadata: Set(series_model.custom_metadata.clone()),
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
        use crate::api::permissions::ADMIN_PERMISSIONS;
        use crate::db::entities::users;
        use crate::db::repositories::{ReadProgressRepository, UserRepository};
        use crate::utils::password;

        let password_hash = password::hash_password("test123").unwrap();
        let permissions_vec: Vec<_> = ADMIN_PERMISSIONS.iter().cloned().collect();
        let user = users::Model {
            id: Uuid::new_v4(),
            username: "testuser".to_string(),
            email: "test@example.com".to_string(),
            password_hash,
            is_admin: false,
            is_active: true,
            email_verified: true,
            permissions: serde_json::to_value(&permissions_vec).unwrap(),
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
}
