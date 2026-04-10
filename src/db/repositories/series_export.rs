//! Series Export Repository
//!
//! CRUD and query operations for series export jobs.

use crate::db::entities::series_exports::{self, Entity as SeriesExport};
use anyhow::Result;
use chrono::{DateTime, Utc};
use sea_orm::*;
use uuid::Uuid;

pub struct SeriesExportRepository;

impl SeriesExportRepository {
    // =========================================================================
    // Create
    // =========================================================================

    /// Create a new export record in "pending" status.
    pub async fn create(
        db: &DatabaseConnection,
        user_id: Uuid,
        format: &str,
        library_ids: serde_json::Value,
        fields: serde_json::Value,
        expires_at: DateTime<Utc>,
    ) -> Result<series_exports::Model> {
        let now = Utc::now();
        let model = series_exports::ActiveModel {
            id: Set(Uuid::new_v4()),
            user_id: Set(user_id),
            format: Set(format.to_string()),
            status: Set("pending".to_string()),
            library_ids: Set(library_ids),
            fields: Set(fields),
            file_path: Set(None),
            file_size_bytes: Set(None),
            row_count: Set(None),
            error: Set(None),
            task_id: Set(None),
            created_at: Set(now),
            started_at: Set(None),
            completed_at: Set(None),
            expires_at: Set(expires_at),
        };
        let result = model.insert(db).await?;
        Ok(result)
    }

    // =========================================================================
    // Read
    // =========================================================================

    /// Find an export by ID.
    pub async fn find_by_id(
        db: &DatabaseConnection,
        id: Uuid,
    ) -> Result<Option<series_exports::Model>> {
        let result = SeriesExport::find_by_id(id).one(db).await?;
        Ok(result)
    }

    /// Find an export by ID, scoped to a specific user.
    pub async fn find_by_id_and_user(
        db: &DatabaseConnection,
        id: Uuid,
        user_id: Uuid,
    ) -> Result<Option<series_exports::Model>> {
        let result = SeriesExport::find_by_id(id)
            .filter(series_exports::Column::UserId.eq(user_id))
            .one(db)
            .await?;
        Ok(result)
    }

    /// List all exports for a user, ordered by created_at descending.
    pub async fn list_by_user(
        db: &DatabaseConnection,
        user_id: Uuid,
    ) -> Result<Vec<series_exports::Model>> {
        let results = SeriesExport::find()
            .filter(series_exports::Column::UserId.eq(user_id))
            .order_by_desc(series_exports::Column::CreatedAt)
            .all(db)
            .await?;
        Ok(results)
    }

    /// Count exports for a user that are in non-terminal status (pending or running).
    pub async fn count_non_terminal_by_user(db: &DatabaseConnection, user_id: Uuid) -> Result<u64> {
        let count = SeriesExport::find()
            .filter(series_exports::Column::UserId.eq(user_id))
            .filter(
                series_exports::Column::Status
                    .is_in(["pending".to_string(), "running".to_string()]),
            )
            .count(db)
            .await?;
        Ok(count)
    }

    /// Count all exports for a user.
    pub async fn count_by_user(db: &DatabaseConnection, user_id: Uuid) -> Result<u64> {
        let count = SeriesExport::find()
            .filter(series_exports::Column::UserId.eq(user_id))
            .count(db)
            .await?;
        Ok(count)
    }

    /// Sum of file_size_bytes across all exports (for storage cap enforcement).
    pub async fn total_size_bytes(db: &DatabaseConnection) -> Result<i64> {
        use sea_orm::sea_query::Expr;

        let result = SeriesExport::find()
            .select_only()
            .column_as(
                Expr::col(series_exports::Column::FileSizeBytes).sum(),
                "total",
            )
            .into_tuple::<Option<i64>>()
            .one(db)
            .await?;

        Ok(result.flatten().unwrap_or(0))
    }

    /// List completed exports that have expired (expires_at < now).
    pub async fn list_expired(
        db: &DatabaseConnection,
        now: DateTime<Utc>,
    ) -> Result<Vec<series_exports::Model>> {
        let results = SeriesExport::find()
            .filter(series_exports::Column::Status.eq("completed"))
            .filter(series_exports::Column::ExpiresAt.lt(now))
            .all(db)
            .await?;
        Ok(results)
    }

    /// List the oldest completed exports for a user beyond a keep limit.
    /// Returns exports that should be evicted, ordered oldest first.
    pub async fn list_oldest_for_user(
        db: &DatabaseConnection,
        user_id: Uuid,
        keep: u64,
    ) -> Result<Vec<series_exports::Model>> {
        // Get all completed exports for the user, newest first
        let all_completed = SeriesExport::find()
            .filter(series_exports::Column::UserId.eq(user_id))
            .filter(series_exports::Column::Status.eq("completed"))
            .order_by_desc(series_exports::Column::CreatedAt)
            .all(db)
            .await?;

        // Skip the newest `keep` entries, return the rest (to be evicted)
        let to_evict: Vec<_> = all_completed.into_iter().skip(keep as usize).collect();
        Ok(to_evict)
    }

    /// List all exports in non-terminal status (for restart reconciliation).
    pub async fn list_non_terminal(db: &DatabaseConnection) -> Result<Vec<series_exports::Model>> {
        let results = SeriesExport::find()
            .filter(
                series_exports::Column::Status
                    .is_in(["pending".to_string(), "running".to_string()]),
            )
            .all(db)
            .await?;
        Ok(results)
    }

    // =========================================================================
    // Update
    // =========================================================================

    /// Mark an export as running and associate it with a task.
    pub async fn mark_running(
        db: &DatabaseConnection,
        id: Uuid,
        task_id: Uuid,
    ) -> Result<series_exports::Model> {
        let export = SeriesExport::find_by_id(id)
            .one(db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Export not found: {id}"))?;

        let mut active: series_exports::ActiveModel = export.into();
        active.status = Set("running".to_string());
        active.task_id = Set(Some(task_id));
        active.started_at = Set(Some(Utc::now()));
        let result = active.update(db).await?;
        Ok(result)
    }

    /// Mark an export as completed with file metadata.
    pub async fn mark_completed(
        db: &DatabaseConnection,
        id: Uuid,
        file_path: &str,
        file_size_bytes: i64,
        row_count: i32,
    ) -> Result<series_exports::Model> {
        let export = SeriesExport::find_by_id(id)
            .one(db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Export not found: {id}"))?;

        let mut active: series_exports::ActiveModel = export.into();
        active.status = Set("completed".to_string());
        active.file_path = Set(Some(file_path.to_string()));
        active.file_size_bytes = Set(Some(file_size_bytes));
        active.row_count = Set(Some(row_count));
        active.completed_at = Set(Some(Utc::now()));
        let result = active.update(db).await?;
        Ok(result)
    }

    /// Mark an export as failed with an error message.
    pub async fn mark_failed(
        db: &DatabaseConnection,
        id: Uuid,
        error: &str,
    ) -> Result<series_exports::Model> {
        let export = SeriesExport::find_by_id(id)
            .one(db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Export not found: {id}"))?;

        let mut active: series_exports::ActiveModel = export.into();
        active.status = Set("failed".to_string());
        active.error = Set(Some(error.to_string()));
        active.completed_at = Set(Some(Utc::now()));
        let result = active.update(db).await?;
        Ok(result)
    }

    // =========================================================================
    // Delete
    // =========================================================================

    /// Delete an export by ID. Returns true if it existed.
    pub async fn delete_by_id(db: &DatabaseConnection, id: Uuid) -> Result<bool> {
        let result = SeriesExport::delete_by_id(id).exec(db).await?;
        Ok(result.rows_affected > 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::repositories::UserRepository;
    use crate::db::test_helpers::setup_test_db;
    use chrono::Duration;

    async fn create_test_user(db: &DatabaseConnection) -> crate::db::entities::users::Model {
        let user = crate::db::entities::users::Model {
            id: Uuid::new_v4(),
            username: format!("export_user_{}", Uuid::new_v4()),
            email: format!("export_{}@example.com", Uuid::new_v4()),
            password_hash: "hash123".to_string(),
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

    fn default_library_ids() -> serde_json::Value {
        serde_json::json!([Uuid::new_v4().to_string()])
    }

    fn default_fields() -> serde_json::Value {
        serde_json::json!(["title", "summary", "genres"])
    }

    #[tokio::test]
    async fn test_create_and_find_by_id() {
        let db = setup_test_db().await;
        let user = create_test_user(&db).await;
        let expires = Utc::now() + Duration::days(7);

        let export = SeriesExportRepository::create(
            &db,
            user.id,
            "json",
            default_library_ids(),
            default_fields(),
            expires,
        )
        .await
        .unwrap();

        assert_eq!(export.format, "json");
        assert_eq!(export.status, "pending");
        assert!(export.file_path.is_none());

        let found = SeriesExportRepository::find_by_id(&db, export.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(found.id, export.id);
    }

    #[tokio::test]
    async fn test_find_by_id_and_user() {
        let db = setup_test_db().await;
        let user1 = create_test_user(&db).await;
        let user2 = create_test_user(&db).await;
        let expires = Utc::now() + Duration::days(7);

        let export = SeriesExportRepository::create(
            &db,
            user1.id,
            "csv",
            default_library_ids(),
            default_fields(),
            expires,
        )
        .await
        .unwrap();

        // Owner can find it
        let found = SeriesExportRepository::find_by_id_and_user(&db, export.id, user1.id)
            .await
            .unwrap();
        assert!(found.is_some());

        // Other user cannot
        let not_found = SeriesExportRepository::find_by_id_and_user(&db, export.id, user2.id)
            .await
            .unwrap();
        assert!(not_found.is_none());
    }

    #[tokio::test]
    async fn test_list_by_user_ordered() {
        let db = setup_test_db().await;
        let user = create_test_user(&db).await;
        let expires = Utc::now() + Duration::days(7);

        let e1 = SeriesExportRepository::create(
            &db,
            user.id,
            "json",
            default_library_ids(),
            default_fields(),
            expires,
        )
        .await
        .unwrap();

        let e2 = SeriesExportRepository::create(
            &db,
            user.id,
            "csv",
            default_library_ids(),
            default_fields(),
            expires,
        )
        .await
        .unwrap();

        let list = SeriesExportRepository::list_by_user(&db, user.id)
            .await
            .unwrap();

        assert_eq!(list.len(), 2);
        // Most recent first
        assert_eq!(list[0].id, e2.id);
        assert_eq!(list[1].id, e1.id);
    }

    #[tokio::test]
    async fn test_count_non_terminal() {
        let db = setup_test_db().await;
        let user = create_test_user(&db).await;
        let expires = Utc::now() + Duration::days(7);

        // Create 2 pending exports
        SeriesExportRepository::create(
            &db,
            user.id,
            "json",
            default_library_ids(),
            default_fields(),
            expires,
        )
        .await
        .unwrap();

        let e2 = SeriesExportRepository::create(
            &db,
            user.id,
            "csv",
            default_library_ids(),
            default_fields(),
            expires,
        )
        .await
        .unwrap();

        assert_eq!(
            SeriesExportRepository::count_non_terminal_by_user(&db, user.id)
                .await
                .unwrap(),
            2
        );

        // Complete one
        SeriesExportRepository::mark_completed(&db, e2.id, "/tmp/test.csv", 1024, 10)
            .await
            .unwrap();

        assert_eq!(
            SeriesExportRepository::count_non_terminal_by_user(&db, user.id)
                .await
                .unwrap(),
            1
        );
    }

    #[tokio::test]
    async fn test_mark_running() {
        let db = setup_test_db().await;
        let user = create_test_user(&db).await;
        let expires = Utc::now() + Duration::days(7);

        let export = SeriesExportRepository::create(
            &db,
            user.id,
            "json",
            default_library_ids(),
            default_fields(),
            expires,
        )
        .await
        .unwrap();

        let task_id = Uuid::new_v4();
        let updated = SeriesExportRepository::mark_running(&db, export.id, task_id)
            .await
            .unwrap();

        assert_eq!(updated.status, "running");
        assert_eq!(updated.task_id, Some(task_id));
        assert!(updated.started_at.is_some());
    }

    #[tokio::test]
    async fn test_mark_completed() {
        let db = setup_test_db().await;
        let user = create_test_user(&db).await;
        let expires = Utc::now() + Duration::days(7);

        let export = SeriesExportRepository::create(
            &db,
            user.id,
            "json",
            default_library_ids(),
            default_fields(),
            expires,
        )
        .await
        .unwrap();

        let updated =
            SeriesExportRepository::mark_completed(&db, export.id, "/exports/test.json", 2048, 42)
                .await
                .unwrap();

        assert_eq!(updated.status, "completed");
        assert_eq!(updated.file_path.as_deref(), Some("/exports/test.json"));
        assert_eq!(updated.file_size_bytes, Some(2048));
        assert_eq!(updated.row_count, Some(42));
        assert!(updated.completed_at.is_some());
    }

    #[tokio::test]
    async fn test_mark_failed() {
        let db = setup_test_db().await;
        let user = create_test_user(&db).await;
        let expires = Utc::now() + Duration::days(7);

        let export = SeriesExportRepository::create(
            &db,
            user.id,
            "json",
            default_library_ids(),
            default_fields(),
            expires,
        )
        .await
        .unwrap();

        let updated = SeriesExportRepository::mark_failed(&db, export.id, "disk full")
            .await
            .unwrap();

        assert_eq!(updated.status, "failed");
        assert_eq!(updated.error.as_deref(), Some("disk full"));
        assert!(updated.completed_at.is_some());
    }

    #[tokio::test]
    async fn test_delete_by_id() {
        let db = setup_test_db().await;
        let user = create_test_user(&db).await;
        let expires = Utc::now() + Duration::days(7);

        let export = SeriesExportRepository::create(
            &db,
            user.id,
            "json",
            default_library_ids(),
            default_fields(),
            expires,
        )
        .await
        .unwrap();

        let deleted = SeriesExportRepository::delete_by_id(&db, export.id)
            .await
            .unwrap();
        assert!(deleted);

        let found = SeriesExportRepository::find_by_id(&db, export.id)
            .await
            .unwrap();
        assert!(found.is_none());

        // Deleting non-existent returns false
        let deleted_again = SeriesExportRepository::delete_by_id(&db, export.id)
            .await
            .unwrap();
        assert!(!deleted_again);
    }

    #[tokio::test]
    async fn test_total_size_bytes() {
        let db = setup_test_db().await;
        let user = create_test_user(&db).await;
        let expires = Utc::now() + Duration::days(7);

        // Empty total
        assert_eq!(
            SeriesExportRepository::total_size_bytes(&db).await.unwrap(),
            0
        );

        let e1 = SeriesExportRepository::create(
            &db,
            user.id,
            "json",
            default_library_ids(),
            default_fields(),
            expires,
        )
        .await
        .unwrap();
        SeriesExportRepository::mark_completed(&db, e1.id, "/a.json", 1000, 5)
            .await
            .unwrap();

        let e2 = SeriesExportRepository::create(
            &db,
            user.id,
            "csv",
            default_library_ids(),
            default_fields(),
            expires,
        )
        .await
        .unwrap();
        SeriesExportRepository::mark_completed(&db, e2.id, "/b.csv", 2500, 10)
            .await
            .unwrap();

        assert_eq!(
            SeriesExportRepository::total_size_bytes(&db).await.unwrap(),
            3500
        );
    }

    #[tokio::test]
    async fn test_list_expired() {
        let db = setup_test_db().await;
        let user = create_test_user(&db).await;
        let now = Utc::now();

        // Expired export
        let e1 = SeriesExportRepository::create(
            &db,
            user.id,
            "json",
            default_library_ids(),
            default_fields(),
            now - Duration::hours(1),
        )
        .await
        .unwrap();
        SeriesExportRepository::mark_completed(&db, e1.id, "/a.json", 100, 1)
            .await
            .unwrap();

        // Not expired
        let e2 = SeriesExportRepository::create(
            &db,
            user.id,
            "csv",
            default_library_ids(),
            default_fields(),
            now + Duration::days(7),
        )
        .await
        .unwrap();
        SeriesExportRepository::mark_completed(&db, e2.id, "/b.csv", 200, 2)
            .await
            .unwrap();

        // Pending (not completed, so not eligible even if expired)
        SeriesExportRepository::create(
            &db,
            user.id,
            "json",
            default_library_ids(),
            default_fields(),
            now - Duration::hours(2),
        )
        .await
        .unwrap();

        let expired = SeriesExportRepository::list_expired(&db, now)
            .await
            .unwrap();
        assert_eq!(expired.len(), 1);
        assert_eq!(expired[0].id, e1.id);
    }

    #[tokio::test]
    async fn test_list_oldest_for_user_eviction() {
        let db = setup_test_db().await;
        let user = create_test_user(&db).await;
        let expires = Utc::now() + Duration::days(7);

        // Create 5 completed exports
        let mut ids = Vec::new();
        for _ in 0..5 {
            let e = SeriesExportRepository::create(
                &db,
                user.id,
                "json",
                default_library_ids(),
                default_fields(),
                expires,
            )
            .await
            .unwrap();
            SeriesExportRepository::mark_completed(&db, e.id, "/test.json", 100, 1)
                .await
                .unwrap();
            ids.push(e.id);
        }

        // Keep 3 → should return 2 to evict (the oldest)
        let to_evict = SeriesExportRepository::list_oldest_for_user(&db, user.id, 3)
            .await
            .unwrap();
        assert_eq!(to_evict.len(), 2);
        // The evicted ones should be the first two created
        assert_eq!(to_evict[0].id, ids[1]);
        assert_eq!(to_evict[1].id, ids[0]);
    }

    #[tokio::test]
    async fn test_list_non_terminal() {
        let db = setup_test_db().await;
        let user = create_test_user(&db).await;
        let expires = Utc::now() + Duration::days(7);

        let e1 = SeriesExportRepository::create(
            &db,
            user.id,
            "json",
            default_library_ids(),
            default_fields(),
            expires,
        )
        .await
        .unwrap();

        let e2 = SeriesExportRepository::create(
            &db,
            user.id,
            "csv",
            default_library_ids(),
            default_fields(),
            expires,
        )
        .await
        .unwrap();
        SeriesExportRepository::mark_running(&db, e2.id, Uuid::new_v4())
            .await
            .unwrap();

        let e3 = SeriesExportRepository::create(
            &db,
            user.id,
            "json",
            default_library_ids(),
            default_fields(),
            expires,
        )
        .await
        .unwrap();
        SeriesExportRepository::mark_completed(&db, e3.id, "/a.json", 100, 1)
            .await
            .unwrap();

        let non_terminal = SeriesExportRepository::list_non_terminal(&db)
            .await
            .unwrap();
        assert_eq!(non_terminal.len(), 2);
        let ids: Vec<Uuid> = non_terminal.iter().map(|e| e.id).collect();
        assert!(ids.contains(&e1.id)); // pending
        assert!(ids.contains(&e2.id)); // running
    }
}
