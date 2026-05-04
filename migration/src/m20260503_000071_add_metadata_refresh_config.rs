//! Create the `library_jobs` table (Phase 9 of scheduled-metadata-refresh).
//!
//! Replaces the original Phase 1 design (a `metadata_refresh_config` JSON
//! column on `libraries`) with a generic, type-discriminated table that
//! supports N independent jobs per library. The `type` column dispatches to
//! type-specific config; `metadata_refresh` is the first type. Future job
//! types (scan, cleanup) extend the discriminator without schema changes.
//!
//! The migration filename is preserved (timestamp stays the same) because
//! the original Phase 1 migration never shipped to production.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let is_postgres = manager.get_database_backend() == sea_orm::DatabaseBackend::Postgres;

        let mut table = Table::create();
        table.table(LibraryJobs::Table).if_not_exists();

        if is_postgres {
            table.col(
                ColumnDef::new(LibraryJobs::Id)
                    .uuid()
                    .not_null()
                    .primary_key()
                    .extra("DEFAULT gen_random_uuid()"),
            );
        } else {
            table.col(
                ColumnDef::new(LibraryJobs::Id)
                    .uuid()
                    .not_null()
                    .primary_key(),
            );
        }

        manager
            .create_table(
                table
                    .col(ColumnDef::new(LibraryJobs::LibraryId).uuid().not_null())
                    // Type discriminator. "metadata_refresh" today; future:
                    // "scan", "cleanup", etc.
                    .col(
                        ColumnDef::new(LibraryJobs::JobType)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(ColumnDef::new(LibraryJobs::Name).string_len(200).not_null())
                    .col(
                        ColumnDef::new(LibraryJobs::Enabled)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .col(
                        ColumnDef::new(LibraryJobs::CronSchedule)
                            .string_len(120)
                            .not_null(),
                    )
                    // Optional per-job timezone override; falls back to server tz.
                    .col(ColumnDef::new(LibraryJobs::Timezone).string_len(80))
                    // Type-specific JSON payload.
                    .col(ColumnDef::new(LibraryJobs::Config).text().not_null())
                    .col(ColumnDef::new(LibraryJobs::LastRunAt).timestamp_with_time_zone())
                    // "success" | "failure" | NULL
                    .col(ColumnDef::new(LibraryJobs::LastRunStatus).string_len(32))
                    .col(ColumnDef::new(LibraryJobs::LastRunMessage).text())
                    .col({
                        let mut col = ColumnDef::new(LibraryJobs::CreatedAt);
                        col.timestamp_with_time_zone().not_null();
                        if is_postgres {
                            col.extra("DEFAULT NOW()");
                        } else {
                            col.extra("DEFAULT CURRENT_TIMESTAMP");
                        }
                        col
                    })
                    .col({
                        let mut col = ColumnDef::new(LibraryJobs::UpdatedAt);
                        col.timestamp_with_time_zone().not_null();
                        if is_postgres {
                            col.extra("DEFAULT NOW()");
                        } else {
                            col.extra("DEFAULT CURRENT_TIMESTAMP");
                        }
                        col
                    })
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_library_jobs_library_id")
                            .from(LibraryJobs::Table, LibraryJobs::LibraryId)
                            .to(Libraries::Table, Libraries::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::NoAction),
                    )
                    .to_owned(),
            )
            .await?;

        // Lookup by library (list jobs for a given library).
        manager
            .create_index(
                Index::create()
                    .name("idx_library_jobs_library_id")
                    .table(LibraryJobs::Table)
                    .col(LibraryJobs::LibraryId)
                    .to_owned(),
            )
            .await?;

        // Filter to only enabled jobs at scheduler boot.
        manager
            .create_index(
                Index::create()
                    .name("idx_library_jobs_enabled")
                    .table(LibraryJobs::Table)
                    .col(LibraryJobs::Enabled)
                    .to_owned(),
            )
            .await?;

        // Filter by type when listing (future-proofing for multi-type queries).
        manager
            .create_index(
                Index::create()
                    .name("idx_library_jobs_type")
                    .table(LibraryJobs::Table)
                    .col(LibraryJobs::JobType)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(LibraryJobs::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum LibraryJobs {
    Table,
    Id,
    LibraryId,
    #[sea_orm(iden = "type")]
    JobType,
    Name,
    Enabled,
    CronSchedule,
    Timezone,
    Config,
    LastRunAt,
    LastRunStatus,
    LastRunMessage,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum Libraries {
    Table,
    Id,
}
