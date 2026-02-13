use anyhow::{Context, Result};
use chrono::{DateTime, Duration, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, DbBackend, EntityTrait,
    QueryFilter, QueryOrder, QuerySelect, Set, Statement, TransactionTrait, entity::prelude::*,
};
use tracing::{info, warn};
use uuid::Uuid;

use crate::db::entities::{prelude::*, tasks};
use crate::tasks::error::DEFAULT_MAX_RESCHEDULES;
use crate::tasks::types::{TaskStats, TaskType};

/// Repository for Task operations
pub struct TaskRepository;

/// Returns the ORDER BY clause for task priority ordering.
/// Used by both PostgreSQL and SQLite query builders.
fn task_priority_order_by(prioritize_scans: bool) -> &'static str {
    if prioritize_scans {
        // Task priority order (highest to lowest):
        // 0. scan_library
        // 1. purge_deleted
        // 2. generate_thumbnail
        // 3. generate_series_thumbnail
        // 4. analyze_book
        // 5. analyze_series
        // 6. generate_thumbnails (batch)
        // 7. find_duplicates
        // 8. refresh_metadata
        // 9-12. cleanup tasks (lowest priority - run after core operations)
        "ORDER BY (CASE
            WHEN task_type = 'scan_library' THEN 0
            WHEN task_type = 'purge_deleted' THEN 1
            WHEN task_type = 'generate_thumbnail' THEN 2
            WHEN task_type = 'analyze_book' THEN 3
            WHEN task_type = 'analyze_series' THEN 4
            WHEN task_type = 'generate_series_thumbnail' THEN 5
            WHEN task_type = 'generate_thumbnails' THEN 6
            WHEN task_type = 'find_duplicates' THEN 7
            WHEN task_type = 'refresh_metadata' THEN 8
            WHEN task_type = 'cleanup_book_files' THEN 9
            WHEN task_type = 'cleanup_series_files' THEN 10
            WHEN task_type = 'cleanup_orphaned_files' THEN 11
            WHEN task_type = 'cleanup_pdf_cache' THEN 12
            ELSE 99
        END), priority DESC, scheduled_for ASC"
    } else {
        // Standard priority-based ordering
        "ORDER BY priority DESC, scheduled_for ASC"
    }
}

impl TaskRepository {
    /// Enqueue a new task
    /// If a task with the same entity and type is already pending/processing, returns the existing task's ID
    pub async fn enqueue(
        db: &DatabaseConnection,
        task_type: TaskType,
        priority: i32,
        scheduled_for: Option<DateTime<Utc>>,
    ) -> Result<Uuid> {
        let type_str = task_type.type_string();
        let library_id = task_type.library_id();
        let series_id = task_type.series_id();
        let book_id = task_type.book_id();
        let params = task_type.params();

        // Check if a task already exists for this entity
        if let Some(existing_task) =
            Self::find_existing_task(db, type_str, library_id, series_id, book_id).await?
        {
            info!(
                "Task already exists: {} ({}) - skipping duplicate",
                existing_task.id, type_str
            );
            return Ok(existing_task.id);
        }

        let task_id = Uuid::new_v4();
        let now = Utc::now();

        let task = tasks::ActiveModel {
            id: Set(task_id),
            task_type: Set(type_str.to_string()),
            library_id: Set(library_id),
            series_id: Set(series_id),
            book_id: Set(book_id),
            params: Set(Some(params)),
            status: Set("pending".to_string()),
            priority: Set(priority),
            locked_by: Set(None),
            locked_until: Set(None),
            attempts: Set(0),
            max_attempts: Set(3),
            last_error: Set(None),
            reschedule_count: Set(0),
            max_reschedules: Set(DEFAULT_MAX_RESCHEDULES),
            result: Set(None),
            scheduled_for: Set(scheduled_for.unwrap_or(now)),
            created_at: Set(now),
            started_at: Set(None),
            completed_at: Set(None),
        };

        // Try to insert, but handle unique constraint violations gracefully
        match task.insert(db).await {
            Ok(_) => {
                info!("Enqueued task {} ({})", task_id, type_str);
                Ok(task_id)
            }
            Err(e) => {
                // Check if this is a unique constraint violation
                let err_str = e.to_string().to_lowercase();
                if err_str.contains("unique") || err_str.contains("duplicate") {
                    // Race condition: another task was inserted between our check and insert
                    // Find and return the existing task
                    if let Some(existing_task) =
                        Self::find_existing_task(db, type_str, library_id, series_id, book_id)
                            .await?
                    {
                        info!(
                            "Task was created concurrently: {} ({}) - using existing task",
                            existing_task.id, type_str
                        );
                        Ok(existing_task.id)
                    } else {
                        anyhow::bail!(
                            "Unique constraint violation but could not find existing task"
                        )
                    }
                } else {
                    Err(e).context("Failed to enqueue task")
                }
            }
        }
    }

    /// Enqueue multiple tasks in a batch operation
    ///
    /// This is significantly more efficient than calling `enqueue()` for each task
    /// individually. Skips tasks that already exist (based on type and entity).
    ///
    /// # Arguments
    /// * `db` - Database connection
    /// * `task_types` - List of task types to enqueue
    /// * `priority` - Priority for all tasks (higher = more urgent)
    /// * `scheduled_for` - Optional scheduled time for all tasks
    ///
    /// # Returns
    /// Number of tasks actually enqueued (excluding duplicates)
    pub async fn enqueue_batch(
        db: &DatabaseConnection,
        task_types: Vec<TaskType>,
        priority: i32,
        scheduled_for: Option<DateTime<Utc>>,
    ) -> Result<u64> {
        if task_types.is_empty() {
            return Ok(0);
        }

        let now = Utc::now();
        let scheduled = scheduled_for.unwrap_or(now);
        let mut enqueued = 0u64;

        // Build list of tasks to insert, filtering out existing ones
        let mut tasks_to_insert: Vec<tasks::ActiveModel> = Vec::with_capacity(task_types.len());

        // Get all book IDs from the task types for batch existence check
        let book_ids: Vec<Uuid> = task_types.iter().filter_map(|t| t.book_id()).collect();

        // Batch check for existing tasks with these book IDs
        let existing_book_ids: std::collections::HashSet<Uuid> = if !book_ids.is_empty() {
            // Get pending/processing tasks for these book IDs
            let existing_tasks = Tasks::find()
                .filter(tasks::Column::BookId.is_in(book_ids.clone()))
                .filter(tasks::Column::Status.is_in(["pending", "processing"]))
                .select_only()
                .column(tasks::Column::BookId)
                .into_tuple::<Option<Uuid>>()
                .all(db)
                .await
                .context("Failed to check existing tasks")?;

            existing_tasks.into_iter().flatten().collect()
        } else {
            std::collections::HashSet::new()
        };

        for task_type in task_types {
            let type_str = task_type.type_string();
            let library_id = task_type.library_id();
            let series_id = task_type.series_id();
            let book_id = task_type.book_id();
            let params = task_type.params();

            // Skip if task already exists for this book
            if let Some(bk_id) = book_id
                && existing_book_ids.contains(&bk_id)
            {
                continue;
            }

            let task_id = Uuid::new_v4();

            let task = tasks::ActiveModel {
                id: Set(task_id),
                task_type: Set(type_str.to_string()),
                library_id: Set(library_id),
                series_id: Set(series_id),
                book_id: Set(book_id),
                params: Set(Some(params)),
                status: Set("pending".to_string()),
                priority: Set(priority),
                locked_by: Set(None),
                locked_until: Set(None),
                attempts: Set(0),
                max_attempts: Set(3),
                last_error: Set(None),
                reschedule_count: Set(0),
                max_reschedules: Set(DEFAULT_MAX_RESCHEDULES),
                result: Set(None),
                scheduled_for: Set(scheduled),
                created_at: Set(now),
                started_at: Set(None),
                completed_at: Set(None),
            };

            tasks_to_insert.push(task);
            enqueued += 1;
        }

        if !tasks_to_insert.is_empty() {
            // Bulk insert all tasks - use on_conflict to ignore duplicates
            Tasks::insert_many(tasks_to_insert)
                .on_conflict(
                    sea_orm::sea_query::OnConflict::new()
                        .do_nothing()
                        .to_owned(),
                )
                .exec(db)
                .await
                .context("Failed to batch enqueue tasks")?;

            info!("Batch enqueued {} tasks", enqueued);
        }

        Ok(enqueued)
    }

    /// Find an existing pending/processing task for the given entity
    async fn find_existing_task(
        db: &DatabaseConnection,
        task_type: &str,
        library_id: Option<Uuid>,
        series_id: Option<Uuid>,
        book_id: Option<Uuid>,
    ) -> Result<Option<tasks::Model>> {
        let mut query = Tasks::find()
            .filter(tasks::Column::TaskType.eq(task_type))
            .filter(tasks::Column::Status.is_in(["pending", "processing"]));

        // Match on the most specific entity identifier
        if let Some(bk_id) = book_id {
            query = query.filter(tasks::Column::BookId.eq(bk_id));
        } else if let Some(ser_id) = series_id {
            query = query.filter(tasks::Column::SeriesId.eq(ser_id));
        } else if let Some(lib_id) = library_id {
            query = query.filter(tasks::Column::LibraryId.eq(lib_id));
        }

        query.one(db).await.context("Failed to find existing task")
    }

    /// Check if a pending or processing task exists with matching params.
    ///
    /// Used for task types that store their identity in JSON params rather than
    /// FK columns (e.g., UserPluginSync, UserPluginRecommendations). Uses
    /// database-level JSON filtering to avoid loading all tasks into memory.
    pub async fn has_pending_or_processing(
        db: &DatabaseConnection,
        task_type: &str,
        plugin_id: Uuid,
        user_id: Uuid,
    ) -> Result<bool> {
        let plugin_id_str = plugin_id.to_string();
        let user_id_str = user_id.to_string();
        let backend = db.get_database_backend();

        let stmt = match backend {
            DbBackend::Postgres => Statement::from_sql_and_values(
                DbBackend::Postgres,
                r#"SELECT 1 FROM tasks
                   WHERE task_type = $1
                     AND status IN ('pending', 'processing')
                     AND params->>'plugin_id' = $2
                     AND params->>'user_id' = $3
                   LIMIT 1"#,
                vec![task_type.into(), plugin_id_str.into(), user_id_str.into()],
            ),
            _ => Statement::from_sql_and_values(
                DbBackend::Sqlite,
                r#"SELECT 1 FROM tasks
                   WHERE task_type = ?
                     AND status IN ('pending', 'processing')
                     AND json_extract(params, '$.plugin_id') = ?
                     AND json_extract(params, '$.user_id') = ?
                   LIMIT 1"#,
                vec![task_type.into(), plugin_id_str.into(), user_id_str.into()],
            ),
        };

        let result = db
            .query_one(stmt)
            .await
            .context("Failed to check for existing tasks")?;

        Ok(result.is_some())
    }

    /// Find a pending or processing task with matching params, returning its ID and status.
    ///
    /// Like `has_pending_or_processing` but returns the task ID and status string
    /// so callers can expose task progress to the frontend.
    pub async fn find_pending_or_processing_task(
        db: &DatabaseConnection,
        task_type: &str,
        plugin_id: Uuid,
        user_id: Uuid,
    ) -> Result<Option<(Uuid, String)>> {
        let plugin_id_str = plugin_id.to_string();
        let user_id_str = user_id.to_string();
        let backend = db.get_database_backend();

        let stmt = match backend {
            DbBackend::Postgres => Statement::from_sql_and_values(
                DbBackend::Postgres,
                r#"SELECT id, status FROM tasks
                   WHERE task_type = $1
                     AND status IN ('pending', 'processing')
                     AND params->>'plugin_id' = $2
                     AND params->>'user_id' = $3
                   LIMIT 1"#,
                vec![task_type.into(), plugin_id_str.into(), user_id_str.into()],
            ),
            _ => Statement::from_sql_and_values(
                DbBackend::Sqlite,
                r#"SELECT id, status FROM tasks
                   WHERE task_type = ?
                     AND status IN ('pending', 'processing')
                     AND json_extract(params, '$.plugin_id') = ?
                     AND json_extract(params, '$.user_id') = ?
                   LIMIT 1"#,
                vec![task_type.into(), plugin_id_str.into(), user_id_str.into()],
            ),
        };

        let result = db
            .query_one(stmt)
            .await
            .context("Failed to find pending/processing task")?;

        match result {
            Some(row) => {
                // PostgreSQL returns native UUID; SQLite returns TEXT
                let task_id: Uuid = row.try_get::<Uuid>("", "id").or_else(|_| {
                    let id_str: String = row.try_get("", "id")?;
                    Uuid::parse_str(&id_str).map_err(|e| sea_orm::DbErr::Type(e.to_string()))
                })?;
                let status: String = row.try_get("", "status")?;
                Ok(Some((task_id, status)))
            }
            None => Ok(None),
        }
    }

    /// Find the most recent task for a user+plugin combination, optionally filtered by task type.
    ///
    /// Returns the full task model so callers can build response DTOs.
    /// Used for user-scoped plugin task endpoints that don't require TasksRead permission.
    pub async fn find_latest_user_plugin_task(
        db: &DatabaseConnection,
        plugin_id: Uuid,
        user_id: Uuid,
        task_type_filter: Option<&str>,
    ) -> Result<Option<tasks::Model>> {
        let plugin_id_str = plugin_id.to_string();
        let user_id_str = user_id.to_string();
        let backend = db.get_database_backend();

        let stmt = match (backend, task_type_filter) {
            (DbBackend::Postgres, Some(task_type)) => Statement::from_sql_and_values(
                DbBackend::Postgres,
                r#"SELECT * FROM tasks
                   WHERE task_type = $1
                     AND params->>'plugin_id' = $2
                     AND params->>'user_id' = $3
                   ORDER BY created_at DESC
                   LIMIT 1"#,
                vec![task_type.into(), plugin_id_str.into(), user_id_str.into()],
            ),
            (DbBackend::Postgres, None) => Statement::from_sql_and_values(
                DbBackend::Postgres,
                r#"SELECT * FROM tasks
                   WHERE params->>'plugin_id' = $1
                     AND params->>'user_id' = $2
                   ORDER BY created_at DESC
                   LIMIT 1"#,
                vec![plugin_id_str.into(), user_id_str.into()],
            ),
            (_, Some(task_type)) => Statement::from_sql_and_values(
                DbBackend::Sqlite,
                r#"SELECT * FROM tasks
                   WHERE task_type = ?
                     AND json_extract(params, '$.plugin_id') = ?
                     AND json_extract(params, '$.user_id') = ?
                   ORDER BY created_at DESC
                   LIMIT 1"#,
                vec![task_type.into(), plugin_id_str.into(), user_id_str.into()],
            ),
            (_, None) => Statement::from_sql_and_values(
                DbBackend::Sqlite,
                r#"SELECT * FROM tasks
                   WHERE json_extract(params, '$.plugin_id') = ?
                     AND json_extract(params, '$.user_id') = ?
                   ORDER BY created_at DESC
                   LIMIT 1"#,
                vec![plugin_id_str.into(), user_id_str.into()],
            ),
        };

        let result = tasks::Entity::find()
            .from_raw_sql(stmt)
            .one(db)
            .await
            .context("Failed to find latest user plugin task")?;

        Ok(result)
    }

    /// Claim next available task (atomic operation using SKIP LOCKED for Postgres, transaction for SQLite)
    ///
    /// # Arguments
    /// * `db` - Database connection
    /// * `worker_id` - ID of the worker claiming the task
    /// * `lock_duration_secs` - Duration in seconds to lock the task
    /// * `prioritize_scans` - If true, scan_library tasks are prioritized over analysis tasks
    pub async fn claim_next(
        db: &DatabaseConnection,
        worker_id: &str,
        lock_duration_secs: i64,
        prioritize_scans: bool,
    ) -> Result<Option<tasks::Model>> {
        let worker_id = worker_id.to_string(); // Clone for move into closure
        let is_postgres = db.get_database_backend() == DbBackend::Postgres;

        let result = db
            .transaction::<_, Option<tasks::Model>, DbErr>(move |txn| {
                let worker_id = worker_id.clone(); // Clone again for the async move block
                Box::pin(async move {
                    let now = Utc::now();
                    let lock_expires = now + Duration::seconds(lock_duration_secs);

                    let task_option = if is_postgres {
                        // PostgreSQL: Use FOR UPDATE SKIP LOCKED for multi-worker safety
                        let order_by = task_priority_order_by(prioritize_scans);

                        let sql = format!(
                            r#"
                            SELECT * FROM tasks
                            WHERE (
                                status = 'pending'
                                OR (status = 'processing' AND locked_until < $1)
                            )
                            AND scheduled_for <= $1
                            AND attempts < max_attempts
                            {}
                            LIMIT 1
                            FOR UPDATE SKIP LOCKED
                            "#,
                            order_by
                        );

                        let stmt = Statement::from_sql_and_values(
                            DbBackend::Postgres,
                            sql,
                            vec![now.into()],
                        );
                        let query_result = txn.query_one(stmt).await?;

                        query_result.and_then(|row| {
                            Some(tasks::Model {
                                id: row.try_get("", "id").ok()?,
                                task_type: row.try_get("", "task_type").ok()?,
                                library_id: row.try_get("", "library_id").ok()?,
                                series_id: row.try_get("", "series_id").ok()?,
                                book_id: row.try_get("", "book_id").ok()?,
                                params: row.try_get("", "params").ok()?,
                                status: row.try_get("", "status").ok()?,
                                priority: row.try_get("", "priority").ok()?,
                                locked_by: row.try_get("", "locked_by").ok()?,
                                locked_until: row.try_get("", "locked_until").ok()?,
                                attempts: row.try_get("", "attempts").ok()?,
                                max_attempts: row.try_get("", "max_attempts").ok()?,
                                last_error: row.try_get("", "last_error").ok()?,
                                reschedule_count: row.try_get("", "reschedule_count").ok()?,
                                max_reschedules: row.try_get("", "max_reschedules").ok()?,
                                result: row.try_get("", "result").ok()?,
                                scheduled_for: row.try_get("", "scheduled_for").ok()?,
                                created_at: row.try_get("", "created_at").ok()?,
                                started_at: row.try_get("", "started_at").ok()?,
                                completed_at: row.try_get("", "completed_at").ok()?,
                            })
                        })
                    } else {
                        // SQLite: Use raw SQL query (similar to PostgreSQL but without SKIP LOCKED)
                        // SQLite serializes transactions, so we don't need SKIP LOCKED
                        let order_by = task_priority_order_by(prioritize_scans);

                        let sql = format!(
                            r#"
                            SELECT * FROM tasks
                            WHERE (
                                status = 'pending'
                                OR (status = 'processing' AND locked_until < ?)
                            )
                            AND scheduled_for <= ?
                            AND attempts < max_attempts
                            {}
                            LIMIT 1
                            "#,
                            order_by
                        );

                        let stmt = Statement::from_sql_and_values(
                            DbBackend::Sqlite,
                            &sql,
                            vec![now.into(), now.into()],
                        );
                        let query_result = txn.query_one(stmt).await?;

                        query_result.and_then(|row| {
                            Some(tasks::Model {
                                id: row.try_get("", "id").ok()?,
                                task_type: row.try_get("", "task_type").ok()?,
                                library_id: row.try_get("", "library_id").ok()?,
                                series_id: row.try_get("", "series_id").ok()?,
                                book_id: row.try_get("", "book_id").ok()?,
                                params: row.try_get("", "params").ok()?,
                                status: row.try_get("", "status").ok()?,
                                priority: row.try_get("", "priority").ok()?,
                                locked_by: row.try_get("", "locked_by").ok()?,
                                locked_until: row.try_get("", "locked_until").ok()?,
                                attempts: row.try_get("", "attempts").ok()?,
                                max_attempts: row.try_get("", "max_attempts").ok()?,
                                last_error: row.try_get("", "last_error").ok()?,
                                reschedule_count: row.try_get("", "reschedule_count").ok()?,
                                max_reschedules: row.try_get("", "max_reschedules").ok()?,
                                result: row.try_get("", "result").ok()?,
                                scheduled_for: row.try_get("", "scheduled_for").ok()?,
                                created_at: row.try_get("", "created_at").ok()?,
                                started_at: row.try_get("", "started_at").ok()?,
                                completed_at: row.try_get("", "completed_at").ok()?,
                            })
                        })
                    };

                    if let Some(task) = task_option {
                        // Claim it
                        let mut active: tasks::ActiveModel = task.clone().into();
                        active.status = Set("processing".to_string());
                        active.locked_by = Set(Some(worker_id));
                        active.locked_until = Set(Some(lock_expires));
                        active.started_at = Set(Some(now));
                        active.attempts = Set(task.attempts + 1);

                        let updated = active.update(txn).await?;
                        Ok(Some(updated))
                    } else {
                        Ok(None)
                    }
                })
            })
            .await
            .context("Failed to claim task")?;

        Ok(result)
    }

    /// Mark task as completed
    pub async fn mark_completed(
        db: &DatabaseConnection,
        task_id: Uuid,
        result: Option<serde_json::Value>,
    ) -> Result<()> {
        let mut task: tasks::ActiveModel = Tasks::find_by_id(task_id)
            .one(db)
            .await
            .context("Failed to find task")?
            .ok_or_else(|| anyhow::anyhow!("Task not found"))?
            .into();

        task.status = Set("completed".to_string());
        task.completed_at = Set(Some(Utc::now()));
        task.result = Set(result);
        task.locked_by = Set(None);
        task.locked_until = Set(None);

        task.update(db)
            .await
            .context("Failed to mark task as completed")?;

        Ok(())
    }

    /// Mark task as failed (will retry if attempts < max_attempts)
    pub async fn mark_failed(db: &DatabaseConnection, task_id: Uuid, error: String) -> Result<()> {
        let task = Tasks::find_by_id(task_id)
            .one(db)
            .await
            .context("Failed to find task")?
            .ok_or_else(|| anyhow::anyhow!("Task not found"))?;

        let mut active: tasks::ActiveModel = task.clone().into();

        // Check if we should retry
        if task.attempts < task.max_attempts {
            // Retry - back to pending with exponential backoff
            let backoff_secs = 2_i64.pow(task.attempts as u32) * 60; // 1min, 2min, 4min
            active.status = Set("pending".to_string());
            active.scheduled_for = Set(Utc::now() + Duration::seconds(backoff_secs));
            active.locked_by = Set(None);
            active.locked_until = Set(None);
            info!(
                "Task {} will retry in {} seconds (attempt {}/{})",
                task_id, backoff_secs, task.attempts, task.max_attempts
            );
        } else {
            // Max attempts reached
            active.status = Set("failed".to_string());
            active.completed_at = Set(Some(Utc::now()));
            warn!(
                "Task {} failed permanently after {} attempts",
                task_id, task.attempts
            );
        }

        active.last_error = Set(Some(error));
        active
            .update(db)
            .await
            .context("Failed to mark task as failed")?;

        Ok(())
    }

    /// Mark task as rate-limited (will reschedule with short delay)
    ///
    /// Unlike `mark_failed`, this does NOT consume retry attempts.
    /// Instead, it increments `reschedule_count`. If `reschedule_count`
    /// exceeds `max_reschedules`, the task is marked as failed.
    ///
    /// # Arguments
    /// * `db` - Database connection
    /// * `task_id` - Task ID
    /// * `retry_after_secs` - Delay before rescheduling (default: 30 seconds)
    pub async fn mark_rate_limited(
        db: &DatabaseConnection,
        task_id: Uuid,
        retry_after_secs: u64,
    ) -> Result<()> {
        let task = Tasks::find_by_id(task_id)
            .one(db)
            .await
            .context("Failed to find task")?
            .ok_or_else(|| anyhow::anyhow!("Task not found"))?;

        let mut active: tasks::ActiveModel = task.clone().into();
        let new_reschedule_count = task.reschedule_count + 1;

        // Check if we should reschedule or fail
        if new_reschedule_count <= task.max_reschedules {
            // Reschedule with the specified delay
            active.status = Set("pending".to_string());
            active.scheduled_for = Set(Utc::now() + Duration::seconds(retry_after_secs as i64));
            active.locked_by = Set(None);
            active.locked_until = Set(None);
            active.reschedule_count = Set(new_reschedule_count);
            // Decrement attempts since mark_rate_limited is called after claim_next incremented it
            // and rate-limiting shouldn't consume retry attempts
            active.attempts = Set(task.attempts - 1);

            info!(
                "Task {} rate-limited, rescheduled in {} seconds (reschedule {}/{})",
                task_id, retry_after_secs, new_reschedule_count, task.max_reschedules
            );
        } else {
            // Max reschedules reached - fail the task
            active.status = Set("failed".to_string());
            active.completed_at = Set(Some(Utc::now()));
            active.locked_by = Set(None);
            active.locked_until = Set(None);
            active.last_error = Set(Some(format!(
                "Exceeded max reschedules ({}) due to rate limiting",
                task.max_reschedules
            )));

            warn!(
                "Task {} failed permanently after {} rate-limit reschedules",
                task_id, task.max_reschedules
            );
        }

        active
            .update(db)
            .await
            .context("Failed to mark task as rate-limited")?;

        Ok(())
    }

    /// List tasks with optional filters
    pub async fn list(
        db: &DatabaseConnection,
        status: Option<String>,
        task_type: Option<String>,
        limit: Option<u64>,
    ) -> Result<Vec<tasks::Model>> {
        let mut query = Tasks::find();

        if let Some(s) = status {
            query = query.filter(tasks::Column::Status.eq(s));
        }

        if let Some(t) = task_type {
            query = query.filter(tasks::Column::TaskType.eq(t));
        }

        if let Some(l) = limit {
            query = query.limit(l);
        }

        query
            .order_by_desc(tasks::Column::CreatedAt)
            .all(db)
            .await
            .context("Failed to list tasks")
    }

    /// Get task by ID
    pub async fn get_by_id(db: &DatabaseConnection, task_id: Uuid) -> Result<Option<tasks::Model>> {
        Tasks::find_by_id(task_id)
            .one(db)
            .await
            .context("Failed to get task by ID")
    }

    /// Cancel a pending or processing task
    pub async fn cancel(db: &DatabaseConnection, task_id: Uuid) -> Result<()> {
        let task = Tasks::find_by_id(task_id)
            .one(db)
            .await
            .context("Failed to find task")?
            .ok_or_else(|| anyhow::anyhow!("Task not found"))?;

        // Only cancel if not completed
        if task.status == "completed" || task.status == "failed" {
            anyhow::bail!("Cannot cancel {} task", task.status);
        }

        let mut active: tasks::ActiveModel = task.into();
        active.status = Set("cancelled".to_string());
        active.completed_at = Set(Some(Utc::now()));
        active.locked_by = Set(None);
        active.locked_until = Set(None);

        active.update(db).await.context("Failed to cancel task")?;

        Ok(())
    }

    /// Unlock a stuck task (force release lock)
    pub async fn unlock(db: &DatabaseConnection, task_id: Uuid) -> Result<()> {
        let mut task: tasks::ActiveModel = Tasks::find_by_id(task_id)
            .one(db)
            .await
            .context("Failed to find task")?
            .ok_or_else(|| anyhow::anyhow!("Task not found"))?
            .into();

        task.status = Set("pending".to_string());
        task.locked_by = Set(None);
        task.locked_until = Set(None);
        task.attempts = Set(0); // Reset attempts
        task.last_error = Set(None);

        task.update(db).await.context("Failed to unlock task")?;

        info!("Unlocked task {}", task_id);
        Ok(())
    }

    /// Purge completed/failed tasks older than N days
    pub async fn purge_old_tasks(db: &DatabaseConnection, days: i64) -> Result<u64> {
        let cutoff = Utc::now() - Duration::days(days);

        let result = Tasks::delete_many()
            .filter(tasks::Column::Status.is_in(["completed", "failed", "cancelled"]))
            .filter(tasks::Column::CompletedAt.lt(cutoff))
            .exec(db)
            .await
            .context("Failed to purge old tasks")?;

        Ok(result.rows_affected)
    }

    /// Purge recently completed tasks (older than N seconds)
    /// This is meant for automatic cleanup of finished tasks
    pub async fn purge_completed_tasks(db: &DatabaseConnection, seconds: i64) -> Result<u64> {
        let cutoff = Utc::now() - Duration::seconds(seconds);

        let result = Tasks::delete_many()
            .filter(tasks::Column::Status.is_in(["completed", "failed", "cancelled"]))
            .filter(tasks::Column::CompletedAt.lt(cutoff))
            .exec(db)
            .await
            .context("Failed to purge completed tasks")?;

        if result.rows_affected > 0 {
            info!(
                "Purged {} completed tasks older than {} seconds",
                result.rows_affected, seconds
            );
        }

        Ok(result.rows_affected)
    }

    /// Nuclear option: Empty the entire tasks table
    pub async fn nuke_all_tasks(db: &DatabaseConnection) -> Result<u64> {
        warn!("Nuking all tasks from the queue!");

        let result = Tasks::delete_many()
            .exec(db)
            .await
            .context("Failed to nuke all tasks")?;

        info!("Deleted {} tasks", result.rows_affected);
        Ok(result.rows_affected)
    }

    /// Get queue statistics
    pub async fn get_stats(db: &DatabaseConnection) -> Result<TaskStats> {
        use crate::tasks::types::TaskTypeStats;
        use std::collections::HashMap;

        // Get all tasks to calculate both aggregate and per-type stats
        let all_tasks = Tasks::find()
            .all(db)
            .await
            .context("Failed to fetch tasks")?;

        // Initialize aggregates
        let mut pending = 0u64;
        let mut processing = 0u64;
        let mut completed = 0u64;
        let mut failed = 0u64;
        let mut stale = 0u64;

        // Initialize per-type breakdown
        let mut by_type: HashMap<String, TaskTypeStats> = HashMap::new();

        // Find stale locks (tasks locked for > 10 minutes)
        let stale_cutoff = Utc::now() - Duration::minutes(10);

        for task in all_tasks {
            let is_stale = task.status == "processing"
                && task.locked_until.is_some_and(|until| until < stale_cutoff);

            // Update aggregates
            match task.status.as_str() {
                "pending" => pending += 1,
                "processing" => {
                    processing += 1;
                    if is_stale {
                        stale += 1;
                    }
                }
                "completed" => completed += 1,
                "failed" => failed += 1,
                _ => {}
            }

            // Update per-type stats
            let type_stats = by_type
                .entry(task.task_type.clone())
                .or_insert(TaskTypeStats {
                    pending: 0,
                    processing: 0,
                    completed: 0,
                    failed: 0,
                    stale: 0,
                    total: 0,
                });

            match task.status.as_str() {
                "pending" => type_stats.pending += 1,
                "processing" => {
                    type_stats.processing += 1;
                    if is_stale {
                        type_stats.stale += 1;
                    }
                }
                "completed" => type_stats.completed += 1,
                "failed" => type_stats.failed += 1,
                _ => {}
            }
            type_stats.total += 1;
        }

        Ok(TaskStats {
            pending,
            processing,
            completed,
            failed,
            stale,
            total: pending + processing + completed + failed,
            by_type,
        })
    }

    /// Recover stale tasks that have been locked longer than the threshold
    /// This handles crashed workers that never released their locks
    ///
    /// # Arguments
    /// * `db` - Database connection
    /// * `stale_threshold_secs` - Time in seconds after which a locked task is considered stale
    ///
    /// # Returns
    /// Number of tasks recovered
    pub async fn recover_stale_tasks(
        db: &DatabaseConnection,
        stale_threshold_secs: i64,
    ) -> Result<u64> {
        let stale_cutoff = Utc::now() - Duration::seconds(stale_threshold_secs);

        // Find tasks that are "processing" but locked too long ago
        let stale_tasks = Tasks::find()
            .filter(tasks::Column::Status.eq("processing"))
            .filter(tasks::Column::LockedUntil.lt(stale_cutoff))
            .all(db)
            .await
            .context("Failed to find stale tasks")?;

        let mut recovered = 0u64;

        for task in stale_tasks {
            // Only recover if not at max attempts
            if task.attempts < task.max_attempts {
                let mut active: tasks::ActiveModel = task.clone().into();
                active.status = Set("pending".to_string());
                active.locked_by = Set(None);
                active.locked_until = Set(None);
                // Don't increment attempts - worker crash wasn't task's fault

                active
                    .update(db)
                    .await
                    .context("Failed to recover stale task")?;
                recovered += 1;

                info!(
                    "Recovered stale task {} (type: {}, was locked by {:?})",
                    task.id, task.task_type, task.locked_by
                );
            } else {
                // Mark as failed if max attempts reached
                let mut active: tasks::ActiveModel = task.clone().into();
                active.status = Set("failed".to_string());
                active.last_error = Set(Some("Task stale after max attempts".to_string()));
                active.completed_at = Set(Some(Utc::now()));
                active.locked_by = Set(None);
                active.locked_until = Set(None);

                active
                    .update(db)
                    .await
                    .context("Failed to mark stale task as failed")?;
                recovered += 1;

                warn!(
                    "Marked stale task {} as failed after {} attempts",
                    task.id, task.attempts
                );
            }
        }

        if recovered > 0 {
            info!("Recovered {} stale tasks", recovered);
        }

        Ok(recovered)
    }
}
