use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let is_postgres = manager.get_database_backend() == sea_orm::DatabaseBackend::Postgres;

        let mut table = Table::create();
        table.table(TaskMetrics::Table).if_not_exists();

        // ID column - different defaults for Postgres vs SQLite
        if is_postgres {
            table.col(
                ColumnDef::new(TaskMetrics::Id)
                    .uuid()
                    .not_null()
                    .primary_key()
                    .extra("DEFAULT gen_random_uuid()"),
            );
        } else {
            table.col(
                ColumnDef::new(TaskMetrics::Id)
                    .uuid()
                    .not_null()
                    .primary_key(),
            );
        }

        manager
            .create_table(
                table
                    // Time bucket
                    .col(
                        ColumnDef::new(TaskMetrics::PeriodStart)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(TaskMetrics::PeriodType)
                            .string_len(10)
                            .not_null(),
                    ) // 'hour' or 'day'
                    // Task identification
                    .col(
                        ColumnDef::new(TaskMetrics::TaskType)
                            .string_len(50)
                            .not_null(),
                    )
                    .col(ColumnDef::new(TaskMetrics::LibraryId).uuid())
                    // Counts
                    .col(
                        ColumnDef::new(TaskMetrics::Count)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(TaskMetrics::Succeeded)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(TaskMetrics::Failed)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(TaskMetrics::Retried)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    // Timing (milliseconds)
                    .col(
                        ColumnDef::new(TaskMetrics::TotalDurationMs)
                            .big_integer()
                            .not_null()
                            .default(0),
                    )
                    .col(ColumnDef::new(TaskMetrics::MinDurationMs).big_integer())
                    .col(ColumnDef::new(TaskMetrics::MaxDurationMs).big_integer())
                    .col(
                        ColumnDef::new(TaskMetrics::TotalQueueWaitMs)
                            .big_integer()
                            .not_null()
                            .default(0),
                    )
                    // Percentile samples (JSON array of recent durations)
                    .col(ColumnDef::new(TaskMetrics::DurationSamples).json_binary())
                    // Task-specific counters
                    .col(
                        ColumnDef::new(TaskMetrics::ItemsProcessed)
                            .big_integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(TaskMetrics::BytesProcessed)
                            .big_integer()
                            .not_null()
                            .default(0),
                    )
                    // Errors
                    .col(
                        ColumnDef::new(TaskMetrics::ErrorCount)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(ColumnDef::new(TaskMetrics::LastError).text())
                    .col(ColumnDef::new(TaskMetrics::LastErrorAt).timestamp_with_time_zone())
                    // Metadata
                    .col({
                        let mut col = ColumnDef::new(TaskMetrics::CreatedAt);
                        col.timestamp_with_time_zone().not_null();
                        if is_postgres {
                            col.extra("DEFAULT NOW()");
                        } else {
                            col.extra("DEFAULT CURRENT_TIMESTAMP");
                        }
                        col
                    })
                    .col({
                        let mut col = ColumnDef::new(TaskMetrics::UpdatedAt);
                        col.timestamp_with_time_zone().not_null();
                        if is_postgres {
                            col.extra("DEFAULT NOW()");
                        } else {
                            col.extra("DEFAULT CURRENT_TIMESTAMP");
                        }
                        col
                    })
                    // Foreign key to libraries (optional - cascade delete when library is removed)
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_task_metrics_library")
                            .from(TaskMetrics::Table, TaskMetrics::LibraryId)
                            .to(Libraries::Table, Libraries::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::NoAction),
                    )
                    .to_owned(),
            )
            .await?;

        // Create unique constraint on (period_start, period_type, task_type, library_id)
        // This is tricky because library_id can be NULL
        if is_postgres {
            // PostgreSQL: Use COALESCE in unique index
            manager
                .get_connection()
                .execute_unprepared(
                    r#"
                    CREATE UNIQUE INDEX idx_task_metrics_period_unique
                    ON task_metrics (period_start, period_type, task_type, COALESCE(library_id, '00000000-0000-0000-0000-000000000000'))
                    "#,
                )
                .await?;
        } else {
            // SQLite: Create a composite unique index with IFNULL
            manager
                .get_connection()
                .execute_unprepared(
                    r#"
                    CREATE UNIQUE INDEX idx_task_metrics_period_unique
                    ON task_metrics (period_start, period_type, task_type, IFNULL(library_id, '00000000-0000-0000-0000-000000000000'))
                    "#,
                )
                .await?;
        }

        // Index on period_start for cleanup queries (DESC for recent-first access)
        manager
            .create_index(
                Index::create()
                    .name("idx_task_metrics_period")
                    .table(TaskMetrics::Table)
                    .col(TaskMetrics::PeriodStart)
                    .to_owned(),
            )
            .await?;

        // Index on task_type for filtering
        manager
            .create_index(
                Index::create()
                    .name("idx_task_metrics_type")
                    .table(TaskMetrics::Table)
                    .col(TaskMetrics::TaskType)
                    .to_owned(),
            )
            .await?;

        // Compound index for cleanup queries
        manager
            .create_index(
                Index::create()
                    .name("idx_task_metrics_cleanup")
                    .table(TaskMetrics::Table)
                    .col(TaskMetrics::PeriodStart)
                    .col(TaskMetrics::PeriodType)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(TaskMetrics::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum TaskMetrics {
    Table,
    Id,
    PeriodStart,
    PeriodType,
    TaskType,
    LibraryId,
    Count,
    Succeeded,
    Failed,
    Retried,
    TotalDurationMs,
    MinDurationMs,
    MaxDurationMs,
    TotalQueueWaitMs,
    DurationSamples,
    ItemsProcessed,
    BytesProcessed,
    ErrorCount,
    LastError,
    LastErrorAt,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum Libraries {
    Table,
    Id,
}
