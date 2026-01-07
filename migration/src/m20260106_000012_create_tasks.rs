use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let is_postgres = manager.get_database_backend() == sea_orm::DatabaseBackend::Postgres;

        let mut table = Table::create();
        table.table(Tasks::Table).if_not_exists();

        // ID column - different defaults for Postgres vs SQLite
        if is_postgres {
            table.col(
                ColumnDef::new(Tasks::Id)
                    .uuid()
                    .not_null()
                    .primary_key()
                    .extra("DEFAULT gen_random_uuid()"),
            );
        } else {
            table.col(ColumnDef::new(Tasks::Id).uuid().not_null().primary_key());
        }

        manager
            .create_table(
                table
                    .col(ColumnDef::new(Tasks::TaskType).string_len(50).not_null())
                    // Common foreign keys (for queries and CASCADE deletes)
                    .col(ColumnDef::new(Tasks::LibraryId).uuid())
                    .col(ColumnDef::new(Tasks::SeriesId).uuid())
                    .col(ColumnDef::new(Tasks::BookId).uuid())
                    // Extra task-specific params
                    .col(ColumnDef::new(Tasks::Params).json_binary())
                    // Task execution state
                    .col(
                        ColumnDef::new(Tasks::Status)
                            .string_len(20)
                            .not_null()
                            .default("pending"),
                    )
                    .col(
                        ColumnDef::new(Tasks::Priority)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    // Distributed locking
                    .col(ColumnDef::new(Tasks::LockedBy).string_len(100))
                    .col(ColumnDef::new(Tasks::LockedUntil).timestamp_with_time_zone())
                    // Retry handling
                    .col(
                        ColumnDef::new(Tasks::Attempts)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(Tasks::MaxAttempts)
                            .integer()
                            .not_null()
                            .default(3),
                    )
                    .col(ColumnDef::new(Tasks::LastError).text())
                    // Results (optional)
                    .col(ColumnDef::new(Tasks::Result).json_binary())
                    // Timestamps - different syntax for Postgres vs SQLite
                    .col({
                        let mut col = ColumnDef::new(Tasks::ScheduledFor);
                        col.timestamp_with_time_zone().not_null();
                        if is_postgres {
                            col.extra("DEFAULT NOW()");
                        } else {
                            col.extra("DEFAULT CURRENT_TIMESTAMP");
                        }
                        col
                    })
                    .col({
                        let mut col = ColumnDef::new(Tasks::CreatedAt);
                        col.timestamp_with_time_zone().not_null();
                        if is_postgres {
                            col.extra("DEFAULT NOW()");
                        } else {
                            col.extra("DEFAULT CURRENT_TIMESTAMP");
                        }
                        col
                    })
                    .col(ColumnDef::new(Tasks::StartedAt).timestamp_with_time_zone())
                    .col(ColumnDef::new(Tasks::CompletedAt).timestamp_with_time_zone())
                    // Foreign keys with CASCADE delete
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_tasks_library_id")
                            .from(Tasks::Table, Tasks::LibraryId)
                            .to(Libraries::Table, Libraries::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::NoAction),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_tasks_series_id")
                            .from(Tasks::Table, Tasks::SeriesId)
                            .to(Series::Table, Series::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::NoAction),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_tasks_book_id")
                            .from(Tasks::Table, Tasks::BookId)
                            .to(Books::Table, Books::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::NoAction),
                    )
                    .to_owned(),
            )
            .await?;

        // Index for claiming pending tasks efficiently
        // Different syntax for PostgreSQL vs SQLite
        if manager.get_database_backend() == sea_orm::DatabaseBackend::Postgres {
            manager
                .create_index(
                    Index::create()
                        .name("idx_tasks_ready")
                        .table(Tasks::Table)
                        .col((Tasks::Priority, IndexOrder::Desc))
                        .col(Tasks::ScheduledFor)
                        .to_owned(),
                )
                .await?;
        } else {
            // SQLite version without explicit ordering
            manager
                .create_index(
                    Index::create()
                        .name("idx_tasks_ready")
                        .table(Tasks::Table)
                        .col(Tasks::Priority)
                        .col(Tasks::ScheduledFor)
                        .to_owned(),
                )
                .await?;
        }

        // Partial index for pending tasks only (PostgreSQL-specific optimization)
        // For SQLite, this will create a regular index
        if manager.get_database_backend() == sea_orm::DatabaseBackend::Postgres {
            manager
                .get_connection()
                .execute_unprepared(
                    r#"
                    CREATE INDEX idx_tasks_ready_pending ON tasks(priority DESC, scheduled_for)
                    WHERE status = 'pending'
                    "#,
                )
                .await?;
        }

        // Index for processing tasks with expired locks
        manager
            .create_index(
                Index::create()
                    .name("idx_tasks_processing")
                    .table(Tasks::Table)
                    .col(Tasks::LockedUntil)
                    .to_owned(),
            )
            .await?;

        // Partial index for processing tasks (PostgreSQL-specific)
        if manager.get_database_backend() == sea_orm::DatabaseBackend::Postgres {
            manager
                .get_connection()
                .execute_unprepared(
                    r#"
                    CREATE INDEX idx_tasks_processing_locked ON tasks(locked_until)
                    WHERE status = 'processing'
                    "#,
                )
                .await?;
        }

        // Indexes on foreign keys for CASCADE delete performance
        manager
            .create_index(
                Index::create()
                    .name("idx_tasks_library")
                    .table(Tasks::Table)
                    .col(Tasks::LibraryId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_tasks_series")
                    .table(Tasks::Table)
                    .col(Tasks::SeriesId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_tasks_book")
                    .table(Tasks::Table)
                    .col(Tasks::BookId)
                    .to_owned(),
            )
            .await?;

        // Index on task type for filtering
        manager
            .create_index(
                Index::create()
                    .name("idx_tasks_type")
                    .table(Tasks::Table)
                    .col(Tasks::TaskType)
                    .to_owned(),
            )
            .await?;

        // Index on status for filtering
        manager
            .create_index(
                Index::create()
                    .name("idx_tasks_status")
                    .table(Tasks::Table)
                    .col(Tasks::Status)
                    .to_owned(),
            )
            .await?;

        // Unique constraints to prevent duplicate pending tasks for same entity
        // PostgreSQL-specific partial unique constraints
        if manager.get_database_backend() == sea_orm::DatabaseBackend::Postgres {
            manager
                .get_connection()
                .execute_unprepared(
                    r#"
                    CREATE UNIQUE INDEX unique_pending_library ON tasks(library_id, task_type)
                    WHERE status IN ('pending', 'processing') AND library_id IS NOT NULL
                    "#,
                )
                .await?;

            manager
                .get_connection()
                .execute_unprepared(
                    r#"
                    CREATE UNIQUE INDEX unique_pending_series ON tasks(series_id, task_type)
                    WHERE status IN ('pending', 'processing') AND series_id IS NOT NULL
                    "#,
                )
                .await?;

            manager
                .get_connection()
                .execute_unprepared(
                    r#"
                    CREATE UNIQUE INDEX unique_pending_book ON tasks(book_id, task_type)
                    WHERE status IN ('pending', 'processing') AND book_id IS NOT NULL
                    "#,
                )
                .await?;
        }

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Tasks::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum Tasks {
    Table,
    Id,
    TaskType,
    LibraryId,
    SeriesId,
    BookId,
    Params,
    Status,
    Priority,
    LockedBy,
    LockedUntil,
    Attempts,
    MaxAttempts,
    LastError,
    Result,
    ScheduledFor,
    CreatedAt,
    StartedAt,
    CompletedAt,
}

#[derive(DeriveIden)]
enum Libraries {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum Series {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum Books {
    Table,
    Id,
}
