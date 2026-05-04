//! Repository for `library_jobs` rows.
//!
//! Generic CRUD across job types. The `r#type` discriminator + `config` JSON
//! shape are validated at the service layer (`services::library_jobs`), not
//! here — this module only persists strings.

use anyhow::{Context, Result};
use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
};
use uuid::Uuid;

use crate::db::entities::{library_jobs, prelude::*};

/// Parameters for creating a new library job row.
#[derive(Debug, Clone)]
pub struct CreateLibraryJobParams {
    pub library_id: Uuid,
    /// Discriminator (e.g. `"metadata_refresh"`).
    pub job_type: String,
    pub name: String,
    pub enabled: bool,
    pub cron_schedule: String,
    pub timezone: Option<String>,
    /// Type-specific JSON config (already validated + serialized).
    pub config: String,
}

/// Outcome of a job run, used by [`LibraryJobRepository::record_run`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordRunStatus {
    Success,
    Failure,
}

impl RecordRunStatus {
    fn as_str(self) -> &'static str {
        match self {
            RecordRunStatus::Success => "success",
            RecordRunStatus::Failure => "failure",
        }
    }
}

/// Repository for [`library_jobs::Model`].
pub struct LibraryJobRepository;

impl LibraryJobRepository {
    /// Insert a new job row.
    pub async fn create(
        db: &DatabaseConnection,
        params: CreateLibraryJobParams,
    ) -> Result<library_jobs::Model> {
        let now = Utc::now();
        let row = library_jobs::ActiveModel {
            id: Set(Uuid::new_v4()),
            library_id: Set(params.library_id),
            r#type: Set(params.job_type),
            name: Set(params.name),
            enabled: Set(params.enabled),
            cron_schedule: Set(params.cron_schedule),
            timezone: Set(params.timezone),
            config: Set(params.config),
            last_run_at: Set(None),
            last_run_status: Set(None),
            last_run_message: Set(None),
            created_at: Set(now),
            updated_at: Set(now),
        };

        row.insert(db).await.context("Failed to create library job")
    }

    /// Look up a single job by primary key.
    pub async fn get_by_id(
        db: &DatabaseConnection,
        id: Uuid,
    ) -> Result<Option<library_jobs::Model>> {
        LibraryJobs::find_by_id(id)
            .one(db)
            .await
            .context("Failed to load library job by id")
    }

    /// List all jobs for a library, ordered by `created_at` ascending so the
    /// UI shows them in insertion order.
    pub async fn list_for_library(
        db: &DatabaseConnection,
        library_id: Uuid,
    ) -> Result<Vec<library_jobs::Model>> {
        LibraryJobs::find()
            .filter(library_jobs::Column::LibraryId.eq(library_id))
            .order_by_asc(library_jobs::Column::CreatedAt)
            .all(db)
            .await
            .context("Failed to list library jobs")
    }

    /// List all enabled jobs across every library, optionally filtered by type.
    /// Used by the scheduler at boot to register cron entries.
    pub async fn list_enabled(
        db: &DatabaseConnection,
        type_filter: Option<&str>,
    ) -> Result<Vec<library_jobs::Model>> {
        let mut query = LibraryJobs::find().filter(library_jobs::Column::Enabled.eq(true));
        if let Some(t) = type_filter {
            query = query.filter(library_jobs::Column::Type.eq(t));
        }
        query
            .order_by_asc(library_jobs::Column::CreatedAt)
            .all(db)
            .await
            .context("Failed to list enabled library jobs")
    }

    /// Update mutable fields on a job. The caller mutates the model first and
    /// then passes it back; we set `updated_at` here.
    pub async fn update(db: &DatabaseConnection, model: &library_jobs::Model) -> Result<()> {
        let active = library_jobs::ActiveModel {
            id: Set(model.id),
            library_id: Set(model.library_id),
            r#type: Set(model.r#type.clone()),
            name: Set(model.name.clone()),
            enabled: Set(model.enabled),
            cron_schedule: Set(model.cron_schedule.clone()),
            timezone: Set(model.timezone.clone()),
            config: Set(model.config.clone()),
            last_run_at: Set(model.last_run_at),
            last_run_status: Set(model.last_run_status.clone()),
            last_run_message: Set(model.last_run_message.clone()),
            created_at: Set(model.created_at),
            updated_at: Set(Utc::now()),
        };
        active
            .update(db)
            .await
            .context("Failed to update library job")?;
        Ok(())
    }

    /// Delete a job by id. No-op if the row doesn't exist.
    pub async fn delete(db: &DatabaseConnection, id: Uuid) -> Result<u64> {
        let res = LibraryJobs::delete_by_id(id)
            .exec(db)
            .await
            .context("Failed to delete library job")?;
        Ok(res.rows_affected)
    }

    /// Record the outcome of a run.
    pub async fn record_run(
        db: &DatabaseConnection,
        id: Uuid,
        status: RecordRunStatus,
        message: Option<String>,
    ) -> Result<()> {
        let model = Self::get_by_id(db, id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Library job not found: {}", id))?;
        let mut active: library_jobs::ActiveModel = model.into();
        active.last_run_at = Set(Some(Utc::now()));
        active.last_run_status = Set(Some(status.as_str().to_string()));
        active.last_run_message = Set(message);
        active.updated_at = Set(Utc::now());
        active
            .update(db)
            .await
            .context("Failed to record library job run")?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::ScanningStrategy;
    use crate::db::repositories::LibraryRepository;
    use crate::db::test_helpers::create_test_db;

    async fn seed_library(db: &DatabaseConnection, name: &str, path: &str) -> Uuid {
        LibraryRepository::create(db, name, path, ScanningStrategy::Default)
            .await
            .unwrap()
            .id
    }

    fn sample_params(library_id: Uuid, name: &str) -> CreateLibraryJobParams {
        CreateLibraryJobParams {
            library_id,
            job_type: "metadata_refresh".to_string(),
            name: name.to_string(),
            enabled: false,
            cron_schedule: "0 0 4 * * *".to_string(),
            timezone: None,
            config: r#"{"provider":"plugin:mangabaka"}"#.to_string(),
        }
    }

    #[tokio::test]
    async fn create_round_trips() {
        let (db, _tmp) = create_test_db().await;
        let lib = seed_library(db.sea_orm_connection(), "L", "/p").await;

        let row = LibraryJobRepository::create(db.sea_orm_connection(), sample_params(lib, "Test"))
            .await
            .unwrap();

        assert_eq!(row.library_id, lib);
        assert_eq!(row.r#type, "metadata_refresh");
        assert_eq!(row.name, "Test");
        assert!(!row.enabled);
        assert!(row.last_run_at.is_none());

        let loaded = LibraryJobRepository::get_by_id(db.sea_orm_connection(), row.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(loaded.id, row.id);
        assert_eq!(loaded.config, row.config);
    }

    #[tokio::test]
    async fn list_for_library_returns_only_that_library_in_order() {
        let (db, _tmp) = create_test_db().await;
        let lib_a = seed_library(db.sea_orm_connection(), "A", "/a").await;
        let lib_b = seed_library(db.sea_orm_connection(), "B", "/b").await;

        let _ja1 =
            LibraryJobRepository::create(db.sea_orm_connection(), sample_params(lib_a, "A1"))
                .await
                .unwrap();
        // Sleep micro-tick to keep created_at ordering deterministic on fast clocks.
        tokio::time::sleep(std::time::Duration::from_millis(2)).await;
        let _ja2 =
            LibraryJobRepository::create(db.sea_orm_connection(), sample_params(lib_a, "A2"))
                .await
                .unwrap();
        let _jb = LibraryJobRepository::create(db.sea_orm_connection(), sample_params(lib_b, "B"))
            .await
            .unwrap();

        let rows = LibraryJobRepository::list_for_library(db.sea_orm_connection(), lib_a)
            .await
            .unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].name, "A1");
        assert_eq!(rows[1].name, "A2");
    }

    #[tokio::test]
    async fn list_enabled_filters_by_enabled_and_type() {
        let (db, _tmp) = create_test_db().await;
        let lib = seed_library(db.sea_orm_connection(), "L", "/p").await;

        let mut p = sample_params(lib, "Disabled");
        p.enabled = false;
        let _ = LibraryJobRepository::create(db.sea_orm_connection(), p)
            .await
            .unwrap();

        let mut p2 = sample_params(lib, "Enabled");
        p2.enabled = true;
        let enabled = LibraryJobRepository::create(db.sea_orm_connection(), p2)
            .await
            .unwrap();

        let mut p3 = sample_params(lib, "OtherType");
        p3.enabled = true;
        p3.job_type = "scan".to_string();
        let _ = LibraryJobRepository::create(db.sea_orm_connection(), p3)
            .await
            .unwrap();

        let all = LibraryJobRepository::list_enabled(db.sea_orm_connection(), None)
            .await
            .unwrap();
        assert_eq!(all.len(), 2);

        let only_refresh =
            LibraryJobRepository::list_enabled(db.sea_orm_connection(), Some("metadata_refresh"))
                .await
                .unwrap();
        assert_eq!(only_refresh.len(), 1);
        assert_eq!(only_refresh[0].id, enabled.id);
    }

    #[tokio::test]
    async fn update_persists_changes() {
        let (db, _tmp) = create_test_db().await;
        let lib = seed_library(db.sea_orm_connection(), "L", "/p").await;

        let mut row =
            LibraryJobRepository::create(db.sea_orm_connection(), sample_params(lib, "Original"))
                .await
                .unwrap();
        row.name = "Updated".to_string();
        row.enabled = true;
        row.cron_schedule = "0 0 6 * * *".to_string();
        LibraryJobRepository::update(db.sea_orm_connection(), &row)
            .await
            .unwrap();

        let loaded = LibraryJobRepository::get_by_id(db.sea_orm_connection(), row.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(loaded.name, "Updated");
        assert!(loaded.enabled);
        assert_eq!(loaded.cron_schedule, "0 0 6 * * *");
    }

    #[tokio::test]
    async fn delete_removes_row() {
        let (db, _tmp) = create_test_db().await;
        let lib = seed_library(db.sea_orm_connection(), "L", "/p").await;
        let row = LibraryJobRepository::create(db.sea_orm_connection(), sample_params(lib, "X"))
            .await
            .unwrap();
        let n = LibraryJobRepository::delete(db.sea_orm_connection(), row.id)
            .await
            .unwrap();
        assert_eq!(n, 1);
        assert!(
            LibraryJobRepository::get_by_id(db.sea_orm_connection(), row.id)
                .await
                .unwrap()
                .is_none()
        );
    }

    #[tokio::test]
    async fn record_run_updates_last_run_fields() {
        let (db, _tmp) = create_test_db().await;
        let lib = seed_library(db.sea_orm_connection(), "L", "/p").await;
        let row = LibraryJobRepository::create(db.sea_orm_connection(), sample_params(lib, "X"))
            .await
            .unwrap();

        LibraryJobRepository::record_run(
            db.sea_orm_connection(),
            row.id,
            RecordRunStatus::Success,
            Some("done".to_string()),
        )
        .await
        .unwrap();
        let loaded = LibraryJobRepository::get_by_id(db.sea_orm_connection(), row.id)
            .await
            .unwrap()
            .unwrap();
        assert!(loaded.last_run_at.is_some());
        assert_eq!(loaded.last_run_status.as_deref(), Some("success"));
        assert_eq!(loaded.last_run_message.as_deref(), Some("done"));

        LibraryJobRepository::record_run(
            db.sea_orm_connection(),
            row.id,
            RecordRunStatus::Failure,
            None,
        )
        .await
        .unwrap();
        let loaded = LibraryJobRepository::get_by_id(db.sea_orm_connection(), row.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(loaded.last_run_status.as_deref(), Some("failure"));
        assert!(loaded.last_run_message.is_none());
    }

    #[tokio::test]
    async fn cascade_delete_removes_jobs_when_library_deleted() {
        use crate::db::entities::libraries::Entity as Libs;

        let (db, _tmp) = create_test_db().await;
        let lib = seed_library(db.sea_orm_connection(), "L", "/p").await;
        let row = LibraryJobRepository::create(db.sea_orm_connection(), sample_params(lib, "X"))
            .await
            .unwrap();

        // Drop the library row directly to exercise the FK.
        Libs::delete_by_id(lib)
            .exec(db.sea_orm_connection())
            .await
            .unwrap();

        let loaded = LibraryJobRepository::get_by_id(db.sea_orm_connection(), row.id)
            .await
            .unwrap();
        assert!(loaded.is_none(), "cascade should have removed the job");
    }
}
