use crate::api::dto::{BookCondition, BoolOperator, FieldOperator, SeriesCondition, UuidOperator};
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
    /// Get series IDs matching a condition
    ///
    /// Returns the set of series IDs that match the given condition.
    /// If candidate_ids is provided, only those series are considered.
    pub fn get_matching_series<'a>(
        db: &'a DatabaseConnection,
        condition: &'a SeriesCondition,
        candidate_ids: Option<&'a HashSet<Uuid>>,
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
                        Self::get_matching_series(db, &all_of[0], candidate_ids).await?;

                    // Intersect with remaining conditions
                    for cond in &all_of[1..] {
                        if result.is_empty() {
                            break; // Short-circuit
                        }
                        let matching = Self::get_matching_series(db, cond, Some(&result)).await?;
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
                        let matching = Self::get_matching_series(db, cond, candidate_ids).await?;
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

                SeriesCondition::ReadStatus { read_status } => {
                    Self::filter_by_read_status(db, read_status, candidate_ids).await
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
                query.filter(series_metadata::Column::Status.not_like(&format!("%{}%", value)))
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
                query.filter(series_metadata::Column::Publisher.not_like(&format!("%{}%", value)))
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
                query.filter(series_metadata::Column::Language.not_like(&format!("%{}%", value)))
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
        use crate::db::entities::series;
        use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QuerySelect};

        let query = series::Entity::find();

        let filtered_query = match operator {
            FieldOperator::Is { value } => query.filter(series::Column::Name.eq(value.clone())),
            FieldOperator::IsNot { value } => query.filter(series::Column::Name.ne(value.clone())),
            FieldOperator::IsNull => query.filter(series::Column::Name.is_null()),
            FieldOperator::IsNotNull => query.filter(series::Column::Name.is_not_null()),
            FieldOperator::Contains { value } => {
                query.filter(series::Column::Name.contains(value.clone()))
            }
            FieldOperator::DoesNotContain { value } => {
                query.filter(series::Column::Name.not_like(&format!("%{}%", value)))
            }
            FieldOperator::BeginsWith { value } => {
                query.filter(series::Column::Name.starts_with(value.clone()))
            }
            FieldOperator::EndsWith { value } => {
                query.filter(series::Column::Name.ends_with(value.clone()))
            }
        };

        let series_ids: Vec<Uuid> = filtered_query
            .select_only()
            .column(series::Column::Id)
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

    async fn filter_by_read_status(
        _db: &DatabaseConnection,
        _operator: &FieldOperator,
        _candidate_ids: Option<&HashSet<Uuid>>,
    ) -> Result<HashSet<Uuid>> {
        // TODO: Implement read status filtering using read_progress
        Ok(HashSet::new())
    }
}

// Book condition evaluation
impl FilterService {
    /// Get book IDs matching a condition
    ///
    /// Returns the set of book IDs that match the given condition.
    /// If candidate_ids is provided, only those books are considered.
    pub fn get_matching_books<'a>(
        db: &'a DatabaseConnection,
        condition: &'a BookCondition,
        candidate_ids: Option<&'a HashSet<Uuid>>,
    ) -> Pin<Box<dyn Future<Output = Result<HashSet<Uuid>>> + Send + 'a>> {
        Box::pin(async move {
            match condition {
                BookCondition::AllOf { all_of } => {
                    if all_of.is_empty() {
                        return Ok(candidate_ids.cloned().unwrap_or_default());
                    }

                    let mut result =
                        Self::get_matching_books(db, &all_of[0], candidate_ids).await?;

                    for cond in &all_of[1..] {
                        if result.is_empty() {
                            break;
                        }
                        let matching = Self::get_matching_books(db, cond, Some(&result)).await?;
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
                        let matching = Self::get_matching_books(db, cond, candidate_ids).await?;
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
                    Self::filter_books_by_read_status(db, read_status, candidate_ids).await
                }

                BookCondition::HasError { has_error } => {
                    Self::filter_books_by_error(db, has_error, candidate_ids).await
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
        use crate::db::entities::books;
        use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QuerySelect};

        let query = books::Entity::find();

        let filtered_query = match operator {
            FieldOperator::Is { value } => query.filter(books::Column::Title.eq(value.clone())),
            FieldOperator::IsNot { value } => query.filter(books::Column::Title.ne(value.clone())),
            FieldOperator::IsNull => query.filter(books::Column::Title.is_null()),
            FieldOperator::IsNotNull => query.filter(books::Column::Title.is_not_null()),
            FieldOperator::Contains { value } => {
                query.filter(books::Column::Title.contains(value.clone()))
            }
            FieldOperator::DoesNotContain { value } => {
                query.filter(books::Column::Title.not_like(&format!("%{}%", value)))
            }
            FieldOperator::BeginsWith { value } => {
                query.filter(books::Column::Title.starts_with(value.clone()))
            }
            FieldOperator::EndsWith { value } => {
                query.filter(books::Column::Title.ends_with(value.clone()))
            }
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

    async fn filter_books_by_read_status(
        _db: &DatabaseConnection,
        _operator: &FieldOperator,
        _candidate_ids: Option<&HashSet<Uuid>>,
    ) -> Result<HashSet<Uuid>> {
        // TODO: Implement read status filtering using read_progress table
        // Would need user_id context to filter by their read status
        Ok(HashSet::new())
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::dto::{BookCondition, FieldOperator, SeriesCondition, UuidOperator};

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
}
