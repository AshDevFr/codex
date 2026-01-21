//! Repository for sharing_tags, series_sharing_tags, and user_sharing_tags table operations
//!
//! Sharing tags control content access. Series can be tagged, and users can be granted
//! access (allow/deny) to content via these tags.

use anyhow::Result;
use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, Set,
};
use uuid::Uuid;

use crate::db::entities::{
    series_sharing_tags, sharing_tags,
    sharing_tags::Entity as SharingTags,
    user_sharing_tags::{self, AccessMode},
};

/// Repository for sharing tag operations
pub struct SharingTagRepository;

impl SharingTagRepository {
    /// Get a sharing tag by ID
    pub async fn get_by_id(
        db: &DatabaseConnection,
        id: Uuid,
    ) -> Result<Option<sharing_tags::Model>> {
        let result = SharingTags::find_by_id(id).one(db).await?;
        Ok(result)
    }

    /// Get a sharing tag by normalized name
    pub async fn get_by_name(
        db: &DatabaseConnection,
        name: &str,
    ) -> Result<Option<sharing_tags::Model>> {
        let normalized = name.to_lowercase().trim().to_string();
        let result = SharingTags::find()
            .filter(sharing_tags::Column::NormalizedName.eq(&normalized))
            .one(db)
            .await?;
        Ok(result)
    }

    /// List all sharing tags sorted by name
    pub async fn list_all(db: &DatabaseConnection) -> Result<Vec<sharing_tags::Model>> {
        let results = SharingTags::find()
            .order_by_asc(sharing_tags::Column::Name)
            .all(db)
            .await?;
        Ok(results)
    }

    /// Create a new sharing tag
    pub async fn create(
        db: &DatabaseConnection,
        name: &str,
        description: Option<String>,
    ) -> Result<sharing_tags::Model> {
        let normalized = name.to_lowercase().trim().to_string();
        let trimmed_name = name.trim().to_string();
        let now = Utc::now();

        let active_model = sharing_tags::ActiveModel {
            id: Set(Uuid::new_v4()),
            name: Set(trimmed_name),
            normalized_name: Set(normalized),
            description: Set(description),
            created_at: Set(now),
            updated_at: Set(now),
        };

        let model = active_model.insert(db).await?;
        Ok(model)
    }

    /// Update a sharing tag
    pub async fn update(
        db: &DatabaseConnection,
        id: Uuid,
        name: Option<String>,
        description: Option<Option<String>>,
    ) -> Result<Option<sharing_tags::Model>> {
        let existing = Self::get_by_id(db, id).await?;
        let Some(existing) = existing else {
            return Ok(None);
        };

        let mut active_model: sharing_tags::ActiveModel = existing.into();

        if let Some(new_name) = name {
            let trimmed = new_name.trim().to_string();
            let normalized = trimmed.to_lowercase();
            active_model.name = Set(trimmed);
            active_model.normalized_name = Set(normalized);
        }

        if let Some(new_desc) = description {
            active_model.description = Set(new_desc);
        }

        active_model.updated_at = Set(Utc::now());
        let model = active_model.update(db).await?;
        Ok(Some(model))
    }

    /// Delete a sharing tag by ID
    pub async fn delete(db: &DatabaseConnection, id: Uuid) -> Result<bool> {
        let result = SharingTags::delete_by_id(id).exec(db).await?;
        Ok(result.rows_affected > 0)
    }

    /// Count series using a sharing tag
    pub async fn count_series_with_tag(db: &DatabaseConnection, tag_id: Uuid) -> Result<u64> {
        use crate::db::entities::series_sharing_tags::Entity as SeriesSharingTags;

        let count = SeriesSharingTags::find()
            .filter(series_sharing_tags::Column::SharingTagId.eq(tag_id))
            .count(db)
            .await?;

        Ok(count)
    }

    /// Count users with grants for a sharing tag
    pub async fn count_users_with_tag(db: &DatabaseConnection, tag_id: Uuid) -> Result<u64> {
        use crate::db::entities::user_sharing_tags::Entity as UserSharingTags;

        let count = UserSharingTags::find()
            .filter(user_sharing_tags::Column::SharingTagId.eq(tag_id))
            .count(db)
            .await?;

        Ok(count)
    }

    // ==================== Series-Tag Operations ====================

    /// Get all sharing tags for a series
    pub async fn get_tags_for_series(
        db: &DatabaseConnection,
        series_id: Uuid,
    ) -> Result<Vec<sharing_tags::Model>> {
        use crate::db::entities::series_sharing_tags::Entity as SeriesSharingTags;

        let tag_ids: Vec<Uuid> = SeriesSharingTags::find()
            .filter(series_sharing_tags::Column::SeriesId.eq(series_id))
            .all(db)
            .await?
            .into_iter()
            .map(|st| st.sharing_tag_id)
            .collect();

        if tag_ids.is_empty() {
            return Ok(vec![]);
        }

        let tags = SharingTags::find()
            .filter(sharing_tags::Column::Id.is_in(tag_ids))
            .order_by_asc(sharing_tags::Column::Name)
            .all(db)
            .await?;

        Ok(tags)
    }

    /// Set sharing tags for a series (replaces existing)
    pub async fn set_tags_for_series(
        db: &DatabaseConnection,
        series_id: Uuid,
        tag_ids: Vec<Uuid>,
    ) -> Result<Vec<sharing_tags::Model>> {
        use crate::db::entities::series_sharing_tags::Entity as SeriesSharingTags;

        // Remove existing tag links for this series
        SeriesSharingTags::delete_many()
            .filter(series_sharing_tags::Column::SeriesId.eq(series_id))
            .exec(db)
            .await?;

        if tag_ids.is_empty() {
            return Ok(vec![]);
        }

        // Link each tag
        for tag_id in &tag_ids {
            let link = series_sharing_tags::ActiveModel {
                series_id: Set(series_id),
                sharing_tag_id: Set(*tag_id),
            };
            link.insert(db).await?;
        }

        // Return the tags
        let tags = SharingTags::find()
            .filter(sharing_tags::Column::Id.is_in(tag_ids))
            .order_by_asc(sharing_tags::Column::Name)
            .all(db)
            .await?;

        Ok(tags)
    }

    /// Add a sharing tag to a series
    pub async fn add_tag_to_series(
        db: &DatabaseConnection,
        series_id: Uuid,
        tag_id: Uuid,
    ) -> Result<bool> {
        use crate::db::entities::series_sharing_tags::Entity as SeriesSharingTags;

        // Check if already linked
        let existing = SeriesSharingTags::find()
            .filter(series_sharing_tags::Column::SeriesId.eq(series_id))
            .filter(series_sharing_tags::Column::SharingTagId.eq(tag_id))
            .one(db)
            .await?;

        if existing.is_some() {
            return Ok(false); // Already linked
        }

        let link = series_sharing_tags::ActiveModel {
            series_id: Set(series_id),
            sharing_tag_id: Set(tag_id),
        };
        link.insert(db).await?;

        Ok(true)
    }

    /// Remove a sharing tag from a series
    pub async fn remove_tag_from_series(
        db: &DatabaseConnection,
        series_id: Uuid,
        tag_id: Uuid,
    ) -> Result<bool> {
        use crate::db::entities::series_sharing_tags::Entity as SeriesSharingTags;

        let result = SeriesSharingTags::delete_many()
            .filter(series_sharing_tags::Column::SeriesId.eq(series_id))
            .filter(series_sharing_tags::Column::SharingTagId.eq(tag_id))
            .exec(db)
            .await?;

        Ok(result.rows_affected > 0)
    }

    /// Get all series IDs that have a specific sharing tag
    pub async fn get_series_with_tag(db: &DatabaseConnection, tag_id: Uuid) -> Result<Vec<Uuid>> {
        use crate::db::entities::series_sharing_tags::Entity as SeriesSharingTags;

        let series_ids: Vec<Uuid> = SeriesSharingTags::find()
            .filter(series_sharing_tags::Column::SharingTagId.eq(tag_id))
            .all(db)
            .await?
            .into_iter()
            .map(|st| st.series_id)
            .collect();

        Ok(series_ids)
    }

    // ==================== Sharing Tag Filter Operations (for FilterService) ====================

    /// Get all series IDs that have a sharing tag with the exact name (case-insensitive)
    pub async fn get_series_with_sharing_tag_name(
        db: &DatabaseConnection,
        tag_name: &str,
    ) -> Result<Vec<Uuid>> {
        use crate::db::entities::series_sharing_tags::Entity as SeriesSharingTags;

        let normalized = tag_name.to_lowercase().trim().to_string();

        // Find the tag by normalized name
        let tag = SharingTags::find()
            .filter(sharing_tags::Column::NormalizedName.eq(&normalized))
            .one(db)
            .await?;

        let Some(tag) = tag else {
            return Ok(vec![]);
        };

        // Get series with this tag
        let series_ids: Vec<Uuid> = SeriesSharingTags::find()
            .filter(series_sharing_tags::Column::SharingTagId.eq(tag.id))
            .all(db)
            .await?
            .into_iter()
            .map(|st| st.series_id)
            .collect();

        Ok(series_ids)
    }

    /// Get series IDs with sharing tag names containing substring (case-insensitive)
    pub async fn get_series_with_sharing_tag_containing(
        db: &DatabaseConnection,
        substring: &str,
    ) -> Result<Vec<Uuid>> {
        use crate::db::entities::series_sharing_tags::Entity as SeriesSharingTags;

        let pattern = format!("%{}%", substring.to_lowercase());

        // Find tags matching the pattern
        let matching_tags: Vec<Uuid> = SharingTags::find()
            .filter(sharing_tags::Column::NormalizedName.like(&pattern))
            .all(db)
            .await?
            .into_iter()
            .map(|t| t.id)
            .collect();

        if matching_tags.is_empty() {
            return Ok(vec![]);
        }

        // Get series with any of these tags
        let series_ids: Vec<Uuid> = SeriesSharingTags::find()
            .filter(series_sharing_tags::Column::SharingTagId.is_in(matching_tags))
            .all(db)
            .await?
            .into_iter()
            .map(|st| st.series_id)
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();

        Ok(series_ids)
    }

    /// Get series IDs with sharing tag names starting with prefix (case-insensitive)
    pub async fn get_series_with_sharing_tag_starting_with(
        db: &DatabaseConnection,
        prefix: &str,
    ) -> Result<Vec<Uuid>> {
        use crate::db::entities::series_sharing_tags::Entity as SeriesSharingTags;

        let pattern = format!("{}%", prefix.to_lowercase());

        let matching_tags: Vec<Uuid> = SharingTags::find()
            .filter(sharing_tags::Column::NormalizedName.like(&pattern))
            .all(db)
            .await?
            .into_iter()
            .map(|t| t.id)
            .collect();

        if matching_tags.is_empty() {
            return Ok(vec![]);
        }

        let series_ids: Vec<Uuid> = SeriesSharingTags::find()
            .filter(series_sharing_tags::Column::SharingTagId.is_in(matching_tags))
            .all(db)
            .await?
            .into_iter()
            .map(|st| st.series_id)
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();

        Ok(series_ids)
    }

    /// Get series IDs with sharing tag names ending with suffix (case-insensitive)
    pub async fn get_series_with_sharing_tag_ending_with(
        db: &DatabaseConnection,
        suffix: &str,
    ) -> Result<Vec<Uuid>> {
        use crate::db::entities::series_sharing_tags::Entity as SeriesSharingTags;

        let pattern = format!("%{}", suffix.to_lowercase());

        let matching_tags: Vec<Uuid> = SharingTags::find()
            .filter(sharing_tags::Column::NormalizedName.like(&pattern))
            .all(db)
            .await?
            .into_iter()
            .map(|t| t.id)
            .collect();

        if matching_tags.is_empty() {
            return Ok(vec![]);
        }

        let series_ids: Vec<Uuid> = SeriesSharingTags::find()
            .filter(series_sharing_tags::Column::SharingTagId.is_in(matching_tags))
            .all(db)
            .await?
            .into_iter()
            .map(|st| st.series_id)
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();

        Ok(series_ids)
    }

    // ==================== User-Tag Grant Operations ====================

    /// Get all sharing tag grants for a user
    #[allow(dead_code)]
    pub async fn get_grants_for_user(
        db: &DatabaseConnection,
        user_id: Uuid,
    ) -> Result<Vec<user_sharing_tags::Model>> {
        use crate::db::entities::user_sharing_tags::Entity as UserSharingTags;

        let grants = UserSharingTags::find()
            .filter(user_sharing_tags::Column::UserId.eq(user_id))
            .all(db)
            .await?;

        Ok(grants)
    }

    /// Get user grants with their sharing tag details
    pub async fn get_grants_with_tags_for_user(
        db: &DatabaseConnection,
        user_id: Uuid,
    ) -> Result<Vec<(user_sharing_tags::Model, sharing_tags::Model)>> {
        use crate::db::entities::user_sharing_tags::Entity as UserSharingTags;

        let grants = UserSharingTags::find()
            .filter(user_sharing_tags::Column::UserId.eq(user_id))
            .all(db)
            .await?;

        let mut results = Vec::new();
        for grant in grants {
            if let Some(tag) = Self::get_by_id(db, grant.sharing_tag_id).await? {
                results.push((grant, tag));
            }
        }

        Ok(results)
    }

    /// Get allowed tag IDs for a user (tags with 'allow' access mode)
    #[allow(dead_code)]
    pub async fn get_allowed_tag_ids_for_user(
        db: &DatabaseConnection,
        user_id: Uuid,
    ) -> Result<Vec<Uuid>> {
        use crate::db::entities::user_sharing_tags::Entity as UserSharingTags;

        let tag_ids: Vec<Uuid> = UserSharingTags::find()
            .filter(user_sharing_tags::Column::UserId.eq(user_id))
            .filter(user_sharing_tags::Column::AccessMode.eq(AccessMode::Allow.as_str()))
            .all(db)
            .await?
            .into_iter()
            .map(|g| g.sharing_tag_id)
            .collect();

        Ok(tag_ids)
    }

    /// Get denied tag IDs for a user (tags with 'deny' access mode)
    pub async fn get_denied_tag_ids_for_user(
        db: &DatabaseConnection,
        user_id: Uuid,
    ) -> Result<Vec<Uuid>> {
        use crate::db::entities::user_sharing_tags::Entity as UserSharingTags;

        let tag_ids: Vec<Uuid> = UserSharingTags::find()
            .filter(user_sharing_tags::Column::UserId.eq(user_id))
            .filter(user_sharing_tags::Column::AccessMode.eq(AccessMode::Deny.as_str()))
            .all(db)
            .await?
            .into_iter()
            .map(|g| g.sharing_tag_id)
            .collect();

        Ok(tag_ids)
    }

    /// Set a user's grant for a sharing tag (upsert)
    pub async fn set_user_grant(
        db: &DatabaseConnection,
        user_id: Uuid,
        tag_id: Uuid,
        access_mode: AccessMode,
    ) -> Result<user_sharing_tags::Model> {
        use crate::db::entities::user_sharing_tags::Entity as UserSharingTags;

        // Check if grant already exists
        let existing = UserSharingTags::find()
            .filter(user_sharing_tags::Column::UserId.eq(user_id))
            .filter(user_sharing_tags::Column::SharingTagId.eq(tag_id))
            .one(db)
            .await?;

        if let Some(existing) = existing {
            // Update existing grant
            let mut active_model: user_sharing_tags::ActiveModel = existing.into();
            active_model.access_mode = Set(access_mode.as_str().to_string());
            let model = active_model.update(db).await?;
            Ok(model)
        } else {
            // Create new grant
            let active_model = user_sharing_tags::ActiveModel {
                id: Set(Uuid::new_v4()),
                user_id: Set(user_id),
                sharing_tag_id: Set(tag_id),
                access_mode: Set(access_mode.as_str().to_string()),
                created_at: Set(Utc::now()),
            };
            let model = active_model.insert(db).await?;
            Ok(model)
        }
    }

    /// Remove a user's grant for a sharing tag
    pub async fn remove_user_grant(
        db: &DatabaseConnection,
        user_id: Uuid,
        tag_id: Uuid,
    ) -> Result<bool> {
        use crate::db::entities::user_sharing_tags::Entity as UserSharingTags;

        let result = UserSharingTags::delete_many()
            .filter(user_sharing_tags::Column::UserId.eq(user_id))
            .filter(user_sharing_tags::Column::SharingTagId.eq(tag_id))
            .exec(db)
            .await?;

        Ok(result.rows_affected > 0)
    }

    /// Remove all grants for a user
    #[allow(dead_code)]
    pub async fn remove_all_grants_for_user(db: &DatabaseConnection, user_id: Uuid) -> Result<u64> {
        use crate::db::entities::user_sharing_tags::Entity as UserSharingTags;

        let result = UserSharingTags::delete_many()
            .filter(user_sharing_tags::Column::UserId.eq(user_id))
            .exec(db)
            .await?;

        Ok(result.rows_affected)
    }

    /// Get all users who have grants for a specific sharing tag
    #[allow(dead_code)]
    pub async fn get_users_with_tag(db: &DatabaseConnection, tag_id: Uuid) -> Result<Vec<Uuid>> {
        use crate::db::entities::user_sharing_tags::Entity as UserSharingTags;

        let user_ids: Vec<Uuid> = UserSharingTags::find()
            .filter(user_sharing_tags::Column::SharingTagId.eq(tag_id))
            .all(db)
            .await?
            .into_iter()
            .map(|g| g.user_id)
            .collect();

        Ok(user_ids)
    }

    // ==================== Content Filtering ====================

    /// Check if a series is visible to a user based on sharing tags
    ///
    /// Visibility rules:
    /// 1. If series has no sharing tags -> visible to everyone
    /// 2. If series has sharing tags -> user needs at least one 'allow' grant for those tags
    /// 3. If user has any 'deny' grant for any of the series' tags -> not visible
    #[allow(dead_code)]
    pub async fn is_series_visible_to_user(
        db: &DatabaseConnection,
        series_id: Uuid,
        user_id: Uuid,
    ) -> Result<bool> {
        // Get series sharing tags
        let series_tags = Self::get_tags_for_series(db, series_id).await?;

        // If series has no sharing tags, it's visible to everyone
        if series_tags.is_empty() {
            return Ok(true);
        }

        let series_tag_ids: Vec<Uuid> = series_tags.iter().map(|t| t.id).collect();

        // Get user's denied tags
        let denied_tag_ids = Self::get_denied_tag_ids_for_user(db, user_id).await?;

        // If user has any deny grant for this series' tags, not visible
        for tag_id in &series_tag_ids {
            if denied_tag_ids.contains(tag_id) {
                return Ok(false);
            }
        }

        // Get user's allowed tags
        let allowed_tag_ids = Self::get_allowed_tag_ids_for_user(db, user_id).await?;

        // User needs at least one allow grant for the series' tags
        for tag_id in &series_tag_ids {
            if allowed_tag_ids.contains(tag_id) {
                return Ok(true);
            }
        }

        // No allow grants match the series' tags
        Ok(false)
    }

    /// Get all series IDs that are visible to a user based on sharing tags
    ///
    /// This is used to filter series queries. Returns None if user has no
    /// sharing tag restrictions (can see all content).
    #[allow(dead_code)]
    pub async fn get_visible_series_ids_for_user(
        db: &DatabaseConnection,
        user_id: Uuid,
    ) -> Result<Option<Vec<Uuid>>> {
        use crate::db::entities::series_sharing_tags::Entity as SeriesSharingTags;

        // Get user grants
        let allowed_tag_ids = Self::get_allowed_tag_ids_for_user(db, user_id).await?;
        let denied_tag_ids = Self::get_denied_tag_ids_for_user(db, user_id).await?;

        // If user has no grants at all, they can see all unrestricted content
        if allowed_tag_ids.is_empty() && denied_tag_ids.is_empty() {
            return Ok(None);
        }

        // Get all series that have sharing tags
        let _all_tagged_series: std::collections::HashSet<Uuid> = SeriesSharingTags::find()
            .all(db)
            .await?
            .into_iter()
            .map(|st| st.series_id)
            .collect();

        // Get series IDs for denied tags (these are always hidden)
        let mut denied_series: std::collections::HashSet<Uuid> = std::collections::HashSet::new();
        for tag_id in &denied_tag_ids {
            let series_ids = Self::get_series_with_tag(db, *tag_id).await?;
            denied_series.extend(series_ids);
        }

        // Get series IDs for allowed tags
        let mut allowed_series: std::collections::HashSet<Uuid> = std::collections::HashSet::new();
        for tag_id in &allowed_tag_ids {
            let series_ids = Self::get_series_with_tag(db, *tag_id).await?;
            allowed_series.extend(series_ids);
        }

        // Remove denied series from allowed
        allowed_series.retain(|id| !denied_series.contains(id));

        // Series without sharing tags are visible to everyone
        // But we need to exclude series that have tags the user doesn't have grants for

        // Result: allowed_series (from grants) - denied_series
        // Plus: series without any sharing tags (which are always visible)
        // We return None to indicate "no special filtering needed" if:
        // - User has only deny grants (hide specific content but see everything else)

        // If user has only deny grants and no allow grants
        if allowed_tag_ids.is_empty() && !denied_tag_ids.is_empty() {
            // User can see all series except those with denied tags
            // Return None to indicate broad access, but caller should filter out denied series
            return Ok(None);
        }

        Ok(Some(allowed_series.into_iter().collect()))
    }

    /// Get series IDs to exclude for a user (series with denied tags)
    pub async fn get_excluded_series_ids_for_user(
        db: &DatabaseConnection,
        user_id: Uuid,
    ) -> Result<Vec<Uuid>> {
        let denied_tag_ids = Self::get_denied_tag_ids_for_user(db, user_id).await?;

        let mut excluded_series = std::collections::HashSet::new();
        for tag_id in denied_tag_ids {
            let series_ids = Self::get_series_with_tag(db, tag_id).await?;
            excluded_series.extend(series_ids);
        }

        Ok(excluded_series.into_iter().collect())
    }

    /// Get all series IDs that have any of the given tags
    pub async fn get_series_ids_with_any_tags(
        db: &DatabaseConnection,
        tag_ids: &[Uuid],
    ) -> Result<Vec<Uuid>> {
        use crate::db::entities::series_sharing_tags::Entity as SeriesSharingTags;

        if tag_ids.is_empty() {
            return Ok(vec![]);
        }

        let series_ids: Vec<Uuid> = SeriesSharingTags::find()
            .filter(series_sharing_tags::Column::SharingTagId.is_in(tag_ids.to_vec()))
            .all(db)
            .await?
            .into_iter()
            .map(|st| st.series_id)
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();

        Ok(series_ids)
    }

    /// Get set of all series IDs that have any sharing tags
    pub async fn get_tagged_series_ids(
        db: &DatabaseConnection,
    ) -> Result<std::collections::HashSet<Uuid>> {
        use crate::db::entities::series_sharing_tags::Entity as SeriesSharingTags;

        let series_ids: std::collections::HashSet<Uuid> = SeriesSharingTags::find()
            .all(db)
            .await?
            .into_iter()
            .map(|st| st.series_id)
            .collect();

        Ok(series_ids)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::repositories::{LibraryRepository, SeriesRepository, UserRepository};
    use crate::db::test_helpers::create_test_db;
    use crate::db::ScanningStrategy;

    async fn create_test_user(
        db: &DatabaseConnection,
        username: &str,
    ) -> crate::db::entities::users::Model {
        use crate::db::entities::users;
        use chrono::Utc;

        let now = Utc::now();
        let model = users::Model {
            id: Uuid::new_v4(),
            username: username.to_string(),
            email: format!("{}@test.com", username),
            password_hash: "hash".to_string(),
            role: "reader".to_string(),
            is_active: true,
            email_verified: true,
            permissions: serde_json::json!([]),
            created_at: now,
            updated_at: now,
            last_login_at: None,
        };

        UserRepository::create(db, &model).await.unwrap()
    }

    #[tokio::test]
    async fn test_create_and_get_sharing_tag() {
        let (db, _temp_dir) = create_test_db().await;

        let tag = SharingTagRepository::create(
            db.sea_orm_connection(),
            "Kids Content",
            Some("Content appropriate for children".to_string()),
        )
        .await
        .unwrap();

        assert_eq!(tag.name, "Kids Content");
        assert_eq!(tag.normalized_name, "kids content");
        assert_eq!(
            tag.description,
            Some("Content appropriate for children".to_string())
        );

        let fetched = SharingTagRepository::get_by_id(db.sea_orm_connection(), tag.id)
            .await
            .unwrap();
        assert!(fetched.is_some());
        assert_eq!(fetched.unwrap().name, "Kids Content");
    }

    #[tokio::test]
    async fn test_get_by_name() {
        let (db, _temp_dir) = create_test_db().await;

        SharingTagRepository::create(db.sea_orm_connection(), "Adults Only", None)
            .await
            .unwrap();

        // Case insensitive lookup
        let found = SharingTagRepository::get_by_name(db.sea_orm_connection(), "ADULTS ONLY")
            .await
            .unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().name, "Adults Only");
    }

    #[tokio::test]
    async fn test_list_all_sharing_tags() {
        let (db, _temp_dir) = create_test_db().await;

        SharingTagRepository::create(db.sea_orm_connection(), "Zulu", None)
            .await
            .unwrap();
        SharingTagRepository::create(db.sea_orm_connection(), "Alpha", None)
            .await
            .unwrap();
        SharingTagRepository::create(db.sea_orm_connection(), "Beta", None)
            .await
            .unwrap();

        let tags = SharingTagRepository::list_all(db.sea_orm_connection())
            .await
            .unwrap();

        assert_eq!(tags.len(), 3);
        // Should be sorted by name
        assert_eq!(tags[0].name, "Alpha");
        assert_eq!(tags[1].name, "Beta");
        assert_eq!(tags[2].name, "Zulu");
    }

    #[tokio::test]
    async fn test_update_sharing_tag() {
        let (db, _temp_dir) = create_test_db().await;

        let tag = SharingTagRepository::create(db.sea_orm_connection(), "Old Name", None)
            .await
            .unwrap();

        let updated = SharingTagRepository::update(
            db.sea_orm_connection(),
            tag.id,
            Some("New Name".to_string()),
            Some(Some("New description".to_string())),
        )
        .await
        .unwrap();

        assert!(updated.is_some());
        let updated = updated.unwrap();
        assert_eq!(updated.name, "New Name");
        assert_eq!(updated.normalized_name, "new name");
        assert_eq!(updated.description, Some("New description".to_string()));
    }

    #[tokio::test]
    async fn test_delete_sharing_tag() {
        let (db, _temp_dir) = create_test_db().await;

        let tag = SharingTagRepository::create(db.sea_orm_connection(), "ToDelete", None)
            .await
            .unwrap();

        let deleted = SharingTagRepository::delete(db.sea_orm_connection(), tag.id)
            .await
            .unwrap();
        assert!(deleted);

        let fetched = SharingTagRepository::get_by_id(db.sea_orm_connection(), tag.id)
            .await
            .unwrap();
        assert!(fetched.is_none());
    }

    #[tokio::test]
    async fn test_series_sharing_tags() {
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

        let tag1 = SharingTagRepository::create(db.sea_orm_connection(), "Tag1", None)
            .await
            .unwrap();
        let tag2 = SharingTagRepository::create(db.sea_orm_connection(), "Tag2", None)
            .await
            .unwrap();

        // Add tag to series
        let added =
            SharingTagRepository::add_tag_to_series(db.sea_orm_connection(), series.id, tag1.id)
                .await
                .unwrap();
        assert!(added);

        // Adding same tag again should return false
        let added_again =
            SharingTagRepository::add_tag_to_series(db.sea_orm_connection(), series.id, tag1.id)
                .await
                .unwrap();
        assert!(!added_again);

        // Get tags for series
        let tags = SharingTagRepository::get_tags_for_series(db.sea_orm_connection(), series.id)
            .await
            .unwrap();
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0].name, "Tag1");

        // Set multiple tags (replaces existing)
        let tags = SharingTagRepository::set_tags_for_series(
            db.sea_orm_connection(),
            series.id,
            vec![tag1.id, tag2.id],
        )
        .await
        .unwrap();
        assert_eq!(tags.len(), 2);

        // Remove tag from series
        let removed = SharingTagRepository::remove_tag_from_series(
            db.sea_orm_connection(),
            series.id,
            tag1.id,
        )
        .await
        .unwrap();
        assert!(removed);

        let tags = SharingTagRepository::get_tags_for_series(db.sea_orm_connection(), series.id)
            .await
            .unwrap();
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0].name, "Tag2");
    }

    #[tokio::test]
    async fn test_user_sharing_tag_grants() {
        let (db, _temp_dir) = create_test_db().await;

        let user = create_test_user(db.sea_orm_connection(), "testuser").await;

        let tag1 = SharingTagRepository::create(db.sea_orm_connection(), "AllowedTag", None)
            .await
            .unwrap();
        let tag2 = SharingTagRepository::create(db.sea_orm_connection(), "DeniedTag", None)
            .await
            .unwrap();

        // Set allow grant
        SharingTagRepository::set_user_grant(
            db.sea_orm_connection(),
            user.id,
            tag1.id,
            AccessMode::Allow,
        )
        .await
        .unwrap();

        // Set deny grant
        SharingTagRepository::set_user_grant(
            db.sea_orm_connection(),
            user.id,
            tag2.id,
            AccessMode::Deny,
        )
        .await
        .unwrap();

        // Get grants
        let grants = SharingTagRepository::get_grants_for_user(db.sea_orm_connection(), user.id)
            .await
            .unwrap();
        assert_eq!(grants.len(), 2);

        // Get allowed tags
        let allowed =
            SharingTagRepository::get_allowed_tag_ids_for_user(db.sea_orm_connection(), user.id)
                .await
                .unwrap();
        assert_eq!(allowed.len(), 1);
        assert!(allowed.contains(&tag1.id));

        // Get denied tags
        let denied =
            SharingTagRepository::get_denied_tag_ids_for_user(db.sea_orm_connection(), user.id)
                .await
                .unwrap();
        assert_eq!(denied.len(), 1);
        assert!(denied.contains(&tag2.id));

        // Update grant from allow to deny
        SharingTagRepository::set_user_grant(
            db.sea_orm_connection(),
            user.id,
            tag1.id,
            AccessMode::Deny,
        )
        .await
        .unwrap();

        let allowed =
            SharingTagRepository::get_allowed_tag_ids_for_user(db.sea_orm_connection(), user.id)
                .await
                .unwrap();
        assert_eq!(allowed.len(), 0);

        // Remove grant
        let removed =
            SharingTagRepository::remove_user_grant(db.sea_orm_connection(), user.id, tag1.id)
                .await
                .unwrap();
        assert!(removed);

        let grants = SharingTagRepository::get_grants_for_user(db.sea_orm_connection(), user.id)
            .await
            .unwrap();
        assert_eq!(grants.len(), 1);
    }

    #[tokio::test]
    async fn test_series_visibility() {
        let (db, _temp_dir) = create_test_db().await;

        let library = LibraryRepository::create(
            db.sea_orm_connection(),
            "Test Library",
            "/test/path",
            ScanningStrategy::Default,
        )
        .await
        .unwrap();

        let user = create_test_user(db.sea_orm_connection(), "viewer").await;

        // Series without sharing tags is visible to everyone
        let series_public =
            SeriesRepository::create(db.sea_orm_connection(), library.id, "Public Series", None)
                .await
                .unwrap();

        let visible = SharingTagRepository::is_series_visible_to_user(
            db.sea_orm_connection(),
            series_public.id,
            user.id,
        )
        .await
        .unwrap();
        assert!(visible);

        // Create restricted series with a sharing tag
        let kids_tag = SharingTagRepository::create(db.sea_orm_connection(), "Kids", None)
            .await
            .unwrap();

        let series_restricted =
            SeriesRepository::create(db.sea_orm_connection(), library.id, "Kids Series", None)
                .await
                .unwrap();

        SharingTagRepository::add_tag_to_series(
            db.sea_orm_connection(),
            series_restricted.id,
            kids_tag.id,
        )
        .await
        .unwrap();

        // User without grant cannot see restricted series
        let visible = SharingTagRepository::is_series_visible_to_user(
            db.sea_orm_connection(),
            series_restricted.id,
            user.id,
        )
        .await
        .unwrap();
        assert!(!visible);

        // Grant user access
        SharingTagRepository::set_user_grant(
            db.sea_orm_connection(),
            user.id,
            kids_tag.id,
            AccessMode::Allow,
        )
        .await
        .unwrap();

        // Now user can see the series
        let visible = SharingTagRepository::is_series_visible_to_user(
            db.sea_orm_connection(),
            series_restricted.id,
            user.id,
        )
        .await
        .unwrap();
        assert!(visible);

        // Change to deny grant
        SharingTagRepository::set_user_grant(
            db.sea_orm_connection(),
            user.id,
            kids_tag.id,
            AccessMode::Deny,
        )
        .await
        .unwrap();

        // User cannot see the series anymore
        let visible = SharingTagRepository::is_series_visible_to_user(
            db.sea_orm_connection(),
            series_restricted.id,
            user.id,
        )
        .await
        .unwrap();
        assert!(!visible);
    }

    #[tokio::test]
    async fn test_excluded_series_for_user() {
        let (db, _temp_dir) = create_test_db().await;

        let library = LibraryRepository::create(
            db.sea_orm_connection(),
            "Test Library",
            "/test/path",
            ScanningStrategy::Default,
        )
        .await
        .unwrap();

        let user = create_test_user(db.sea_orm_connection(), "viewer").await;

        let adults_tag = SharingTagRepository::create(db.sea_orm_connection(), "Adults", None)
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

        // Tag series2 as adults only
        SharingTagRepository::add_tag_to_series(db.sea_orm_connection(), series2.id, adults_tag.id)
            .await
            .unwrap();

        // User has deny grant for adults tag
        SharingTagRepository::set_user_grant(
            db.sea_orm_connection(),
            user.id,
            adults_tag.id,
            AccessMode::Deny,
        )
        .await
        .unwrap();

        // Get excluded series
        let excluded = SharingTagRepository::get_excluded_series_ids_for_user(
            db.sea_orm_connection(),
            user.id,
        )
        .await
        .unwrap();

        assert_eq!(excluded.len(), 1);
        assert!(excluded.contains(&series2.id));
        assert!(!excluded.contains(&series1.id));
    }
}
