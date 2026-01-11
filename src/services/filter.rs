use crate::api::dto::{
    BookCondition, BoolOperator, FieldOperator, SeriesCondition, UuidOperator,
};
use crate::db::repositories::{GenreRepository, TagRepository};
use anyhow::Result;
use sea_orm::DatabaseConnection;
use std::collections::HashSet;
use std::pin::Pin;
use std::future::Future;
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

                SeriesCondition::Tag { tag } => {
                    Self::filter_by_tag(db, tag, candidate_ids).await
                }

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
        _db: &DatabaseConnection,
        _operator: &FieldOperator,
        _candidate_ids: Option<&HashSet<Uuid>>,
    ) -> Result<HashSet<Uuid>> {
        // TODO: Implement status filtering when series_metadata.status is added
        // For now, return empty set (no matches)
        Ok(HashSet::new())
    }

    async fn filter_by_publisher(
        _db: &DatabaseConnection,
        _operator: &FieldOperator,
        _candidate_ids: Option<&HashSet<Uuid>>,
    ) -> Result<HashSet<Uuid>> {
        // TODO: Implement publisher filtering using series_metadata.publisher
        Ok(HashSet::new())
    }

    async fn filter_by_language(
        _db: &DatabaseConnection,
        _operator: &FieldOperator,
        _candidate_ids: Option<&HashSet<Uuid>>,
    ) -> Result<HashSet<Uuid>> {
        // TODO: Implement language filtering using series_metadata.language
        Ok(HashSet::new())
    }

    async fn filter_by_name(
        _db: &DatabaseConnection,
        _operator: &FieldOperator,
        _candidate_ids: Option<&HashSet<Uuid>>,
    ) -> Result<HashSet<Uuid>> {
        // TODO: Implement name filtering using series.name
        Ok(HashSet::new())
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
