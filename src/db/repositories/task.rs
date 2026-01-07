use anyhow::{Context, Result};
use chrono::{DateTime, Duration, Utc};
use sea_orm::{
    entity::prelude::*, sea_query::Expr, ActiveModelTrait, ColumnTrait, Condition,
    DatabaseConnection, DbBackend, EntityTrait, Order, QueryFilter, QueryOrder, QuerySelect, Set,
    Statement, TransactionTrait,
};
use tracing::{info, warn};
use uuid::Uuid;

use crate::db::entities::{prelude::*, tasks};
use crate::tasks::types::{TaskStats, TaskType};

/// Repository for Task operations
pub struct TaskRepository;

impl TaskRepository {
    /// Enqueue a new task
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
            result: Set(None),
            scheduled_for: Set(scheduled_for.unwrap_or(now)),
            created_at: Set(now),
            started_at: Set(None),
            completed_at: Set(None),
        };

        task.insert(db).await.context("Failed to enqueue task")?;

        info!("Enqueued task {} ({})", task_id, type_str);

        Ok(task_id)
    }

    /// Claim next available task (atomic operation using SKIP LOCKED for Postgres, transaction for SQLite)
    pub async fn claim_next(
        db: &DatabaseConnection,
        worker_id: &str,
        lock_duration_secs: i64,
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
                        let sql = r#"
                            SELECT * FROM tasks
                            WHERE (
                                status = 'pending'
                                OR (status = 'processing' AND locked_until < $1)
                            )
                            AND scheduled_for <= $1
                            AND attempts < max_attempts
                            ORDER BY priority DESC, scheduled_for ASC
                            LIMIT 1
                            FOR UPDATE SKIP LOCKED
                        "#;

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
                                result: row.try_get("", "result").ok()?,
                                scheduled_for: row.try_get("", "scheduled_for").ok()?,
                                created_at: row.try_get("", "created_at").ok()?,
                                started_at: row.try_get("", "started_at").ok()?,
                                completed_at: row.try_get("", "completed_at").ok()?,
                            })
                        })
                    } else {
                        // SQLite: Use regular transaction (serial execution is sufficient for tests)
                        // Note: attempts < max_attempts comparison isn't supported in SeaORM filter
                        // so we'll filter it manually after fetching
                        let task = Tasks::find()
                            .filter(
                                Condition::any()
                                    .add(tasks::Column::Status.eq("pending"))
                                    .add(
                                        Condition::all()
                                            .add(tasks::Column::Status.eq("processing"))
                                            .add(tasks::Column::LockedUntil.lt(now)),
                                    ),
                            )
                            .filter(tasks::Column::ScheduledFor.lte(now))
                            .order_by_desc(tasks::Column::Priority)
                            .order_by_asc(tasks::Column::ScheduledFor)
                            .one(txn)
                            .await?;

                        // Filter out tasks that have reached max attempts
                        task.filter(|t| t.attempts < t.max_attempts)
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
        let pending = Tasks::find()
            .filter(tasks::Column::Status.eq("pending"))
            .count(db)
            .await
            .context("Failed to count pending tasks")?;

        let processing = Tasks::find()
            .filter(tasks::Column::Status.eq("processing"))
            .count(db)
            .await
            .context("Failed to count processing tasks")?;

        let completed = Tasks::find()
            .filter(tasks::Column::Status.eq("completed"))
            .count(db)
            .await
            .context("Failed to count completed tasks")?;

        let failed = Tasks::find()
            .filter(tasks::Column::Status.eq("failed"))
            .count(db)
            .await
            .context("Failed to count failed tasks")?;

        // Find stale locks (tasks locked for > 10 minutes)
        let stale_cutoff = Utc::now() - Duration::minutes(10);
        let stale = Tasks::find()
            .filter(tasks::Column::Status.eq("processing"))
            .filter(tasks::Column::LockedUntil.lt(stale_cutoff))
            .count(db)
            .await
            .context("Failed to count stale tasks")?;

        Ok(TaskStats {
            pending,
            processing,
            completed,
            failed,
            stale,
            total: pending + processing + completed + failed,
        })
    }

    /// Check if a task already exists for the given entity and type
    pub async fn task_exists(
        db: &DatabaseConnection,
        task_type: &str,
        library_id: Option<Uuid>,
        series_id: Option<Uuid>,
        book_id: Option<Uuid>,
    ) -> Result<bool> {
        let mut query = Tasks::find()
            .filter(tasks::Column::TaskType.eq(task_type))
            .filter(tasks::Column::Status.is_in(["pending", "processing"]));

        if let Some(lib_id) = library_id {
            query = query.filter(tasks::Column::LibraryId.eq(lib_id));
        }

        if let Some(ser_id) = series_id {
            query = query.filter(tasks::Column::SeriesId.eq(ser_id));
        }

        if let Some(bk_id) = book_id {
            query = query.filter(tasks::Column::BookId.eq(bk_id));
        }

        let count = query
            .count(db)
            .await
            .context("Failed to check task existence")?;

        Ok(count > 0)
    }
}
