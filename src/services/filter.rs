//! Filter service for evaluating conditions against series/books
//!
//! TODO: Remove allow(dead_code) once all filter features are fully integrated

#![allow(dead_code)]

use crate::api::routes::v1::dto::{
    BookCondition, BoolOperator, FieldOperator, SeriesCondition, UuidOperator,
};
use crate::db::repositories::{GenreRepository, TagRepository};
use anyhow::Result;
use sea_orm::DatabaseConnection;
use std::collections::HashSet;
use std::future::Future;
use std::pin::Pin;
use uuid::Uuid;

/// Service for evaluating filter conditions against series/books
pub struct FilterService;

impl FilterService {
    /// Get series IDs matching a condition (without user context)
    ///
    /// Returns the set of series IDs that match the given condition.
    /// If candidate_ids is provided, only those series are considered.
    ///
    /// Note: ReadStatus filtering requires user context. Use `get_matching_series_for_user`
    /// if you need ReadStatus filtering support.
    pub fn get_matching_series<'a>(
        db: &'a DatabaseConnection,
        condition: &'a SeriesCondition,
        candidate_ids: Option<&'a HashSet<Uuid>>,
    ) -> Pin<Box<dyn Future<Output = Result<HashSet<Uuid>>> + Send + 'a>> {
        Self::get_matching_series_for_user(db, condition, candidate_ids, None)
    }

    /// Get series IDs matching a condition with user context for ReadStatus filtering
    ///
    /// Returns the set of series IDs that match the given condition.
    /// If candidate_ids is provided, only those series are considered.
    /// If user_id is provided, ReadStatus filtering will work correctly.
    pub fn get_matching_series_for_user<'a>(
        db: &'a DatabaseConnection,
        condition: &'a SeriesCondition,
        candidate_ids: Option<&'a HashSet<Uuid>>,
        user_id: Option<Uuid>,
    ) -> Pin<Box<dyn Future<Output = Result<HashSet<Uuid>>> + Send + 'a>> {
        Box::pin(async move {
            match condition {
                SeriesCondition::AllOf { all_of } => {
                    if all_of.is_empty() {
                        // Empty AllOf matches everything
                        return Ok(candidate_ids.cloned().unwrap_or_default());
                    }

                    // Start with first condition's result
                    let mut result =
                        Self::get_matching_series_for_user(db, &all_of[0], candidate_ids, user_id)
                            .await?;

                    // Intersect with remaining conditions
                    for cond in &all_of[1..] {
                        if result.is_empty() {
                            break; // Short-circuit
                        }
                        let matching =
                            Self::get_matching_series_for_user(db, cond, Some(&result), user_id)
                                .await?;
                        result = result.intersection(&matching).cloned().collect();
                    }

                    Ok(result)
                }

                SeriesCondition::AnyOf { any_of } => {
                    if any_of.is_empty() {
                        // Empty AnyOf matches nothing
                        return Ok(HashSet::new());
                    }

                    let mut result = HashSet::new();
                    for cond in any_of {
                        let matching =
                            Self::get_matching_series_for_user(db, cond, candidate_ids, user_id)
                                .await?;
                        result.extend(matching);
                    }

                    Ok(result)
                }

                SeriesCondition::LibraryId { library_id } => {
                    Self::filter_by_library_id(db, library_id, candidate_ids).await
                }

                SeriesCondition::Genre { genre } => {
                    Self::filter_by_genre(db, genre, candidate_ids).await
                }

                SeriesCondition::Tag { tag } => Self::filter_by_tag(db, tag, candidate_ids).await,

                SeriesCondition::Status { status } => {
                    Self::filter_by_status(db, status, candidate_ids).await
                }

                SeriesCondition::Publisher { publisher } => {
                    Self::filter_by_publisher(db, publisher, candidate_ids).await
                }

                SeriesCondition::Language { language } => {
                    Self::filter_by_language(db, language, candidate_ids).await
                }

                SeriesCondition::Name { name } => {
                    Self::filter_by_name(db, name, candidate_ids).await
                }

                SeriesCondition::TitleSort { title_sort } => {
                    Self::filter_by_title_sort(db, title_sort, candidate_ids).await
                }

                SeriesCondition::ReadStatus { read_status } => {
                    Self::filter_by_read_status(db, read_status, candidate_ids, user_id).await
                }

                SeriesCondition::SharingTag { sharing_tag } => {
                    Self::filter_by_sharing_tag(db, sharing_tag, candidate_ids).await
                }

                SeriesCondition::Completion { completion } => {
                    Self::filter_by_completion(db, completion, candidate_ids).await
                }

                SeriesCondition::HasExternalSourceId {
                    has_external_source_id,
                } => {
                    Self::filter_by_has_external_source_id(
                        db,
                        has_external_source_id,
                        candidate_ids,
                    )
                    .await
                }

                SeriesCondition::HasUserRating { has_user_rating } => {
                    Self::filter_by_has_user_rating(db, has_user_rating, candidate_ids, user_id)
                        .await
                }
            }
        })
    }

    async fn filter_by_library_id(
        db: &DatabaseConnection,
        operator: &UuidOperator,
        candidate_ids: Option<&HashSet<Uuid>>,
    ) -> Result<HashSet<Uuid>> {
        use crate::db::entities::series;
        use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

        match operator {
            UuidOperator::Is { value } => {
                let series_in_library: Vec<Uuid> = series::Entity::find()
                    .filter(series::Column::LibraryId.eq(*value))
                    .all(db)
                    .await?
                    .into_iter()
                    .map(|s| s.id)
                    .collect();

                let result: HashSet<Uuid> = if let Some(candidates) = candidate_ids {
                    series_in_library
                        .into_iter()
                        .filter(|id| candidates.contains(id))
                        .collect()
                } else {
                    series_in_library.into_iter().collect()
                };

                Ok(result)
            }
            UuidOperator::IsNot { value } => {
                let series_in_library: HashSet<Uuid> = series::Entity::find()
                    .filter(series::Column::LibraryId.eq(*value))
                    .all(db)
                    .await?
                    .into_iter()
                    .map(|s| s.id)
                    .collect();

                if let Some(candidates) = candidate_ids {
                    Ok(candidates
                        .iter()
                        .filter(|id| !series_in_library.contains(id))
                        .cloned()
                        .collect())
                } else {
                    // Need to get all series and exclude the ones in this library
                    let all_series: HashSet<Uuid> = series::Entity::find()
                        .all(db)
                        .await?
                        .into_iter()
                        .map(|s| s.id)
                        .collect();

                    Ok(all_series
                        .into_iter()
                        .filter(|id| !series_in_library.contains(id))
                        .collect())
                }
            }
        }
    }

    async fn filter_by_genre(
        db: &DatabaseConnection,
        operator: &FieldOperator,
        candidate_ids: Option<&HashSet<Uuid>>,
    ) -> Result<HashSet<Uuid>> {
        match operator {
            FieldOperator::Is { value } => {
                let series_with_genre = GenreRepository::get_series_with_genre(db, value).await?;
                let result: HashSet<Uuid> = series_with_genre.into_iter().collect();

                if let Some(candidates) = candidate_ids {
                    Ok(result.intersection(candidates).cloned().collect())
                } else {
                    Ok(result)
                }
            }
            FieldOperator::IsNot { value } => {
                let series_with_genre: HashSet<Uuid> =
                    GenreRepository::get_series_with_genre(db, value)
                        .await?
                        .into_iter()
                        .collect();

                if let Some(candidates) = candidate_ids {
                    Ok(candidates
                        .iter()
                        .filter(|id| !series_with_genre.contains(id))
                        .cloned()
                        .collect())
                } else {
                    // Without candidates, we need all series
                    use crate::db::entities::series;
                    use sea_orm::EntityTrait;

                    let all_series: HashSet<Uuid> = series::Entity::find()
                        .all(db)
                        .await?
                        .into_iter()
                        .map(|s| s.id)
                        .collect();

                    Ok(all_series
                        .into_iter()
                        .filter(|id| !series_with_genre.contains(id))
                        .collect())
                }
            }
            FieldOperator::Contains { value } => {
                // Get all genres that contain the value, then get series with those genres
                let series_ids =
                    GenreRepository::get_series_with_genre_containing(db, value).await?;
                let result: HashSet<Uuid> = series_ids.into_iter().collect();

                if let Some(candidates) = candidate_ids {
                    Ok(result.intersection(candidates).cloned().collect())
                } else {
                    Ok(result)
                }
            }
            FieldOperator::DoesNotContain { value } => {
                let series_with_matching: HashSet<Uuid> =
                    GenreRepository::get_series_with_genre_containing(db, value)
                        .await?
                        .into_iter()
                        .collect();

                if let Some(candidates) = candidate_ids {
                    Ok(candidates
                        .iter()
                        .filter(|id| !series_with_matching.contains(id))
                        .cloned()
                        .collect())
                } else {
                    use crate::db::entities::series;
                    use sea_orm::EntityTrait;

                    let all_series: HashSet<Uuid> = series::Entity::find()
                        .all(db)
                        .await?
                        .into_iter()
                        .map(|s| s.id)
                        .collect();

                    Ok(all_series
                        .into_iter()
                        .filter(|id| !series_with_matching.contains(id))
                        .collect())
                }
            }
            FieldOperator::BeginsWith { value } => {
                let series_ids =
                    GenreRepository::get_series_with_genre_starting_with(db, value).await?;
                let result: HashSet<Uuid> = series_ids.into_iter().collect();

                if let Some(candidates) = candidate_ids {
                    Ok(result.intersection(candidates).cloned().collect())
                } else {
                    Ok(result)
                }
            }
            FieldOperator::EndsWith { value } => {
                let series_ids =
                    GenreRepository::get_series_with_genre_ending_with(db, value).await?;
                let result: HashSet<Uuid> = series_ids.into_iter().collect();

                if let Some(candidates) = candidate_ids {
                    Ok(result.intersection(candidates).cloned().collect())
                } else {
                    Ok(result)
                }
            }
            FieldOperator::IsNull => {
                // Series with no genres
                let series_with_genres = GenreRepository::get_all_series_with_genres(db).await?;
                let with_genres: HashSet<Uuid> = series_with_genres.into_iter().collect();

                if let Some(candidates) = candidate_ids {
                    Ok(candidates
                        .iter()
                        .filter(|id| !with_genres.contains(id))
                        .cloned()
                        .collect())
                } else {
                    use crate::db::entities::series;
                    use sea_orm::EntityTrait;

                    let all_series: HashSet<Uuid> = series::Entity::find()
                        .all(db)
                        .await?
                        .into_iter()
                        .map(|s| s.id)
                        .collect();

                    Ok(all_series
                        .into_iter()
                        .filter(|id| !with_genres.contains(id))
                        .collect())
                }
            }
            FieldOperator::IsNotNull => {
                // Series with at least one genre
                let series_with_genres = GenreRepository::get_all_series_with_genres(db).await?;
                let result: HashSet<Uuid> = series_with_genres.into_iter().collect();

                if let Some(candidates) = candidate_ids {
                    Ok(result.intersection(candidates).cloned().collect())
                } else {
                    Ok(result)
                }
            }
        }
    }

    async fn filter_by_tag(
        db: &DatabaseConnection,
        operator: &FieldOperator,
        candidate_ids: Option<&HashSet<Uuid>>,
    ) -> Result<HashSet<Uuid>> {
        match operator {
            FieldOperator::Is { value } => {
                let series_with_tag = TagRepository::get_series_with_tag(db, value).await?;
                let result: HashSet<Uuid> = series_with_tag.into_iter().collect();

                if let Some(candidates) = candidate_ids {
                    Ok(result.intersection(candidates).cloned().collect())
                } else {
                    Ok(result)
                }
            }
            FieldOperator::IsNot { value } => {
                let series_with_tag: HashSet<Uuid> = TagRepository::get_series_with_tag(db, value)
                    .await?
                    .into_iter()
                    .collect();

                if let Some(candidates) = candidate_ids {
                    Ok(candidates
                        .iter()
                        .filter(|id| !series_with_tag.contains(id))
                        .cloned()
                        .collect())
                } else {
                    use crate::db::entities::series;
                    use sea_orm::EntityTrait;

                    let all_series: HashSet<Uuid> = series::Entity::find()
                        .all(db)
                        .await?
                        .into_iter()
                        .map(|s| s.id)
                        .collect();

                    Ok(all_series
                        .into_iter()
                        .filter(|id| !series_with_tag.contains(id))
                        .collect())
                }
            }
            FieldOperator::Contains { value } => {
                let series_ids = TagRepository::get_series_with_tag_containing(db, value).await?;
                let result: HashSet<Uuid> = series_ids.into_iter().collect();

                if let Some(candidates) = candidate_ids {
                    Ok(result.intersection(candidates).cloned().collect())
                } else {
                    Ok(result)
                }
            }
            FieldOperator::DoesNotContain { value } => {
                let series_with_matching: HashSet<Uuid> =
                    TagRepository::get_series_with_tag_containing(db, value)
                        .await?
                        .into_iter()
                        .collect();

                if let Some(candidates) = candidate_ids {
                    Ok(candidates
                        .iter()
                        .filter(|id| !series_with_matching.contains(id))
                        .cloned()
                        .collect())
                } else {
                    use crate::db::entities::series;
                    use sea_orm::EntityTrait;

                    let all_series: HashSet<Uuid> = series::Entity::find()
                        .all(db)
                        .await?
                        .into_iter()
                        .map(|s| s.id)
                        .collect();

                    Ok(all_series
                        .into_iter()
                        .filter(|id| !series_with_matching.contains(id))
                        .collect())
                }
            }
            FieldOperator::BeginsWith { value } => {
                let series_ids =
                    TagRepository::get_series_with_tag_starting_with(db, value).await?;
                let result: HashSet<Uuid> = series_ids.into_iter().collect();

                if let Some(candidates) = candidate_ids {
                    Ok(result.intersection(candidates).cloned().collect())
                } else {
                    Ok(result)
                }
            }
            FieldOperator::EndsWith { value } => {
                let series_ids = TagRepository::get_series_with_tag_ending_with(db, value).await?;
                let result: HashSet<Uuid> = series_ids.into_iter().collect();

                if let Some(candidates) = candidate_ids {
                    Ok(result.intersection(candidates).cloned().collect())
                } else {
                    Ok(result)
                }
            }
            FieldOperator::IsNull => {
                let series_with_tags = TagRepository::get_all_series_with_tags(db).await?;
                let with_tags: HashSet<Uuid> = series_with_tags.into_iter().collect();

                if let Some(candidates) = candidate_ids {
                    Ok(candidates
                        .iter()
                        .filter(|id| !with_tags.contains(id))
                        .cloned()
                        .collect())
                } else {
                    use crate::db::entities::series;
                    use sea_orm::EntityTrait;

                    let all_series: HashSet<Uuid> = series::Entity::find()
                        .all(db)
                        .await?
                        .into_iter()
                        .map(|s| s.id)
                        .collect();

                    Ok(all_series
                        .into_iter()
                        .filter(|id| !with_tags.contains(id))
                        .collect())
                }
            }
            FieldOperator::IsNotNull => {
                let series_with_tags = TagRepository::get_all_series_with_tags(db).await?;
                let result: HashSet<Uuid> = series_with_tags.into_iter().collect();

                if let Some(candidates) = candidate_ids {
                    Ok(result.intersection(candidates).cloned().collect())
                } else {
                    Ok(result)
                }
            }
        }
    }

    async fn filter_by_sharing_tag(
        db: &DatabaseConnection,
        operator: &FieldOperator,
        candidate_ids: Option<&HashSet<Uuid>>,
    ) -> Result<HashSet<Uuid>> {
        use crate::db::repositories::SharingTagRepository;

        match operator {
            FieldOperator::Is { value } => {
                let series_with_tag =
                    SharingTagRepository::get_series_with_sharing_tag_name(db, value).await?;
                let result: HashSet<Uuid> = series_with_tag.into_iter().collect();

                if let Some(candidates) = candidate_ids {
                    Ok(result.intersection(candidates).cloned().collect())
                } else {
                    Ok(result)
                }
            }
            FieldOperator::IsNot { value } => {
                let series_with_tag: HashSet<Uuid> =
                    SharingTagRepository::get_series_with_sharing_tag_name(db, value)
                        .await?
                        .into_iter()
                        .collect();

                if let Some(candidates) = candidate_ids {
                    Ok(candidates
                        .iter()
                        .filter(|id| !series_with_tag.contains(id))
                        .cloned()
                        .collect())
                } else {
                    use crate::db::entities::series;
                    use sea_orm::EntityTrait;

                    let all_series: HashSet<Uuid> = series::Entity::find()
                        .all(db)
                        .await?
                        .into_iter()
                        .map(|s| s.id)
                        .collect();

                    Ok(all_series
                        .into_iter()
                        .filter(|id| !series_with_tag.contains(id))
                        .collect())
                }
            }
            FieldOperator::Contains { value } => {
                let series_ids =
                    SharingTagRepository::get_series_with_sharing_tag_containing(db, value).await?;
                let result: HashSet<Uuid> = series_ids.into_iter().collect();

                if let Some(candidates) = candidate_ids {
                    Ok(result.intersection(candidates).cloned().collect())
                } else {
                    Ok(result)
                }
            }
            FieldOperator::DoesNotContain { value } => {
                let series_with_matching: HashSet<Uuid> =
                    SharingTagRepository::get_series_with_sharing_tag_containing(db, value)
                        .await?
                        .into_iter()
                        .collect();

                if let Some(candidates) = candidate_ids {
                    Ok(candidates
                        .iter()
                        .filter(|id| !series_with_matching.contains(id))
                        .cloned()
                        .collect())
                } else {
                    use crate::db::entities::series;
                    use sea_orm::EntityTrait;

                    let all_series: HashSet<Uuid> = series::Entity::find()
                        .all(db)
                        .await?
                        .into_iter()
                        .map(|s| s.id)
                        .collect();

                    Ok(all_series
                        .into_iter()
                        .filter(|id| !series_with_matching.contains(id))
                        .collect())
                }
            }
            FieldOperator::BeginsWith { value } => {
                let series_ids =
                    SharingTagRepository::get_series_with_sharing_tag_starting_with(db, value)
                        .await?;
                let result: HashSet<Uuid> = series_ids.into_iter().collect();

                if let Some(candidates) = candidate_ids {
                    Ok(result.intersection(candidates).cloned().collect())
                } else {
                    Ok(result)
                }
            }
            FieldOperator::EndsWith { value } => {
                let series_ids =
                    SharingTagRepository::get_series_with_sharing_tag_ending_with(db, value)
                        .await?;
                let result: HashSet<Uuid> = series_ids.into_iter().collect();

                if let Some(candidates) = candidate_ids {
                    Ok(result.intersection(candidates).cloned().collect())
                } else {
                    Ok(result)
                }
            }
            FieldOperator::IsNull => {
                // Series with no sharing tags
                let series_with_tags = SharingTagRepository::get_tagged_series_ids(db).await?;

                if let Some(candidates) = candidate_ids {
                    Ok(candidates
                        .iter()
                        .filter(|id| !series_with_tags.contains(id))
                        .cloned()
                        .collect())
                } else {
                    use crate::db::entities::series;
                    use sea_orm::EntityTrait;

                    let all_series: HashSet<Uuid> = series::Entity::find()
                        .all(db)
                        .await?
                        .into_iter()
                        .map(|s| s.id)
                        .collect();

                    Ok(all_series
                        .into_iter()
                        .filter(|id| !series_with_tags.contains(id))
                        .collect())
                }
            }
            FieldOperator::IsNotNull => {
                // Series with at least one sharing tag
                let series_with_tags = SharingTagRepository::get_tagged_series_ids(db).await?;

                if let Some(candidates) = candidate_ids {
                    Ok(series_with_tags.intersection(candidates).cloned().collect())
                } else {
                    Ok(series_with_tags)
                }
            }
        }
    }

    /// Filter series by completion status
    ///
    /// A series is considered "complete" when:
    /// - It has a total_book_count set in metadata AND
    /// - The actual book_count equals total_book_count
    ///
    /// A series is considered "incomplete" (missing books) when:
    /// - It has a total_book_count set in metadata AND
    /// - The actual book_count is less than total_book_count
    ///
    /// Series without total_book_count are excluded from both filters.
    async fn filter_by_completion(
        db: &DatabaseConnection,
        operator: &BoolOperator,
        candidate_ids: Option<&HashSet<Uuid>>,
    ) -> Result<HashSet<Uuid>> {
        use crate::db::entities::{books, series_metadata};
        use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QuerySelect};

        // Get all series with total_book_count set
        let series_with_total: Vec<(Uuid, i32)> = series_metadata::Entity::find()
            .filter(series_metadata::Column::TotalBookCount.is_not_null())
            .select_only()
            .column(series_metadata::Column::SeriesId)
            .column(series_metadata::Column::TotalBookCount)
            .into_tuple()
            .all(db)
            .await?;

        if series_with_total.is_empty() {
            return Ok(HashSet::new());
        }

        // Filter by candidates if provided
        let series_with_total: Vec<(Uuid, i32)> = if let Some(candidates) = candidate_ids {
            series_with_total
                .into_iter()
                .filter(|(id, _)| candidates.contains(id))
                .collect()
        } else {
            series_with_total
        };

        if series_with_total.is_empty() {
            return Ok(HashSet::new());
        }

        // Get actual book counts for these series
        let series_ids: Vec<Uuid> = series_with_total.iter().map(|(id, _)| *id).collect();

        // Count non-deleted books for each series
        let book_counts: Vec<(Uuid, i64)> = books::Entity::find()
            .filter(books::Column::SeriesId.is_in(series_ids.clone()))
            .filter(books::Column::Deleted.eq(false))
            .select_only()
            .column(books::Column::SeriesId)
            .column_as(books::Column::Id.count(), "count")
            .group_by(books::Column::SeriesId)
            .into_tuple()
            .all(db)
            .await?;

        let book_count_map: std::collections::HashMap<Uuid, i64> =
            book_counts.into_iter().collect();

        // Determine which series match the completion filter
        let mut result = HashSet::new();

        for (series_id, total_book_count) in series_with_total {
            let actual_count = book_count_map.get(&series_id).copied().unwrap_or(0);
            let is_complete = actual_count >= total_book_count as i64;

            let matches = match operator {
                BoolOperator::IsTrue => is_complete,
                BoolOperator::IsFalse => !is_complete,
            };

            if matches {
                result.insert(series_id);
            }
        }

        Ok(result)
    }

    /// Filter series by whether they have an external source ID linked
    ///
    /// A series "has external source ID" if there's at least one entry in the
    /// series_external_ids table for that series.
    ///
    /// - IsTrue: return series that have at least one external source ID
    /// - IsFalse: return series that have no external source IDs
    async fn filter_by_has_external_source_id(
        db: &DatabaseConnection,
        operator: &BoolOperator,
        candidate_ids: Option<&HashSet<Uuid>>,
    ) -> Result<HashSet<Uuid>> {
        use crate::db::entities::{series, series_external_ids};
        use sea_orm::{EntityTrait, QuerySelect};

        // Get all series IDs that have at least one external ID
        let series_with_external_ids: HashSet<Uuid> = series_external_ids::Entity::find()
            .select_only()
            .column(series_external_ids::Column::SeriesId)
            .distinct()
            .into_tuple()
            .all(db)
            .await?
            .into_iter()
            .collect();

        match operator {
            BoolOperator::IsTrue => {
                // Return series WITH external IDs
                if let Some(candidates) = candidate_ids {
                    Ok(series_with_external_ids
                        .intersection(candidates)
                        .cloned()
                        .collect())
                } else {
                    Ok(series_with_external_ids)
                }
            }
            BoolOperator::IsFalse => {
                // Return series WITHOUT external IDs
                if let Some(candidates) = candidate_ids {
                    Ok(candidates
                        .iter()
                        .filter(|id| !series_with_external_ids.contains(id))
                        .cloned()
                        .collect())
                } else {
                    // Need all series, then exclude those with external IDs
                    let all_series: HashSet<Uuid> = series::Entity::find()
                        .select_only()
                        .column(series::Column::Id)
                        .into_tuple()
                        .all(db)
                        .await?
                        .into_iter()
                        .collect();

                    Ok(all_series
                        .into_iter()
                        .filter(|id| !series_with_external_ids.contains(id))
                        .collect())
                }
            }
        }
    }

    /// Filter series by whether the current user has rated them
    ///
    /// - IsTrue: return series that the user has rated
    /// - IsFalse: return series that the user has not rated
    ///
    /// If no user_id is provided, returns an empty set (no user = no ratings).
    async fn filter_by_has_user_rating(
        db: &DatabaseConnection,
        operator: &BoolOperator,
        candidate_ids: Option<&HashSet<Uuid>>,
        user_id: Option<Uuid>,
    ) -> Result<HashSet<Uuid>> {
        use crate::db::entities::{series, user_series_ratings};
        use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QuerySelect};

        let Some(uid) = user_id else {
            // No user context — no ratings possible
            return match operator {
                BoolOperator::IsTrue => Ok(HashSet::new()),
                BoolOperator::IsFalse => {
                    if let Some(candidates) = candidate_ids {
                        Ok(candidates.clone())
                    } else {
                        let all_series: HashSet<Uuid> = series::Entity::find()
                            .select_only()
                            .column(series::Column::Id)
                            .into_tuple()
                            .all(db)
                            .await?
                            .into_iter()
                            .collect();
                        Ok(all_series)
                    }
                }
            };
        };

        // Get all series IDs that the user has rated
        let rated_series: HashSet<Uuid> = user_series_ratings::Entity::find()
            .select_only()
            .column(user_series_ratings::Column::SeriesId)
            .filter(user_series_ratings::Column::UserId.eq(uid))
            .distinct()
            .into_tuple()
            .all(db)
            .await?
            .into_iter()
            .collect();

        match operator {
            BoolOperator::IsTrue => {
                // Return series WITH user ratings
                if let Some(candidates) = candidate_ids {
                    Ok(rated_series.intersection(candidates).cloned().collect())
                } else {
                    Ok(rated_series)
                }
            }
            BoolOperator::IsFalse => {
                // Return series WITHOUT user ratings
                if let Some(candidates) = candidate_ids {
                    Ok(candidates
                        .iter()
                        .filter(|id| !rated_series.contains(id))
                        .cloned()
                        .collect())
                } else {
                    // Need all series, then exclude those with ratings
                    let all_series: HashSet<Uuid> = series::Entity::find()
                        .select_only()
                        .column(series::Column::Id)
                        .into_tuple()
                        .all(db)
                        .await?
                        .into_iter()
                        .collect();

                    Ok(all_series
                        .into_iter()
                        .filter(|id| !rated_series.contains(id))
                        .collect())
                }
            }
        }
    }

    async fn filter_by_status(
        db: &DatabaseConnection,
        operator: &FieldOperator,
        candidate_ids: Option<&HashSet<Uuid>>,
    ) -> Result<HashSet<Uuid>> {
        use crate::db::entities::series_metadata;
        use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QuerySelect};

        let query = series_metadata::Entity::find();

        let filtered_query = match operator {
            FieldOperator::Is { value } => {
                query.filter(series_metadata::Column::Status.eq(value.clone()))
            }
            FieldOperator::IsNot { value } => {
                query.filter(series_metadata::Column::Status.ne(value.clone()))
            }
            FieldOperator::IsNull => query.filter(series_metadata::Column::Status.is_null()),
            FieldOperator::IsNotNull => query.filter(series_metadata::Column::Status.is_not_null()),
            FieldOperator::Contains { value } => {
                query.filter(series_metadata::Column::Status.contains(value.clone()))
            }
            FieldOperator::DoesNotContain { value } => {
                query.filter(series_metadata::Column::Status.not_like(format!("%{}%", value)))
            }
            FieldOperator::BeginsWith { value } => {
                query.filter(series_metadata::Column::Status.starts_with(value.clone()))
            }
            FieldOperator::EndsWith { value } => {
                query.filter(series_metadata::Column::Status.ends_with(value.clone()))
            }
        };

        let series_ids: Vec<Uuid> = filtered_query
            .select_only()
            .column(series_metadata::Column::SeriesId)
            .into_tuple()
            .all(db)
            .await?;

        let result: HashSet<Uuid> = if let Some(candidates) = candidate_ids {
            series_ids
                .into_iter()
                .filter(|id| candidates.contains(id))
                .collect()
        } else {
            series_ids.into_iter().collect()
        };

        Ok(result)
    }

    async fn filter_by_publisher(
        db: &DatabaseConnection,
        operator: &FieldOperator,
        candidate_ids: Option<&HashSet<Uuid>>,
    ) -> Result<HashSet<Uuid>> {
        use crate::db::entities::series_metadata;
        use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QuerySelect};

        let query = series_metadata::Entity::find();

        let filtered_query = match operator {
            FieldOperator::Is { value } => {
                query.filter(series_metadata::Column::Publisher.eq(value.clone()))
            }
            FieldOperator::IsNot { value } => {
                query.filter(series_metadata::Column::Publisher.ne(value.clone()))
            }
            FieldOperator::IsNull => query.filter(series_metadata::Column::Publisher.is_null()),
            FieldOperator::IsNotNull => {
                query.filter(series_metadata::Column::Publisher.is_not_null())
            }
            FieldOperator::Contains { value } => {
                query.filter(series_metadata::Column::Publisher.contains(value.clone()))
            }
            FieldOperator::DoesNotContain { value } => {
                query.filter(series_metadata::Column::Publisher.not_like(format!("%{}%", value)))
            }
            FieldOperator::BeginsWith { value } => {
                query.filter(series_metadata::Column::Publisher.starts_with(value.clone()))
            }
            FieldOperator::EndsWith { value } => {
                query.filter(series_metadata::Column::Publisher.ends_with(value.clone()))
            }
        };

        let series_ids: Vec<Uuid> = filtered_query
            .select_only()
            .column(series_metadata::Column::SeriesId)
            .into_tuple()
            .all(db)
            .await?;

        let result: HashSet<Uuid> = if let Some(candidates) = candidate_ids {
            series_ids
                .into_iter()
                .filter(|id| candidates.contains(id))
                .collect()
        } else {
            series_ids.into_iter().collect()
        };

        Ok(result)
    }

    async fn filter_by_language(
        db: &DatabaseConnection,
        operator: &FieldOperator,
        candidate_ids: Option<&HashSet<Uuid>>,
    ) -> Result<HashSet<Uuid>> {
        use crate::db::entities::series_metadata;
        use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QuerySelect};

        let query = series_metadata::Entity::find();

        let filtered_query = match operator {
            FieldOperator::Is { value } => {
                query.filter(series_metadata::Column::Language.eq(value.clone()))
            }
            FieldOperator::IsNot { value } => {
                query.filter(series_metadata::Column::Language.ne(value.clone()))
            }
            FieldOperator::IsNull => query.filter(series_metadata::Column::Language.is_null()),
            FieldOperator::IsNotNull => {
                query.filter(series_metadata::Column::Language.is_not_null())
            }
            FieldOperator::Contains { value } => {
                query.filter(series_metadata::Column::Language.contains(value.clone()))
            }
            FieldOperator::DoesNotContain { value } => {
                query.filter(series_metadata::Column::Language.not_like(format!("%{}%", value)))
            }
            FieldOperator::BeginsWith { value } => {
                query.filter(series_metadata::Column::Language.starts_with(value.clone()))
            }
            FieldOperator::EndsWith { value } => {
                query.filter(series_metadata::Column::Language.ends_with(value.clone()))
            }
        };

        let series_ids: Vec<Uuid> = filtered_query
            .select_only()
            .column(series_metadata::Column::SeriesId)
            .into_tuple()
            .all(db)
            .await?;

        let result: HashSet<Uuid> = if let Some(candidates) = candidate_ids {
            series_ids
                .into_iter()
                .filter(|id| candidates.contains(id))
                .collect()
        } else {
            series_ids.into_iter().collect()
        };

        Ok(result)
    }

    async fn filter_by_name(
        db: &DatabaseConnection,
        operator: &FieldOperator,
        candidate_ids: Option<&HashSet<Uuid>>,
    ) -> Result<HashSet<Uuid>> {
        use crate::db::entities::series_metadata;
        use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QuerySelect};

        // Name is now stored as title in series_metadata table
        let query = series_metadata::Entity::find();

        let filtered_query = match operator {
            FieldOperator::Is { value } => {
                query.filter(series_metadata::Column::Title.eq(value.clone()))
            }
            FieldOperator::IsNot { value } => {
                query.filter(series_metadata::Column::Title.ne(value.clone()))
            }
            FieldOperator::IsNull => {
                // This doesn't make sense for series_metadata.title (required field)
                // Return empty set
                return Ok(HashSet::new());
            }
            FieldOperator::IsNotNull => {
                // All series_metadata records have a title, so return all
                query.filter(series_metadata::Column::Title.is_not_null())
            }
            FieldOperator::Contains { value } => {
                query.filter(series_metadata::Column::Title.contains(value.clone()))
            }
            FieldOperator::DoesNotContain { value } => {
                query.filter(series_metadata::Column::Title.not_like(format!("%{}%", value)))
            }
            FieldOperator::BeginsWith { value } => {
                query.filter(series_metadata::Column::Title.starts_with(value.clone()))
            }
            FieldOperator::EndsWith { value } => {
                query.filter(series_metadata::Column::Title.ends_with(value.clone()))
            }
        };

        // Select series_id from series_metadata (not the primary key)
        let series_ids: Vec<Uuid> = filtered_query
            .select_only()
            .column(series_metadata::Column::SeriesId)
            .into_tuple()
            .all(db)
            .await?;

        let result: HashSet<Uuid> = if let Some(candidates) = candidate_ids {
            series_ids
                .into_iter()
                .filter(|id| candidates.contains(id))
                .collect()
        } else {
            series_ids.into_iter().collect()
        };

        Ok(result)
    }

    /// Filter series by title_sort field in series_metadata
    ///
    /// This is used for alphabetical A-Z filtering where we filter by the first letter
    /// of the title_sort field. The matching is case-insensitive.
    async fn filter_by_title_sort(
        db: &DatabaseConnection,
        operator: &FieldOperator,
        candidate_ids: Option<&HashSet<Uuid>>,
    ) -> Result<HashSet<Uuid>> {
        use crate::db::entities::series_metadata;
        use sea_orm::{ColumnTrait, Condition, EntityTrait, QueryFilter, QuerySelect};

        let query = series_metadata::Entity::find();

        // For title_sort, we fall back to title if title_sort is null
        // We use COALESCE-like logic: if title_sort is set, use it; otherwise use title
        let filtered_query = match operator {
            FieldOperator::Is { value } => {
                // Exact match on title_sort or title (case-insensitive)
                let lower_value = value.to_lowercase();
                query.filter(
                    Condition::any()
                        .add(
                            series_metadata::Column::TitleSort.is_not_null().and(
                                sea_orm::prelude::Expr::expr(sea_orm::sea_query::Func::lower(
                                    sea_orm::prelude::Expr::col(series_metadata::Column::TitleSort),
                                ))
                                .eq(lower_value.clone()),
                            ),
                        )
                        .add(
                            series_metadata::Column::TitleSort.is_null().and(
                                sea_orm::prelude::Expr::expr(sea_orm::sea_query::Func::lower(
                                    sea_orm::prelude::Expr::col(series_metadata::Column::Title),
                                ))
                                .eq(lower_value),
                            ),
                        ),
                )
            }
            FieldOperator::IsNot { value } => {
                let lower_value = value.to_lowercase();
                query.filter(
                    Condition::all()
                        .add(
                            Condition::any()
                                .add(series_metadata::Column::TitleSort.is_null())
                                .add(
                                    sea_orm::prelude::Expr::expr(sea_orm::sea_query::Func::lower(
                                        sea_orm::prelude::Expr::col(
                                            series_metadata::Column::TitleSort,
                                        ),
                                    ))
                                    .ne(lower_value.clone()),
                                ),
                        )
                        .add(
                            Condition::any()
                                .add(series_metadata::Column::TitleSort.is_not_null())
                                .add(
                                    sea_orm::prelude::Expr::expr(sea_orm::sea_query::Func::lower(
                                        sea_orm::prelude::Expr::col(series_metadata::Column::Title),
                                    ))
                                    .ne(lower_value),
                                ),
                        ),
                )
            }
            FieldOperator::IsNull => {
                // Both title_sort and title are null - shouldn't happen as title is required
                return Ok(HashSet::new());
            }
            FieldOperator::IsNotNull => {
                // title is always set, so return all
                query.filter(series_metadata::Column::Title.is_not_null())
            }
            FieldOperator::Contains { value } => {
                let pattern = format!("%{}%", value.to_lowercase());
                query.filter(
                    Condition::any()
                        .add(
                            series_metadata::Column::TitleSort.is_not_null().and(
                                sea_orm::prelude::Expr::expr(sea_orm::sea_query::Func::lower(
                                    sea_orm::prelude::Expr::col(series_metadata::Column::TitleSort),
                                ))
                                .like(pattern.clone()),
                            ),
                        )
                        .add(
                            series_metadata::Column::TitleSort.is_null().and(
                                sea_orm::prelude::Expr::expr(sea_orm::sea_query::Func::lower(
                                    sea_orm::prelude::Expr::col(series_metadata::Column::Title),
                                ))
                                .like(pattern),
                            ),
                        ),
                )
            }
            FieldOperator::DoesNotContain { value } => {
                let pattern = format!("%{}%", value.to_lowercase());
                query.filter(
                    Condition::all()
                        .add(
                            Condition::any()
                                .add(series_metadata::Column::TitleSort.is_null())
                                .add(
                                    sea_orm::prelude::Expr::expr(sea_orm::sea_query::Func::lower(
                                        sea_orm::prelude::Expr::col(
                                            series_metadata::Column::TitleSort,
                                        ),
                                    ))
                                    .not_like(pattern.clone()),
                                ),
                        )
                        .add(
                            Condition::any()
                                .add(series_metadata::Column::TitleSort.is_not_null())
                                .add(
                                    sea_orm::prelude::Expr::expr(sea_orm::sea_query::Func::lower(
                                        sea_orm::prelude::Expr::col(series_metadata::Column::Title),
                                    ))
                                    .not_like(pattern),
                                ),
                        ),
                )
            }
            FieldOperator::BeginsWith { value } => {
                let pattern = format!("{}%", value.to_lowercase());
                query.filter(
                    Condition::any()
                        .add(
                            series_metadata::Column::TitleSort.is_not_null().and(
                                sea_orm::prelude::Expr::expr(sea_orm::sea_query::Func::lower(
                                    sea_orm::prelude::Expr::col(series_metadata::Column::TitleSort),
                                ))
                                .like(pattern.clone()),
                            ),
                        )
                        .add(
                            series_metadata::Column::TitleSort.is_null().and(
                                sea_orm::prelude::Expr::expr(sea_orm::sea_query::Func::lower(
                                    sea_orm::prelude::Expr::col(series_metadata::Column::Title),
                                ))
                                .like(pattern),
                            ),
                        ),
                )
            }
            FieldOperator::EndsWith { value } => {
                let pattern = format!("%{}", value.to_lowercase());
                query.filter(
                    Condition::any()
                        .add(
                            series_metadata::Column::TitleSort.is_not_null().and(
                                sea_orm::prelude::Expr::expr(sea_orm::sea_query::Func::lower(
                                    sea_orm::prelude::Expr::col(series_metadata::Column::TitleSort),
                                ))
                                .like(pattern.clone()),
                            ),
                        )
                        .add(
                            series_metadata::Column::TitleSort.is_null().and(
                                sea_orm::prelude::Expr::expr(sea_orm::sea_query::Func::lower(
                                    sea_orm::prelude::Expr::col(series_metadata::Column::Title),
                                ))
                                .like(pattern),
                            ),
                        ),
                )
            }
        };

        let series_ids: Vec<Uuid> = filtered_query
            .select_only()
            .column(series_metadata::Column::SeriesId)
            .into_tuple()
            .all(db)
            .await?;

        let result: HashSet<Uuid> = if let Some(candidates) = candidate_ids {
            series_ids
                .into_iter()
                .filter(|id| candidates.contains(id))
                .collect()
        } else {
            series_ids.into_iter().collect()
        };

        Ok(result)
    }

    /// Filter series by read status
    ///
    /// Read status values:
    /// - "unread": Series where all books have no read_progress OR all books have completed=false and current_page=0
    /// - "in_progress": Series where at least one book has read_progress with completed=false and current_page > 0
    /// - "read": Series where all books have read_progress with completed=true
    async fn filter_by_read_status(
        db: &DatabaseConnection,
        operator: &FieldOperator,
        candidate_ids: Option<&HashSet<Uuid>>,
        user_id: Option<Uuid>,
    ) -> Result<HashSet<Uuid>> {
        use crate::db::entities::{books, read_progress, series};
        use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QuerySelect};

        // If no user_id provided, we can't filter by read status
        let user_id = match user_id {
            Some(id) => id,
            None => return Ok(candidate_ids.cloned().unwrap_or_default()),
        };

        let status_value = match operator {
            FieldOperator::Is { value } => value.to_lowercase(),
            FieldOperator::IsNot { value } => value.to_lowercase(),
            // IsNull/IsNotNull don't make sense for read status
            _ => return Ok(candidate_ids.cloned().unwrap_or_default()),
        };

        let is_negated = matches!(operator, FieldOperator::IsNot { .. });

        // Get all series (or candidates)
        let series_ids: Vec<Uuid> = if let Some(candidates) = candidate_ids {
            candidates.iter().cloned().collect()
        } else {
            series::Entity::find()
                .select_only()
                .column(series::Column::Id)
                .into_tuple()
                .all(db)
                .await?
        };

        // For each series, determine its read status based on its books
        let mut matching_series = HashSet::new();

        for series_id in &series_ids {
            // Get all book IDs for this series
            let book_ids: Vec<Uuid> = books::Entity::find()
                .filter(books::Column::SeriesId.eq(*series_id))
                .filter(books::Column::Deleted.eq(false))
                .select_only()
                .column(books::Column::Id)
                .into_tuple()
                .all(db)
                .await?;

            if book_ids.is_empty() {
                // Series with no books is considered "unread"
                if (status_value == "unread") != is_negated {
                    matching_series.insert(*series_id);
                }
                continue;
            }

            // Get read progress for all books in this series for this user
            let progress_records: Vec<read_progress::Model> = read_progress::Entity::find()
                .filter(read_progress::Column::UserId.eq(user_id))
                .filter(read_progress::Column::BookId.is_in(book_ids.clone()))
                .all(db)
                .await?;

            // Build a map of book_id -> progress
            let progress_map: std::collections::HashMap<Uuid, &read_progress::Model> =
                progress_records.iter().map(|p| (p.book_id, p)).collect();

            // Determine series read status
            let total_books = book_ids.len();
            let mut read_count = 0;
            let mut in_progress_count = 0;

            for book_id in &book_ids {
                if let Some(progress) = progress_map.get(book_id) {
                    if progress.completed {
                        read_count += 1;
                    } else if progress.current_page > 0 {
                        in_progress_count += 1;
                    }
                }
            }

            let series_status = if read_count == total_books {
                "read"
            } else if in_progress_count > 0 || (read_count > 0 && read_count < total_books) {
                // In progress if any book is being read, OR if some books are read but not all
                "in_progress"
            } else {
                "unread"
            };

            let matches = (series_status == status_value) != is_negated;
            if matches {
                matching_series.insert(*series_id);
            }
        }

        Ok(matching_series)
    }
}

// Book condition evaluation
impl FilterService {
    /// Get book IDs matching a condition (without user context)
    ///
    /// Returns the set of book IDs that match the given condition.
    /// If candidate_ids is provided, only those books are considered.
    ///
    /// Note: ReadStatus filtering requires user context. Use `get_matching_books_for_user`
    /// if you need ReadStatus filtering support.
    pub fn get_matching_books<'a>(
        db: &'a DatabaseConnection,
        condition: &'a BookCondition,
        candidate_ids: Option<&'a HashSet<Uuid>>,
    ) -> Pin<Box<dyn Future<Output = Result<HashSet<Uuid>>> + Send + 'a>> {
        Self::get_matching_books_for_user(db, condition, candidate_ids, None)
    }

    /// Get book IDs matching a condition with user context for ReadStatus filtering
    ///
    /// Returns the set of book IDs that match the given condition.
    /// If candidate_ids is provided, only those books are considered.
    /// If user_id is provided, ReadStatus filtering will work correctly.
    pub fn get_matching_books_for_user<'a>(
        db: &'a DatabaseConnection,
        condition: &'a BookCondition,
        candidate_ids: Option<&'a HashSet<Uuid>>,
        user_id: Option<Uuid>,
    ) -> Pin<Box<dyn Future<Output = Result<HashSet<Uuid>>> + Send + 'a>> {
        Box::pin(async move {
            match condition {
                BookCondition::AllOf { all_of } => {
                    if all_of.is_empty() {
                        return Ok(candidate_ids.cloned().unwrap_or_default());
                    }

                    let mut result =
                        Self::get_matching_books_for_user(db, &all_of[0], candidate_ids, user_id)
                            .await?;

                    for cond in &all_of[1..] {
                        if result.is_empty() {
                            break;
                        }
                        let matching =
                            Self::get_matching_books_for_user(db, cond, Some(&result), user_id)
                                .await?;
                        result = result.intersection(&matching).cloned().collect();
                    }

                    Ok(result)
                }

                BookCondition::AnyOf { any_of } => {
                    if any_of.is_empty() {
                        return Ok(HashSet::new());
                    }

                    let mut result = HashSet::new();
                    for cond in any_of {
                        let matching =
                            Self::get_matching_books_for_user(db, cond, candidate_ids, user_id)
                                .await?;
                        result.extend(matching);
                    }

                    Ok(result)
                }

                BookCondition::LibraryId { library_id } => {
                    Self::filter_books_by_library_id(db, library_id, candidate_ids).await
                }

                BookCondition::SeriesId { series_id } => {
                    Self::filter_books_by_series_id(db, series_id, candidate_ids).await
                }

                BookCondition::Genre { genre } => {
                    Self::filter_books_by_genre(db, genre, candidate_ids).await
                }

                BookCondition::Tag { tag } => {
                    Self::filter_books_by_tag(db, tag, candidate_ids).await
                }

                BookCondition::Title { title } => {
                    Self::filter_books_by_title(db, title, candidate_ids).await
                }

                BookCondition::ReadStatus { read_status } => {
                    Self::filter_books_by_read_status(db, read_status, candidate_ids, user_id).await
                }

                BookCondition::HasError { has_error } => {
                    Self::filter_books_by_error(db, has_error, candidate_ids).await
                }

                BookCondition::BookType { book_type } => {
                    Self::filter_books_by_book_type(db, book_type, candidate_ids).await
                }
            }
        })
    }

    async fn filter_books_by_library_id(
        db: &DatabaseConnection,
        operator: &UuidOperator,
        candidate_ids: Option<&HashSet<Uuid>>,
    ) -> Result<HashSet<Uuid>> {
        use crate::db::entities::{books, series};
        use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QuerySelect};

        match operator {
            UuidOperator::Is { value } => {
                // Get series in this library, then get books from those series
                let series_ids: Vec<Uuid> = series::Entity::find()
                    .filter(series::Column::LibraryId.eq(*value))
                    .select_only()
                    .column(series::Column::Id)
                    .into_tuple()
                    .all(db)
                    .await?;

                let books_in_library: Vec<Uuid> = books::Entity::find()
                    .filter(books::Column::SeriesId.is_in(series_ids))
                    .select_only()
                    .column(books::Column::Id)
                    .into_tuple()
                    .all(db)
                    .await?;

                let result: HashSet<Uuid> = if let Some(candidates) = candidate_ids {
                    books_in_library
                        .into_iter()
                        .filter(|id| candidates.contains(id))
                        .collect()
                } else {
                    books_in_library.into_iter().collect()
                };

                Ok(result)
            }
            UuidOperator::IsNot { value } => {
                let series_ids: Vec<Uuid> = series::Entity::find()
                    .filter(series::Column::LibraryId.eq(*value))
                    .select_only()
                    .column(series::Column::Id)
                    .into_tuple()
                    .all(db)
                    .await?;

                let books_in_library: HashSet<Uuid> = books::Entity::find()
                    .filter(books::Column::SeriesId.is_in(series_ids))
                    .select_only()
                    .column(books::Column::Id)
                    .into_tuple()
                    .all(db)
                    .await?
                    .into_iter()
                    .collect();

                if let Some(candidates) = candidate_ids {
                    Ok(candidates
                        .iter()
                        .filter(|id| !books_in_library.contains(id))
                        .cloned()
                        .collect())
                } else {
                    let all_books: HashSet<Uuid> = books::Entity::find()
                        .select_only()
                        .column(books::Column::Id)
                        .into_tuple()
                        .all(db)
                        .await?
                        .into_iter()
                        .collect();

                    Ok(all_books
                        .into_iter()
                        .filter(|id| !books_in_library.contains(id))
                        .collect())
                }
            }
        }
    }

    async fn filter_books_by_series_id(
        db: &DatabaseConnection,
        operator: &UuidOperator,
        candidate_ids: Option<&HashSet<Uuid>>,
    ) -> Result<HashSet<Uuid>> {
        use crate::db::entities::books;
        use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QuerySelect};

        match operator {
            UuidOperator::Is { value } => {
                let books_in_series: Vec<Uuid> = books::Entity::find()
                    .filter(books::Column::SeriesId.eq(*value))
                    .select_only()
                    .column(books::Column::Id)
                    .into_tuple()
                    .all(db)
                    .await?;

                let result: HashSet<Uuid> = if let Some(candidates) = candidate_ids {
                    books_in_series
                        .into_iter()
                        .filter(|id| candidates.contains(id))
                        .collect()
                } else {
                    books_in_series.into_iter().collect()
                };

                Ok(result)
            }
            UuidOperator::IsNot { value } => {
                let books_in_series: HashSet<Uuid> = books::Entity::find()
                    .filter(books::Column::SeriesId.eq(*value))
                    .select_only()
                    .column(books::Column::Id)
                    .into_tuple()
                    .all(db)
                    .await?
                    .into_iter()
                    .collect();

                if let Some(candidates) = candidate_ids {
                    Ok(candidates
                        .iter()
                        .filter(|id| !books_in_series.contains(id))
                        .cloned()
                        .collect())
                } else {
                    let all_books: HashSet<Uuid> = books::Entity::find()
                        .select_only()
                        .column(books::Column::Id)
                        .into_tuple()
                        .all(db)
                        .await?
                        .into_iter()
                        .collect();

                    Ok(all_books
                        .into_iter()
                        .filter(|id| !books_in_series.contains(id))
                        .collect())
                }
            }
        }
    }

    async fn filter_books_by_genre(
        db: &DatabaseConnection,
        operator: &FieldOperator,
        candidate_ids: Option<&HashSet<Uuid>>,
    ) -> Result<HashSet<Uuid>> {
        use crate::db::entities::books;
        use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QuerySelect};

        // First get series matching the genre condition
        let series_condition = SeriesCondition::Genre {
            genre: operator.clone(),
        };
        let matching_series = Self::get_matching_series(db, &series_condition, None).await?;

        if matching_series.is_empty() {
            return Ok(HashSet::new());
        }

        // Then get books from those series
        let books_in_series: Vec<Uuid> = books::Entity::find()
            .filter(books::Column::SeriesId.is_in(matching_series))
            .select_only()
            .column(books::Column::Id)
            .into_tuple()
            .all(db)
            .await?;

        let result: HashSet<Uuid> = if let Some(candidates) = candidate_ids {
            books_in_series
                .into_iter()
                .filter(|id| candidates.contains(id))
                .collect()
        } else {
            books_in_series.into_iter().collect()
        };

        Ok(result)
    }

    async fn filter_books_by_tag(
        db: &DatabaseConnection,
        operator: &FieldOperator,
        candidate_ids: Option<&HashSet<Uuid>>,
    ) -> Result<HashSet<Uuid>> {
        use crate::db::entities::books;
        use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QuerySelect};

        // First get series matching the tag condition
        let series_condition = SeriesCondition::Tag {
            tag: operator.clone(),
        };
        let matching_series = Self::get_matching_series(db, &series_condition, None).await?;

        if matching_series.is_empty() {
            return Ok(HashSet::new());
        }

        let books_in_series: Vec<Uuid> = books::Entity::find()
            .filter(books::Column::SeriesId.is_in(matching_series))
            .select_only()
            .column(books::Column::Id)
            .into_tuple()
            .all(db)
            .await?;

        let result: HashSet<Uuid> = if let Some(candidates) = candidate_ids {
            books_in_series
                .into_iter()
                .filter(|id| candidates.contains(id))
                .collect()
        } else {
            books_in_series.into_iter().collect()
        };

        Ok(result)
    }

    async fn filter_books_by_title(
        db: &DatabaseConnection,
        operator: &FieldOperator,
        candidate_ids: Option<&HashSet<Uuid>>,
    ) -> Result<HashSet<Uuid>> {
        use crate::db::entities::book_metadata;
        use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QuerySelect};

        // Title is now stored in book_metadata table
        let query = book_metadata::Entity::find();

        let filtered_query = match operator {
            FieldOperator::Is { value } => {
                query.filter(book_metadata::Column::Title.eq(Some(value.clone())))
            }
            FieldOperator::IsNot { value } => {
                query.filter(book_metadata::Column::Title.ne(Some(value.clone())))
            }
            FieldOperator::IsNull => query.filter(book_metadata::Column::Title.is_null()),
            FieldOperator::IsNotNull => query.filter(book_metadata::Column::Title.is_not_null()),
            FieldOperator::Contains { value } => {
                query.filter(book_metadata::Column::Title.contains(value.clone()))
            }
            FieldOperator::DoesNotContain { value } => {
                query.filter(book_metadata::Column::Title.not_like(format!("%{}%", value)))
            }
            FieldOperator::BeginsWith { value } => {
                query.filter(book_metadata::Column::Title.starts_with(value.clone()))
            }
            FieldOperator::EndsWith { value } => {
                query.filter(book_metadata::Column::Title.ends_with(value.clone()))
            }
        };

        // Select book_id from book_metadata (not id)
        let book_ids: Vec<Uuid> = filtered_query
            .select_only()
            .column(book_metadata::Column::BookId)
            .into_tuple()
            .all(db)
            .await?;

        let result: HashSet<Uuid> = if let Some(candidates) = candidate_ids {
            book_ids
                .into_iter()
                .filter(|id| candidates.contains(id))
                .collect()
        } else {
            book_ids.into_iter().collect()
        };

        Ok(result)
    }

    /// Filter books by read status
    ///
    /// Read status values:
    /// - "unread": Books with no read_progress OR completed=false and current_page=0
    /// - "in_progress": Books with read_progress where completed=false and current_page > 0
    /// - "read": Books with read_progress where completed=true
    async fn filter_books_by_read_status(
        db: &DatabaseConnection,
        operator: &FieldOperator,
        candidate_ids: Option<&HashSet<Uuid>>,
        user_id: Option<Uuid>,
    ) -> Result<HashSet<Uuid>> {
        use crate::db::entities::{books, read_progress};
        use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QuerySelect};

        // If no user_id provided, we can't filter by read status
        let user_id = match user_id {
            Some(id) => id,
            None => return Ok(candidate_ids.cloned().unwrap_or_default()),
        };

        let status_value = match operator {
            FieldOperator::Is { value } => value.to_lowercase(),
            FieldOperator::IsNot { value } => value.to_lowercase(),
            // IsNull/IsNotNull don't make sense for read status
            _ => return Ok(candidate_ids.cloned().unwrap_or_default()),
        };

        let is_negated = matches!(operator, FieldOperator::IsNot { .. });

        // Get all books (or candidates)
        let book_ids: Vec<Uuid> = if let Some(candidates) = candidate_ids {
            candidates.iter().cloned().collect()
        } else {
            books::Entity::find()
                .filter(books::Column::Deleted.eq(false))
                .select_only()
                .column(books::Column::Id)
                .into_tuple()
                .all(db)
                .await?
        };

        if book_ids.is_empty() {
            return Ok(HashSet::new());
        }

        // Get read progress for all candidate books for this user
        let progress_records: Vec<read_progress::Model> = read_progress::Entity::find()
            .filter(read_progress::Column::UserId.eq(user_id))
            .filter(read_progress::Column::BookId.is_in(book_ids.clone()))
            .all(db)
            .await?;

        // Build a map of book_id -> progress
        let progress_map: std::collections::HashMap<Uuid, &read_progress::Model> =
            progress_records.iter().map(|p| (p.book_id, p)).collect();

        let mut matching_books = HashSet::new();

        for book_id in &book_ids {
            let book_status = match progress_map.get(book_id) {
                Some(progress) => {
                    if progress.completed {
                        "read"
                    } else if progress.current_page > 0 {
                        "in_progress"
                    } else {
                        "unread"
                    }
                }
                None => "unread",
            };

            let matches = (book_status == status_value) != is_negated;
            if matches {
                matching_books.insert(*book_id);
            }
        }

        Ok(matching_books)
    }

    async fn filter_books_by_error(
        db: &DatabaseConnection,
        operator: &BoolOperator,
        candidate_ids: Option<&HashSet<Uuid>>,
    ) -> Result<HashSet<Uuid>> {
        use crate::db::entities::books;
        use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QuerySelect};

        let query = books::Entity::find();

        let filtered_query = match operator {
            BoolOperator::IsTrue => query.filter(books::Column::AnalysisError.is_not_null()),
            BoolOperator::IsFalse => query.filter(books::Column::AnalysisError.is_null()),
        };

        let book_ids: Vec<Uuid> = filtered_query
            .select_only()
            .column(books::Column::Id)
            .into_tuple()
            .all(db)
            .await?;

        let result: HashSet<Uuid> = if let Some(candidates) = candidate_ids {
            book_ids
                .into_iter()
                .filter(|id| candidates.contains(id))
                .collect()
        } else {
            book_ids.into_iter().collect()
        };

        Ok(result)
    }

    async fn filter_books_by_book_type(
        db: &DatabaseConnection,
        operator: &FieldOperator,
        candidate_ids: Option<&HashSet<Uuid>>,
    ) -> Result<HashSet<Uuid>> {
        use crate::db::entities::book_metadata;
        use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QuerySelect};

        let query = book_metadata::Entity::find();

        let filtered_query = match operator {
            FieldOperator::Is { value } => {
                query.filter(book_metadata::Column::BookType.eq(value.clone()))
            }
            FieldOperator::IsNot { value } => {
                query.filter(book_metadata::Column::BookType.ne(value.clone()))
            }
            FieldOperator::IsNull => query.filter(book_metadata::Column::BookType.is_null()),
            FieldOperator::IsNotNull => query.filter(book_metadata::Column::BookType.is_not_null()),
            FieldOperator::Contains { value } => {
                query.filter(book_metadata::Column::BookType.contains(value.as_str()))
            }
            FieldOperator::DoesNotContain { value } => {
                query.filter(book_metadata::Column::BookType.not_like(format!("%{value}%")))
            }
            FieldOperator::BeginsWith { value } => {
                query.filter(book_metadata::Column::BookType.starts_with(value.as_str()))
            }
            FieldOperator::EndsWith { value } => {
                query.filter(book_metadata::Column::BookType.ends_with(value.as_str()))
            }
        };

        let mut book_ids_query = filtered_query
            .select_only()
            .column(book_metadata::Column::BookId);

        if let Some(candidates) = candidate_ids {
            let candidate_vec: Vec<Uuid> = candidates.iter().cloned().collect();
            book_ids_query =
                book_ids_query.filter(book_metadata::Column::BookId.is_in(candidate_vec));
        }

        let result: HashSet<Uuid> = book_ids_query
            .into_tuple()
            .all(db)
            .await?
            .into_iter()
            .collect();

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::routes::v1::dto::{
        BookCondition, FieldOperator, SeriesCondition, UuidOperator,
    };

    // Unit tests for condition building and basic logic

    #[test]
    fn test_empty_all_of_condition() {
        let condition = SeriesCondition::AllOf { all_of: vec![] };
        match condition {
            SeriesCondition::AllOf { all_of } => {
                assert!(all_of.is_empty());
            }
            _ => panic!("Expected AllOf condition"),
        }
    }

    #[test]
    fn test_empty_any_of_condition() {
        let condition = SeriesCondition::AnyOf { any_of: vec![] };
        match condition {
            SeriesCondition::AnyOf { any_of } => {
                assert!(any_of.is_empty());
            }
            _ => panic!("Expected AnyOf condition"),
        }
    }

    #[test]
    fn test_nested_all_of_any_of_condition() {
        // (Genre = Action AND Genre != Horror) OR (Genre = Comedy)
        let condition = SeriesCondition::AnyOf {
            any_of: vec![
                SeriesCondition::AllOf {
                    all_of: vec![
                        SeriesCondition::Genre {
                            genre: FieldOperator::Is {
                                value: "Action".to_string(),
                            },
                        },
                        SeriesCondition::Genre {
                            genre: FieldOperator::IsNot {
                                value: "Horror".to_string(),
                            },
                        },
                    ],
                },
                SeriesCondition::Genre {
                    genre: FieldOperator::Is {
                        value: "Comedy".to_string(),
                    },
                },
            ],
        };

        match condition {
            SeriesCondition::AnyOf { any_of } => {
                assert_eq!(any_of.len(), 2);
                match &any_of[0] {
                    SeriesCondition::AllOf { all_of } => {
                        assert_eq!(all_of.len(), 2);
                    }
                    _ => panic!("Expected first item to be AllOf"),
                }
            }
            _ => panic!("Expected AnyOf condition"),
        }
    }

    #[test]
    fn test_library_id_condition_is() {
        let uuid = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let condition = SeriesCondition::LibraryId {
            library_id: UuidOperator::Is { value: uuid },
        };

        match condition {
            SeriesCondition::LibraryId { library_id } => match library_id {
                UuidOperator::Is { value } => {
                    assert_eq!(value, uuid);
                }
                _ => panic!("Expected Is operator"),
            },
            _ => panic!("Expected LibraryId condition"),
        }
    }

    #[test]
    fn test_library_id_condition_is_not() {
        let uuid = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let condition = SeriesCondition::LibraryId {
            library_id: UuidOperator::IsNot { value: uuid },
        };

        match condition {
            SeriesCondition::LibraryId { library_id } => match library_id {
                UuidOperator::IsNot { value } => {
                    assert_eq!(value, uuid);
                }
                _ => panic!("Expected IsNot operator"),
            },
            _ => panic!("Expected LibraryId condition"),
        }
    }

    #[test]
    fn test_field_operator_is() {
        let operator = FieldOperator::Is {
            value: "Action".to_string(),
        };
        match operator {
            FieldOperator::Is { value } => {
                assert_eq!(value, "Action");
            }
            _ => panic!("Expected Is operator"),
        }
    }

    #[test]
    fn test_field_operator_is_not() {
        let operator = FieldOperator::IsNot {
            value: "Horror".to_string(),
        };
        match operator {
            FieldOperator::IsNot { value } => {
                assert_eq!(value, "Horror");
            }
            _ => panic!("Expected IsNot operator"),
        }
    }

    #[test]
    fn test_field_operator_contains() {
        let operator = FieldOperator::Contains {
            value: "Act".to_string(),
        };
        match operator {
            FieldOperator::Contains { value } => {
                assert_eq!(value, "Act");
            }
            _ => panic!("Expected Contains operator"),
        }
    }

    #[test]
    fn test_field_operator_is_null() {
        let operator = FieldOperator::IsNull;
        assert!(matches!(operator, FieldOperator::IsNull));
    }

    #[test]
    fn test_field_operator_is_not_null() {
        let operator = FieldOperator::IsNotNull;
        assert!(matches!(operator, FieldOperator::IsNotNull));
    }

    #[test]
    fn test_book_condition_series_id() {
        let uuid = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let condition = BookCondition::SeriesId {
            series_id: UuidOperator::Is { value: uuid },
        };

        match condition {
            BookCondition::SeriesId { series_id } => match series_id {
                UuidOperator::Is { value } => {
                    assert_eq!(value, uuid);
                }
                _ => panic!("Expected Is operator"),
            },
            _ => panic!("Expected SeriesId condition"),
        }
    }

    #[test]
    fn test_book_condition_has_error() {
        let condition = BookCondition::HasError {
            has_error: BoolOperator::IsTrue,
        };

        match condition {
            BookCondition::HasError { has_error } => {
                assert!(matches!(has_error, BoolOperator::IsTrue));
            }
            _ => panic!("Expected HasError condition"),
        }
    }

    #[test]
    fn test_book_condition_title() {
        let condition = BookCondition::Title {
            title: FieldOperator::Contains {
                value: "Chapter".to_string(),
            },
        };

        match condition {
            BookCondition::Title { title } => match title {
                FieldOperator::Contains { value } => {
                    assert_eq!(value, "Chapter");
                }
                _ => panic!("Expected Contains operator"),
            },
            _ => panic!("Expected Title condition"),
        }
    }

    #[test]
    fn test_series_condition_status() {
        let condition = SeriesCondition::Status {
            status: FieldOperator::Is {
                value: "ongoing".to_string(),
            },
        };

        match condition {
            SeriesCondition::Status { status } => match status {
                FieldOperator::Is { value } => {
                    assert_eq!(value, "ongoing");
                }
                _ => panic!("Expected Is operator"),
            },
            _ => panic!("Expected Status condition"),
        }
    }

    #[test]
    fn test_series_condition_publisher() {
        let condition = SeriesCondition::Publisher {
            publisher: FieldOperator::Contains {
                value: "Viz".to_string(),
            },
        };

        match condition {
            SeriesCondition::Publisher { publisher } => match publisher {
                FieldOperator::Contains { value } => {
                    assert_eq!(value, "Viz");
                }
                _ => panic!("Expected Contains operator"),
            },
            _ => panic!("Expected Publisher condition"),
        }
    }

    #[test]
    fn test_series_condition_language() {
        let condition = SeriesCondition::Language {
            language: FieldOperator::Is {
                value: "ja".to_string(),
            },
        };

        match condition {
            SeriesCondition::Language { language } => match language {
                FieldOperator::Is { value } => {
                    assert_eq!(value, "ja");
                }
                _ => panic!("Expected Is operator"),
            },
            _ => panic!("Expected Language condition"),
        }
    }

    #[test]
    fn test_series_condition_name() {
        let condition = SeriesCondition::Name {
            name: FieldOperator::BeginsWith {
                value: "Naruto".to_string(),
            },
        };

        match condition {
            SeriesCondition::Name { name } => match name {
                FieldOperator::BeginsWith { value } => {
                    assert_eq!(value, "Naruto");
                }
                _ => panic!("Expected BeginsWith operator"),
            },
            _ => panic!("Expected Name condition"),
        }
    }

    #[test]
    fn test_complex_book_condition() {
        // Books in library X AND (has Action genre OR has Comedy genre) AND NOT Horror
        let library_uuid = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let condition = BookCondition::AllOf {
            all_of: vec![
                BookCondition::LibraryId {
                    library_id: UuidOperator::Is {
                        value: library_uuid,
                    },
                },
                BookCondition::AnyOf {
                    any_of: vec![
                        BookCondition::Genre {
                            genre: FieldOperator::Is {
                                value: "Action".to_string(),
                            },
                        },
                        BookCondition::Genre {
                            genre: FieldOperator::Is {
                                value: "Comedy".to_string(),
                            },
                        },
                    ],
                },
                BookCondition::Genre {
                    genre: FieldOperator::IsNot {
                        value: "Horror".to_string(),
                    },
                },
            ],
        };

        match condition {
            BookCondition::AllOf { all_of } => {
                assert_eq!(all_of.len(), 3);
                // First should be LibraryId
                assert!(matches!(&all_of[0], BookCondition::LibraryId { .. }));
                // Second should be AnyOf
                match &all_of[1] {
                    BookCondition::AnyOf { any_of } => {
                        assert_eq!(any_of.len(), 2);
                    }
                    _ => panic!("Expected AnyOf"),
                }
                // Third should be Genre with IsNot
                match &all_of[2] {
                    BookCondition::Genre { genre } => {
                        assert!(matches!(genre, FieldOperator::IsNot { .. }));
                    }
                    _ => panic!("Expected Genre"),
                }
            }
            _ => panic!("Expected AllOf condition"),
        }
    }

    #[test]
    fn test_series_condition_sharing_tag_is() {
        let condition = SeriesCondition::SharingTag {
            sharing_tag: FieldOperator::Is {
                value: "Kids Content".to_string(),
            },
        };

        match condition {
            SeriesCondition::SharingTag { sharing_tag } => match sharing_tag {
                FieldOperator::Is { value } => {
                    assert_eq!(value, "Kids Content");
                }
                _ => panic!("Expected Is operator"),
            },
            _ => panic!("Expected SharingTag condition"),
        }
    }

    #[test]
    fn test_series_condition_sharing_tag_is_not() {
        let condition = SeriesCondition::SharingTag {
            sharing_tag: FieldOperator::IsNot {
                value: "Adults Only".to_string(),
            },
        };

        match condition {
            SeriesCondition::SharingTag { sharing_tag } => match sharing_tag {
                FieldOperator::IsNot { value } => {
                    assert_eq!(value, "Adults Only");
                }
                _ => panic!("Expected IsNot operator"),
            },
            _ => panic!("Expected SharingTag condition"),
        }
    }

    #[test]
    fn test_series_condition_sharing_tag_contains() {
        let condition = SeriesCondition::SharingTag {
            sharing_tag: FieldOperator::Contains {
                value: "Kids".to_string(),
            },
        };

        match condition {
            SeriesCondition::SharingTag { sharing_tag } => match sharing_tag {
                FieldOperator::Contains { value } => {
                    assert_eq!(value, "Kids");
                }
                _ => panic!("Expected Contains operator"),
            },
            _ => panic!("Expected SharingTag condition"),
        }
    }

    #[test]
    fn test_series_condition_sharing_tag_is_null() {
        let condition = SeriesCondition::SharingTag {
            sharing_tag: FieldOperator::IsNull,
        };

        match condition {
            SeriesCondition::SharingTag { sharing_tag } => {
                assert!(matches!(sharing_tag, FieldOperator::IsNull));
            }
            _ => panic!("Expected SharingTag condition"),
        }
    }

    #[test]
    fn test_series_condition_sharing_tag_is_not_null() {
        let condition = SeriesCondition::SharingTag {
            sharing_tag: FieldOperator::IsNotNull,
        };

        match condition {
            SeriesCondition::SharingTag { sharing_tag } => {
                assert!(matches!(sharing_tag, FieldOperator::IsNotNull));
            }
            _ => panic!("Expected SharingTag condition"),
        }
    }

    #[test]
    fn test_series_condition_has_external_source_id_is_true() {
        let condition = SeriesCondition::HasExternalSourceId {
            has_external_source_id: BoolOperator::IsTrue,
        };

        match condition {
            SeriesCondition::HasExternalSourceId {
                has_external_source_id,
            } => {
                assert!(matches!(has_external_source_id, BoolOperator::IsTrue));
            }
            _ => panic!("Expected HasExternalSourceId condition"),
        }
    }

    #[test]
    fn test_series_condition_has_external_source_id_is_false() {
        let condition = SeriesCondition::HasExternalSourceId {
            has_external_source_id: BoolOperator::IsFalse,
        };

        match condition {
            SeriesCondition::HasExternalSourceId {
                has_external_source_id,
            } => {
                assert!(matches!(has_external_source_id, BoolOperator::IsFalse));
            }
            _ => panic!("Expected HasExternalSourceId condition"),
        }
    }
}
