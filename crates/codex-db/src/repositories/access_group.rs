//! Repository for access groups, group memberships, group grants, and OIDC mappings.
//!
//! Access groups bundle a set of sharing-tag allow/deny rules that can be
//! assigned to many users. Per-user grants in `user_sharing_tags` act as
//! overrides on top of group rules (see `SharingTagRepository::get_effective_grants`).

use anyhow::Result;
use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
};
use uuid::Uuid;

use crate::entities::{
    access_group_oidc_mappings, access_group_sharing_tags, access_groups, user_access_groups,
    user_access_groups::MembershipSource, user_sharing_tags::AccessMode, users,
};

pub struct AccessGroupRepository;

impl AccessGroupRepository {
    // ==================== Group CRUD ====================

    /// Create a new access group.
    pub async fn create(
        db: &DatabaseConnection,
        name: &str,
        description: Option<String>,
    ) -> Result<access_groups::Model> {
        let now = Utc::now();
        let active = access_groups::ActiveModel {
            id: Set(Uuid::new_v4()),
            name: Set(name.trim().to_string()),
            description: Set(description),
            created_at: Set(now),
            updated_at: Set(now),
        };
        Ok(active.insert(db).await?)
    }

    /// Get an access group by id.
    pub async fn get_by_id(
        db: &DatabaseConnection,
        id: Uuid,
    ) -> Result<Option<access_groups::Model>> {
        Ok(access_groups::Entity::find_by_id(id).one(db).await?)
    }

    /// Get an access group by exact name.
    pub async fn get_by_name(
        db: &DatabaseConnection,
        name: &str,
    ) -> Result<Option<access_groups::Model>> {
        Ok(access_groups::Entity::find()
            .filter(access_groups::Column::Name.eq(name.trim()))
            .one(db)
            .await?)
    }

    /// List all access groups sorted by name.
    pub async fn list_all(db: &DatabaseConnection) -> Result<Vec<access_groups::Model>> {
        Ok(access_groups::Entity::find()
            .order_by_asc(access_groups::Column::Name)
            .all(db)
            .await?)
    }

    /// Update name and/or description.
    ///
    /// Pass `Some(value)` to change a field; `None` to leave it untouched.
    /// For `description`, the outer `Some` opts in and the inner `Option`
    /// decides between setting a string and clearing it.
    pub async fn update(
        db: &DatabaseConnection,
        id: Uuid,
        name: Option<String>,
        description: Option<Option<String>>,
    ) -> Result<Option<access_groups::Model>> {
        let Some(existing) = Self::get_by_id(db, id).await? else {
            return Ok(None);
        };
        let mut active: access_groups::ActiveModel = existing.into();
        if let Some(new_name) = name {
            active.name = Set(new_name.trim().to_string());
        }
        if let Some(new_desc) = description {
            active.description = Set(new_desc);
        }
        active.updated_at = Set(Utc::now());
        Ok(Some(active.update(db).await?))
    }

    /// Delete an access group (cascades memberships, grants, OIDC mappings via FK).
    pub async fn delete(db: &DatabaseConnection, id: Uuid) -> Result<bool> {
        let res = access_groups::Entity::delete_by_id(id).exec(db).await?;
        Ok(res.rows_affected > 0)
    }

    // ==================== Memberships ====================

    /// Add a user to a group. Idempotent: if the membership already exists
    /// returns the existing row. Updates the `source` if it differs.
    pub async fn add_member(
        db: &DatabaseConnection,
        group_id: Uuid,
        user_id: Uuid,
        source: MembershipSource,
    ) -> Result<user_access_groups::Model> {
        let existing = user_access_groups::Entity::find()
            .filter(user_access_groups::Column::UserId.eq(user_id))
            .filter(user_access_groups::Column::AccessGroupId.eq(group_id))
            .one(db)
            .await?;
        if let Some(row) = existing {
            if row.source == source.as_str() {
                return Ok(row);
            }
            let mut active: user_access_groups::ActiveModel = row.into();
            active.source = Set(source.as_str().to_string());
            return Ok(active.update(db).await?);
        }
        let active = user_access_groups::ActiveModel {
            id: Set(Uuid::new_v4()),
            user_id: Set(user_id),
            access_group_id: Set(group_id),
            source: Set(source.as_str().to_string()),
            created_at: Set(Utc::now()),
        };
        Ok(active.insert(db).await?)
    }

    /// Remove a user from a group. Returns true if a row was deleted.
    pub async fn remove_member(
        db: &DatabaseConnection,
        group_id: Uuid,
        user_id: Uuid,
    ) -> Result<bool> {
        let res = user_access_groups::Entity::delete_many()
            .filter(user_access_groups::Column::UserId.eq(user_id))
            .filter(user_access_groups::Column::AccessGroupId.eq(group_id))
            .exec(db)
            .await?;
        Ok(res.rows_affected > 0)
    }

    /// List membership rows for a group.
    pub async fn list_members(
        db: &DatabaseConnection,
        group_id: Uuid,
    ) -> Result<Vec<user_access_groups::Model>> {
        Ok(user_access_groups::Entity::find()
            .filter(user_access_groups::Column::AccessGroupId.eq(group_id))
            .order_by_asc(user_access_groups::Column::CreatedAt)
            .all(db)
            .await?)
    }

    /// List the users that belong to a group (joined to `users`).
    pub async fn list_member_users(
        db: &DatabaseConnection,
        group_id: Uuid,
    ) -> Result<Vec<users::Model>> {
        let memberships = Self::list_members(db, group_id).await?;
        if memberships.is_empty() {
            return Ok(vec![]);
        }
        let user_ids: Vec<Uuid> = memberships.iter().map(|m| m.user_id).collect();
        Ok(users::Entity::find()
            .filter(users::Column::Id.is_in(user_ids))
            .order_by_asc(users::Column::Username)
            .all(db)
            .await?)
    }

    /// List the groups a user belongs to.
    pub async fn list_for_user(
        db: &DatabaseConnection,
        user_id: Uuid,
    ) -> Result<Vec<access_groups::Model>> {
        let memberships = user_access_groups::Entity::find()
            .filter(user_access_groups::Column::UserId.eq(user_id))
            .all(db)
            .await?;
        if memberships.is_empty() {
            return Ok(vec![]);
        }
        let group_ids: Vec<Uuid> = memberships.iter().map(|m| m.access_group_id).collect();
        Ok(access_groups::Entity::find()
            .filter(access_groups::Column::Id.is_in(group_ids))
            .order_by_asc(access_groups::Column::Name)
            .all(db)
            .await?)
    }

    // ==================== Grants ====================

    /// Set a group's grant for a sharing tag (upsert). If a row exists with a
    /// different access_mode, it is updated; otherwise inserted.
    pub async fn set_grant(
        db: &DatabaseConnection,
        group_id: Uuid,
        tag_id: Uuid,
        access_mode: AccessMode,
    ) -> Result<access_group_sharing_tags::Model> {
        let existing = access_group_sharing_tags::Entity::find()
            .filter(access_group_sharing_tags::Column::AccessGroupId.eq(group_id))
            .filter(access_group_sharing_tags::Column::SharingTagId.eq(tag_id))
            .one(db)
            .await?;
        if let Some(row) = existing {
            let mut active: access_group_sharing_tags::ActiveModel = row.into();
            active.access_mode = Set(access_mode.as_str().to_string());
            return Ok(active.update(db).await?);
        }
        let active = access_group_sharing_tags::ActiveModel {
            id: Set(Uuid::new_v4()),
            access_group_id: Set(group_id),
            sharing_tag_id: Set(tag_id),
            access_mode: Set(access_mode.as_str().to_string()),
            created_at: Set(Utc::now()),
        };
        Ok(active.insert(db).await?)
    }

    /// Remove a group's grant for a sharing tag. Returns true if a row was deleted.
    pub async fn remove_grant(
        db: &DatabaseConnection,
        group_id: Uuid,
        tag_id: Uuid,
    ) -> Result<bool> {
        let res = access_group_sharing_tags::Entity::delete_many()
            .filter(access_group_sharing_tags::Column::AccessGroupId.eq(group_id))
            .filter(access_group_sharing_tags::Column::SharingTagId.eq(tag_id))
            .exec(db)
            .await?;
        Ok(res.rows_affected > 0)
    }

    /// List grants for a group, oldest first.
    pub async fn list_grants(
        db: &DatabaseConnection,
        group_id: Uuid,
    ) -> Result<Vec<access_group_sharing_tags::Model>> {
        Ok(access_group_sharing_tags::Entity::find()
            .filter(access_group_sharing_tags::Column::AccessGroupId.eq(group_id))
            .order_by_asc(access_group_sharing_tags::Column::CreatedAt)
            .all(db)
            .await?)
    }

    // ==================== OIDC Mappings ====================

    /// Add an OIDC group-name mapping. Idempotent: returns the existing row
    /// if one with the same `(group_id, oidc_group_name)` already exists.
    pub async fn add_oidc_mapping(
        db: &DatabaseConnection,
        group_id: Uuid,
        oidc_group_name: &str,
    ) -> Result<access_group_oidc_mappings::Model> {
        let existing = access_group_oidc_mappings::Entity::find()
            .filter(access_group_oidc_mappings::Column::AccessGroupId.eq(group_id))
            .filter(access_group_oidc_mappings::Column::OidcGroupName.eq(oidc_group_name))
            .one(db)
            .await?;
        if let Some(row) = existing {
            return Ok(row);
        }
        let active = access_group_oidc_mappings::ActiveModel {
            id: Set(Uuid::new_v4()),
            access_group_id: Set(group_id),
            oidc_group_name: Set(oidc_group_name.to_string()),
            created_at: Set(Utc::now()),
        };
        Ok(active.insert(db).await?)
    }

    /// Remove an OIDC mapping by id.
    pub async fn remove_oidc_mapping(db: &DatabaseConnection, mapping_id: Uuid) -> Result<bool> {
        let res = access_group_oidc_mappings::Entity::delete_by_id(mapping_id)
            .exec(db)
            .await?;
        Ok(res.rows_affected > 0)
    }

    /// Remove an OIDC mapping by `(group_id, oidc_group_name)` pair.
    pub async fn remove_oidc_mapping_by_name(
        db: &DatabaseConnection,
        group_id: Uuid,
        oidc_group_name: &str,
    ) -> Result<bool> {
        let res = access_group_oidc_mappings::Entity::delete_many()
            .filter(access_group_oidc_mappings::Column::AccessGroupId.eq(group_id))
            .filter(access_group_oidc_mappings::Column::OidcGroupName.eq(oidc_group_name))
            .exec(db)
            .await?;
        Ok(res.rows_affected > 0)
    }

    /// List OIDC mappings for a group.
    pub async fn list_oidc_mappings(
        db: &DatabaseConnection,
        group_id: Uuid,
    ) -> Result<Vec<access_group_oidc_mappings::Model>> {
        Ok(access_group_oidc_mappings::Entity::find()
            .filter(access_group_oidc_mappings::Column::AccessGroupId.eq(group_id))
            .order_by_asc(access_group_oidc_mappings::Column::OidcGroupName)
            .all(db)
            .await?)
    }

    // ==================== OIDC Reconciliation ====================

    /// Reconcile a user's OIDC-sourced group memberships with their current
    /// IdP group claims.
    ///
    /// - Computes the set of access groups the user *should* belong to based
    ///   on `access_group_oidc_mappings` that match any of `oidc_groups`.
    /// - Adds memberships for groups in `desired - current` (with `source='oidc'`).
    /// - Removes memberships for groups in `current - desired` (only `source='oidc'`).
    /// - Manual memberships are never touched.
    ///
    /// Returns `(added, removed)` counts.
    pub async fn reconcile_oidc_group_memberships(
        db: &DatabaseConnection,
        user_id: Uuid,
        oidc_groups: &[String],
    ) -> Result<(usize, usize)> {
        use std::collections::HashSet;

        // 1. Desired: access groups mapped to any of the user's OIDC group names
        let desired_group_ids: HashSet<Uuid> = if oidc_groups.is_empty() {
            HashSet::new()
        } else {
            access_group_oidc_mappings::Entity::find()
                .filter(
                    access_group_oidc_mappings::Column::OidcGroupName.is_in(oidc_groups.to_vec()),
                )
                .all(db)
                .await?
                .into_iter()
                .map(|m| m.access_group_id)
                .collect()
        };

        // 2. Current: access groups the user is already in via OIDC
        let current_oidc_memberships: Vec<user_access_groups::Model> =
            user_access_groups::Entity::find()
                .filter(user_access_groups::Column::UserId.eq(user_id))
                .filter(user_access_groups::Column::Source.eq("oidc"))
                .all(db)
                .await?;
        let current_group_ids: HashSet<Uuid> = current_oidc_memberships
            .iter()
            .map(|m| m.access_group_id)
            .collect();

        // 3. Add: desired - current
        let to_add: Vec<Uuid> = desired_group_ids
            .difference(&current_group_ids)
            .copied()
            .collect();
        for group_id in &to_add {
            Self::add_member(db, *group_id, user_id, MembershipSource::Oidc).await?;
        }

        // 4. Remove: current - desired (only OIDC-sourced)
        let to_remove: Vec<Uuid> = current_group_ids
            .difference(&desired_group_ids)
            .copied()
            .collect();
        for group_id in &to_remove {
            // Only remove if source is OIDC (which it is, since we filtered above)
            Self::remove_member(db, *group_id, user_id).await?;
        }

        Ok((to_add.len(), to_remove.len()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repositories::{SharingTagRepository, UserRepository};
    use crate::test_helpers::setup_test_db;

    async fn create_test_user(db: &DatabaseConnection, username: &str) -> users::Model {
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

    // ---------- Group CRUD ----------

    #[tokio::test]
    async fn create_get_list_delete_group() {
        let db = setup_test_db().await;

        let g = AccessGroupRepository::create(&db, "Manga Readers", Some("desc".into()))
            .await
            .unwrap();
        assert_eq!(g.name, "Manga Readers");
        assert_eq!(g.description.as_deref(), Some("desc"));

        let fetched = AccessGroupRepository::get_by_id(&db, g.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(fetched.id, g.id);

        let by_name = AccessGroupRepository::get_by_name(&db, "Manga Readers")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(by_name.id, g.id);

        let all = AccessGroupRepository::list_all(&db).await.unwrap();
        assert_eq!(all.len(), 1);

        assert!(AccessGroupRepository::delete(&db, g.id).await.unwrap());
        assert!(
            AccessGroupRepository::get_by_id(&db, g.id)
                .await
                .unwrap()
                .is_none()
        );
    }

    #[tokio::test]
    async fn update_group_fields_independently() {
        let db = setup_test_db().await;
        let g = AccessGroupRepository::create(&db, "Staff", Some("original".into()))
            .await
            .unwrap();

        // Update name only
        let updated = AccessGroupRepository::update(&db, g.id, Some("Library Staff".into()), None)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(updated.name, "Library Staff");
        assert_eq!(updated.description.as_deref(), Some("original"));

        // Clear description
        let cleared = AccessGroupRepository::update(&db, g.id, None, Some(None))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(cleared.description, None);

        // Missing id returns None (not an error)
        let missing = AccessGroupRepository::update(&db, Uuid::new_v4(), Some("x".into()), None)
            .await
            .unwrap();
        assert!(missing.is_none());
    }

    #[tokio::test]
    async fn list_all_sorts_by_name() {
        let db = setup_test_db().await;
        AccessGroupRepository::create(&db, "Zebra", None)
            .await
            .unwrap();
        AccessGroupRepository::create(&db, "Alpha", None)
            .await
            .unwrap();
        AccessGroupRepository::create(&db, "Mango", None)
            .await
            .unwrap();

        let groups = AccessGroupRepository::list_all(&db).await.unwrap();
        let names: Vec<&str> = groups.iter().map(|g| g.name.as_str()).collect();
        assert_eq!(names, vec!["Alpha", "Mango", "Zebra"]);
    }

    // ---------- Memberships ----------

    #[tokio::test]
    async fn add_member_idempotent_and_source_aware() {
        let db = setup_test_db().await;
        let group = AccessGroupRepository::create(&db, "G", None).await.unwrap();
        let user = create_test_user(&db, "u").await;

        // First add: insert
        let m1 =
            AccessGroupRepository::add_member(&db, group.id, user.id, MembershipSource::Manual)
                .await
                .unwrap();
        assert_eq!(m1.source, "manual");

        // Same source: returns existing row id
        let m2 =
            AccessGroupRepository::add_member(&db, group.id, user.id, MembershipSource::Manual)
                .await
                .unwrap();
        assert_eq!(m1.id, m2.id);
        assert_eq!(
            AccessGroupRepository::list_members(&db, group.id)
                .await
                .unwrap()
                .len(),
            1
        );

        // Different source: same row, source updated
        let m3 = AccessGroupRepository::add_member(&db, group.id, user.id, MembershipSource::Oidc)
            .await
            .unwrap();
        assert_eq!(m1.id, m3.id);
        assert_eq!(m3.source, "oidc");
        assert_eq!(m3.get_source(), MembershipSource::Oidc);
    }

    #[tokio::test]
    async fn remove_member_returns_false_when_missing() {
        let db = setup_test_db().await;
        let group = AccessGroupRepository::create(&db, "G", None).await.unwrap();
        let user = create_test_user(&db, "u").await;

        assert!(
            !AccessGroupRepository::remove_member(&db, group.id, user.id)
                .await
                .unwrap()
        );

        AccessGroupRepository::add_member(&db, group.id, user.id, MembershipSource::Manual)
            .await
            .unwrap();
        assert!(
            AccessGroupRepository::remove_member(&db, group.id, user.id)
                .await
                .unwrap()
        );
        assert!(
            AccessGroupRepository::list_members(&db, group.id)
                .await
                .unwrap()
                .is_empty()
        );
    }

    #[tokio::test]
    async fn list_for_user_and_list_member_users() {
        let db = setup_test_db().await;
        let g1 = AccessGroupRepository::create(&db, "Alpha", None)
            .await
            .unwrap();
        let g2 = AccessGroupRepository::create(&db, "Beta", None)
            .await
            .unwrap();
        let u1 = create_test_user(&db, "alice").await;
        let u2 = create_test_user(&db, "bob").await;

        AccessGroupRepository::add_member(&db, g1.id, u1.id, MembershipSource::Manual)
            .await
            .unwrap();
        AccessGroupRepository::add_member(&db, g2.id, u1.id, MembershipSource::Manual)
            .await
            .unwrap();
        AccessGroupRepository::add_member(&db, g1.id, u2.id, MembershipSource::Manual)
            .await
            .unwrap();

        let alice_groups = AccessGroupRepository::list_for_user(&db, u1.id)
            .await
            .unwrap();
        let names: Vec<&str> = alice_groups.iter().map(|g| g.name.as_str()).collect();
        assert_eq!(names, vec!["Alpha", "Beta"]);

        let alpha_users = AccessGroupRepository::list_member_users(&db, g1.id)
            .await
            .unwrap();
        let usernames: Vec<&str> = alpha_users.iter().map(|u| u.username.as_str()).collect();
        assert_eq!(usernames, vec!["alice", "bob"]);
    }

    #[tokio::test]
    async fn deleting_group_cascades_memberships_and_grants() {
        let db = setup_test_db().await;
        let group = AccessGroupRepository::create(&db, "Doomed", None)
            .await
            .unwrap();
        let user = create_test_user(&db, "u").await;
        let tag = SharingTagRepository::create(&db, "tag", None)
            .await
            .unwrap();

        AccessGroupRepository::add_member(&db, group.id, user.id, MembershipSource::Manual)
            .await
            .unwrap();
        AccessGroupRepository::set_grant(&db, group.id, tag.id, AccessMode::Allow)
            .await
            .unwrap();
        AccessGroupRepository::add_oidc_mapping(&db, group.id, "library-staff")
            .await
            .unwrap();

        assert!(AccessGroupRepository::delete(&db, group.id).await.unwrap());

        assert!(
            AccessGroupRepository::list_for_user(&db, user.id)
                .await
                .unwrap()
                .is_empty()
        );
        // The grants/mappings tables are scoped to the group; verify via list_*.
        // After the FK cascade, querying by the deleted group id must return empty.
        assert!(
            AccessGroupRepository::list_grants(&db, group.id)
                .await
                .unwrap()
                .is_empty()
        );
        assert!(
            AccessGroupRepository::list_oidc_mappings(&db, group.id)
                .await
                .unwrap()
                .is_empty()
        );
    }

    // ---------- Grants ----------

    #[tokio::test]
    async fn set_grant_inserts_then_updates_mode() {
        let db = setup_test_db().await;
        let group = AccessGroupRepository::create(&db, "G", None).await.unwrap();
        let tag = SharingTagRepository::create(&db, "manga", None)
            .await
            .unwrap();

        let g1 = AccessGroupRepository::set_grant(&db, group.id, tag.id, AccessMode::Allow)
            .await
            .unwrap();
        assert_eq!(g1.access_mode, "allow");

        // Re-setting with a different mode flips it in place (same id).
        let g2 = AccessGroupRepository::set_grant(&db, group.id, tag.id, AccessMode::Deny)
            .await
            .unwrap();
        assert_eq!(g1.id, g2.id);
        assert_eq!(g2.access_mode, "deny");
        assert_eq!(g2.get_access_mode(), AccessMode::Deny);

        assert_eq!(
            AccessGroupRepository::list_grants(&db, group.id)
                .await
                .unwrap()
                .len(),
            1
        );
    }

    #[tokio::test]
    async fn remove_grant_is_idempotent() {
        let db = setup_test_db().await;
        let group = AccessGroupRepository::create(&db, "G", None).await.unwrap();
        let tag = SharingTagRepository::create(&db, "tag", None)
            .await
            .unwrap();

        // Removing when nothing is granted is a no-op.
        assert!(
            !AccessGroupRepository::remove_grant(&db, group.id, tag.id)
                .await
                .unwrap()
        );

        AccessGroupRepository::set_grant(&db, group.id, tag.id, AccessMode::Allow)
            .await
            .unwrap();
        assert!(
            AccessGroupRepository::remove_grant(&db, group.id, tag.id)
                .await
                .unwrap()
        );
    }

    // ---------- OIDC mappings ----------

    #[tokio::test]
    async fn add_oidc_mapping_is_idempotent() {
        let db = setup_test_db().await;
        let group = AccessGroupRepository::create(&db, "Staff", None)
            .await
            .unwrap();

        let m1 = AccessGroupRepository::add_oidc_mapping(&db, group.id, "library-staff")
            .await
            .unwrap();
        let m2 = AccessGroupRepository::add_oidc_mapping(&db, group.id, "library-staff")
            .await
            .unwrap();
        assert_eq!(m1.id, m2.id);

        let m3 = AccessGroupRepository::add_oidc_mapping(&db, group.id, "library-admins")
            .await
            .unwrap();
        assert_ne!(m1.id, m3.id);

        let mappings = AccessGroupRepository::list_oidc_mappings(&db, group.id)
            .await
            .unwrap();
        let names: Vec<&str> = mappings
            .iter()
            .map(|m| m.oidc_group_name.as_str())
            .collect();
        assert_eq!(names, vec!["library-admins", "library-staff"]);
    }

    #[tokio::test]
    async fn remove_oidc_mapping_by_id_and_by_name() {
        let db = setup_test_db().await;
        let group = AccessGroupRepository::create(&db, "G", None).await.unwrap();

        let m = AccessGroupRepository::add_oidc_mapping(&db, group.id, "team-a")
            .await
            .unwrap();
        assert!(
            AccessGroupRepository::remove_oidc_mapping(&db, m.id)
                .await
                .unwrap()
        );
        assert!(
            AccessGroupRepository::list_oidc_mappings(&db, group.id)
                .await
                .unwrap()
                .is_empty()
        );

        AccessGroupRepository::add_oidc_mapping(&db, group.id, "team-b")
            .await
            .unwrap();
        assert!(
            AccessGroupRepository::remove_oidc_mapping_by_name(&db, group.id, "team-b")
                .await
                .unwrap()
        );
        assert!(
            !AccessGroupRepository::remove_oidc_mapping_by_name(&db, group.id, "team-b")
                .await
                .unwrap()
        );
    }

    // ---------- OIDC reconciliation ----------

    #[tokio::test]
    async fn reconcile_joins_matching_groups() {
        let db = setup_test_db().await;
        let user = create_test_user(&db, "alice").await;

        let staff = AccessGroupRepository::create(&db, "Staff", None)
            .await
            .unwrap();
        AccessGroupRepository::add_oidc_mapping(&db, staff.id, "library-staff")
            .await
            .unwrap();

        let (added, removed) = AccessGroupRepository::reconcile_oidc_group_memberships(
            &db,
            user.id,
            &["library-staff".to_string()],
        )
        .await
        .unwrap();

        assert_eq!(added, 1);
        assert_eq!(removed, 0);

        let groups = AccessGroupRepository::list_for_user(&db, user.id)
            .await
            .unwrap();
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].id, staff.id);
    }

    #[tokio::test]
    async fn reconcile_leaves_removed_groups() {
        let db = setup_test_db().await;
        let user = create_test_user(&db, "alice").await;

        let staff = AccessGroupRepository::create(&db, "Staff", None)
            .await
            .unwrap();
        AccessGroupRepository::add_oidc_mapping(&db, staff.id, "library-staff")
            .await
            .unwrap();

        // First login: join
        AccessGroupRepository::reconcile_oidc_group_memberships(
            &db,
            user.id,
            &["library-staff".to_string()],
        )
        .await
        .unwrap();
        assert_eq!(
            AccessGroupRepository::list_for_user(&db, user.id)
                .await
                .unwrap()
                .len(),
            1
        );

        // Second login: group removed from IdP
        let (added, removed) =
            AccessGroupRepository::reconcile_oidc_group_memberships(&db, user.id, &[])
                .await
                .unwrap();
        assert_eq!(added, 0);
        assert_eq!(removed, 1);
        assert!(
            AccessGroupRepository::list_for_user(&db, user.id)
                .await
                .unwrap()
                .is_empty()
        );
    }

    #[tokio::test]
    async fn reconcile_no_change_when_groups_match() {
        let db = setup_test_db().await;
        let user = create_test_user(&db, "alice").await;

        let staff = AccessGroupRepository::create(&db, "Staff", None)
            .await
            .unwrap();
        AccessGroupRepository::add_oidc_mapping(&db, staff.id, "library-staff")
            .await
            .unwrap();

        // First login
        AccessGroupRepository::reconcile_oidc_group_memberships(
            &db,
            user.id,
            &["library-staff".to_string()],
        )
        .await
        .unwrap();

        // Second login with same groups: no-op
        let (added, removed) = AccessGroupRepository::reconcile_oidc_group_memberships(
            &db,
            user.id,
            &["library-staff".to_string()],
        )
        .await
        .unwrap();
        assert_eq!(added, 0);
        assert_eq!(removed, 0);

        assert_eq!(
            AccessGroupRepository::list_for_user(&db, user.id)
                .await
                .unwrap()
                .len(),
            1
        );
    }

    #[tokio::test]
    async fn reconcile_no_mappings_exist() {
        let db = setup_test_db().await;
        let user = create_test_user(&db, "alice").await;

        // No access_group_oidc_mappings exist at all
        let (added, removed) = AccessGroupRepository::reconcile_oidc_group_memberships(
            &db,
            user.id,
            &["some-idp-group".to_string()],
        )
        .await
        .unwrap();
        assert_eq!(added, 0);
        assert_eq!(removed, 0);
    }

    #[tokio::test]
    async fn reconcile_preserves_manual_memberships() {
        let db = setup_test_db().await;
        let user = create_test_user(&db, "alice").await;

        let manual_group = AccessGroupRepository::create(&db, "Manual Group", None)
            .await
            .unwrap();
        let oidc_group = AccessGroupRepository::create(&db, "OIDC Group", None)
            .await
            .unwrap();

        // Manually assign user to manual_group
        AccessGroupRepository::add_member(&db, manual_group.id, user.id, MembershipSource::Manual)
            .await
            .unwrap();

        // Map oidc_group to an IdP group
        AccessGroupRepository::add_oidc_mapping(&db, oidc_group.id, "idp-team")
            .await
            .unwrap();

        // Reconcile with the IdP group
        AccessGroupRepository::reconcile_oidc_group_memberships(
            &db,
            user.id,
            &["idp-team".to_string()],
        )
        .await
        .unwrap();

        let groups = AccessGroupRepository::list_for_user(&db, user.id)
            .await
            .unwrap();
        assert_eq!(groups.len(), 2);

        // Now reconcile with empty groups (user left IdP group)
        AccessGroupRepository::reconcile_oidc_group_memberships(&db, user.id, &[])
            .await
            .unwrap();

        // Manual membership should be preserved, OIDC membership removed
        let groups = AccessGroupRepository::list_for_user(&db, user.id)
            .await
            .unwrap();
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].id, manual_group.id);
    }

    #[tokio::test]
    async fn reconcile_idempotent_back_to_back() {
        let db = setup_test_db().await;
        let user = create_test_user(&db, "alice").await;

        let group = AccessGroupRepository::create(&db, "Staff", None)
            .await
            .unwrap();
        AccessGroupRepository::add_oidc_mapping(&db, group.id, "staff")
            .await
            .unwrap();

        let groups = vec!["staff".to_string()];

        // First call
        let (a1, r1) =
            AccessGroupRepository::reconcile_oidc_group_memberships(&db, user.id, &groups)
                .await
                .unwrap();
        assert_eq!(a1, 1);
        assert_eq!(r1, 0);

        // Record membership timestamp
        let memberships_after_first = AccessGroupRepository::list_members(&db, group.id)
            .await
            .unwrap();
        assert_eq!(memberships_after_first.len(), 1);
        let first_created_at = memberships_after_first[0].created_at;

        // Second call: no changes
        let (a2, r2) =
            AccessGroupRepository::reconcile_oidc_group_memberships(&db, user.id, &groups)
                .await
                .unwrap();
        assert_eq!(a2, 0);
        assert_eq!(r2, 0);

        // Timestamp unchanged (no spurious delete+insert)
        let memberships_after_second = AccessGroupRepository::list_members(&db, group.id)
            .await
            .unwrap();
        assert_eq!(memberships_after_second.len(), 1);
        assert_eq!(memberships_after_second[0].created_at, first_created_at);
    }
}
