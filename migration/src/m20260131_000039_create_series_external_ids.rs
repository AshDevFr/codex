//! Create series_external_ids table for tracking external provider IDs
//!
//! This table enables:
//! - Tracking which external source a series was matched from (e.g., plugin:mangabaka, comicinfo, epub, manual)
//! - Storing the external ID for efficient re-fetching without search
//! - Recording when metadata was last synced and a hash for change detection
//!
//! Key differences from series_external_links:
//! - External IDs are the primary data, URL is optional convenience
//! - Focused on metadata provider IDs, not general links
//! - Includes metadata_hash for incremental sync
//! - Has last_synced_at for tracking freshness

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let is_postgres = manager.get_database_backend() == sea_orm::DatabaseBackend::Postgres;

        let mut table = Table::create();
        table.table(SeriesExternalIds::Table).if_not_exists();

        // ID column - different defaults for Postgres vs SQLite
        if is_postgres {
            table.col(
                ColumnDef::new(SeriesExternalIds::Id)
                    .uuid()
                    .not_null()
                    .primary_key()
                    .extra("DEFAULT gen_random_uuid()"),
            );
        } else {
            table.col(
                ColumnDef::new(SeriesExternalIds::Id)
                    .uuid()
                    .not_null()
                    .primary_key(),
            );
        }

        manager
            .create_table(
                table
                    // Foreign key to series
                    .col(
                        ColumnDef::new(SeriesExternalIds::SeriesId)
                            .uuid()
                            .not_null(),
                    )
                    // Source identifier: 'plugin:mangabaka', 'comicinfo', 'epub', 'manual'
                    .col(
                        ColumnDef::new(SeriesExternalIds::Source)
                            .string_len(100)
                            .not_null(),
                    )
                    // ID in the external system (required)
                    .col(
                        ColumnDef::new(SeriesExternalIds::ExternalId)
                            .text()
                            .not_null(),
                    )
                    // Full URL to the source page (optional convenience)
                    .col(ColumnDef::new(SeriesExternalIds::ExternalUrl).text())
                    // Hash of last fetched metadata for change detection
                    .col(ColumnDef::new(SeriesExternalIds::MetadataHash).string_len(64))
                    // When metadata was last synced from this source
                    .col(ColumnDef::new(SeriesExternalIds::LastSyncedAt).timestamp_with_time_zone())
                    // Timestamps
                    .col({
                        let mut col = ColumnDef::new(SeriesExternalIds::CreatedAt);
                        col.timestamp_with_time_zone().not_null();
                        if is_postgres {
                            col.extra("DEFAULT NOW()");
                        } else {
                            col.extra("DEFAULT CURRENT_TIMESTAMP");
                        }
                        col
                    })
                    .col({
                        let mut col = ColumnDef::new(SeriesExternalIds::UpdatedAt);
                        col.timestamp_with_time_zone().not_null();
                        if is_postgres {
                            col.extra("DEFAULT NOW()");
                        } else {
                            col.extra("DEFAULT CURRENT_TIMESTAMP");
                        }
                        col
                    })
                    // Foreign key constraint
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_series_external_ids_series_id")
                            .from(SeriesExternalIds::Table, SeriesExternalIds::SeriesId)
                            .to(Series::Table, Series::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::NoAction),
                    )
                    .to_owned(),
            )
            .await?;

        // Unique constraint: one external ID per source per series
        manager
            .create_index(
                Index::create()
                    .name("idx_series_external_ids_unique")
                    .table(SeriesExternalIds::Table)
                    .col(SeriesExternalIds::SeriesId)
                    .col(SeriesExternalIds::Source)
                    .unique()
                    .to_owned(),
            )
            .await?;

        // Index on series_id for efficient lookups by series
        manager
            .create_index(
                Index::create()
                    .name("idx_series_external_ids_series_id")
                    .table(SeriesExternalIds::Table)
                    .col(SeriesExternalIds::SeriesId)
                    .to_owned(),
            )
            .await?;

        // Index on source for filtering by source type
        manager
            .create_index(
                Index::create()
                    .name("idx_series_external_ids_source")
                    .table(SeriesExternalIds::Table)
                    .col(SeriesExternalIds::Source)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(SeriesExternalIds::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum SeriesExternalIds {
    Table,
    Id,
    SeriesId,
    Source,
    ExternalId,
    ExternalUrl,
    MetadataHash,
    LastSyncedAt,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum Series {
    Table,
    Id,
}
