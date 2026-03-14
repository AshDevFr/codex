//! Repository for series operations
//!
//! TODO: Remove allow(dead_code) once all series features are fully integrated

#![allow(dead_code)]

use anyhow::{Context, Result};
use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Condition, DatabaseConnection, EntityTrait, FromQueryResult,
    JoinType, Order, PaginatorTrait, QueryFilter, QueryOrder, QuerySelect, RelationTrait, Set,
    sea_query::{Alias, Expr, Func, IntoCondition, NullOrdering, SimpleExpr},
};
use uuid::Uuid;

use crate::api::routes::v1::dto::series::{SeriesSortField, SeriesSortParam, SortDirection};
use crate::db::entities::{
    books, prelude::*, read_progress, series, series_external_ratings, series_metadata,
    user_series_ratings,
};
use crate::events::{EntityChangeEvent, EntityEvent, EventBroadcaster};
use crate::utils::normalize_for_search;
use std::sync::Arc;

/// Options for querying series with filtering, sorting, and pagination
#[derive(Debug, Clone, Default)]
pub struct SeriesQueryOptions<'a> {
    /// Filter by library ID
    pub library_id: Option<Uuid>,
    /// User ID for user-specific sorts (date_read)
    pub user_id: Option<Uuid>,
    /// Text search query (searches title)
    pub search: Option<&'a str>,
    /// Sort field and direction
    pub sort: Option<SeriesQuerySort>,
    /// Page offset (0-indexed)
    pub page: u64,
    /// Page size
    pub page_size: u64,
}

/// Sort configuration for series queries
#[derive(Debug, Clone, Copy)]
pub struct SeriesQuerySort {
    pub field: SeriesSortFieldRepo,
    pub ascending: bool,
}

/// Sort field options for series queries (repository-level)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeriesSortFieldRepo {
    /// Sort by title (title_sort then title from metadata)
    Title,
    /// Sort by date added (created_at)
    DateAdded,
    /// Sort by date updated (updated_at)
    DateUpdated,
    /// Sort by release date (year from metadata)
    ReleaseDate,
    /// Sort by last read date (requires user_id)
    DateRead,
    /// Sort by number of books in the series
    BookCount,
    /// Sort by user rating (requires user_id)
    Rating,
    /// Sort by community average rating
    CommunityRating,
    /// Sort by external rating (highest external source rating)
    ExternalRating,
}

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

/// Result type for series with book count aggregate (used for book_count sorting)
#[derive(Debug, FromQueryResult)]
pub struct SeriesWithBookCount {
    pub id: Uuid,
    pub library_id: Uuid,
    pub fingerprint: Option<String>,
    pub path: String,
    pub name: String,
    pub normalized_name: String,
    pub created_at: chrono::DateTime<Utc>,
    pub updated_at: chrono::DateTime<Utc>,
    /// Aggregated book count - used for SQL ORDER BY mapping
    #[allow(dead_code)]
    pub book_count: i64,
}

impl From<SeriesWithBookCount> for series::Model {
    fn from(s: SeriesWithBookCount) -> Self {
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

/// Result type for series with rating aggregate (used for rating sorting)
#[derive(Debug, FromQueryResult)]
pub struct SeriesWithRating {
    pub id: Uuid,
    pub library_id: Uuid,
    pub fingerprint: Option<String>,
    pub path: String,
    pub name: String,
    pub normalized_name: String,
    pub created_at: chrono::DateTime<Utc>,
    pub updated_at: chrono::DateTime<Utc>,
    /// Aggregated rating value - used for SQL ORDER BY mapping
    #[allow(dead_code)]
    pub sort_rating: Option<f64>,
}

impl From<SeriesWithRating> for series::Model {
    fn from(s: SeriesWithRating) -> Self {
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
    /// Query series with flexible filtering, sorting, and pagination.
    ///
    /// This is the primary composable query method that supports all filtering
    /// and sorting options. Use `SeriesQueryOptions` to configure the query.
    pub async fn query(
        db: &DatabaseConnection,
        options: SeriesQueryOptions<'_>,
    ) -> Result<(Vec<series::Model>, u64)> {
        let order = options
            .sort
            .map(|s| if s.ascending { Order::Asc } else { Order::Desc })
            .unwrap_or(Order::Asc);

        let sort_field = options.sort.map(|s| s.field);

        // Handle DateRead sort separately as it requires special aggregation
        if matches!(sort_field, Some(SeriesSortFieldRepo::DateRead))
            && let Some(user_id) = options.user_id
        {
            return Self::query_with_date_read_sort(db, options, user_id, order).await;
        }

        // Handle BookCount sort separately as it requires GROUP BY aggregation
        if matches!(sort_field, Some(SeriesSortFieldRepo::BookCount)) {
            return Self::query_with_book_count_sort(db, options, order).await;
        }

        // Handle rating sorts separately as they require JOIN + aggregation
        if matches!(
            sort_field,
            Some(SeriesSortFieldRepo::Rating)
                | Some(SeriesSortFieldRepo::CommunityRating)
                | Some(SeriesSortFieldRepo::ExternalRating)
        ) {
            return Self::query_with_rating_sort(db, options, order).await;
        }

        let mut query =
            Series::find().join(JoinType::LeftJoin, series::Relation::SeriesMetadata.def());

        // Apply filters
        if let Some(library_id) = options.library_id {
            query = query.filter(series::Column::LibraryId.eq(library_id));
        }

        // Handle text search
        if let Some(search) = options.search
            && !search.is_empty()
        {
            let pattern = format!("%{}%", search.to_lowercase());
            let lower_title = Func::lower(Expr::col((
                series_metadata::Entity,
                series_metadata::Column::Title,
            )));
            query = query.filter(Expr::expr(lower_title).like(&pattern));
        }

        // Get total count before pagination
        let total = query
            .clone()
            .select_only()
            .column(series::Column::Id)
            .paginate(db, 1)
            .num_items()
            .await
            .context("Failed to count series")?;

        // Apply sorting
        query = match sort_field {
            Some(SeriesSortFieldRepo::Title) | None => {
                // Use COALESCE(title_sort, title) so that series with NULL title_sort
                // are sorted by title rather than clustering at the start/end
                let sort_expr = Func::coalesce([
                    Expr::col((series_metadata::Entity, series_metadata::Column::TitleSort)).into(),
                    Expr::col((series_metadata::Entity, series_metadata::Column::Title)).into(),
                ]);
                query.order_by(Expr::expr(sort_expr), order)
            }
            Some(SeriesSortFieldRepo::DateAdded) => {
                query.order_by(series::Column::CreatedAt, order)
            }
            Some(SeriesSortFieldRepo::DateUpdated) => {
                query.order_by(series::Column::UpdatedAt, order)
            }
            Some(SeriesSortFieldRepo::ReleaseDate) => {
                query.order_by(series_metadata::Column::Year, order)
            }
            Some(SeriesSortFieldRepo::DateRead)
            | Some(SeriesSortFieldRepo::BookCount)
            | Some(SeriesSortFieldRepo::Rating)
            | Some(SeriesSortFieldRepo::CommunityRating)
            | Some(SeriesSortFieldRepo::ExternalRating) => {
                // Fallback: shouldn't reach here, handled above
                query.order_by(series::Column::UpdatedAt, order)
            }
        };

        // Apply pagination
        let series_list = query
            .offset(options.page * options.page_size)
            .limit(options.page_size)
            .all(db)
            .await
            .context("Failed to query series")?;

        Ok((series_list, total))
    }

    /// Query series with DateRead sort (requires user_id for aggregation)
    async fn query_with_date_read_sort(
        db: &DatabaseConnection,
        options: SeriesQueryOptions<'_>,
        user_id: Uuid,
        order: Order,
    ) -> Result<(Vec<series::Model>, u64)> {
        let mut query = Series::find()
            .join(JoinType::LeftJoin, series::Relation::SeriesMetadata.def())
            .join(JoinType::LeftJoin, series::Relation::Books.def())
            .join(
                JoinType::LeftJoin,
                books::Relation::ReadProgress
                    .def()
                    .on_condition(move |_left, right| {
                        Expr::col((right, read_progress::Column::UserId))
                            .eq(user_id)
                            .into_condition()
                    }),
            );

        // Apply filters
        if let Some(library_id) = options.library_id {
            query = query.filter(series::Column::LibraryId.eq(library_id));
        }

        // Handle text search
        if let Some(search) = options.search
            && !search.is_empty()
        {
            let pattern = format!("%{}%", search.to_lowercase());
            let lower_title = Func::lower(Expr::col((
                series_metadata::Entity,
                series_metadata::Column::Title,
            )));
            query = query.filter(Expr::expr(lower_title).like(&pattern));
        }

        // Get total count (before aggregation)
        let count_query =
            Series::find().join(JoinType::LeftJoin, series::Relation::SeriesMetadata.def());

        let mut count_query = if let Some(library_id) = options.library_id {
            count_query.filter(series::Column::LibraryId.eq(library_id))
        } else {
            count_query
        };

        if let Some(search) = options.search
            && !search.is_empty()
        {
            let pattern = format!("%{}%", search.to_lowercase());
            let lower_title = Func::lower(Expr::col((
                series_metadata::Entity,
                series_metadata::Column::Title,
            )));
            count_query = count_query.filter(Expr::expr(lower_title).like(&pattern));
        }

        let total = count_query
            .select_only()
            .column(series::Column::Id)
            .paginate(db, 1)
            .num_items()
            .await
            .context("Failed to count series")?;

        // Add aggregation for date_read sort
        let series_list: Vec<SeriesWithAggregates> = query
            .select_only()
            .column(series::Column::Id)
            .column(series::Column::LibraryId)
            .column(series::Column::Fingerprint)
            .column(series::Column::Path)
            .column(series::Column::Name)
            .column(series::Column::NormalizedName)
            .column(series::Column::CreatedAt)
            .column(series::Column::UpdatedAt)
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
            .offset(options.page * options.page_size)
            .limit(options.page_size)
            .into_model::<SeriesWithAggregates>()
            .all(db)
            .await
            .context("Failed to query series with date_read sort")?;

        // Convert to series::Model
        let series_models: Vec<series::Model> = series_list.into_iter().map(|s| s.into()).collect();

        Ok((series_models, total))
    }

    /// Query series with BookCount sort (requires GROUP BY aggregation)
    async fn query_with_book_count_sort(
        db: &DatabaseConnection,
        options: SeriesQueryOptions<'_>,
        order: Order,
    ) -> Result<(Vec<series::Model>, u64)> {
        let mut query = Series::find()
            .join(JoinType::LeftJoin, series::Relation::SeriesMetadata.def())
            .join(JoinType::LeftJoin, series::Relation::Books.def());

        // Apply filters
        if let Some(library_id) = options.library_id {
            query = query.filter(series::Column::LibraryId.eq(library_id));
        }

        // Handle text search
        if let Some(search) = options.search
            && !search.is_empty()
        {
            let pattern = format!("%{}%", search.to_lowercase());
            let lower_title = Func::lower(Expr::col((
                series_metadata::Entity,
                series_metadata::Column::Title,
            )));
            query = query.filter(Expr::expr(lower_title).like(&pattern));
        }

        // Get total count (before aggregation)
        let count_query =
            Series::find().join(JoinType::LeftJoin, series::Relation::SeriesMetadata.def());

        let mut count_query = if let Some(library_id) = options.library_id {
            count_query.filter(series::Column::LibraryId.eq(library_id))
        } else {
            count_query
        };

        if let Some(search) = options.search
            && !search.is_empty()
        {
            let pattern = format!("%{}%", search.to_lowercase());
            let lower_title = Func::lower(Expr::col((
                series_metadata::Entity,
                series_metadata::Column::Title,
            )));
            count_query = count_query.filter(Expr::expr(lower_title).like(&pattern));
        }

        let total = count_query
            .select_only()
            .column(series::Column::Id)
            .paginate(db, 1)
            .num_items()
            .await
            .context("Failed to count series")?;

        // Add aggregation for book_count sort
        let series_list: Vec<SeriesWithBookCount> = query
            .select_only()
            .column(series::Column::Id)
            .column(series::Column::LibraryId)
            .column(series::Column::Fingerprint)
            .column(series::Column::Path)
            .column(series::Column::Name)
            .column(series::Column::NormalizedName)
            .column(series::Column::CreatedAt)
            .column(series::Column::UpdatedAt)
            .column_as(
                Expr::col((books::Entity, books::Column::Id)).count(),
                "book_count",
            )
            .group_by(series::Column::Id)
            .order_by(Expr::col(Alias::new("book_count")), order)
            .offset(options.page * options.page_size)
            .limit(options.page_size)
            .into_model::<SeriesWithBookCount>()
            .all(db)
            .await
            .context("Failed to query series with book_count sort")?;

        // Convert to series::Model
        let series_models: Vec<series::Model> = series_list.into_iter().map(|s| s.into()).collect();

        Ok((series_models, total))
    }

    /// Query series with rating sort (user rating, community average, or external rating)
    async fn query_with_rating_sort(
        db: &DatabaseConnection,
        options: SeriesQueryOptions<'_>,
        order: Order,
    ) -> Result<(Vec<series::Model>, u64)> {
        let sort_field = options.sort.map(|s| s.field);

        let mut base_query =
            Series::find().join(JoinType::LeftJoin, series::Relation::SeriesMetadata.def());

        // Apply the appropriate rating JOIN based on sort type
        match sort_field {
            Some(SeriesSortFieldRepo::Rating) => {
                if let Some(user_id) = options.user_id {
                    base_query = base_query.join(
                        JoinType::LeftJoin,
                        series::Relation::UserSeriesRatings.def().on_condition(
                            move |_left, right| {
                                Expr::col((right, user_series_ratings::Column::UserId))
                                    .eq(user_id)
                                    .into_condition()
                            },
                        ),
                    );
                } else {
                    base_query = base_query.join(
                        JoinType::LeftJoin,
                        series::Relation::UserSeriesRatings.def(),
                    );
                }
            }
            Some(SeriesSortFieldRepo::CommunityRating) => {
                base_query = base_query.join(
                    JoinType::LeftJoin,
                    series::Relation::UserSeriesRatings.def(),
                );
            }
            Some(SeriesSortFieldRepo::ExternalRating) => {
                base_query = base_query.join(
                    JoinType::LeftJoin,
                    series::Relation::SeriesExternalRatings.def(),
                );
            }
            _ => {}
        }

        // Apply filters
        if let Some(library_id) = options.library_id {
            base_query = base_query.filter(series::Column::LibraryId.eq(library_id));
        }

        if let Some(search) = options.search
            && !search.is_empty()
        {
            let pattern = format!("%{}%", search.to_lowercase());
            let lower_title = Func::lower(Expr::col((
                series_metadata::Entity,
                series_metadata::Column::Title,
            )));
            base_query = base_query.filter(Expr::expr(lower_title).like(&pattern));
        }

        // Get total count (before aggregation)
        let mut count_query =
            Series::find().join(JoinType::LeftJoin, series::Relation::SeriesMetadata.def());
        if let Some(library_id) = options.library_id {
            count_query = count_query.filter(series::Column::LibraryId.eq(library_id));
        }
        if let Some(search) = options.search
            && !search.is_empty()
        {
            let pattern = format!("%{}%", search.to_lowercase());
            let lower_title = Func::lower(Expr::col((
                series_metadata::Entity,
                series_metadata::Column::Title,
            )));
            count_query = count_query.filter(Expr::expr(lower_title).like(&pattern));
        }
        let total = count_query
            .select_only()
            .column(series::Column::Id)
            .paginate(db, 1)
            .num_items()
            .await
            .context("Failed to count series")?;

        // Build sort expression for ratings.
        // - Rating: user's own rating (direct column, JOIN already filters to one row per series)
        // - CommunityRating: AVG of all user ratings
        // - ExternalRating: AVG of external source ratings
        // CAST to DOUBLE PRECISION ensures PostgreSQL compatibility (AVG returns NUMERIC).
        let rating_expr: SimpleExpr = match sort_field {
            Some(SeriesSortFieldRepo::Rating) => {
                // User's own rating - MAX() is a no-op aggregate (JOIN filters to one row
                // per series via user_id) but satisfies PostgreSQL's GROUP BY requirement.
                Expr::expr(Func::max(Expr::col((
                    user_series_ratings::Entity,
                    user_series_ratings::Column::Rating,
                ))))
                .cast_as(Alias::new("DOUBLE PRECISION"))
            }
            Some(SeriesSortFieldRepo::CommunityRating) => {
                // Average of all user ratings
                Expr::expr(Func::avg(Expr::col((
                    user_series_ratings::Entity,
                    user_series_ratings::Column::Rating,
                ))))
                .cast_as(Alias::new("DOUBLE PRECISION"))
            }
            Some(SeriesSortFieldRepo::ExternalRating) => {
                // Average of external source ratings
                Expr::expr(Func::avg(Expr::col((
                    series_external_ratings::Entity,
                    series_external_ratings::Column::Rating,
                ))))
                .cast_as(Alias::new("DOUBLE PRECISION"))
            }
            _ => unreachable!(),
        };

        // Treat NULLs as smallest value (matches SQLite behavior):
        // ASC → NULLS FIRST, DESC → NULLS LAST
        let null_order = if matches!(order, Order::Asc) {
            NullOrdering::First
        } else {
            NullOrdering::Last
        };

        let series_list: Vec<SeriesWithRating> = base_query
            .select_only()
            .column(series::Column::Id)
            .column(series::Column::LibraryId)
            .column(series::Column::Fingerprint)
            .column(series::Column::Path)
            .column(series::Column::Name)
            .column(series::Column::NormalizedName)
            .column(series::Column::CreatedAt)
            .column(series::Column::UpdatedAt)
            .column_as(rating_expr, "sort_rating")
            .group_by(series::Column::Id)
            .order_by_with_nulls(Expr::col(Alias::new("sort_rating")), order, null_order)
            .offset(options.page * options.page_size)
            .limit(options.page_size)
            .into_model::<SeriesWithRating>()
            .all(db)
            .await
            .context("Failed to query series with rating sort")?;

        let series_models: Vec<series::Model> = series_list.into_iter().map(|s| s.into()).collect();

        Ok((series_models, total))
    }

    /// Normalize name for matching (accent-stripped, lowercase, alphanumeric only)
    pub fn normalize_name(name: &str) -> String {
        normalize_for_search(name)
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
    ///
    /// The `metadata_title` parameter allows specifying a preprocessed title for the metadata.
    /// If None, the `name` will be used as the metadata title.
    pub async fn create_with_fingerprint(
        db: &DatabaseConnection,
        library_id: Uuid,
        name: &str,
        fingerprint: Option<String>,
        path: String,
        event_broadcaster: Option<&Arc<EventBroadcaster>>,
    ) -> Result<series::Model> {
        Self::create_with_fingerprint_and_title(
            db,
            library_id,
            name,
            fingerprint,
            path,
            None, // Use name as metadata title
            event_broadcaster,
        )
        .await
    }

    /// Create a new series with optional fingerprint, path, and custom metadata title
    /// Also creates the corresponding series_metadata record
    ///
    /// - `name`: Directory name (preserved for file recognition in `series.name`)
    /// - `metadata_title`: Title for `series_metadata.title` (defaults to `name` if None)
    ///
    /// This allows preprocessing rules to clean the title (e.g., removing "(Digital)" suffix)
    /// while preserving the original directory name for file matching.
    pub async fn create_with_fingerprint_and_title(
        db: &DatabaseConnection,
        library_id: Uuid,
        name: &str,
        fingerprint: Option<String>,
        path: String,
        metadata_title: Option<&str>,
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

        // Use metadata_title if provided, otherwise use name
        let title = metadata_title.unwrap_or(name);

        // Create the corresponding series_metadata record
        let metadata = series_metadata::ActiveModel {
            series_id: Set(series_id),
            title: Set(title.to_string()),
            title_sort: Set(None),
            search_title: Set(normalize_for_search(title)),
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
            authors_json: Set(None),
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
            authors_json_lock: Set(false),
            cover_lock: Set(false),
            alternate_titles_lock: Set(false),
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

    /// Get series by their IDs (simple, no pagination)
    ///
    /// Returns all series matching the given IDs. This is useful for batch operations
    /// where all matching series need to be processed.
    pub async fn get_by_ids(db: &DatabaseConnection, ids: &[Uuid]) -> Result<Vec<series::Model>> {
        if ids.is_empty() {
            return Ok(vec![]);
        }

        Series::find()
            .filter(series::Column::Id.is_in(ids.to_vec()))
            .all(db)
            .await
            .context("Failed to get series by IDs")
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
                let results = Series::find()
                    .filter(series::Column::LibraryId.eq(library_id))
                    .join(JoinType::LeftJoin, series::Relation::Books.def())
                    .column_as(
                        Expr::col((books::Entity, books::Column::Id)).count(),
                        "book_count",
                    )
                    .group_by(series::Column::Id)
                    .order_by(Expr::col(Alias::new("book_count")), order)
                    .offset(offset)
                    .limit(limit)
                    .into_model::<SeriesWithBookCount>()
                    .all(db)
                    .await
                    .context("Failed to list series with book count sort")?;

                Ok(results.into_iter().map(Into::into).collect())
            }
            SeriesSortField::Rating
            | SeriesSortField::CommunityRating
            | SeriesSortField::ExternalRating => {
                let lib_filter = series::Column::LibraryId.eq(library_id);
                let mut query = Series::find().filter(lib_filter);

                // Apply the appropriate rating JOIN
                let rating_expr: SimpleExpr = match sort.field {
                    SeriesSortField::Rating => {
                        if let Some(uid) = user_id {
                            query = query.join(
                                JoinType::LeftJoin,
                                series::Relation::UserSeriesRatings.def().on_condition(
                                    move |_left, right| {
                                        Expr::col((right, user_series_ratings::Column::UserId))
                                            .eq(uid)
                                            .into_condition()
                                    },
                                ),
                            );
                        } else {
                            query = query.join(
                                JoinType::LeftJoin,
                                series::Relation::UserSeriesRatings.def(),
                            );
                        }
                        // User's own rating - MAX() is a no-op aggregate (JOIN filters to one
                        // row per series via user_id) but satisfies PostgreSQL's GROUP BY.
                        Expr::expr(Func::max(Expr::col((
                            user_series_ratings::Entity,
                            user_series_ratings::Column::Rating,
                        ))))
                        .cast_as(Alias::new("DOUBLE PRECISION"))
                    }
                    SeriesSortField::CommunityRating => {
                        query = query.join(
                            JoinType::LeftJoin,
                            series::Relation::UserSeriesRatings.def(),
                        );
                        Expr::expr(Func::avg(Expr::col((
                            user_series_ratings::Entity,
                            user_series_ratings::Column::Rating,
                        ))))
                        .cast_as(Alias::new("DOUBLE PRECISION"))
                    }
                    SeriesSortField::ExternalRating => {
                        query = query.join(
                            JoinType::LeftJoin,
                            series::Relation::SeriesExternalRatings.def(),
                        );
                        Expr::expr(Func::avg(Expr::col((
                            series_external_ratings::Entity,
                            series_external_ratings::Column::Rating,
                        ))))
                        .cast_as(Alias::new("DOUBLE PRECISION"))
                    }
                    _ => unreachable!(),
                };

                // Treat NULLs as smallest value (matches SQLite behavior)
                let null_order = if matches!(order, Order::Asc) {
                    NullOrdering::First
                } else {
                    NullOrdering::Last
                };

                let results: Vec<SeriesWithRating> = query
                    .select_only()
                    .column(series::Column::Id)
                    .column(series::Column::LibraryId)
                    .column(series::Column::Fingerprint)
                    .column(series::Column::Path)
                    .column(series::Column::Name)
                    .column(series::Column::NormalizedName)
                    .column(series::Column::CreatedAt)
                    .column(series::Column::UpdatedAt)
                    .column_as(rating_expr, "sort_rating")
                    .group_by(series::Column::Id)
                    .order_by_with_nulls(Expr::col(Alias::new("sort_rating")), order, null_order)
                    .offset(offset)
                    .limit(limit)
                    .into_model::<SeriesWithRating>()
                    .all(db)
                    .await
                    .context("Failed to list series with rating sort")?;

                Ok(results.into_iter().map(Into::into).collect())
            }
            _ => {
                // Simple sorts that may use metadata for name sort
                let query = Series::find().filter(series::Column::LibraryId.eq(library_id));

                // Apply sort
                let query = match sort.field {
                    SeriesSortField::Name => {
                        // Use COALESCE(title_sort, title) so that series with NULL title_sort
                        // are sorted by title rather than clustering at the start/end
                        let sort_expr = Func::coalesce([
                            Expr::col((
                                series_metadata::Entity,
                                series_metadata::Column::TitleSort,
                            ))
                            .into(),
                            Expr::col((series_metadata::Entity, series_metadata::Column::Title))
                                .into(),
                        ]);
                        query
                            .join(JoinType::LeftJoin, series::Relation::SeriesMetadata.def())
                            .order_by(Expr::expr(sort_expr), order)
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
                // Use COALESCE(title_sort, title) so that series with NULL title_sort
                // are sorted by title rather than clustering at the start/end
                let sort_expr = Func::coalesce([
                    Expr::col((series_metadata::Entity, series_metadata::Column::TitleSort)).into(),
                    Expr::col((series_metadata::Entity, series_metadata::Column::Title)).into(),
                ]);
                Series::find()
                    .filter(base_condition)
                    .join(JoinType::LeftJoin, series::Relation::SeriesMetadata.def())
                    .order_by(Expr::expr(sort_expr), order)
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
                let results = Series::find()
                    .filter(base_condition)
                    .join(JoinType::LeftJoin, series::Relation::Books.def())
                    .column_as(
                        Expr::col((books::Entity, books::Column::Id)).count(),
                        "book_count",
                    )
                    .group_by(series::Column::Id)
                    .order_by(Expr::col(Alias::new("book_count")), order)
                    .offset(offset)
                    .limit(limit)
                    .into_model::<SeriesWithBookCount>()
                    .all(db)
                    .await
                    .context("Failed to list series by IDs with book count sort")?;

                results.into_iter().map(Into::into).collect()
            }
            SeriesSortField::DateRead => {
                // User-specific sort - requires user_id
                Self::list_by_ids_with_date_read_sort(db, ids, &order, user_id, offset, limit)
                    .await?
            }
            SeriesSortField::Rating
            | SeriesSortField::CommunityRating
            | SeriesSortField::ExternalRating => {
                Self::list_by_ids_with_rating_sort(
                    db,
                    ids,
                    &sort.field,
                    &order,
                    user_id,
                    offset,
                    limit,
                )
                .await?
            }
        };

        Ok((series, total))
    }

    /// Helper for list_by_ids_sorted with rating sorts
    async fn list_by_ids_with_rating_sort(
        db: &DatabaseConnection,
        ids: &[Uuid],
        sort_field: &SeriesSortField,
        order: &Order,
        user_id: Option<Uuid>,
        offset: u64,
        limit: u64,
    ) -> Result<Vec<series::Model>> {
        let base_condition = series::Column::Id.is_in(ids.to_vec());

        let mut query = Series::find().filter(base_condition);

        // Apply the appropriate rating JOIN
        match sort_field {
            SeriesSortField::Rating => {
                if let Some(uid) = user_id {
                    query = query.join(
                        JoinType::LeftJoin,
                        series::Relation::UserSeriesRatings.def().on_condition(
                            move |_left, right| {
                                Expr::col((right, user_series_ratings::Column::UserId))
                                    .eq(uid)
                                    .into_condition()
                            },
                        ),
                    );
                } else {
                    query = query.join(
                        JoinType::LeftJoin,
                        series::Relation::UserSeriesRatings.def(),
                    );
                }
            }
            SeriesSortField::CommunityRating => {
                query = query.join(
                    JoinType::LeftJoin,
                    series::Relation::UserSeriesRatings.def(),
                );
            }
            SeriesSortField::ExternalRating => {
                query = query.join(
                    JoinType::LeftJoin,
                    series::Relation::SeriesExternalRatings.def(),
                );
            }
            _ => {}
        }

        // Build sort expression for ratings.
        // - Rating: user's own rating (direct column, JOIN already filters to one row per series)
        // - CommunityRating: AVG of all user ratings
        // - ExternalRating: AVG of external source ratings
        // CAST to DOUBLE PRECISION ensures PostgreSQL compatibility (AVG returns NUMERIC).
        let rating_expr: SimpleExpr = match sort_field {
            SeriesSortField::Rating => {
                // User's own rating - MAX() is a no-op aggregate (JOIN filters to one row
                // per series via user_id) but satisfies PostgreSQL's GROUP BY requirement.
                Expr::expr(Func::max(Expr::col((
                    user_series_ratings::Entity,
                    user_series_ratings::Column::Rating,
                ))))
                .cast_as(Alias::new("DOUBLE PRECISION"))
            }
            SeriesSortField::CommunityRating => Expr::expr(Func::avg(Expr::col((
                user_series_ratings::Entity,
                user_series_ratings::Column::Rating,
            ))))
            .cast_as(Alias::new("DOUBLE PRECISION")),
            SeriesSortField::ExternalRating => Expr::expr(Func::avg(Expr::col((
                series_external_ratings::Entity,
                series_external_ratings::Column::Rating,
            ))))
            .cast_as(Alias::new("DOUBLE PRECISION")),
            _ => unreachable!(),
        };

        let results: Vec<SeriesWithRating> = query
            .select_only()
            .column(series::Column::Id)
            .column(series::Column::LibraryId)
            .column(series::Column::Fingerprint)
            .column(series::Column::Path)
            .column(series::Column::Name)
            .column(series::Column::NormalizedName)
            .column(series::Column::CreatedAt)
            .column(series::Column::UpdatedAt)
            .column_as(rating_expr, "sort_rating")
            .group_by(series::Column::Id)
            // Treat NULLs as smallest value (matches SQLite behavior)
            .order_by_with_nulls(
                Expr::col(Alias::new("sort_rating")),
                order.clone(),
                if matches!(order, Order::Asc) {
                    NullOrdering::First
                } else {
                    NullOrdering::Last
                },
            )
            .offset(offset)
            .limit(limit)
            .into_model::<SeriesWithRating>()
            .all(db)
            .await
            .context("Failed to list series by IDs with rating sort")?;

        Ok(results.into_iter().map(Into::into).collect())
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

    /// Search series by metadata title with optional pagination (case-insensitive)
    ///
    /// Unified search method with optional filters:
    /// - `library_id`: Filter to a specific library (None = all libraries)
    /// - `candidate_ids`: Filter to specific series IDs (None = no ID filter)
    /// - `pagination`: Optional (page, page_size) tuple. If None, returns all results.
    ///
    /// Returns (results, total_count). If pagination is None, total_count equals results.len().
    /// Returns empty vec if candidate_ids is Some but empty.
    pub async fn search_by_title(
        db: &DatabaseConnection,
        query: &str,
        library_id: Option<Uuid>,
        candidate_ids: Option<&[Uuid]>,
        pagination: Option<(u64, u64)>,
    ) -> Result<(Vec<series::Model>, u64)> {
        // Short-circuit if candidate_ids is explicitly empty
        if let Some(ids) = candidate_ids
            && ids.is_empty()
        {
            return Ok((vec![], 0));
        }

        let pattern = format!("%{}%", normalize_for_search(query));

        // Use search_title LIKE pattern for accent-insensitive, case-insensitive search
        let mut search_condition = Condition::all().add(
            Expr::col((
                series_metadata::Entity,
                series_metadata::Column::SearchTitle,
            ))
            .like(&pattern),
        );

        // Add library filter if specified
        if let Some(lib_id) = library_id {
            search_condition = search_condition.add(series::Column::LibraryId.eq(lib_id));
        }

        // Add candidate IDs filter if specified
        if let Some(ids) = candidate_ids {
            search_condition = search_condition.add(series::Column::Id.is_in(ids.to_vec()));
        }

        let base_query = Series::find()
            .join(JoinType::LeftJoin, series::Relation::SeriesMetadata.def())
            .filter(search_condition.clone());

        if let Some((page, page_size)) = pagination {
            // With pagination: count total and fetch page
            let total = Series::find()
                .join(JoinType::LeftJoin, series::Relation::SeriesMetadata.def())
                .filter(search_condition)
                .count(db)
                .await
                .context("Failed to count search results")?;

            let results = base_query
                .order_by_asc(series_metadata::Column::Title)
                .offset(page * page_size)
                .limit(page_size)
                .all(db)
                .await
                .context("Failed to search series by title")?;

            Ok((results, total))
        } else {
            // Without pagination: return all results
            let results = base_query
                .order_by_asc(series_metadata::Column::Title)
                .all(db)
                .await
                .context("Failed to search series by title")?;

            let total = results.len() as u64;
            Ok((results, total))
        }
    }

    /// Search series by title (case-insensitive via series_metadata)
    /// Convenience wrapper for search_by_title with no filters, returns all results
    pub async fn search_by_name(
        db: &DatabaseConnection,
        query: &str,
    ) -> Result<Vec<series::Model>> {
        let (results, _) = Self::search_by_title(db, query, None, None, None).await?;
        Ok(results)
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

    /// Get book counts for multiple series by their IDs
    ///
    /// Returns a HashMap keyed by series_id for efficient lookups
    pub async fn get_book_counts_for_series_ids(
        db: &DatabaseConnection,
        series_ids: &[Uuid],
    ) -> Result<std::collections::HashMap<Uuid, i64>> {
        use sea_orm::{FromQueryResult, QuerySelect, sea_query::Expr};

        if series_ids.is_empty() {
            return Ok(std::collections::HashMap::new());
        }

        #[derive(Debug, FromQueryResult)]
        struct BookCountResult {
            series_id: Uuid,
            count: i64,
        }

        let results: Vec<BookCountResult> = books::Entity::find()
            .select_only()
            .column(books::Column::SeriesId)
            .column_as(Expr::col(books::Column::Id).count(), "count")
            .filter(books::Column::SeriesId.is_in(series_ids.to_vec()))
            .filter(books::Column::Deleted.eq(false))
            .group_by(books::Column::SeriesId)
            .into_model::<BookCountResult>()
            .all(db)
            .await
            .context("Failed to count books for series")?;

        let mut map: std::collections::HashMap<Uuid, i64> = results
            .into_iter()
            .map(|r| (r.series_id, r.count))
            .collect();

        // Fill in zeros for series with no books
        for id in series_ids {
            map.entry(*id).or_insert(0);
        }

        Ok(map)
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

    // =========================================================================
    // Cursor-Based Pagination Methods
    // =========================================================================

    /// List series by library using cursor-based pagination
    ///
    /// This method is more efficient than offset-based pagination for large datasets.
    /// It uses the `(title_sort, id)` tuple as the cursor position, where title_sort
    /// comes from series_metadata.
    ///
    /// # Arguments
    /// * `db` - Database connection
    /// * `library_id` - Library ID to filter by
    /// * `cursor` - Optional cursor from a previous page (title_sort, series_id)
    /// * `page_size` - Number of items to return
    ///
    /// # Returns
    /// * `Vec<series::Model>` - Series for this page (may have page_size + 1 to detect has_more)
    pub async fn list_by_library_cursor(
        db: &DatabaseConnection,
        library_id: Uuid,
        cursor: Option<(&str, Uuid)>,
        page_size: u64,
    ) -> Result<Vec<series::Model>> {
        let mut query = Series::find()
            .filter(series::Column::LibraryId.eq(library_id))
            .join(JoinType::LeftJoin, series::Relation::SeriesMetadata.def());

        // Apply cursor condition if provided
        // We use (title_sort, id) as the cursor tuple
        // Rows after cursor: (title_sort > cursor_title) OR (title_sort = cursor_title AND id > cursor_id)
        if let Some((cursor_title, cursor_id)) = cursor {
            query = query.filter(
                Condition::any()
                    .add(
                        series_metadata::Column::TitleSort.gt(cursor_title).or(
                            series_metadata::Column::TitleSort
                                .is_null()
                                .and(Expr::val(cursor_title).ne("")),
                        ),
                    )
                    .add(
                        Condition::all()
                            .add(
                                series_metadata::Column::TitleSort.eq(cursor_title).or(
                                    series_metadata::Column::TitleSort
                                        .is_null()
                                        .and(Expr::val(cursor_title).eq("")),
                                ),
                            )
                            .add(series::Column::Id.gt(cursor_id)),
                    ),
            );
        }

        // Order by title_sort ASC, then id ASC for stability
        query
            .order_by_asc(series_metadata::Column::TitleSort)
            .order_by_asc(series::Column::Id)
            // Fetch one extra to determine if there are more pages
            .limit(page_size + 1)
            .all(db)
            .await
            .context("Failed to list series by library with cursor")
    }

    /// List recently added series using cursor-based pagination
    ///
    /// Uses `(created_at, id)` as the cursor for descending date order.
    ///
    /// # Arguments
    /// * `db` - Database connection
    /// * `library_id` - Optional library ID to filter by
    /// * `cursor` - Optional cursor from a previous page (created_at timestamp, series_id)
    /// * `page_size` - Number of items to return
    ///
    /// # Returns
    /// * `Vec<series::Model>` - Series for this page (may have page_size + 1 to detect has_more)
    pub async fn list_recently_added_cursor(
        db: &DatabaseConnection,
        library_id: Option<Uuid>,
        cursor: Option<(i64, Uuid)>,
        page_size: u64,
    ) -> Result<Vec<series::Model>> {
        use chrono::TimeZone;

        let mut query = Series::find();

        // Filter by library if specified
        if let Some(lib_id) = library_id {
            query = query.filter(series::Column::LibraryId.eq(lib_id));
        }

        // Apply cursor condition if provided
        // For descending order: (created_at < cursor_timestamp) OR (created_at = cursor_timestamp AND id < cursor_id)
        if let Some((cursor_timestamp, cursor_id)) = cursor {
            let cursor_datetime = Utc.timestamp_millis_opt(cursor_timestamp).single();
            if let Some(dt) = cursor_datetime {
                query = query.filter(
                    Condition::any().add(series::Column::CreatedAt.lt(dt)).add(
                        Condition::all()
                            .add(series::Column::CreatedAt.eq(dt))
                            .add(series::Column::Id.lt(cursor_id)),
                    ),
                );
            }
        }

        // Order by created_at DESC (most recent first), then id DESC for stability
        query
            .order_by_desc(series::Column::CreatedAt)
            .order_by_desc(series::Column::Id)
            // Fetch one extra to determine if there are more pages
            .limit(page_size + 1)
            .all(db)
            .await
            .context("Failed to list recently added series with cursor")
    }

    /// Get title_sort for a series (used for cursor construction)
    pub async fn get_title_sort(
        db: &DatabaseConnection,
        series_id: Uuid,
    ) -> Result<Option<String>> {
        let result: Option<String> = series_metadata::Entity::find()
            .filter(series_metadata::Column::SeriesId.eq(series_id))
            .select_only()
            .column(series_metadata::Column::TitleSort)
            .into_tuple()
            .one(db)
            .await
            .context("Failed to get title_sort for series")?;

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::ScanningStrategy;
    use crate::db::entities::books;
    use crate::db::repositories::{BookRepository, LibraryRepository, SeriesMetadataRepository};
    use crate::db::test_helpers::create_test_db;

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
    async fn test_search_series_unicode_accent_insensitive() {
        let (db, _temp_dir) = create_test_db().await;

        let library = LibraryRepository::create(
            db.sea_orm_connection(),
            "Test Library",
            "/test/path",
            ScanningStrategy::Default,
        )
        .await
        .unwrap();

        // Create series with accented titles
        SeriesRepository::create(db.sea_orm_connection(), library.id, "MÄR", None)
            .await
            .unwrap();
        SeriesRepository::create(db.sea_orm_connection(), library.id, "Café Stories", None)
            .await
            .unwrap();
        SeriesRepository::create(db.sea_orm_connection(), library.id, "Naruto", None)
            .await
            .unwrap();

        // Searching "mar" should find "MÄR" (accent-insensitive + case-insensitive)
        let results = SeriesRepository::search_by_name(db.sea_orm_connection(), "mar")
            .await
            .unwrap();
        assert_eq!(results.len(), 1);
        let metadata =
            SeriesMetadataRepository::get_by_series_id(db.sea_orm_connection(), results[0].id)
                .await
                .unwrap()
                .unwrap();
        assert_eq!(metadata.title, "MÄR");

        // Searching "MÄR" should also find "MÄR" (exact match still works)
        let results = SeriesRepository::search_by_name(db.sea_orm_connection(), "MÄR")
            .await
            .unwrap();
        assert_eq!(results.len(), 1);

        // Searching "cafe" should find "Café Stories"
        let results = SeriesRepository::search_by_name(db.sea_orm_connection(), "cafe")
            .await
            .unwrap();
        assert_eq!(results.len(), 1);
        let metadata =
            SeriesMetadataRepository::get_by_series_id(db.sea_orm_connection(), results[0].id)
                .await
                .unwrap()
                .unwrap();
        assert_eq!(metadata.title, "Café Stories");

        // Searching "café" should also find "Café Stories"
        let results = SeriesRepository::search_by_name(db.sea_orm_connection(), "café")
            .await
            .unwrap();
        assert_eq!(results.len(), 1);

        // Searching "naruto" should still find "Naruto" (basic ASCII case-insensitive)
        let results = SeriesRepository::search_by_name(db.sea_orm_connection(), "naruto")
            .await
            .unwrap();
        assert_eq!(results.len(), 1);
    }

    #[tokio::test]
    async fn test_search_title_populated_on_create() {
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
            SeriesRepository::create(db.sea_orm_connection(), library.id, "MÄR Omega", None)
                .await
                .unwrap();

        // Verify search_title is populated correctly
        let metadata =
            SeriesMetadataRepository::get_by_series_id(db.sea_orm_connection(), series.id)
                .await
                .unwrap()
                .unwrap();
        assert_eq!(metadata.search_title, "mar omega");
    }

    #[tokio::test]
    async fn test_search_title_updated_on_title_change() {
        let (db, _temp_dir) = create_test_db().await;

        let library = LibraryRepository::create(
            db.sea_orm_connection(),
            "Test Library",
            "/test/path",
            ScanningStrategy::Default,
        )
        .await
        .unwrap();

        let series = SeriesRepository::create(db.sea_orm_connection(), library.id, "Test", None)
            .await
            .unwrap();

        // Update title to an accented name
        SeriesMetadataRepository::update_title(
            db.sea_orm_connection(),
            series.id,
            "Crème Brûlée".to_string(),
            None,
        )
        .await
        .unwrap();

        let metadata =
            SeriesMetadataRepository::get_by_series_id(db.sea_orm_connection(), series.id)
                .await
                .unwrap()
                .unwrap();
        assert_eq!(metadata.search_title, "creme brulee");

        // Search should find it by accent-stripped query
        let results = SeriesRepository::search_by_name(db.sea_orm_connection(), "creme brulee")
            .await
            .unwrap();
        assert_eq!(results.len(), 1);
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
            koreader_hash: None,
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
            koreader_hash: None,
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
            koreader_hash: None,
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
            koreader_hash: None,
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
        // Unicode accent stripping
        assert_eq!(SeriesRepository::normalize_name("MÄR"), "mar");
        assert_eq!(SeriesRepository::normalize_name("Café"), "cafe");
        assert_eq!(SeriesRepository::normalize_name("MÄR Omega"), "mar omega");
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

    #[tokio::test]
    async fn test_create_with_fingerprint_and_title() {
        let (db, _temp_dir) = create_test_db().await;

        let library = LibraryRepository::create(
            db.sea_orm_connection(),
            "Test Library",
            "/test/path",
            ScanningStrategy::Default,
        )
        .await
        .unwrap();

        // Create series with different name and metadata title
        // This simulates preprocessing where "(Digital)" suffix is removed
        let series = SeriesRepository::create_with_fingerprint_and_title(
            db.sea_orm_connection(),
            library.id,
            "One Piece (Digital)", // Original directory name
            Some("fingerprint123".to_string()),
            "/test/path/One Piece (Digital)".to_string(),
            Some("One Piece"), // Preprocessed title
            None,
        )
        .await
        .unwrap();

        // series.name should be the original directory name
        assert_eq!(series.name, "One Piece (Digital)");

        // series_metadata.title should be the preprocessed title
        let metadata = crate::db::repositories::SeriesMetadataRepository::get_by_series_id(
            db.sea_orm_connection(),
            series.id,
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(metadata.title, "One Piece");
    }

    #[tokio::test]
    async fn test_create_with_fingerprint_and_title_none() {
        let (db, _temp_dir) = create_test_db().await;

        let library = LibraryRepository::create(
            db.sea_orm_connection(),
            "Test Library",
            "/test/path",
            ScanningStrategy::Default,
        )
        .await
        .unwrap();

        // Create series without preprocessed title (metadata_title = None)
        let series = SeriesRepository::create_with_fingerprint_and_title(
            db.sea_orm_connection(),
            library.id,
            "One Piece",
            Some("fingerprint456".to_string()),
            "/test/path/One Piece".to_string(),
            None, // No preprocessing
            None,
        )
        .await
        .unwrap();

        // series.name should be the original name
        assert_eq!(series.name, "One Piece");

        // series_metadata.title should also be the original name
        let metadata = crate::db::repositories::SeriesMetadataRepository::get_by_series_id(
            db.sea_orm_connection(),
            series.id,
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(metadata.title, "One Piece");
    }
}
