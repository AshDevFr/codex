//! Filter Preset Repository
//!
//! CRUD and lookup operations for the unified `filter_presets` table that
//! backs both the library list-page filter panels (`scope = "list"`) and the
//! advanced search page (`scope = "search"`).

use crate::db::entities::filter_presets::{self, Entity as FilterPreset};
use anyhow::Result;
use chrono::Utc;
use sea_orm::*;
use uuid::Uuid;

/// Field updates accepted by [`FilterPresetRepository::update`].
///
/// Every field is optional: only `Some(_)` values are written. `Option<Option<_>>`
/// fields distinguish "leave untouched" from "set to NULL":
/// - `None` -> column not touched
/// - `Some(None)` -> column cleared
/// - `Some(Some(value))` -> column set to `value`
#[derive(Debug, Default, Clone)]
pub struct UpdateFilterPreset {
    pub name: Option<String>,
    pub condition: Option<serde_json::Value>,
    pub query: Option<Option<String>>,
    pub sort: Option<Option<String>>,
    pub library_id: Option<Option<Uuid>>,
}

/// Optional filter for [`FilterPresetRepository::list_for_user`].
#[derive(Debug, Default, Clone, Copy)]
pub struct ListFilterPresetsQuery<'a> {
    pub scope: Option<&'a str>,
    pub target: Option<&'a str>,
    pub library_id: Option<Uuid>,
}

pub struct FilterPresetRepository;

impl FilterPresetRepository {
    // =========================================================================
    // Create
    // =========================================================================

    #[allow(clippy::too_many_arguments)]
    pub async fn create(
        db: &DatabaseConnection,
        user_id: Uuid,
        scope: &str,
        target: &str,
        name: &str,
        condition: serde_json::Value,
        query: Option<String>,
        sort: Option<String>,
        library_id: Option<Uuid>,
    ) -> Result<filter_presets::Model> {
        let now = Utc::now();
        let model = filter_presets::ActiveModel {
            id: Set(Uuid::new_v4()),
            user_id: Set(user_id),
            library_id: Set(library_id),
            name: Set(name.to_string()),
            scope: Set(scope.to_string()),
            target: Set(target.to_string()),
            condition: Set(condition),
            query: Set(query),
            sort: Set(sort),
            created_at: Set(now),
            updated_at: Set(now),
        };
        Ok(model.insert(db).await?)
    }

    // =========================================================================
    // Read
    // =========================================================================

    #[allow(dead_code)] // useful for admin / test code paths
    pub async fn find_by_id(
        db: &DatabaseConnection,
        id: Uuid,
    ) -> Result<Option<filter_presets::Model>> {
        Ok(FilterPreset::find_by_id(id).one(db).await?)
    }

    /// Find a preset by id, scoped to a specific user. Use this for
    /// authorization-sensitive reads (returns `None` if the preset belongs to
    /// a different user).
    pub async fn find_by_id_and_user(
        db: &DatabaseConnection,
        id: Uuid,
        user_id: Uuid,
    ) -> Result<Option<filter_presets::Model>> {
        Ok(FilterPreset::find_by_id(id)
            .filter(filter_presets::Column::UserId.eq(user_id))
            .one(db)
            .await?)
    }

    /// List a user's presets, optionally filtered by scope/target/library.
    pub async fn list_for_user(
        db: &DatabaseConnection,
        user_id: Uuid,
        filter: ListFilterPresetsQuery<'_>,
    ) -> Result<Vec<filter_presets::Model>> {
        let mut query = FilterPreset::find().filter(filter_presets::Column::UserId.eq(user_id));

        if let Some(scope) = filter.scope {
            query = query.filter(filter_presets::Column::Scope.eq(scope));
        }
        if let Some(target) = filter.target {
            query = query.filter(filter_presets::Column::Target.eq(target));
        }
        if let Some(library_id) = filter.library_id {
            query = query.filter(filter_presets::Column::LibraryId.eq(library_id));
        }

        Ok(query
            .order_by_asc(filter_presets::Column::Name)
            .all(db)
            .await?)
    }

    // =========================================================================
    // Update
    // =========================================================================

    pub async fn update(
        db: &DatabaseConnection,
        id: Uuid,
        user_id: Uuid,
        update: UpdateFilterPreset,
    ) -> Result<Option<filter_presets::Model>> {
        let Some(existing) = Self::find_by_id_and_user(db, id, user_id).await? else {
            return Ok(None);
        };

        let mut active: filter_presets::ActiveModel = existing.into();

        if let Some(name) = update.name {
            active.name = Set(name);
        }
        if let Some(condition) = update.condition {
            active.condition = Set(condition);
        }
        if let Some(query) = update.query {
            active.query = Set(query);
        }
        if let Some(sort) = update.sort {
            active.sort = Set(sort);
        }
        if let Some(library_id) = update.library_id {
            active.library_id = Set(library_id);
        }

        active.updated_at = Set(Utc::now());

        Ok(Some(active.update(db).await?))
    }

    // =========================================================================
    // Delete
    // =========================================================================

    /// Delete a preset scoped to a specific user. Returns `true` if a matching
    /// row existed and was deleted, `false` otherwise (not found OR owned by a
    /// different user, both of which the caller should surface as 404 to avoid
    /// leaking existence).
    pub async fn delete_by_id_for_user(
        db: &DatabaseConnection,
        id: Uuid,
        user_id: Uuid,
    ) -> Result<bool> {
        let result = FilterPreset::delete_many()
            .filter(filter_presets::Column::Id.eq(id))
            .filter(filter_presets::Column::UserId.eq(user_id))
            .exec(db)
            .await?;
        Ok(result.rows_affected > 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::repositories::UserRepository;
    use crate::db::test_helpers::setup_test_db;

    async fn create_test_user(db: &DatabaseConnection) -> crate::db::entities::users::Model {
        let user = crate::db::entities::users::Model {
            id: Uuid::new_v4(),
            username: format!("preset_user_{}", Uuid::new_v4()),
            email: format!("preset_{}@example.com", Uuid::new_v4()),
            password_hash: "hash".to_string(),
            role: "reader".to_string(),
            is_active: true,
            email_verified: false,
            permissions: serde_json::json!([]),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            last_login_at: None,
        };
        UserRepository::create(db, &user).await.unwrap()
    }

    fn sample_condition() -> serde_json::Value {
        serde_json::json!({
            "allOf": [
                { "title": { "operator": "contains", "value": "one punch" } }
            ]
        })
    }

    #[tokio::test]
    async fn test_create_and_find_by_id() {
        let db = setup_test_db().await;
        let user = create_test_user(&db).await;

        let preset = FilterPresetRepository::create(
            &db,
            user.id,
            "search",
            "books",
            "Unread CBZ",
            sample_condition(),
            Some("one punch".to_string()),
            Some("year:desc".to_string()),
            None,
        )
        .await
        .unwrap();

        assert_eq!(preset.name, "Unread CBZ");
        assert_eq!(preset.scope, "search");
        assert_eq!(preset.target, "books");
        assert_eq!(preset.query.as_deref(), Some("one punch"));
        assert_eq!(preset.sort.as_deref(), Some("year:desc"));
        assert!(preset.library_id.is_none());

        let found = FilterPresetRepository::find_by_id(&db, preset.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(found.id, preset.id);
    }

    #[tokio::test]
    async fn test_find_by_id_and_user_isolates_owners() {
        let db = setup_test_db().await;
        let user1 = create_test_user(&db).await;
        let user2 = create_test_user(&db).await;

        let preset = FilterPresetRepository::create(
            &db,
            user1.id,
            "list",
            "series",
            "Mine",
            sample_condition(),
            None,
            None,
            None,
        )
        .await
        .unwrap();

        let as_owner = FilterPresetRepository::find_by_id_and_user(&db, preset.id, user1.id)
            .await
            .unwrap();
        assert!(as_owner.is_some());

        let as_other = FilterPresetRepository::find_by_id_and_user(&db, preset.id, user2.id)
            .await
            .unwrap();
        assert!(as_other.is_none());
    }

    #[tokio::test]
    async fn test_list_for_user_filters() {
        let db = setup_test_db().await;
        let user = create_test_user(&db).await;

        FilterPresetRepository::create(
            &db,
            user.id,
            "search",
            "series",
            "Search Series A",
            sample_condition(),
            None,
            None,
            None,
        )
        .await
        .unwrap();

        FilterPresetRepository::create(
            &db,
            user.id,
            "search",
            "books",
            "Search Books A",
            sample_condition(),
            None,
            None,
            None,
        )
        .await
        .unwrap();

        FilterPresetRepository::create(
            &db,
            user.id,
            "list",
            "series",
            "List Series A",
            sample_condition(),
            None,
            None,
            None,
        )
        .await
        .unwrap();

        let all =
            FilterPresetRepository::list_for_user(&db, user.id, ListFilterPresetsQuery::default())
                .await
                .unwrap();
        assert_eq!(all.len(), 3);
        // Ordered by name asc
        assert_eq!(all[0].name, "List Series A");
        assert_eq!(all[1].name, "Search Books A");
        assert_eq!(all[2].name, "Search Series A");

        let search_only = FilterPresetRepository::list_for_user(
            &db,
            user.id,
            ListFilterPresetsQuery {
                scope: Some("search"),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        assert_eq!(search_only.len(), 2);
        assert!(search_only.iter().all(|p| p.scope == "search"));

        let search_books = FilterPresetRepository::list_for_user(
            &db,
            user.id,
            ListFilterPresetsQuery {
                scope: Some("search"),
                target: Some("books"),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        assert_eq!(search_books.len(), 1);
        assert_eq!(search_books[0].name, "Search Books A");
    }

    #[tokio::test]
    async fn test_list_for_user_isolates_owners() {
        let db = setup_test_db().await;
        let user1 = create_test_user(&db).await;
        let user2 = create_test_user(&db).await;

        FilterPresetRepository::create(
            &db,
            user1.id,
            "search",
            "books",
            "User 1 preset",
            sample_condition(),
            None,
            None,
            None,
        )
        .await
        .unwrap();

        let list_for_2 =
            FilterPresetRepository::list_for_user(&db, user2.id, ListFilterPresetsQuery::default())
                .await
                .unwrap();
        assert!(list_for_2.is_empty());
    }

    #[tokio::test]
    async fn test_update_changes_fields_and_bumps_timestamp() {
        let db = setup_test_db().await;
        let user = create_test_user(&db).await;

        let preset = FilterPresetRepository::create(
            &db,
            user.id,
            "search",
            "books",
            "Original",
            sample_condition(),
            Some("foo".to_string()),
            Some("title:asc".to_string()),
            None,
        )
        .await
        .unwrap();

        let original_updated_at = preset.updated_at;

        // Sleep briefly to ensure the timestamp changes
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

        let new_condition = serde_json::json!({"title": {"operator": "is", "value": "new"}});

        let updated = FilterPresetRepository::update(
            &db,
            preset.id,
            user.id,
            UpdateFilterPreset {
                name: Some("Renamed".to_string()),
                condition: Some(new_condition.clone()),
                query: Some(None), // clear
                sort: Some(Some("year:desc".to_string())),
                library_id: None, // leave alone
            },
        )
        .await
        .unwrap()
        .unwrap();

        assert_eq!(updated.name, "Renamed");
        assert_eq!(updated.condition, new_condition);
        assert!(updated.query.is_none());
        assert_eq!(updated.sort.as_deref(), Some("year:desc"));
        assert!(updated.updated_at > original_updated_at);
    }

    #[tokio::test]
    async fn test_update_other_user_returns_none() {
        let db = setup_test_db().await;
        let user1 = create_test_user(&db).await;
        let user2 = create_test_user(&db).await;

        let preset = FilterPresetRepository::create(
            &db,
            user1.id,
            "list",
            "series",
            "User 1",
            sample_condition(),
            None,
            None,
            None,
        )
        .await
        .unwrap();

        let result = FilterPresetRepository::update(
            &db,
            preset.id,
            user2.id,
            UpdateFilterPreset {
                name: Some("Hacked".to_string()),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        assert!(result.is_none());

        // Original preset is untouched
        let reread = FilterPresetRepository::find_by_id(&db, preset.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(reread.name, "User 1");
    }

    #[tokio::test]
    async fn test_delete_by_id_for_user() {
        let db = setup_test_db().await;
        let user = create_test_user(&db).await;

        let preset = FilterPresetRepository::create(
            &db,
            user.id,
            "search",
            "books",
            "ToDelete",
            sample_condition(),
            None,
            None,
            None,
        )
        .await
        .unwrap();

        assert!(
            FilterPresetRepository::delete_by_id_for_user(&db, preset.id, user.id)
                .await
                .unwrap()
        );
        assert!(
            FilterPresetRepository::find_by_id(&db, preset.id)
                .await
                .unwrap()
                .is_none()
        );

        // Idempotent
        assert!(
            !FilterPresetRepository::delete_by_id_for_user(&db, preset.id, user.id)
                .await
                .unwrap()
        );
    }

    #[tokio::test]
    async fn test_delete_other_user_is_no_op() {
        let db = setup_test_db().await;
        let user1 = create_test_user(&db).await;
        let user2 = create_test_user(&db).await;

        let preset = FilterPresetRepository::create(
            &db,
            user1.id,
            "list",
            "series",
            "Owned by 1",
            sample_condition(),
            None,
            None,
            None,
        )
        .await
        .unwrap();

        assert!(
            !FilterPresetRepository::delete_by_id_for_user(&db, preset.id, user2.id)
                .await
                .unwrap()
        );

        assert!(
            FilterPresetRepository::find_by_id(&db, preset.id)
                .await
                .unwrap()
                .is_some()
        );
    }

    #[tokio::test]
    async fn test_unique_name_per_scope_target_library() {
        let db = setup_test_db().await;
        let user = create_test_user(&db).await;

        FilterPresetRepository::create(
            &db,
            user.id,
            "search",
            "books",
            "DupName",
            sample_condition(),
            None,
            None,
            None,
        )
        .await
        .unwrap();

        // Same (user, scope, target, name=DupName) but library_id is still NULL: conflict
        let err = FilterPresetRepository::create(
            &db,
            user.id,
            "search",
            "books",
            "DupName",
            sample_condition(),
            None,
            None,
            None,
        )
        .await;
        assert!(err.is_err(), "duplicate name in same scope should fail");

        // Different target -> allowed
        FilterPresetRepository::create(
            &db,
            user.id,
            "search",
            "series",
            "DupName",
            sample_condition(),
            None,
            None,
            None,
        )
        .await
        .expect("different target should allow same name");

        // Different scope -> allowed
        FilterPresetRepository::create(
            &db,
            user.id,
            "list",
            "books",
            "DupName",
            sample_condition(),
            None,
            None,
            None,
        )
        .await
        .expect("different scope should allow same name");
    }

    #[tokio::test]
    async fn test_different_users_can_share_name() {
        let db = setup_test_db().await;
        let user1 = create_test_user(&db).await;
        let user2 = create_test_user(&db).await;

        FilterPresetRepository::create(
            &db,
            user1.id,
            "search",
            "books",
            "Shared",
            sample_condition(),
            None,
            None,
            None,
        )
        .await
        .unwrap();

        FilterPresetRepository::create(
            &db,
            user2.id,
            "search",
            "books",
            "Shared",
            sample_condition(),
            None,
            None,
            None,
        )
        .await
        .expect("different users can both own a preset named Shared");
    }
}
