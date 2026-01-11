use anyhow::{Context, Result};
use chrono::{DateTime, Duration, Timelike, Utc};
use sea_orm::{
    entity::prelude::*, ActiveModelTrait, ColumnTrait, DatabaseConnection, DbBackend, EntityTrait,
    QueryFilter, QueryOrder, QuerySelect, Set, Statement,
};
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::db::entities::{prelude::*, task_metrics};

/// Repository for TaskMetrics operations
pub struct TaskMetricsRepository;

/// Period type for metrics aggregation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PeriodType {
    Hour,
    Day,
}

impl PeriodType {
    pub fn as_str(&self) -> &'static str {
        match self {
            PeriodType::Hour => "hour",
            PeriodType::Day => "day",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "hour" => Some(PeriodType::Hour),
            "day" => Some(PeriodType::Day),
            _ => None,
        }
    }
}

/// Data for recording a task completion
#[derive(Debug, Clone)]
pub struct TaskCompletionData {
    pub task_type: String,
    pub library_id: Option<Uuid>,
    pub success: bool,
    pub retried: bool,
    pub duration_ms: i64,
    pub queue_wait_ms: i64,
    pub items_processed: i64,
    pub bytes_processed: i64,
    pub error: Option<String>,
}

impl TaskMetricsRepository {
    /// Get the start of the current hour
    fn hour_start(dt: DateTime<Utc>) -> DateTime<Utc> {
        dt.with_minute(0)
            .and_then(|d| d.with_second(0))
            .and_then(|d| d.with_nanosecond(0))
            .unwrap_or(dt)
    }

    /// Get the start of the current day
    fn day_start(dt: DateTime<Utc>) -> DateTime<Utc> {
        dt.with_hour(0)
            .and_then(|d| d.with_minute(0))
            .and_then(|d| d.with_second(0))
            .and_then(|d| d.with_nanosecond(0))
            .unwrap_or(dt)
    }

    /// Record a task completion by upserting into the hourly bucket
    pub async fn record_completion(
        db: &DatabaseConnection,
        data: TaskCompletionData,
    ) -> Result<()> {
        let now = Utc::now();
        let period_start = Self::hour_start(now);
        let period_type = PeriodType::Hour;

        // Try to find existing record for this period/type/library
        let existing = Self::find_by_period(
            db,
            period_start,
            period_type,
            &data.task_type,
            data.library_id,
        )
        .await?;

        if let Some(existing) = existing {
            // Update existing record
            Self::update_record(db, existing, &data).await
        } else {
            // Insert new record
            Self::insert_record(db, period_start, period_type, &data).await
        }
    }

    /// Find metrics record by period
    async fn find_by_period(
        db: &DatabaseConnection,
        period_start: DateTime<Utc>,
        period_type: PeriodType,
        task_type: &str,
        library_id: Option<Uuid>,
    ) -> Result<Option<task_metrics::Model>> {
        let mut query = TaskMetrics::find()
            .filter(task_metrics::Column::PeriodStart.eq(period_start))
            .filter(task_metrics::Column::PeriodType.eq(period_type.as_str()))
            .filter(task_metrics::Column::TaskType.eq(task_type));

        if let Some(lib_id) = library_id {
            query = query.filter(task_metrics::Column::LibraryId.eq(lib_id));
        } else {
            query = query.filter(task_metrics::Column::LibraryId.is_null());
        }

        query
            .one(db)
            .await
            .context("Failed to find metrics by period")
    }

    /// Insert a new metrics record
    async fn insert_record(
        db: &DatabaseConnection,
        period_start: DateTime<Utc>,
        period_type: PeriodType,
        data: &TaskCompletionData,
    ) -> Result<()> {
        let now = Utc::now();
        let id = Uuid::new_v4();

        // Create initial duration samples array
        let duration_samples = serde_json::json!([data.duration_ms]);

        let record = task_metrics::ActiveModel {
            id: Set(id),
            period_start: Set(period_start),
            period_type: Set(period_type.as_str().to_string()),
            task_type: Set(data.task_type.clone()),
            library_id: Set(data.library_id),
            count: Set(1),
            succeeded: Set(if data.success { 1 } else { 0 }),
            failed: Set(if data.success { 0 } else { 1 }),
            retried: Set(if data.retried { 1 } else { 0 }),
            total_duration_ms: Set(data.duration_ms),
            min_duration_ms: Set(Some(data.duration_ms)),
            max_duration_ms: Set(Some(data.duration_ms)),
            total_queue_wait_ms: Set(data.queue_wait_ms),
            duration_samples: Set(Some(duration_samples)),
            items_processed: Set(data.items_processed),
            bytes_processed: Set(data.bytes_processed),
            error_count: Set(if data.error.is_some() { 1 } else { 0 }),
            last_error: Set(data.error.clone()),
            last_error_at: Set(if data.error.is_some() {
                Some(now)
            } else {
                None
            }),
            created_at: Set(now),
            updated_at: Set(now),
        };

        record
            .insert(db)
            .await
            .context("Failed to insert metrics record")?;

        Ok(())
    }

    /// Update an existing metrics record
    async fn update_record(
        db: &DatabaseConnection,
        existing: task_metrics::Model,
        data: &TaskCompletionData,
    ) -> Result<()> {
        let now = Utc::now();

        // Update duration samples (keep up to 100 samples for percentile calculation)
        let mut samples: Vec<i64> = existing
            .duration_samples
            .as_ref()
            .and_then(|s| serde_json::from_value(s.clone()).ok())
            .unwrap_or_default();
        samples.push(data.duration_ms);
        if samples.len() > 100 {
            samples.remove(0); // Remove oldest sample
        }
        let duration_samples = serde_json::json!(samples);

        let mut record: task_metrics::ActiveModel = existing.clone().into();

        record.count = Set(existing.count + 1);
        record.succeeded = Set(existing.succeeded + if data.success { 1 } else { 0 });
        record.failed = Set(existing.failed + if data.success { 0 } else { 1 });
        record.retried = Set(existing.retried + if data.retried { 1 } else { 0 });
        record.total_duration_ms = Set(existing.total_duration_ms + data.duration_ms);
        record.min_duration_ms = Set(Some(
            existing
                .min_duration_ms
                .map_or(data.duration_ms, |min| min.min(data.duration_ms)),
        ));
        record.max_duration_ms = Set(Some(
            existing
                .max_duration_ms
                .map_or(data.duration_ms, |max| max.max(data.duration_ms)),
        ));
        record.total_queue_wait_ms = Set(existing.total_queue_wait_ms + data.queue_wait_ms);
        record.duration_samples = Set(Some(duration_samples));
        record.items_processed = Set(existing.items_processed + data.items_processed);
        record.bytes_processed = Set(existing.bytes_processed + data.bytes_processed);

        if data.error.is_some() {
            record.error_count = Set(existing.error_count + 1);
            record.last_error = Set(data.error.clone());
            record.last_error_at = Set(Some(now));
        }

        record.updated_at = Set(now);

        record
            .update(db)
            .await
            .context("Failed to update metrics record")?;

        Ok(())
    }

    /// Get all hourly metrics within a time range
    pub async fn get_hourly_metrics(
        db: &DatabaseConnection,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
        task_type: Option<&str>,
    ) -> Result<Vec<task_metrics::Model>> {
        let mut query = TaskMetrics::find()
            .filter(task_metrics::Column::PeriodType.eq("hour"))
            .filter(task_metrics::Column::PeriodStart.gte(from))
            .filter(task_metrics::Column::PeriodStart.lte(to));

        if let Some(tt) = task_type {
            query = query.filter(task_metrics::Column::TaskType.eq(tt));
        }

        query
            .order_by_desc(task_metrics::Column::PeriodStart)
            .all(db)
            .await
            .context("Failed to get hourly metrics")
    }

    /// Get all daily metrics within a time range
    pub async fn get_daily_metrics(
        db: &DatabaseConnection,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
        task_type: Option<&str>,
    ) -> Result<Vec<task_metrics::Model>> {
        let mut query = TaskMetrics::find()
            .filter(task_metrics::Column::PeriodType.eq("day"))
            .filter(task_metrics::Column::PeriodStart.gte(from))
            .filter(task_metrics::Column::PeriodStart.lte(to));

        if let Some(tt) = task_type {
            query = query.filter(task_metrics::Column::TaskType.eq(tt));
        }

        query
            .order_by_desc(task_metrics::Column::PeriodStart)
            .all(db)
            .await
            .context("Failed to get daily metrics")
    }

    /// Get metrics for a specific time range, aggregating hourly or daily as appropriate
    pub async fn get_metrics_history(
        db: &DatabaseConnection,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
        task_type: Option<&str>,
        granularity: &str,
    ) -> Result<Vec<task_metrics::Model>> {
        match granularity {
            "hour" => Self::get_hourly_metrics(db, from, to, task_type).await,
            "day" => Self::get_daily_metrics(db, from, to, task_type).await,
            _ => Self::get_hourly_metrics(db, from, to, task_type).await,
        }
    }

    /// Get aggregate metrics for all task types (current period)
    pub async fn get_current_aggregates(
        db: &DatabaseConnection,
    ) -> Result<Vec<task_metrics::Model>> {
        // Get metrics from the last 24 hours
        let from = Utc::now() - Duration::hours(24);

        TaskMetrics::find()
            .filter(task_metrics::Column::PeriodStart.gte(from))
            .order_by_desc(task_metrics::Column::UpdatedAt)
            .all(db)
            .await
            .context("Failed to get current aggregates")
    }

    /// Get aggregated statistics per task type
    pub async fn get_aggregated_by_type(
        db: &DatabaseConnection,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<Vec<AggregatedTaskMetrics>> {
        let is_postgres = db.get_database_backend() == DbBackend::Postgres;

        let sql = if is_postgres {
            r#"
            SELECT
                task_type,
                SUM(count)::bigint as total_count,
                SUM(succeeded)::bigint as total_succeeded,
                SUM(failed)::bigint as total_failed,
                SUM(retried)::bigint as total_retried,
                SUM(total_duration_ms)::bigint as sum_duration_ms,
                MIN(min_duration_ms)::bigint as min_duration_ms,
                MAX(max_duration_ms)::bigint as max_duration_ms,
                SUM(total_queue_wait_ms)::bigint as sum_queue_wait_ms,
                SUM(items_processed)::bigint as total_items,
                SUM(bytes_processed)::bigint as total_bytes,
                SUM(error_count)::bigint as total_errors
            FROM task_metrics
            WHERE period_start >= $1 AND period_start <= $2
            GROUP BY task_type
            ORDER BY task_type
            "#
        } else {
            r#"
            SELECT
                task_type,
                CAST(SUM(count) AS INTEGER) as total_count,
                CAST(SUM(succeeded) AS INTEGER) as total_succeeded,
                CAST(SUM(failed) AS INTEGER) as total_failed,
                CAST(SUM(retried) AS INTEGER) as total_retried,
                CAST(SUM(total_duration_ms) AS INTEGER) as sum_duration_ms,
                CAST(MIN(min_duration_ms) AS INTEGER) as min_duration_ms,
                CAST(MAX(max_duration_ms) AS INTEGER) as max_duration_ms,
                CAST(SUM(total_queue_wait_ms) AS INTEGER) as sum_queue_wait_ms,
                CAST(SUM(items_processed) AS INTEGER) as total_items,
                CAST(SUM(bytes_processed) AS INTEGER) as total_bytes,
                CAST(SUM(error_count) AS INTEGER) as total_errors
            FROM task_metrics
            WHERE period_start >= ? AND period_start <= ?
            GROUP BY task_type
            ORDER BY task_type
            "#
        };

        let stmt = Statement::from_sql_and_values(
            db.get_database_backend(),
            sql,
            vec![from.into(), to.into()],
        );

        let rows = db
            .query_all(stmt)
            .await
            .context("Failed to get aggregated metrics")?;

        let mut results = Vec::new();
        for row in rows {
            results.push(AggregatedTaskMetrics {
                task_type: row.try_get("", "task_type")?,
                total_count: row.try_get("", "total_count").unwrap_or(0),
                total_succeeded: row.try_get("", "total_succeeded").unwrap_or(0),
                total_failed: row.try_get("", "total_failed").unwrap_or(0),
                total_retried: row.try_get("", "total_retried").unwrap_or(0),
                sum_duration_ms: row.try_get("", "sum_duration_ms").unwrap_or(0),
                min_duration_ms: row.try_get("", "min_duration_ms").ok(),
                max_duration_ms: row.try_get("", "max_duration_ms").ok(),
                sum_queue_wait_ms: row.try_get("", "sum_queue_wait_ms").unwrap_or(0),
                total_items: row.try_get("", "total_items").unwrap_or(0),
                total_bytes: row.try_get("", "total_bytes").unwrap_or(0),
                total_errors: row.try_get("", "total_errors").unwrap_or(0),
            });
        }

        Ok(results)
    }

    /// Rollup hourly metrics to daily (for data older than 7 days)
    pub async fn rollup_hourly_to_daily(db: &DatabaseConnection) -> Result<u64> {
        let cutoff = Utc::now() - Duration::days(7);
        let cutoff_day_start = Self::day_start(cutoff);

        let is_postgres = db.get_database_backend() == DbBackend::Postgres;

        // Find distinct days with hourly data older than 7 days
        let find_days_sql = if is_postgres {
            r#"
            SELECT DISTINCT DATE_TRUNC('day', period_start) as day_start, task_type, library_id
            FROM task_metrics
            WHERE period_type = 'hour' AND period_start < $1
            "#
        } else {
            r#"
            SELECT DISTINCT DATE(period_start) as day_start, task_type, library_id
            FROM task_metrics
            WHERE period_type = 'hour' AND period_start < ?
            "#
        };

        let stmt = Statement::from_sql_and_values(
            db.get_database_backend(),
            find_days_sql,
            vec![cutoff_day_start.into()],
        );

        let rows = db
            .query_all(stmt)
            .await
            .context("Failed to find days to rollup")?;

        let mut rolled_up = 0u64;

        for row in rows {
            let day_start: DateTime<Utc> = row.try_get("", "day_start")?;
            let task_type: String = row.try_get("", "task_type")?;
            let library_id: Option<Uuid> = row.try_get("", "library_id")?;

            let day_end = day_start + Duration::days(1);

            // Aggregate hourly data for this day
            let hourly_data =
                Self::get_hourly_for_rollup(db, day_start, day_end, &task_type, library_id).await?;

            if hourly_data.is_empty() {
                continue;
            }

            // Create or update daily record
            let existing =
                Self::find_by_period(db, day_start, PeriodType::Day, &task_type, library_id)
                    .await?;

            if existing.is_some() {
                // Daily record already exists, skip (already rolled up)
                continue;
            }

            // Aggregate the hourly records
            let aggregated = Self::aggregate_records(&hourly_data);

            // Insert daily record
            let now = Utc::now();
            let record = task_metrics::ActiveModel {
                id: Set(Uuid::new_v4()),
                period_start: Set(day_start),
                period_type: Set("day".to_string()),
                task_type: Set(task_type.clone()),
                library_id: Set(library_id),
                count: Set(aggregated.count),
                succeeded: Set(aggregated.succeeded),
                failed: Set(aggregated.failed),
                retried: Set(aggregated.retried),
                total_duration_ms: Set(aggregated.total_duration_ms),
                min_duration_ms: Set(aggregated.min_duration_ms),
                max_duration_ms: Set(aggregated.max_duration_ms),
                total_queue_wait_ms: Set(aggregated.total_queue_wait_ms),
                duration_samples: Set(None), // Don't keep samples for daily rollups
                items_processed: Set(aggregated.items_processed),
                bytes_processed: Set(aggregated.bytes_processed),
                error_count: Set(aggregated.error_count),
                last_error: Set(aggregated.last_error),
                last_error_at: Set(aggregated.last_error_at),
                created_at: Set(now),
                updated_at: Set(now),
            };

            record
                .insert(db)
                .await
                .context("Failed to insert daily rollup")?;

            // Delete the hourly records that were rolled up
            for hourly in &hourly_data {
                let model: task_metrics::ActiveModel = hourly.clone().into();
                model
                    .delete(db)
                    .await
                    .context("Failed to delete hourly record after rollup")?;
            }

            rolled_up += hourly_data.len() as u64;
            debug!(
                "Rolled up {} hourly records to daily for {} on {}",
                hourly_data.len(),
                task_type,
                day_start
            );
        }

        if rolled_up > 0 {
            info!("Rolled up {} hourly records to daily aggregates", rolled_up);
        }

        Ok(rolled_up)
    }

    /// Get hourly records for rollup
    async fn get_hourly_for_rollup(
        db: &DatabaseConnection,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
        task_type: &str,
        library_id: Option<Uuid>,
    ) -> Result<Vec<task_metrics::Model>> {
        let mut query = TaskMetrics::find()
            .filter(task_metrics::Column::PeriodType.eq("hour"))
            .filter(task_metrics::Column::PeriodStart.gte(from))
            .filter(task_metrics::Column::PeriodStart.lt(to))
            .filter(task_metrics::Column::TaskType.eq(task_type));

        if let Some(lib_id) = library_id {
            query = query.filter(task_metrics::Column::LibraryId.eq(lib_id));
        } else {
            query = query.filter(task_metrics::Column::LibraryId.is_null());
        }

        query
            .all(db)
            .await
            .context("Failed to get hourly records for rollup")
    }

    /// Aggregate multiple records into a single summary
    fn aggregate_records(records: &[task_metrics::Model]) -> AggregatedRecord {
        let mut result = AggregatedRecord {
            count: 0,
            succeeded: 0,
            failed: 0,
            retried: 0,
            total_duration_ms: 0,
            min_duration_ms: None,
            max_duration_ms: None,
            total_queue_wait_ms: 0,
            items_processed: 0,
            bytes_processed: 0,
            error_count: 0,
            last_error: None,
            last_error_at: None,
        };

        for r in records {
            result.count += r.count;
            result.succeeded += r.succeeded;
            result.failed += r.failed;
            result.retried += r.retried;
            result.total_duration_ms += r.total_duration_ms;
            result.total_queue_wait_ms += r.total_queue_wait_ms;
            result.items_processed += r.items_processed;
            result.bytes_processed += r.bytes_processed;
            result.error_count += r.error_count;

            if let Some(min) = r.min_duration_ms {
                result.min_duration_ms = Some(result.min_duration_ms.map_or(min, |m| m.min(min)));
            }
            if let Some(max) = r.max_duration_ms {
                result.max_duration_ms = Some(result.max_duration_ms.map_or(max, |m| m.max(max)));
            }

            // Keep the most recent error
            if let Some(ref error_at) = r.last_error_at {
                if result
                    .last_error_at
                    .map_or(true, |existing| error_at > &existing)
                {
                    result.last_error = r.last_error.clone();
                    result.last_error_at = Some(*error_at);
                }
            }
        }

        result
    }

    /// Delete metrics older than the retention period
    pub async fn cleanup_old_metrics(db: &DatabaseConnection, retention_days: i64) -> Result<u64> {
        let cutoff = Utc::now() - Duration::days(retention_days);

        let result = TaskMetrics::delete_many()
            .filter(task_metrics::Column::PeriodStart.lt(cutoff))
            .exec(db)
            .await
            .context("Failed to cleanup old metrics")?;

        if result.rows_affected > 0 {
            info!(
                "Cleaned up {} metric records older than {} days",
                result.rows_affected, retention_days
            );
        }

        Ok(result.rows_affected)
    }

    /// Delete all metrics (nuclear option)
    pub async fn nuke_all_metrics(db: &DatabaseConnection) -> Result<u64> {
        warn!("Nuking all task metrics!");

        let result = TaskMetrics::delete_many()
            .exec(db)
            .await
            .context("Failed to nuke all metrics")?;

        info!("Deleted {} metric records", result.rows_affected);
        Ok(result.rows_affected)
    }

    /// Get the oldest metric record timestamp
    pub async fn get_oldest_metric(db: &DatabaseConnection) -> Result<Option<DateTime<Utc>>> {
        let oldest = TaskMetrics::find()
            .order_by_asc(task_metrics::Column::PeriodStart)
            .one(db)
            .await
            .context("Failed to get oldest metric")?;

        Ok(oldest.map(|m| m.period_start))
    }

    /// Get the most recent error for a task type
    pub async fn get_last_error(
        db: &DatabaseConnection,
        task_type: &str,
    ) -> Result<Option<(String, DateTime<Utc>)>> {
        let record = TaskMetrics::find()
            .filter(task_metrics::Column::TaskType.eq(task_type))
            .filter(task_metrics::Column::LastError.is_not_null())
            .order_by_desc(task_metrics::Column::LastErrorAt)
            .one(db)
            .await
            .context("Failed to get last error")?;

        Ok(record.and_then(|r| r.last_error.zip(r.last_error_at).map(|(e, t)| (e, t))))
    }
}

/// Aggregated metrics by task type
#[derive(Debug, Clone)]
pub struct AggregatedTaskMetrics {
    pub task_type: String,
    pub total_count: i64,
    pub total_succeeded: i64,
    pub total_failed: i64,
    pub total_retried: i64,
    pub sum_duration_ms: i64,
    pub min_duration_ms: Option<i64>,
    pub max_duration_ms: Option<i64>,
    pub sum_queue_wait_ms: i64,
    pub total_items: i64,
    pub total_bytes: i64,
    pub total_errors: i64,
}

/// Internal struct for aggregating records
struct AggregatedRecord {
    count: i32,
    succeeded: i32,
    failed: i32,
    retried: i32,
    total_duration_ms: i64,
    min_duration_ms: Option<i64>,
    max_duration_ms: Option<i64>,
    total_queue_wait_ms: i64,
    items_processed: i64,
    bytes_processed: i64,
    error_count: i32,
    last_error: Option<String>,
    last_error_at: Option<DateTime<Utc>>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_helpers::create_test_db;

    #[tokio::test]
    async fn test_record_completion() {
        let (db, _temp_dir) = create_test_db().await;
        let db = db.sea_orm_connection();

        let data = TaskCompletionData {
            task_type: "scan_library".to_string(),
            library_id: None,
            success: true,
            retried: false,
            duration_ms: 1000,
            queue_wait_ms: 50,
            items_processed: 10,
            bytes_processed: 1024,
            error: None,
        };

        TaskMetricsRepository::record_completion(&db, data.clone())
            .await
            .expect("Failed to record completion");

        // Verify record was created
        let metrics = TaskMetricsRepository::get_current_aggregates(&db)
            .await
            .expect("Failed to get aggregates");

        assert!(!metrics.is_empty());
        let first = &metrics[0];
        assert_eq!(first.task_type, "scan_library");
        assert_eq!(first.count, 1);
        assert_eq!(first.succeeded, 1);
        assert_eq!(first.failed, 0);
    }

    #[tokio::test]
    async fn test_record_multiple_completions() {
        let (db, _temp_dir) = create_test_db().await;
        let db = db.sea_orm_connection();

        // Record first completion
        let data1 = TaskCompletionData {
            task_type: "analyze_book".to_string(),
            library_id: None,
            success: true,
            retried: false,
            duration_ms: 500,
            queue_wait_ms: 25,
            items_processed: 1,
            bytes_processed: 512,
            error: None,
        };
        TaskMetricsRepository::record_completion(&db, data1)
            .await
            .expect("Failed to record first completion");

        // Record second completion (same type, same hour - should update)
        let data2 = TaskCompletionData {
            task_type: "analyze_book".to_string(),
            library_id: None,
            success: false,
            retried: true,
            duration_ms: 1500,
            queue_wait_ms: 100,
            items_processed: 1,
            bytes_processed: 256,
            error: Some("Test error".to_string()),
        };
        TaskMetricsRepository::record_completion(&db, data2)
            .await
            .expect("Failed to record second completion");

        // Verify aggregates
        let metrics = TaskMetricsRepository::get_current_aggregates(&db)
            .await
            .expect("Failed to get aggregates");

        let analyze_metrics: Vec<_> = metrics
            .iter()
            .filter(|m| m.task_type == "analyze_book")
            .collect();

        assert_eq!(analyze_metrics.len(), 1);
        let m = analyze_metrics[0];
        assert_eq!(m.count, 2);
        assert_eq!(m.succeeded, 1);
        assert_eq!(m.failed, 1);
        assert_eq!(m.retried, 1);
        assert_eq!(m.total_duration_ms, 2000);
        assert_eq!(m.min_duration_ms, Some(500));
        assert_eq!(m.max_duration_ms, Some(1500));
        assert_eq!(m.error_count, 1);
        assert!(m.last_error.is_some());
    }

    #[tokio::test]
    async fn test_cleanup_old_metrics() {
        let (db, _temp_dir) = create_test_db().await;
        let db = db.sea_orm_connection();

        // Record a completion
        let data = TaskCompletionData {
            task_type: "test_task".to_string(),
            library_id: None,
            success: true,
            retried: false,
            duration_ms: 100,
            queue_wait_ms: 10,
            items_processed: 1,
            bytes_processed: 100,
            error: None,
        };
        TaskMetricsRepository::record_completion(&db, data)
            .await
            .expect("Failed to record completion");

        // Cleanup with 30 days retention - the record was just created so it should NOT be deleted
        let deleted = TaskMetricsRepository::cleanup_old_metrics(&db, 30)
            .await
            .expect("Failed to cleanup");

        // Record was just created, so it's not older than 30 days
        assert_eq!(deleted, 0);

        // Now cleanup with -1 days (cutoff = now + 1 day, so everything before tomorrow is deleted)
        let deleted = TaskMetricsRepository::cleanup_old_metrics(&db, -1)
            .await
            .expect("Failed to cleanup");

        // This should delete the record since it's before tomorrow
        assert_eq!(deleted, 1);
    }

    #[tokio::test]
    async fn test_nuke_all_metrics() {
        let (db, _temp_dir) = create_test_db().await;
        let db = db.sea_orm_connection();

        // Record some completions
        for i in 0..3 {
            let data = TaskCompletionData {
                task_type: format!("task_{}", i),
                library_id: None,
                success: true,
                retried: false,
                duration_ms: 100,
                queue_wait_ms: 10,
                items_processed: 1,
                bytes_processed: 100,
                error: None,
            };
            TaskMetricsRepository::record_completion(&db, data)
                .await
                .expect("Failed to record completion");
        }

        // Verify records exist
        let metrics = TaskMetricsRepository::get_current_aggregates(&db)
            .await
            .expect("Failed to get aggregates");
        assert_eq!(metrics.len(), 3);

        // Nuke all
        let deleted = TaskMetricsRepository::nuke_all_metrics(&db)
            .await
            .expect("Failed to nuke");
        assert_eq!(deleted, 3);

        // Verify all deleted
        let metrics = TaskMetricsRepository::get_current_aggregates(&db)
            .await
            .expect("Failed to get aggregates");
        assert!(metrics.is_empty());
    }

    #[tokio::test]
    async fn test_hour_start() {
        let dt = DateTime::parse_from_rfc3339("2026-01-11T14:35:22Z")
            .unwrap()
            .with_timezone(&Utc);
        let hour_start = TaskMetricsRepository::hour_start(dt);
        assert_eq!(hour_start.minute(), 0);
        assert_eq!(hour_start.second(), 0);
        assert_eq!(hour_start.hour(), 14);
    }

    #[tokio::test]
    async fn test_day_start() {
        let dt = DateTime::parse_from_rfc3339("2026-01-11T14:35:22Z")
            .unwrap()
            .with_timezone(&Utc);
        let day_start = TaskMetricsRepository::day_start(dt);
        assert_eq!(day_start.hour(), 0);
        assert_eq!(day_start.minute(), 0);
        assert_eq!(day_start.second(), 0);
    }
}
