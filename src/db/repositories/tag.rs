//! Repository for tags, series_tags, and book_tags table operations
//!
//! TODO: Remove allow(dead_code) when all tag features are fully integrated

#![allow(dead_code)]

use anyhow::Result;
use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, Set,
};
use uuid::Uuid;

use crate::db::entities::{
    book_tags, book_tags::Entity as BookTags, series_tags, tags, tags::Entity as Tags,
};

/// Repository for tag operations
pub struct TagRepository;

impl TagRepository {
    /// Get a tag by ID
    pub async fn get_by_id(db: &DatabaseConnection, id: Uuid) -> Result<Option<tags::Model>> {
        let result = Tags::find_by_id(id).one(db).await?;
        Ok(result)
    }

    /// Get a tag by normalized name
    pub async fn get_by_name(db: &DatabaseConnection, name: &str) -> Result<Option<tags::Model>> {
        let normalized = name.to_lowercase().trim().to_string();
        let result = Tags::find()
            .filter(tags::Column::NormalizedName.eq(&normalized))
            .one(db)
            .await?;
        Ok(result)
    }

    /// List all tags sorted by name
    pub async fn list_all(db: &DatabaseConnection) -> Result<Vec<tags::Model>> {
        let results = Tags::find()
            .order_by_asc(tags::Column::Name)
            .all(db)
            .await?;
        Ok(results)
    }

    /// Create a new tag
    pub async fn create(db: &DatabaseConnection, name: &str) -> Result<tags::Model> {
        let normalized = name.to_lowercase().trim().to_string();
        let trimmed_name = name.trim().to_string();

        let active_model = tags::ActiveModel {
            id: Set(Uuid::new_v4()),
            name: Set(trimmed_name),
            normalized_name: Set(normalized),
            created_at: Set(Utc::now()),
        };

        let model = active_model.insert(db).await?;
        Ok(model)
    }

    /// Find or create a tag by name
    /// Returns the existing tag if found, otherwise creates a new one
    pub async fn find_or_create(db: &DatabaseConnection, name: &str) -> Result<tags::Model> {
        if let Some(existing) = Self::get_by_name(db, name).await? {
            return Ok(existing);
        }
        Self::create(db, name).await
    }

    /// Delete a tag by ID
    pub async fn delete(db: &DatabaseConnection, id: Uuid) -> Result<bool> {
        let result = Tags::delete_by_id(id).exec(db).await?;
        Ok(result.rows_affected > 0)
    }

    /// Get all tags for a series
    pub async fn get_tags_for_series(
        db: &DatabaseConnection,
        series_id: Uuid,
    ) -> Result<Vec<tags::Model>> {
        use crate::db::entities::series_tags::Entity as SeriesTags;

        let tag_ids: Vec<Uuid> = SeriesTags::find()
            .filter(series_tags::Column::SeriesId.eq(series_id))
            .all(db)
            .await?
            .into_iter()
            .map(|st| st.tag_id)
            .collect();

        if tag_ids.is_empty() {
            return Ok(vec![]);
        }

        let tags = Tags::find()
            .filter(tags::Column::Id.is_in(tag_ids))
            .order_by_asc(tags::Column::Name)
            .all(db)
            .await?;

        Ok(tags)
    }

    /// Set tags for a series (replaces existing)
    /// Takes a list of tag names, finds or creates each, then links them
    pub async fn set_tags_for_series(
        db: &DatabaseConnection,
        series_id: Uuid,
        tag_names: Vec<String>,
    ) -> Result<Vec<tags::Model>> {
        use crate::db::entities::series_tags::Entity as SeriesTags;

        // Remove existing tag links for this series
        SeriesTags::delete_many()
            .filter(series_tags::Column::SeriesId.eq(series_id))
            .exec(db)
            .await?;

        if tag_names.is_empty() {
            return Ok(vec![]);
        }

        // Find or create each tag and link it
        let mut tags = Vec::new();
        for name in tag_names {
            let tag = Self::find_or_create(db, &name).await?;

            // Create the link
            let link = series_tags::ActiveModel {
                series_id: Set(series_id),
                tag_id: Set(tag.id),
            };
            link.insert(db).await?;

            tags.push(tag);
        }

        // Sort by name before returning
        tags.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(tags)
    }

    /// Add a single tag to a series
    pub async fn add_tag_to_series(
        db: &DatabaseConnection,
        series_id: Uuid,
        tag_name: &str,
    ) -> Result<tags::Model> {
        let tag = Self::find_or_create(db, tag_name).await?;

        // Check if already linked
        use crate::db::entities::series_tags::Entity as SeriesTags;
        let existing = SeriesTags::find()
            .filter(series_tags::Column::SeriesId.eq(series_id))
            .filter(series_tags::Column::TagId.eq(tag.id))
            .one(db)
            .await?;

        if existing.is_none() {
            let link = series_tags::ActiveModel {
                series_id: Set(series_id),
                tag_id: Set(tag.id),
            };
            link.insert(db).await?;
        }

        Ok(tag)
    }

    /// Remove a tag from a series
    pub async fn remove_tag_from_series(
        db: &DatabaseConnection,
        series_id: Uuid,
        tag_id: Uuid,
    ) -> Result<bool> {
        use crate::db::entities::series_tags::Entity as SeriesTags;

        let result = SeriesTags::delete_many()
            .filter(series_tags::Column::SeriesId.eq(series_id))
            .filter(series_tags::Column::TagId.eq(tag_id))
            .exec(db)
            .await?;

        Ok(result.rows_affected > 0)
    }

    /// Count series using a tag
    pub async fn count_series_with_tag(db: &DatabaseConnection, tag_id: Uuid) -> Result<u64> {
        use crate::db::entities::series_tags::Entity as SeriesTags;

        let count = SeriesTags::find()
            .filter(series_tags::Column::TagId.eq(tag_id))
            .count(db)
            .await?;

        Ok(count)
    }

    /// Get all series IDs that have a specific tag (by normalized name)
    pub async fn get_series_ids_by_tag_name(
        db: &DatabaseConnection,
        tag_name: &str,
    ) -> Result<Vec<Uuid>> {
        use crate::db::entities::series_tags::Entity as SeriesTags;

        let normalized = tag_name.to_lowercase().trim().to_string();

        // First find the tag
        let tag = Tags::find()
            .filter(tags::Column::NormalizedName.eq(&normalized))
            .one(db)
            .await?;

        match tag {
            Some(t) => {
                let series_ids: Vec<Uuid> = SeriesTags::find()
                    .filter(series_tags::Column::TagId.eq(t.id))
                    .all(db)
                    .await?
                    .into_iter()
                    .map(|st| st.series_id)
                    .collect();

                Ok(series_ids)
            }
            None => Ok(vec![]),
        }
    }

    /// Get all series IDs that have ALL of the specified tags (AND logic)
    pub async fn get_series_ids_by_tag_names(
        db: &DatabaseConnection,
        tag_names: &[String],
    ) -> Result<Vec<Uuid>> {
        if tag_names.is_empty() {
            return Ok(vec![]);
        }

        // Get series IDs for the first tag
        let mut result_ids = Self::get_series_ids_by_tag_name(db, &tag_names[0]).await?;

        // Intersect with series IDs for remaining tags
        for name in &tag_names[1..] {
            let ids = Self::get_series_ids_by_tag_name(db, name).await?;
            result_ids.retain(|id| ids.contains(id));

            // Early exit if no matches
            if result_ids.is_empty() {
                break;
            }
        }

        Ok(result_ids)
    }

    /// Get all series IDs that have a specific tag (alias for get_series_ids_by_tag_name)
    pub async fn get_series_with_tag(db: &DatabaseConnection, tag_name: &str) -> Result<Vec<Uuid>> {
        Self::get_series_ids_by_tag_name(db, tag_name).await
    }

    /// Get all series IDs that have any tag containing the given substring (case-insensitive)
    pub async fn get_series_with_tag_containing(
        db: &DatabaseConnection,
        substring: &str,
    ) -> Result<Vec<Uuid>> {
        use crate::db::entities::series_tags::Entity as SeriesTags;

        let normalized = substring.to_lowercase();

        let matching_tags: Vec<tags::Model> = Tags::find()
            .filter(tags::Column::NormalizedName.contains(&normalized))
            .all(db)
            .await?;

        if matching_tags.is_empty() {
            return Ok(vec![]);
        }

        let tag_ids: Vec<Uuid> = matching_tags.iter().map(|t| t.id).collect();

        let series_ids: Vec<Uuid> = SeriesTags::find()
            .filter(series_tags::Column::TagId.is_in(tag_ids))
            .all(db)
            .await?
            .into_iter()
            .map(|st| st.series_id)
            .collect();

        let unique: std::collections::HashSet<Uuid> = series_ids.into_iter().collect();
        Ok(unique.into_iter().collect())
    }

    /// Get all series IDs that have any tag starting with the given prefix (case-insensitive)
    pub async fn get_series_with_tag_starting_with(
        db: &DatabaseConnection,
        prefix: &str,
    ) -> Result<Vec<Uuid>> {
        use crate::db::entities::series_tags::Entity as SeriesTags;

        let normalized = prefix.to_lowercase();

        let matching_tags: Vec<tags::Model> = Tags::find()
            .filter(tags::Column::NormalizedName.starts_with(&normalized))
            .all(db)
            .await?;

        if matching_tags.is_empty() {
            return Ok(vec![]);
        }

        let tag_ids: Vec<Uuid> = matching_tags.iter().map(|t| t.id).collect();

        let series_ids: Vec<Uuid> = SeriesTags::find()
            .filter(series_tags::Column::TagId.is_in(tag_ids))
            .all(db)
            .await?
            .into_iter()
            .map(|st| st.series_id)
            .collect();

        let unique: std::collections::HashSet<Uuid> = series_ids.into_iter().collect();
        Ok(unique.into_iter().collect())
    }

    /// Get all series IDs that have any tag ending with the given suffix (case-insensitive)
    pub async fn get_series_with_tag_ending_with(
        db: &DatabaseConnection,
        suffix: &str,
    ) -> Result<Vec<Uuid>> {
        use crate::db::entities::series_tags::Entity as SeriesTags;

        let normalized = suffix.to_lowercase();

        let matching_tags: Vec<tags::Model> = Tags::find()
            .filter(tags::Column::NormalizedName.ends_with(&normalized))
            .all(db)
            .await?;

        if matching_tags.is_empty() {
            return Ok(vec![]);
        }

        let tag_ids: Vec<Uuid> = matching_tags.iter().map(|t| t.id).collect();

        let series_ids: Vec<Uuid> = SeriesTags::find()
            .filter(series_tags::Column::TagId.is_in(tag_ids))
            .all(db)
            .await?
            .into_iter()
            .map(|st| st.series_id)
            .collect();

        let unique: std::collections::HashSet<Uuid> = series_ids.into_iter().collect();
        Ok(unique.into_iter().collect())
    }

    /// Get all series IDs that have at least one tag
    pub async fn get_all_series_with_tags(db: &DatabaseConnection) -> Result<Vec<Uuid>> {
        use crate::db::entities::series_tags::Entity as SeriesTags;

        let series_ids: Vec<Uuid> = SeriesTags::find()
            .all(db)
            .await?
            .into_iter()
            .map(|st| st.series_id)
            .collect();

        let unique: std::collections::HashSet<Uuid> = series_ids.into_iter().collect();
        Ok(unique.into_iter().collect())
    }

    /// Get tags for multiple series by their IDs
    ///
    /// Returns a HashMap keyed by series_id for efficient lookups
    pub async fn get_tags_for_series_ids(
        db: &DatabaseConnection,
        series_ids: &[Uuid],
    ) -> Result<std::collections::HashMap<Uuid, Vec<tags::Model>>> {
        use crate::db::entities::series_tags::Entity as SeriesTags;

        if series_ids.is_empty() {
            return Ok(std::collections::HashMap::new());
        }

        // Get all series_tags mappings for the given series
        let series_tag_links: Vec<series_tags::Model> = SeriesTags::find()
            .filter(series_tags::Column::SeriesId.is_in(series_ids.to_vec()))
            .all(db)
            .await?;

        if series_tag_links.is_empty() {
            return Ok(std::collections::HashMap::new());
        }

        // Collect unique tag IDs
        let tag_ids: Vec<Uuid> = series_tag_links
            .iter()
            .map(|st| st.tag_id)
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();

        // Fetch all tags at once
        let all_tags: Vec<tags::Model> = Tags::find()
            .filter(tags::Column::Id.is_in(tag_ids))
            .all(db)
            .await?;

        // Create tag lookup map
        let tag_map: std::collections::HashMap<Uuid, tags::Model> =
            all_tags.into_iter().map(|t| (t.id, t)).collect();

        // Build result map
        let mut result: std::collections::HashMap<Uuid, Vec<tags::Model>> =
            std::collections::HashMap::new();

        for link in series_tag_links {
            if let Some(tag) = tag_map.get(&link.tag_id) {
                result.entry(link.series_id).or_default().push(tag.clone());
            }
        }

        // Sort tags by name within each series
        for tags in result.values_mut() {
            tags.sort_by(|a, b| a.name.cmp(&b.name));
        }

        Ok(result)
    }

    /// Get all tags for a book
    pub async fn get_tags_for_book(
        db: &DatabaseConnection,
        book_id: Uuid,
    ) -> Result<Vec<tags::Model>> {
        let tag_ids: Vec<Uuid> = BookTags::find()
            .filter(book_tags::Column::BookId.eq(book_id))
            .all(db)
            .await?
            .into_iter()
            .map(|bt| bt.tag_id)
            .collect();

        if tag_ids.is_empty() {
            return Ok(vec![]);
        }

        let tags = Tags::find()
            .filter(tags::Column::Id.is_in(tag_ids))
            .order_by_asc(tags::Column::Name)
            .all(db)
            .await?;

        Ok(tags)
    }

    /// Set tags for a book (replaces existing)
    /// Takes a list of tag names, finds or creates each, then links them
    pub async fn set_tags_for_book(
        db: &DatabaseConnection,
        book_id: Uuid,
        tag_names: Vec<String>,
    ) -> Result<Vec<tags::Model>> {
        // Remove existing tag links for this book
        BookTags::delete_many()
            .filter(book_tags::Column::BookId.eq(book_id))
            .exec(db)
            .await?;

        if tag_names.is_empty() {
            return Ok(vec![]);
        }

        // Find or create each tag and link it
        let mut tags = Vec::new();
        for name in tag_names {
            let tag = Self::find_or_create(db, &name).await?;

            // Create the link
            let link = book_tags::ActiveModel {
                book_id: Set(book_id),
                tag_id: Set(tag.id),
            };
            link.insert(db).await?;

            tags.push(tag);
        }

        // Sort by name before returning
        tags.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(tags)
    }

    /// Add a single tag to a book
    pub async fn add_tag_to_book(
        db: &DatabaseConnection,
        book_id: Uuid,
        tag_name: &str,
    ) -> Result<tags::Model> {
        let tag = Self::find_or_create(db, tag_name).await?;

        // Check if already linked
        let existing = BookTags::find()
            .filter(book_tags::Column::BookId.eq(book_id))
            .filter(book_tags::Column::TagId.eq(tag.id))
            .one(db)
            .await?;

        if existing.is_none() {
            let link = book_tags::ActiveModel {
                book_id: Set(book_id),
                tag_id: Set(tag.id),
            };
            link.insert(db).await?;
        }

        Ok(tag)
    }

    /// Remove a tag from a book
    pub async fn remove_tag_from_book(
        db: &DatabaseConnection,
        book_id: Uuid,
        tag_id: Uuid,
    ) -> Result<bool> {
        let result = BookTags::delete_many()
            .filter(book_tags::Column::BookId.eq(book_id))
            .filter(book_tags::Column::TagId.eq(tag_id))
            .exec(db)
            .await?;

        Ok(result.rows_affected > 0)
    }

    /// Get tags for multiple books by their IDs
    ///
    /// Returns a HashMap keyed by book_id for efficient lookups
    pub async fn get_tags_for_book_ids(
        db: &DatabaseConnection,
        book_ids: &[Uuid],
    ) -> Result<std::collections::HashMap<Uuid, Vec<tags::Model>>> {
        if book_ids.is_empty() {
            return Ok(std::collections::HashMap::new());
        }

        // Get all book_tags mappings for the given books
        let book_tag_links: Vec<book_tags::Model> = BookTags::find()
            .filter(book_tags::Column::BookId.is_in(book_ids.to_vec()))
            .all(db)
            .await?;

        if book_tag_links.is_empty() {
            return Ok(std::collections::HashMap::new());
        }

        // Collect unique tag IDs
        let tag_ids: Vec<Uuid> = book_tag_links
            .iter()
            .map(|bt| bt.tag_id)
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();

        // Fetch all tags at once
        let all_tags: Vec<tags::Model> = Tags::find()
            .filter(tags::Column::Id.is_in(tag_ids))
            .all(db)
            .await?;

        // Create tag lookup map
        let tag_map: std::collections::HashMap<Uuid, tags::Model> =
            all_tags.into_iter().map(|t| (t.id, t)).collect();

        // Build result map
        let mut result: std::collections::HashMap<Uuid, Vec<tags::Model>> =
            std::collections::HashMap::new();

        for link in book_tag_links {
            if let Some(tag) = tag_map.get(&link.tag_id) {
                result.entry(link.book_id).or_default().push(tag.clone());
            }
        }

        // Sort tags by name within each book
        for tags in result.values_mut() {
            tags.sort_by(|a, b| a.name.cmp(&b.name));
        }

        Ok(result)
    }

    /// Delete all unused tags (tags with no series or books linked)
    /// Returns the names of deleted tags
    pub async fn delete_unused(db: &DatabaseConnection) -> Result<Vec<String>> {
        use crate::db::entities::series_tags::Entity as SeriesTags;

        // Get all tags
        let all_tags = Self::list_all(db).await?;
        let mut deleted_names = Vec::new();

        for tag in all_tags {
            // Check if tag has any series
            let series_count = SeriesTags::find()
                .filter(series_tags::Column::TagId.eq(tag.id))
                .count(db)
                .await?;

            // Check if tag has any books
            let book_count = BookTags::find()
                .filter(book_tags::Column::TagId.eq(tag.id))
                .count(db)
                .await?;

            if series_count == 0 && book_count == 0 {
                // Delete the unused tag
                Tags::delete_by_id(tag.id).exec(db).await?;
                deleted_names.push(tag.name);
            }
        }

        Ok(deleted_names)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::ScanningStrategy;
    use crate::db::repositories::{LibraryRepository, SeriesRepository};
    use crate::db::test_helpers::create_test_db;

    #[tokio::test]
    async fn test_create_and_get_tag() {
        let (db, _temp_dir) = create_test_db().await;

        let tag = TagRepository::create(db.sea_orm_connection(), "Completed")
            .await
            .unwrap();

        assert_eq!(tag.name, "Completed");
        assert_eq!(tag.normalized_name, "completed");

        let fetched = TagRepository::get_by_id(db.sea_orm_connection(), tag.id)
            .await
            .unwrap();
        assert!(fetched.is_some());
        assert_eq!(fetched.unwrap().name, "Completed");
    }

    #[tokio::test]
    async fn test_find_or_create_tag() {
        let (db, _temp_dir) = create_test_db().await;

        // First call creates
        let tag1 = TagRepository::find_or_create(db.sea_orm_connection(), "Ongoing")
            .await
            .unwrap();
        assert_eq!(tag1.name, "Ongoing");

        // Second call finds existing (case insensitive)
        let tag2 = TagRepository::find_or_create(db.sea_orm_connection(), "ONGOING")
            .await
            .unwrap();
        assert_eq!(tag1.id, tag2.id);

        // Third call with different name creates new
        let tag3 = TagRepository::find_or_create(db.sea_orm_connection(), "Hiatus")
            .await
            .unwrap();
        assert_ne!(tag1.id, tag3.id);
    }

    #[tokio::test]
    async fn test_list_all_tags() {
        let (db, _temp_dir) = create_test_db().await;

        TagRepository::create(db.sea_orm_connection(), "Zulu")
            .await
            .unwrap();
        TagRepository::create(db.sea_orm_connection(), "Alpha")
            .await
            .unwrap();
        TagRepository::create(db.sea_orm_connection(), "Beta")
            .await
            .unwrap();

        let tags = TagRepository::list_all(db.sea_orm_connection())
            .await
            .unwrap();

        assert_eq!(tags.len(), 3);
        // Should be sorted by name
        assert_eq!(tags[0].name, "Alpha");
        assert_eq!(tags[1].name, "Beta");
        assert_eq!(tags[2].name, "Zulu");
    }

    #[tokio::test]
    async fn test_set_tags_for_series() {
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

        // Set initial tags
        let tags = TagRepository::set_tags_for_series(
            db.sea_orm_connection(),
            series.id,
            vec!["Completed".to_string(), "Favorite".to_string()],
        )
        .await
        .unwrap();

        assert_eq!(tags.len(), 2);

        // Verify they're linked
        let fetched = TagRepository::get_tags_for_series(db.sea_orm_connection(), series.id)
            .await
            .unwrap();
        assert_eq!(fetched.len(), 2);

        // Replace with different tags
        let new_tags = TagRepository::set_tags_for_series(
            db.sea_orm_connection(),
            series.id,
            vec!["Ongoing".to_string()],
        )
        .await
        .unwrap();

        assert_eq!(new_tags.len(), 1);
        assert_eq!(new_tags[0].name, "Ongoing");

        // Verify old tags are unlinked
        let fetched = TagRepository::get_tags_for_series(db.sea_orm_connection(), series.id)
            .await
            .unwrap();
        assert_eq!(fetched.len(), 1);
        assert_eq!(fetched[0].name, "Ongoing");
    }

    #[tokio::test]
    async fn test_add_and_remove_tag_from_series() {
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

        // Add a tag
        let tag = TagRepository::add_tag_to_series(db.sea_orm_connection(), series.id, "Reading")
            .await
            .unwrap();

        let fetched = TagRepository::get_tags_for_series(db.sea_orm_connection(), series.id)
            .await
            .unwrap();
        assert_eq!(fetched.len(), 1);

        // Adding same tag again should not duplicate
        TagRepository::add_tag_to_series(db.sea_orm_connection(), series.id, "Reading")
            .await
            .unwrap();

        let fetched = TagRepository::get_tags_for_series(db.sea_orm_connection(), series.id)
            .await
            .unwrap();
        assert_eq!(fetched.len(), 1);

        // Remove the tag
        let removed =
            TagRepository::remove_tag_from_series(db.sea_orm_connection(), series.id, tag.id)
                .await
                .unwrap();
        assert!(removed);

        let fetched = TagRepository::get_tags_for_series(db.sea_orm_connection(), series.id)
            .await
            .unwrap();
        assert_eq!(fetched.len(), 0);
    }

    #[tokio::test]
    async fn test_count_series_with_tag() {
        let (db, _temp_dir) = create_test_db().await;

        let library = LibraryRepository::create(
            db.sea_orm_connection(),
            "Test Library",
            "/test/path",
            ScanningStrategy::Default,
        )
        .await
        .unwrap();

        let series1 =
            SeriesRepository::create(db.sea_orm_connection(), library.id, "Series 1", None)
                .await
                .unwrap();
        let series2 =
            SeriesRepository::create(db.sea_orm_connection(), library.id, "Series 2", None)
                .await
                .unwrap();

        let tag = TagRepository::create(db.sea_orm_connection(), "Popular")
            .await
            .unwrap();

        // Initially no series have this tag
        let count = TagRepository::count_series_with_tag(db.sea_orm_connection(), tag.id)
            .await
            .unwrap();
        assert_eq!(count, 0);

        // Add tag to series1
        TagRepository::add_tag_to_series(db.sea_orm_connection(), series1.id, "Popular")
            .await
            .unwrap();

        let count = TagRepository::count_series_with_tag(db.sea_orm_connection(), tag.id)
            .await
            .unwrap();
        assert_eq!(count, 1);

        // Add tag to series2
        TagRepository::add_tag_to_series(db.sea_orm_connection(), series2.id, "Popular")
            .await
            .unwrap();

        let count = TagRepository::count_series_with_tag(db.sea_orm_connection(), tag.id)
            .await
            .unwrap();
        assert_eq!(count, 2);
    }

    #[tokio::test]
    async fn test_delete_tag() {
        let (db, _temp_dir) = create_test_db().await;

        let tag = TagRepository::create(db.sea_orm_connection(), "ToDelete")
            .await
            .unwrap();

        let deleted = TagRepository::delete(db.sea_orm_connection(), tag.id)
            .await
            .unwrap();
        assert!(deleted);

        let fetched = TagRepository::get_by_id(db.sea_orm_connection(), tag.id)
            .await
            .unwrap();
        assert!(fetched.is_none());
    }

    #[tokio::test]
    async fn test_tag_name_trimming() {
        let (db, _temp_dir) = create_test_db().await;

        let tag = TagRepository::create(db.sea_orm_connection(), "  Spaced  ")
            .await
            .unwrap();

        assert_eq!(tag.name, "Spaced");
        assert_eq!(tag.normalized_name, "spaced");

        // Should find by original name with spaces
        let found = TagRepository::get_by_name(db.sea_orm_connection(), "  SPACED  ")
            .await
            .unwrap();
        assert!(found.is_some());
    }

    #[tokio::test]
    async fn test_delete_unused_tags() {
        let (db, _temp_dir) = create_test_db().await;

        // Create a library and series for testing
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

        // Create some tags - one used, two unused
        let used_tag = TagRepository::create(db.sea_orm_connection(), "UsedTag")
            .await
            .unwrap();
        TagRepository::create(db.sea_orm_connection(), "UnusedTag1")
            .await
            .unwrap();
        TagRepository::create(db.sea_orm_connection(), "UnusedTag2")
            .await
            .unwrap();

        // Link one tag to a series
        TagRepository::add_tag_to_series(db.sea_orm_connection(), series.id, "UsedTag")
            .await
            .unwrap();

        // Verify we have 3 tags
        let all_tags = TagRepository::list_all(db.sea_orm_connection())
            .await
            .unwrap();
        assert_eq!(all_tags.len(), 3);

        // Delete unused tags
        let deleted_names = TagRepository::delete_unused(db.sea_orm_connection())
            .await
            .unwrap();

        // Should have deleted 2 unused tags
        assert_eq!(deleted_names.len(), 2);
        assert!(deleted_names.contains(&"UnusedTag1".to_string()));
        assert!(deleted_names.contains(&"UnusedTag2".to_string()));

        // Verify only 1 tag remains
        let remaining_tags = TagRepository::list_all(db.sea_orm_connection())
            .await
            .unwrap();
        assert_eq!(remaining_tags.len(), 1);
        assert_eq!(remaining_tags[0].id, used_tag.id);
    }

    #[tokio::test]
    async fn test_delete_unused_tags_empty() {
        let (db, _temp_dir) = create_test_db().await;

        // Delete unused when no tags exist
        let deleted_names = TagRepository::delete_unused(db.sea_orm_connection())
            .await
            .unwrap();

        assert!(deleted_names.is_empty());
    }

    /// Helper to create a test book for tag tests
    async fn create_test_book_for_tag(
        db: &crate::db::Database,
        series_id: Uuid,
        library_id: Uuid,
    ) -> crate::db::entities::books::Model {
        use crate::db::entities::books;
        use crate::db::repositories::BookRepository;
        use chrono::Utc;

        let book = books::Model {
            id: Uuid::new_v4(),
            series_id,
            library_id,
            file_path: "/test/book.cbz".to_string(),
            file_name: "book.cbz".to_string(),
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
            epub_positions: None,
            epub_spine_items: None,
        };

        BookRepository::create(db.sea_orm_connection(), &book, None)
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn test_set_tags_for_book() {
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

        let book = create_test_book_for_tag(&db, series.id, library.id).await;

        // Set initial tags
        let tags = TagRepository::set_tags_for_book(
            db.sea_orm_connection(),
            book.id,
            vec!["Completed".to_string(), "Favorite".to_string()],
        )
        .await
        .unwrap();

        assert_eq!(tags.len(), 2);

        // Verify they're linked
        let fetched = TagRepository::get_tags_for_book(db.sea_orm_connection(), book.id)
            .await
            .unwrap();
        assert_eq!(fetched.len(), 2);

        // Replace with different tags
        let new_tags = TagRepository::set_tags_for_book(
            db.sea_orm_connection(),
            book.id,
            vec!["Ongoing".to_string()],
        )
        .await
        .unwrap();

        assert_eq!(new_tags.len(), 1);
        assert_eq!(new_tags[0].name, "Ongoing");

        // Verify old tags are unlinked
        let fetched = TagRepository::get_tags_for_book(db.sea_orm_connection(), book.id)
            .await
            .unwrap();
        assert_eq!(fetched.len(), 1);
        assert_eq!(fetched[0].name, "Ongoing");
    }

    #[tokio::test]
    async fn test_add_and_remove_tag_from_book() {
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

        let book = create_test_book_for_tag(&db, series.id, library.id).await;

        // Add a tag
        let tag = TagRepository::add_tag_to_book(db.sea_orm_connection(), book.id, "Reading")
            .await
            .unwrap();

        let fetched = TagRepository::get_tags_for_book(db.sea_orm_connection(), book.id)
            .await
            .unwrap();
        assert_eq!(fetched.len(), 1);

        // Adding same tag again should not duplicate
        TagRepository::add_tag_to_book(db.sea_orm_connection(), book.id, "Reading")
            .await
            .unwrap();

        let fetched = TagRepository::get_tags_for_book(db.sea_orm_connection(), book.id)
            .await
            .unwrap();
        assert_eq!(fetched.len(), 1);

        // Remove the tag
        let removed = TagRepository::remove_tag_from_book(db.sea_orm_connection(), book.id, tag.id)
            .await
            .unwrap();
        assert!(removed);

        let fetched = TagRepository::get_tags_for_book(db.sea_orm_connection(), book.id)
            .await
            .unwrap();
        assert_eq!(fetched.len(), 0);
    }

    #[tokio::test]
    async fn test_delete_unused_tags_with_book_links() {
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

        let book = create_test_book_for_tag(&db, series.id, library.id).await;

        // Create tags: one linked to book only, one unused
        let book_tag = TagRepository::create(db.sea_orm_connection(), "BookOnlyTag")
            .await
            .unwrap();
        TagRepository::create(db.sea_orm_connection(), "UnusedTag")
            .await
            .unwrap();

        // Link one tag to a book (not a series)
        TagRepository::add_tag_to_book(db.sea_orm_connection(), book.id, "BookOnlyTag")
            .await
            .unwrap();

        // Verify we have 2 tags
        let all_tags = TagRepository::list_all(db.sea_orm_connection())
            .await
            .unwrap();
        assert_eq!(all_tags.len(), 2);

        // Delete unused tags — should only delete the truly unused one
        let deleted_names = TagRepository::delete_unused(db.sea_orm_connection())
            .await
            .unwrap();

        assert_eq!(deleted_names.len(), 1);
        assert!(deleted_names.contains(&"UnusedTag".to_string()));

        // Tag linked to book should still exist
        let remaining = TagRepository::list_all(db.sea_orm_connection())
            .await
            .unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].id, book_tag.id);
    }
}
