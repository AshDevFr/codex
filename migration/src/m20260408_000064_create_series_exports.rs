use sea_orm_migration::prelude::*;

use crate::m20260103_000002_create_users::Users;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let is_postgres = manager.get_database_backend() == sea_orm::DatabaseBackend::Postgres;

        let mut table = Table::create();
        table.table(SeriesExports::Table).if_not_exists();

        // ID column - different defaults for Postgres vs SQLite
        if is_postgres {
            table.col(
                ColumnDef::new(SeriesExports::Id)
                    .uuid()
                    .not_null()
                    .primary_key()
                    .extra("DEFAULT gen_random_uuid()"),
            );
        } else {
            table.col(
                ColumnDef::new(SeriesExports::Id)
                    .uuid()
                    .not_null()
                    .primary_key(),
            );
        }

        manager
            .create_table(
                table
                    .col(ColumnDef::new(SeriesExports::UserId).uuid().not_null())
                    // "json" | "csv"
                    .col(
                        ColumnDef::new(SeriesExports::Format)
                            .string_len(10)
                            .not_null(),
                    )
                    // "pending" | "running" | "completed" | "failed" | "cancelled"
                    .col(
                        ColumnDef::new(SeriesExports::Status)
                            .string_len(20)
                            .not_null()
                            .default("pending"),
                    )
                    // JSON array of library UUIDs selected for this export
                    .col(
                        ColumnDef::new(SeriesExports::LibraryIds)
                            .json_binary()
                            .not_null(),
                    )
                    // JSON array of field keys selected for this export
                    .col(
                        ColumnDef::new(SeriesExports::Fields)
                            .json_binary()
                            .not_null(),
                    )
                    // Relative path to the generated file (null until completed)
                    .col(ColumnDef::new(SeriesExports::FilePath).text())
                    .col(ColumnDef::new(SeriesExports::FileSizeBytes).big_integer())
                    .col(ColumnDef::new(SeriesExports::RowCount).integer())
                    .col(ColumnDef::new(SeriesExports::Error).text())
                    // Link to the background task executing this export
                    .col(ColumnDef::new(SeriesExports::TaskId).uuid())
                    // Timestamps
                    .col({
                        let mut col = ColumnDef::new(SeriesExports::CreatedAt);
                        col.timestamp_with_time_zone().not_null();
                        if is_postgres {
                            col.extra("DEFAULT NOW()");
                        } else {
                            col.extra("DEFAULT CURRENT_TIMESTAMP");
                        }
                        col
                    })
                    .col(ColumnDef::new(SeriesExports::StartedAt).timestamp_with_time_zone())
                    .col(ColumnDef::new(SeriesExports::CompletedAt).timestamp_with_time_zone())
                    .col(
                        ColumnDef::new(SeriesExports::ExpiresAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_series_exports_user_id")
                            .from(SeriesExports::Table, SeriesExports::UserId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::NoAction),
                    )
                    .to_owned(),
            )
            .await?;

        // Index for listing a user's exports ordered by creation time
        if is_postgres {
            manager
                .create_index(
                    Index::create()
                        .name("idx_series_exports_user_created")
                        .table(SeriesExports::Table)
                        .col(SeriesExports::UserId)
                        .col((SeriesExports::CreatedAt, IndexOrder::Desc))
                        .to_owned(),
                )
                .await?;
        } else {
            manager
                .create_index(
                    Index::create()
                        .name("idx_series_exports_user_created")
                        .table(SeriesExports::Table)
                        .col(SeriesExports::UserId)
                        .col(SeriesExports::CreatedAt)
                        .to_owned(),
                )
                .await?;
        }

        // Index on status for filtering pending/running/completed
        manager
            .create_index(
                Index::create()
                    .name("idx_series_exports_status")
                    .table(SeriesExports::Table)
                    .col(SeriesExports::Status)
                    .to_owned(),
            )
            .await?;

        // Partial index for retention sweep: only completed rows with expires_at
        manager
            .get_connection()
            .execute_unprepared(
                r#"
                CREATE INDEX idx_series_exports_expires ON series_exports(expires_at)
                WHERE status = 'completed'
                "#,
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(SeriesExports::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum SeriesExports {
    Table,
    Id,
    UserId,
    Format,
    Status,
    LibraryIds,
    Fields,
    FilePath,
    FileSizeBytes,
    RowCount,
    Error,
    TaskId,
    CreatedAt,
    StartedAt,
    CompletedAt,
    ExpiresAt,
}
